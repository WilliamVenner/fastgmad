use crate::{
	error::{fastgmad_error, fastgmad_io_error, FastGmadError},
	util::WriteEx,
	whitelist,
};
use std::{
	fs::File,
	io::{Read, SeekFrom},
	io::{Seek, Write},
	path::{Path, PathBuf},
	sync::Arc,
	sync::{atomic::AtomicUsize, Condvar, Mutex},
	time::SystemTime,
};

mod conf;
pub use conf::CreateGmaConfig;

#[cfg(feature = "binary")]
pub use conf::CreateGmadOut;

/// Creates a GMA file from a directory.
///
/// Prefer [`seekable_create_gma`] if your writer type implements [`std::io::Seek`], as it supports parallel I/O.
pub fn create_gma(conf: &CreateGmaConfig, w: &mut impl Write) -> Result<(), FastGmadError> {
	StandardCreateGma::create_gma_with_done_callback(conf, w, &mut || ())
}

/// Creates a GMA file from a directory.
///
/// Prefer this function over [`create_gma`] if your writer type implements [`std::io::Seek`], as this function supports parallel I/O.
pub fn seekable_create_gma(conf: &CreateGmaConfig, w: &mut (impl Write + Seek)) -> Result<(), FastGmadError> {
	if conf.max_io_threads.get() == 1 {
		StandardCreateGma::create_gma_with_done_callback(conf, w, &mut || ())
	} else {
		ParallelCreateGma::create_gma_with_done_callback(conf, w, &mut || ())
	}
}

#[cfg(feature = "binary")]
pub fn create_gma_with_done_callback(conf: &CreateGmaConfig, w: &mut impl Write, done_callback: &mut dyn FnMut()) -> Result<(), FastGmadError> {
	StandardCreateGma::create_gma_with_done_callback(conf, w, done_callback)
}

#[cfg(feature = "binary")]
pub fn seekable_create_gma_with_done_callback(
	conf: &CreateGmaConfig,
	w: &mut (impl Write + Seek),
	done_callback: &mut dyn FnMut(),
) -> Result<(), FastGmadError> {
	if conf.max_io_threads.get() == 1 {
		StandardCreateGma::create_gma_with_done_callback(conf, w, done_callback)
	} else {
		ParallelCreateGma::create_gma_with_done_callback(conf, w, done_callback)
	}
}

