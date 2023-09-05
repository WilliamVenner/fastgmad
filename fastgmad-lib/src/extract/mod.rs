use crate::{util::BufReadEx, GMA_MAGIC, GMA_VERSION};
use byteorder::{ReadBytesExt, LE};
use std::{
	borrow::Cow,
	fs::File,
	io::{BufRead, BufWriter, Read, Write},
	path::{Component, Path, PathBuf},
	sync::{atomic::AtomicUsize, Mutex},
};

mod conf;
pub use conf::ExtractGmaConfig;

#[cfg(feature = "binary")]
pub use conf::ExtractGmadIn;

/// Extracts a GMA file to a directory.
pub fn extract_gma(conf: &ExtractGmaConfig, r: &mut impl BufRead) -> Result<(), anyhow::Error> {
	if conf.max_io_threads.get() == 1 {
		StandardExtractGma::extract_gma_with_done_callback(conf, r, &mut || ())
	} else {
		ParallelExtractGma::extract_gma_with_done_callback(conf, r, &mut || ())
	}
}

#[cfg(feature = "binary")]
pub fn extract_gma_with_done_callback(conf: &ExtractGmaConfig, r: &mut impl BufRead, done_callback: &mut dyn FnMut()) -> Result<(), anyhow::Error> {
	if conf.max_io_threads.get() == 1 {
		StandardExtractGma::extract_gma_with_done_callback(conf, r, done_callback)
	} else {
		ParallelExtractGma::extract_gma_with_done_callback(conf, r, done_callback)
	}
}

trait ExtractGma {
	fn extract_gma_with_done_callback(conf: &ExtractGmaConfig, r: &mut impl BufRead, done_callback: &mut dyn FnMut()) -> Result<(), anyhow::Error> {
		if conf.out.is_dir() {
			log::warn!(
				"Output directory already exists; files not present in this GMA but present in the existing output directory will NOT be deleted"
			);
		}

		std::fs::create_dir_all(&conf.out)?;

		log::info!("Reading metadata...");

		let mut buf = Vec::new();

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

		let mut file_index = Vec::new();
		while r.read_u32::<LE>()? != 0 {
			let path = {
				buf.clear();
				r.read_nul_str(&mut buf)?.to_vec()
			};

			let size = r.read_i64::<LE>()?;
			let _crc = r.read_u32::<LE>()?;

			if let Some(entry) = GmaEntry::try_new(&conf.out, path, size) {
				#[cfg(feature = "binary")]
				{
					total_size += entry.size as u64;
				}

				file_index.push(entry);
			}
		}

		// File contents
		log::info!("Extracting entries...");

		Self::write_entries(
			conf,
			r,
			#[cfg(feature = "binary")]
			total_size,
			&file_index,
		)?;

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

	fn write_entries(
		conf: &ExtractGmaConfig,
		r: &mut impl BufRead,
		#[cfg(feature = "binary")] total_size: u64,
		file_index: &[GmaEntry],
	) -> Result<(), anyhow::Error>;
}

struct StandardExtractGma;
impl ExtractGma for StandardExtractGma {
	fn write_entries(
		conf: &ExtractGmaConfig,
		mut r: &mut impl BufRead,
		#[cfg(feature = "binary")] total_size: u64,
		file_index: &[GmaEntry],
	) -> Result<(), anyhow::Error> {
		#[cfg(feature = "binary")]
		let mut progress = crate::util::ProgressPrinter::new(total_size);

		for GmaEntry { path, size } in file_index.iter() {
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

			#[cfg(feature = "binary")]
			progress.add_progress(*size as u64);
		}

		Ok(())
	}
}

struct ParallelExtractGma;
impl ExtractGma for ParallelExtractGma {
	fn write_entries(
		conf: &ExtractGmaConfig,
		r: &mut impl BufRead,
		#[cfg(feature = "binary")] total_size: u64,
		file_index: &[GmaEntry],
	) -> Result<(), anyhow::Error> {
		#[cfg(feature = "binary")]
		let mut progress = crate::util::ProgressPrinter::new(total_size);

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

		Ok(())
	}
}

#[derive(serde::Serialize)]
struct StubAddonJson<'a> {
	title: Cow<'a, str>,
	description: Cow<'a, str>,
}

struct GmaEntry {
	path: PathBuf,
	size: usize,
}
impl GmaEntry {
	fn try_new(base_path: &Path, path: Vec<u8>, size: i64) -> Option<Self> {
		let size = match usize::try_from(size) {
			Ok(size) => size,
			Err(_) => {
				log::warn!("Skipping GMA entry with unsupported file size ({size} bytes): {path:?}");
				return None;
			}
		};

		let path = match String::from_utf8(path) {
			Ok(path) => path,
			Err(err) => {
				log::info!(
					"warning: skipping GMA entry with non-UTF-8 file path: {:?}",
					String::from_utf8_lossy(err.as_bytes())
				);
				return None;
			}
		};

		let path = {
			let path = Path::new(&path);
			if path.components().any(|c| matches!(c, Component::ParentDir | Component::Prefix(_))) {
				log::warn!("Skipping GMA entry with invalid file path: {:?}", path);
				return None;
			}
			base_path.join(path)
		};

		Some(Self { path, size })
	}
}
