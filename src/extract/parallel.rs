use crate::{
	extract::{ExtractGmadConfig, GmaEntry, StubAddonJson},
	util::BufReadEx,
	GMA_MAGIC, GMA_VERSION,
};
use byteorder::{ReadBytesExt, LE};
use std::{
	fs::File,
	io::{BufRead, BufWriter, Read, Write},
	sync::{atomic::AtomicUsize, Mutex},
};

pub fn extract_gma_with_done_callback(conf: &ExtractGmadConfig, r: &mut impl BufRead, done_callback: &mut dyn FnMut()) -> Result<(), anyhow::Error> {
	if conf.out.is_dir() {
		log::warn!("Output directory already exists; files not present in this GMA but present in the existing output directory will NOT be deleted");
	}

	std::fs::create_dir_all(&conf.out)?;

	let mut buf = Vec::new();

	log::info!("Reading metadata...");

	{
		let mut magic = [0u8; 4];
		r.read_exact(&mut magic)?;
		if magic != GMA_MAGIC {
			return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "File is not in GMA format").into());
		}
	}

	let version = r.read_u8()?;
	if version != GMA_VERSION {
		log::warn!("File is in GMA version {version}, expected version {GMA_VERSION}, reading anyway...");
	}

	// SteamID (unused)
	r.read_exact(&mut [0u8; 8])?;

	// Timestamp
	r.read_exact(&mut [0u8; 8])?;

	if version > 1 {
		// Required content
		loop {
			buf.clear();
			if r.read_nul_str(&mut buf)?.is_empty() {
				break;
			}
		}
	}

	// Addon name
	let title = {
		buf.clear();
		r.read_nul_str(&mut buf)?.to_vec()
	};

	// addon.json
	let addon_json = {
		buf.clear();
		r.read_nul_str(&mut buf)?.to_vec()
	};

	// Addon author
	r.skip_nul_str()?;

	// Addon version (unused)
	r.read_exact(&mut [0u8; 4])?;

	log::info!("Writing addon.json...");
	let addon_json_path = conf.out.join("addon.json");
	let mut addon_json_f = BufWriter::new(File::create(&addon_json_path)?);
	if let Ok(mut kv) = serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(&addon_json) {
		// Add title key if it doesn't exist
		if let serde_json::map::Entry::Vacant(v) = kv.entry("title".to_string()) {
			v.insert(serde_json::Value::String(String::from_utf8_lossy(&title).into_owned()));
		}
		serde_json::to_writer_pretty(&mut addon_json_f, &kv)?;
	} else {
		serde_json::to_writer_pretty(
			&mut addon_json_f,
			&StubAddonJson {
				title: String::from_utf8_lossy(&title),
				description: String::from_utf8_lossy(&addon_json),
			},
		)?;
	}
	addon_json_f.flush()?;

	// File index
	log::info!("Reading file list...");

	#[cfg(feature = "binary")]
	let mut total_size = 0;

	let mut file_index: Vec<GmaEntry<usize>> = Vec::new();
	while r.read_u32::<LE>()? != 0 {
		let path = {
			buf.clear();
			r.read_nul_str(&mut buf)?.to_vec()
		};

		let size = r.read_i64::<LE>()?;
		let _crc = r.read_u32::<LE>()?;

		if let Some(entry) = GmaEntry::try_new(&conf.out, path, size) {
			#[cfg(feature = "binary")] {
				total_size += entry.size as u64;
			}

			file_index.push(entry);
		}
	}

	// File contents
	{
		#[cfg(feature = "binary")]
		let mut progress = crate::util::ProgressPrinter::new(total_size, "Extracting file contents...");

		let memory_used = AtomicUsize::new(0);
		let error = Mutex::new(None);
		std::thread::scope(|s| {
			let mut r = r;

			for GmaEntry { path, size } in file_index.iter() {
				// Break early if an error occurs
				match error.try_lock().as_deref() {
					Ok(None) => {}
					Ok(Some(_)) | Err(std::sync::TryLockError::WouldBlock) => break,
					Err(err @ std::sync::TryLockError::Poisoned(_)) => Err(err).unwrap(),
				}

				let can_buffer = |memory_used| {
					let new_memory_used = memory_used + *size;
					if new_memory_used <= conf.max_io_memory_usage.get() {
						Some(new_memory_used)
					} else {
						None
					}
				};

				if *size <= conf.max_io_memory_usage.get()
					&& memory_used
						.fetch_update(std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst, can_buffer)
						.is_ok()
				{
					let mut buf = Vec::with_capacity(*size);

					let mut take = r.take(*size as u64);
					take.read_to_end(&mut buf)?;
					r = take.into_inner();

					let memory_used = &memory_used;
					let error = &error;
					s.spawn(move || {
						let res = (move || {
							if let Some(parent) = path.parent() {
								if parent != conf.out {
									std::fs::create_dir_all(parent)?;
								}
							}

							std::fs::write(path, buf)?;

							Ok::<_, anyhow::Error>(())
						})();

						memory_used.fetch_sub(*size, std::sync::atomic::Ordering::SeqCst);

						if let Err(err) = res {
							*error.lock().unwrap() = Some(err);
						}
					});
				} else {
					// Just do it without buffering
					if let Some(parent) = path.parent() {
						if parent != conf.out {
							std::fs::create_dir_all(parent)?;
						}
					}

					let mut take = r.take(*size as u64);
					let mut w = File::create(path)?;
					std::io::copy(&mut take, &mut w)?;
					w.flush()?;
					r = take.into_inner();
				}

				#[cfg(feature = "binary")]
				progress.add_progress(*size as u64);
			}

			Ok::<_, anyhow::Error>(())
		})?;
		if let Some(err) = error.into_inner().unwrap() {
			return Err(err);
		}
	}

	// Explicitly free memory here
	// We may exit the process in done_callback (thereby allowing the OS to free the memory),
	// so make sure the optimiser knows to free all the memory here.
	done_callback();
	drop(addon_json_path);
	drop(addon_json);
	drop(file_index);
	drop(buf);

	Ok(())
}

pub fn extract_gma(conf: &ExtractGmadConfig, r: &mut impl BufRead) -> Result<(), anyhow::Error> {
	extract_gma_with_done_callback(conf, r, &mut || ())
}