trait CreateGma<W: Write> {
	fn create_gma_with_done_callback(conf: &CreateGmaConfig, w: &mut W, done_callback: &mut dyn FnMut()) -> Result<(), FastGmadError> {
		log::info!("Reading addon.json...");
		let addon_json = AddonJson::read(&conf.folder.join("addon.json"))?;

		log::info!("Discovering entries...");
		let entries = discover_entries(&conf.folder, &addon_json.ignore, conf.warn_invalid)?;

		log::info!("Writing GMA metadata...");

		// Magic bytes
		w.write_all(crate::GMA_MAGIC)
			.map_err(|error| fastgmad_io_error!(while "writing magic bytes", error: error))?;

		// Version
		w.write_all(&[crate::GMA_VERSION])
			.map_err(|error| fastgmad_io_error!(while "writing version", error: error))?;

		// SteamID (unused)
		w.write_all(&[0u8; 8])
			.map_err(|error| fastgmad_io_error!(while "writing SteamID", error: error))?;

		// Timestamp
		w.write_all(&u64::to_le_bytes(
			SystemTime::now()
				.duration_since(SystemTime::UNIX_EPOCH)
				.map(|dur| dur.as_secs())
				.unwrap_or(0),
		))
		.map_err(|err| fastgmad_io_error!(while "writing timestamp", error: err))?;

		// Required content (unused)
		w.write_all(&[0u8])
			.map_err(|error| fastgmad_io_error!(while "writing required content", error: error))?;

		// Addon name
		w.write_nul_str(addon_json.title.as_bytes())
			.map_err(|error| fastgmad_io_error!(while "writing addon name", error: error))?;

		// Addon description
		w.write_nul_str(addon_json.json.as_bytes())
			.map_err(|error| fastgmad_io_error!(while "writing addon description", error: error))?;

		// Author name (unused)
		w.write_all(&[0u8])
			.map_err(|error| fastgmad_io_error!(while "writing author name", error: error))?;

		// Addon version (unused)
		w.write_all(&[1, 0, 0, 0])
			.map_err(|error| fastgmad_io_error!(while "writing addon version", error: error))?;

		// File list
		log::info!("Writing file list...");

		#[cfg(feature = "binary")]
		let mut total_size = 0;
		for (num, GmaFileEntry { size, relative_path, .. }) in entries.iter().enumerate() {
			// File number
			w.write_all(&u32::to_le_bytes(num as u32 + 1))
				.map_err(|error| fastgmad_io_error!(while "writing entry index", error: error))?;

			// File path
			w.write_nul_str(relative_path.as_bytes())
				.map_err(|error| fastgmad_io_error!(while "writing entry path", error: error))?;

			// File size
			w.write_all(&i64::to_le_bytes(i64::try_from(*size).map_err(|_| {
				fastgmad_io_error!(while "writing entry size", error: std::io::Error::new(std::io::ErrorKind::InvalidData, "File too large to be included in GMA"))
			})?)).map_err(|error| {
				fastgmad_io_error!(while "writing entry size", error: error)
			})?;

			// CRC (unused)
			w.write_all(&[0u8; 4])
				.map_err(|error| fastgmad_io_error!(while "writing entry CRC", error: error))?;

			#[cfg(feature = "binary")]
			{
				total_size += *size;
			}
		}

		// Zero to signify end of files
		w.write_all(&[0u8; 4])
			.map_err(|error| fastgmad_io_error!(while "writing end of file list", error: error))?;

		// Write entries
		log::info!("Writing file contents...");

		Self::write_entries(
			conf,
			w,
			#[cfg(feature = "binary")]
			total_size,
			&entries,
		)?;

		// Explicitly free memory here
		// We may exit the process in done_callback (thereby allowing the OS to free the memory),
		// so make sure the optimiser knows to free all the memory here.
		done_callback();
		drop(entries);
		drop(addon_json);

		Ok(())
	}

	fn write_entries(
		conf: &CreateGmaConfig,
		w: &mut W,
		#[cfg(feature = "binary")] total_size: u64,
		entries: &[GmaFileEntry],
	) -> Result<(), FastGmadError>;
}

fn discover_entries(folder: &Path, ignore: &[String], warn_invalid: bool) -> Result<Vec<GmaFileEntry>, FastGmadError> {
	let mut entries = Vec::new();
	let mut prev_offset = 0;
	for entry in walkdir::WalkDir::new(folder).follow_links(true).sort_by_file_name() {
		let entry = entry.map_err(|error| {
			let path = error.path().unwrap_or(folder).to_owned();
			if let Some(io_error) = error.into_io_error() {
				fastgmad_io_error!(while "walking directory", error: io_error, path: path)
			} else {
				fastgmad_io_error!(while "walking directory", error: std::io::Error::new(std::io::ErrorKind::Other, "unknown"), path: path)
			}
		})?;
		if !entry.file_type().is_file() {
			continue;
		}

		let path = entry.path();
		let relative_path = path
			.strip_prefix(folder)
			.map_err(|_| fastgmad_io_error!(error: std::io::Error::new(std::io::ErrorKind::InvalidData, "File not in addon directory"), path: path))?
			.to_str()
			.ok_or_else(|| fastgmad_io_error!(error: std::io::Error::new(std::io::ErrorKind::InvalidData, "File path not valid UTF-8"), path: path))?
			.replace('\\', "/");

		if relative_path == "addon.json" {
			continue;
		}

		if whitelist::is_ignored(&relative_path, ignore) {
			continue;
		}

		if !whitelist::check(&relative_path) {
			if warn_invalid {
				log::warn!(
					"File {} not in GMA whitelist - see https://wiki.facepunch.com/gmod/Workshop_Addon_Creation",
					relative_path
				);
				continue;
			} else {
				return Err(fastgmad_error!(error: EntryNotWhitelisted(relative_path)));
			}
		}

		let size = std::fs::metadata(path)
			.map_err(|error| fastgmad_io_error!(while "reading entry metadata", error: error, path: path))?
			.len();

		let new_offset = prev_offset + size;

		entries.push(GmaFileEntry {
			path: path.to_owned(),
			relative_path,
			size,
			offset: core::mem::replace(&mut prev_offset, new_offset),
		});
	}
	Ok(entries)
}

