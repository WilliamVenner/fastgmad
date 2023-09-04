use crate::{create::conf::CreateGmadConfig, create::AddonJson, util::WriteEx, whitelist};
use std::{
	fs::File,
	io::{Read, Seek, SeekFrom, Write, BufReader},
	path::PathBuf,
	sync::{atomic::AtomicUsize, Arc, Condvar, Mutex},
	time::SystemTime,
};

struct GmaFileEntry {
	path: PathBuf,
	relative_path: String,
	size: u64,
	offset: u64,
}
impl Ord for GmaFileEntry {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.relative_path.cmp(&other.relative_path)
	}
}
impl PartialOrd for GmaFileEntry {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}
impl Eq for GmaFileEntry {}
impl PartialEq for GmaFileEntry {
	fn eq(&self, other: &Self) -> bool {
		self.relative_path == other.relative_path
	}
}

pub fn create_gma_with_done_callback(
	conf: &CreateGmadConfig,
	w: &mut (impl Write + Seek),
	done_callback: &mut dyn FnMut(),
) -> Result<(), anyhow::Error> {
	let addon_json = perf!(["addon.json"] => AddonJson::read(&conf.folder.join("addon.json"))?);

	let entries = perf!(["entry discovery"] => {
		let mut entries = Vec::new();
		let mut prev_offset = 0;
		for entry in walkdir::WalkDir::new(&conf.folder).follow_links(true).sort_by_file_name() {
			let entry = entry?;
			if !entry.file_type().is_file() {
				continue;
			}

			let path = entry.path();
			let relative_path = path
				.strip_prefix(&conf.folder)
				.map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("File {:?} not in addon directory", path)))?
				.to_str()
				.ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("File path {:?} not valid UTF-8", path)))?
				.replace('\\', "/");

			if relative_path == "addon.json" {
				continue;
			}

			if whitelist::is_ignored(&relative_path, &addon_json.ignore) {
				continue;
			}

			if !whitelist::check(&relative_path) {
				if conf.warn_invalid {
					eprintln!("Warning: File {} not in GMA whitelist - see https://wiki.facepunch.com/gmod/Workshop_Addon_Creation", relative_path);
					continue;
				} else {
				return Err(std::io::Error::new(
					std::io::ErrorKind::InvalidData,
					format!(
						"File {} not in GMA whitelist - see https://wiki.facepunch.com/gmod/Workshop_Addon_Creation",
						relative_path
					),
				).into());
			}
			}

			let size = entry.metadata()?.len();
			let new_offset = prev_offset + size;
			entries.push(GmaFileEntry {
				path: path.to_owned(),
				relative_path,
				size,
				offset: core::mem::replace(&mut prev_offset, new_offset),
			});
		}
		entries
	});

	// Magic bytes
	w.write_all(crate::GMA_MAGIC)?;

	// Version
	w.write_all(&[crate::GMA_VERSION])?;

	// SteamID (unused)
	w.write_all(&[0u8; 8])?;

	// Timestamp
	w.write_all(&u64::to_le_bytes(
		SystemTime::now()
			.duration_since(SystemTime::UNIX_EPOCH)
			.map(|dur| dur.as_secs())
			.unwrap_or(0),
	))?;

	// Required content (unused)
	w.write_all(&[0u8])?;

	// Addon name
	w.write_nul_str(addon_json.title.as_bytes())?;

	// Addon description
	w.write_nul_str(addon_json.json.as_bytes())?;

	// Author name (unused)
	w.write_all(&[0u8])?;

	// Addon version (unused)
	w.write_all(&[1, 0, 0, 0])?;

	// File list
	perf!(["write file list"] => {
		for (num, GmaFileEntry { size, relative_path, .. }) in entries.iter().enumerate() {
			// File number
			w.write_all(&u32::to_le_bytes(num as u32 + 1))?;

			// File path
			w.write_nul_str(relative_path.as_bytes())?;

			// File size
			w.write_all(&i64::to_le_bytes(i64::try_from(*size).map_err(|_| {
				std::io::Error::new(std::io::ErrorKind::InvalidData, "File too large to be included in GMA")
			})?))?;

			// CRC (unused)
			w.write_all(&[0u8; 4])?;
		}
	});

	// Zero to signify end of files
	w.write_all(&[0u8; 4])?;

	let (tx, rx) = std::sync::mpsc::sync_channel(0);

	// Write entries
	// TODO memory limiting
	let queue = perf!(["write entries"] => {
		let contents_ptr = w.stream_position()?;

		struct EntriesQueue {
			head: AtomicUsize,
			entries: Vec<GmaFileEntry>,
			memory_usage: Mutex<usize>,
			memory_usage_cvar: Condvar,
		}
		impl EntriesQueue {
			pub fn next(&self) -> Option<&GmaFileEntry> {
				// NOTE: technically this can wrap around on overflow, but it won't happen because
				// we only spawn a maximum of MAX_IO_THREADS.
				self.entries.get(self.head.fetch_add(1, std::sync::atomic::Ordering::SeqCst))
			}
		}

		// Split the entries into entries that we can buffer (size <= max_io_memory_usage)
		// and entries that will be copied in full without buffering (size > max_io_memory_usage)
		let (buffered_entries, full_copy_entries) = entries.into_iter().partition::<Vec<_>, _>(|entry| {
			entry.size <= conf.max_io_memory_usage.get() as u64
		});

		let queue = Arc::new(EntriesQueue { entries: buffered_entries, head: AtomicUsize::new(0), memory_usage: Mutex::new(0), memory_usage_cvar: Condvar::new() });
		std::thread::scope(|scope| {
			const IO_THREAD_STACK_SIZE: usize = 2048;

			for _ in 0..queue.entries.len().min(conf.max_io_threads.get()) {
				let queue = queue.clone();
				let tx = tx.clone();
				if std::thread::Builder::new().stack_size(IO_THREAD_STACK_SIZE).spawn_scoped(scope, move || {
					while let Some(GmaFileEntry { offset, path, size, .. }) = queue.next() {
						let mut cur_offset = *offset;
						let max_offset = *offset + size;

						let mut f = match File::open(path) {
							Ok(f) => f,
							Err(err) => {
								tx.send(Err(err)).ok();
								return;
							}
						};

						while cur_offset < max_offset {
							let mut memory_usage = queue.memory_usage_cvar.wait_while(queue.memory_usage.lock().unwrap(), |memory_usage| *memory_usage > 0 && *memory_usage + *size as usize >= conf.max_io_memory_usage.get()).unwrap();

							let res = {
								let available_memory = (conf.max_io_memory_usage.get() - *memory_usage) as u64;
								if available_memory >= *size {
									*memory_usage += *size as usize;
									drop(memory_usage);

									cur_offset += *size;

									let mut buf = Vec::with_capacity(*size as usize);
									f.read_to_end(&mut buf).map(|_| buf)
								} else {
									let will_read = available_memory.min(max_offset - cur_offset);

									*memory_usage += will_read as usize;
									drop(memory_usage);

									cur_offset += will_read;

									let mut buf = Vec::with_capacity(will_read as usize);
									(&mut f).take(will_read).read_to_end(&mut buf).map(|_| buf)
								}
							};

							if tx.send(res.map(|contents| (cur_offset, contents))).is_err() {
								return;
							}
						}
					}
				}).is_err() {
					break;
				}
			}
			drop(tx);

			while let Ok(res) = rx.recv() {
				let (offset, contents) = res?;

				w.seek(SeekFrom::Start(contents_ptr + offset))?;
				w.write_all(&contents)?;

				let contents_size = contents.len();
				drop(contents);
				*queue.memory_usage.lock().unwrap() -= contents_size;
				queue.memory_usage_cvar.notify_all();
			}

			Ok::<_, anyhow::Error>(())
		})?;

		for GmaFileEntry { path, offset, .. } in full_copy_entries.iter() {
			w.seek(SeekFrom::Start(contents_ptr + *offset))?;
			std::io::copy(&mut BufReader::new(File::open(path)?), w)?;
		}

		w.flush()?;

		(full_copy_entries, queue)
	});

	// Explicitly free memory here
	// We may exit the process in done_callback (thereby allowing the OS to free the memory),
	// so make sure the optimiser knows to free all the memory here.
	done_callback();
	drop(queue);
	drop(addon_json);

	Ok(())
}

pub fn create_gma(conf: &CreateGmadConfig, w: &mut (impl Write + Seek)) -> Result<(), anyhow::Error> {
	create_gma_with_done_callback(conf, w, &mut || ())
}