struct StandardCreateGma;
impl<W: Write> CreateGma<W> for StandardCreateGma {
	fn write_entries(
		_conf: &CreateGmaConfig,
		w: &mut W,
		#[cfg(feature = "binary")] total_size: u64,
		entries: &[GmaFileEntry],
	) -> Result<(), FastGmadError> {
		#[cfg(feature = "binary")]
		let mut progress = if !_conf.noprogress {
			Some(crate::util::ProgressPrinter::new(total_size))
		} else {
			None
		};

		for entry in entries.iter() {
			std::io::copy(
				&mut File::open(&entry.path).map_err(|error| fastgmad_io_error!(while "opening GMA entry file", error: error, path: entry.path))?,
				w,
			)
			.map_err(|error| fastgmad_io_error!(while "copying GMA entry data", error: error, path: entry.path))?;

			#[cfg(feature = "binary")]
			if let Some(progress) = &mut progress {
				progress.add_progress(entry.size);
			}
		}

		Ok(())
	}
}

struct ParallelCreateGma;
impl<W: Write + Seek> CreateGma<W> for ParallelCreateGma {
	fn write_entries(
		conf: &CreateGmaConfig,
		w: &mut W,
		#[cfg(feature = "binary")] total_size: u64,
		entries: &[GmaFileEntry],
	) -> Result<(), FastGmadError> {
		#[cfg(feature = "binary")]
		let mut progress = if !conf.noprogress {
			Some(crate::util::ProgressPrinter::new(total_size))
		} else {
			None
		};

		let (tx, rx) = std::sync::mpsc::sync_channel::<Result<(u64, Vec<u8>), FastGmadError>>(0);

		// Write entries
		let contents_ptr = w
			.stream_position()
			.map_err(|error| fastgmad_io_error!(while "getting stream position", error: error))?;

		// Split the entries into entries that we can buffer (size <= max_io_memory_usage)
		// and entries that will be copied in full without buffering (size > max_io_memory_usage)
		let (buffered_entries, full_copy_entries) = entries
			.iter()
			.partition::<Vec<_>, _>(|entry| entry.size <= conf.max_io_memory_usage.get() as u64);

		struct EntriesQueue<'a> {
			head: AtomicUsize,
			entries: Vec<&'a GmaFileEntry>,
			memory_usage: Mutex<usize>,
			memory_usage_cvar: Condvar,
		}
		impl EntriesQueue<'_> {
			pub fn next(&self) -> Option<&GmaFileEntry> {
				// NOTE: technically this can wrap around on overflow, but it won't happen because
				// we only spawn a maximum of MAX_IO_THREADS.
				self.entries.get(self.head.fetch_add(1, std::sync::atomic::Ordering::SeqCst)).copied()
			}
		}

		let queue = Arc::new(EntriesQueue {
			entries: buffered_entries,
			head: AtomicUsize::new(0),
			memory_usage: Mutex::new(0),
			memory_usage_cvar: Condvar::new(),
		});
		std::thread::scope(|scope| {
			const IO_THREAD_STACK_SIZE: usize = 2048;

			for _ in 0..queue.entries.len().min(conf.max_io_threads.get()) {
				let queue = queue.clone();
				let tx = tx.clone();
				if std::thread::Builder::new()
					.stack_size(IO_THREAD_STACK_SIZE)
					.spawn_scoped(scope, move || {
						while let Some(GmaFileEntry { offset, path, size, .. }) = queue.next() {
							let mut cur_offset = *offset;
							let max_offset = *offset + size;

							let mut f = match File::open(path) {
								Ok(f) => f,
								Err(error) => {
									tx.send(Err(fastgmad_io_error!(while "opening GMA entry file", error: error, path: path)))
										.ok();
									return;
								}
							};

							while cur_offset < max_offset {
								let mut memory_usage = queue
									.memory_usage_cvar
									.wait_while(queue.memory_usage.lock().unwrap(), |memory_usage| {
										*memory_usage > 0 && *memory_usage + *size as usize >= conf.max_io_memory_usage.get()
									})
									.unwrap();

								let bytes_left = max_offset - cur_offset;
								let offset = cur_offset;

								let res = {
									let available_memory = (conf.max_io_memory_usage.get() - *memory_usage) as u64;
									let will_read = available_memory.min(bytes_left);

									*memory_usage += will_read as usize;
									drop(memory_usage);

									cur_offset += will_read;

									let mut buf = Vec::with_capacity(will_read as usize);
									(&mut f).take(will_read).read_to_end(&mut buf).map(|_| buf)
								};

								let res = res
									.map(|contents| (offset, contents))
									.map_err(|error| fastgmad_io_error!(while "reading GMA entry data", error: error, path: path));

								if tx.send(res).is_err() {
									return;
								}
							}
						}
					})
					.is_err()
				{
					break;
				}
			}
			drop(tx);

			while let Ok(res) = rx.recv() {
				let (offset, contents) = res?;

				w.seek(SeekFrom::Start(contents_ptr + offset))
					.map_err(|error| fastgmad_io_error!(while "seeking to GMA entry offset", error: error))?;
				w.write_all(&contents)
					.map_err(|error| fastgmad_io_error!(while "writing GMA entry data", error: error))?;

				#[cfg(feature = "binary")]
				if let Some(progress) = &mut progress {
					progress.add_progress(contents.len() as u64);
				}

				let contents_size = contents.len();
				drop(contents);
				*queue.memory_usage.lock().unwrap() -= contents_size;
				queue.memory_usage_cvar.notify_all();
			}

			Ok::<_, FastGmadError>(())
		})?;

		for entry in full_copy_entries.iter() {
			w.seek(SeekFrom::Start(contents_ptr + entry.offset))
				.map_err(|error| fastgmad_io_error!(while "seeking to GMA entry offset", error: error))?;

			std::io::copy(
				&mut File::open(&entry.path).map_err(|error| fastgmad_io_error!(while "opening GMA entry file", error: error, path: entry.path))?,
				w,
			)
			.map_err(|error| fastgmad_io_error!(while "copying GMA entry data", error: error, path: entry.path))?;

			#[cfg(feature = "binary")]
			if let Some(progress) = &mut progress {
				progress.add_progress(entry.size);
			}
		}

		w.flush().map_err(|error| fastgmad_io_error!(while "flushing GMA file", error: error))?;

		Ok(())
	}
}

#[derive(serde::Deserialize)]
struct AddonJson {
	#[serde(skip)]
	json: String,

	title: String,

	#[serde(default)]
	ignore: Vec<String>,
}
impl AddonJson {
	fn read(path: &Path) -> Result<Self, FastGmadError> {
		let json = std::fs::read_to_string(path).map_err(|error| fastgmad_io_error!(while "reading addon.json", error: error, path: path))?;

		let mut addon_json: AddonJson = serde_json::from_str(&json).map_err(|error| fastgmad_error!(while "parsing addon.json", error: error))?;

		addon_json.json = json;

		Ok(addon_json)
	}
}

struct GmaFileEntry {
	path: PathBuf,
	relative_path: String,
	size: u64,
	offset: u64,
}
