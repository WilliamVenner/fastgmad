use crate::{
	error::{fastgmad_error, fastgmad_io_error, FastGmadError},
	util::{BufReadEx, IoSkip},
	GMA_MAGIC, GMA_VERSION,
};
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
pub fn extract_gma(conf: &ExtractGmaConfig, r: &mut (impl BufRead + IoSkip)) -> Result<(), FastGmadError> {
	if conf.max_io_threads.get() == 1 {
		StandardExtractGma::extract_gma_with_done_callback(conf, r, &mut || ())
	} else {
		ParallelExtractGma::extract_gma_with_done_callback(conf, r, &mut || ())
	}
}

#[cfg(feature = "binary")]
pub fn extract_gma_with_done_callback(
	conf: &ExtractGmaConfig,
	r: &mut (impl BufRead + IoSkip),
	done_callback: &mut dyn FnMut(),
) -> Result<(), FastGmadError> {
	if conf.max_io_threads.get() == 1 {
		StandardExtractGma::extract_gma_with_done_callback(conf, r, done_callback)
	} else {
		ParallelExtractGma::extract_gma_with_done_callback(conf, r, done_callback)
	}
}

trait ExtractGma {
	fn extract_gma_with_done_callback(
		conf: &ExtractGmaConfig,
		r: &mut (impl BufRead + IoSkip),
		done_callback: &mut dyn FnMut(),
	) -> Result<(), FastGmadError> {
		if conf.out.is_dir() {
			log::warn!(
				"Output directory already exists; files not present in this GMA but present in the existing output directory will NOT be deleted"
			);
		}

		std::fs::create_dir_all(&conf.out).map_err(|error| fastgmad_io_error!(while "creating output directory", error: error, path: conf.out))?;

		log::info!("Reading metadata...");

		let mut buf = Vec::new();

		{
			let mut magic = [0u8; 4];
			let res = r.read_exact(&mut magic);
			if let Err(error) = res {
				if error.kind() != std::io::ErrorKind::UnexpectedEof {
					return Err(fastgmad_io_error!(while "reading GMA magic bytes", error: error));
				}
			}
			if magic != GMA_MAGIC {
				return Err(fastgmad_io_error!(error: std::io::Error::new(std::io::ErrorKind::InvalidData, "File is not in GMA format")));
			}
		}

		let version = r
			.read_u8()
			.map_err(|error| fastgmad_io_error!(while "reading version byte", error: error))?;
		if version != GMA_VERSION {
			log::warn!("File is in GMA version {version}, expected version {GMA_VERSION}, reading anyway...");
		}

		// SteamID (unused)
		r.read_exact(&mut [0u8; 8])
			.map_err(|error| fastgmad_io_error!(while "reading SteamID", error: error))?;

		// Timestamp
		r.read_exact(&mut [0u8; 8])
			.map_err(|error| fastgmad_io_error!(while "reading timestamp", error: error))?;

		if version > 1 {
			// Required content
			loop {
				buf.clear();

				let content = r
					.read_nul_str(&mut buf)
					.map_err(|error| fastgmad_io_error!(while "reading required content", error: error))?;

				if content.is_empty() {
					break;
				}
			}
		}

		// Addon name
		let title = {
			buf.clear();

			let title = r
				.read_nul_str(&mut buf)
				.map_err(|error| fastgmad_io_error!(while "reading addon name", error: error))?;

			title.to_vec()
		};

		// addon.json
		let addon_json = {
			buf.clear();

			let addon_json = r
				.read_nul_str(&mut buf)
				.map_err(|error| fastgmad_io_error!(while "reading addon description", error: error))?;

			addon_json.to_vec()
		};

		// Addon author
		r.skip_nul_str()
			.map_err(|error| fastgmad_io_error!(while "reading addon author", error: error))?;

		// Addon version (unused)
		r.read_exact(&mut [0u8; 4])
			.map_err(|error| fastgmad_io_error!(while "reading addon version", error: error))?;

		log::info!("Writing addon.json...");
		let addon_json_path;
		{
			addon_json_path = conf.out.join("addon.json");
			let mut addon_json_f = BufWriter::new(
				File::create(&addon_json_path)
					.map_err(|error| fastgmad_io_error!(while "creating addon.json file", error: error, path: addon_json_path))?,
			);
			let res = if let Ok(mut kv) = serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(&addon_json) {
				// Add title key if it doesn't exist
				if let serde_json::map::Entry::Vacant(v) = kv.entry("title".to_string()) {
					v.insert(serde_json::Value::String(String::from_utf8_lossy(&title).into_owned()));
				}
				serde_json::to_writer_pretty(&mut addon_json_f, &kv)
			} else {
				serde_json::to_writer_pretty(
					&mut addon_json_f,
					&StubAddonJson {
						title: String::from_utf8_lossy(&title),
						description: String::from_utf8_lossy(&addon_json),
					},
				)
			};
			res.map_err(|error| {
				if let Some(io_error) = error.io_error_kind() {
					fastgmad_io_error!(while "writing addon.json", error: std::io::Error::from(io_error), path: addon_json_path)
				} else {
					fastgmad_error!(while "serializing addon.json", error: error)
				}
			})?;
			addon_json_f
				.flush()
				.map_err(|error| fastgmad_io_error!(while "flushing addon.json", error: error, path: addon_json_path))?;
		}

		// File index
		log::info!("Reading file list...");

		#[cfg(feature = "binary")]
		let mut total_size = 0;

		let mut file_index = Vec::new();
		while r
			.read_u32::<LE>()
			.map_err(|error| fastgmad_io_error!(while "reading entry index", error: error))?
			!= 0
		{
			let path = {
				buf.clear();
				let path = r
					.read_nul_str(&mut buf)
					.map_err(|error| fastgmad_io_error!(while "reading entry path", error: error))?;
				path.to_vec()
			};

			let size = r
				.read_i64::<LE>()
				.map_err(|error| fastgmad_io_error!(while "reading entry size", error: error))?;

			let _crc = r
				.read_u32::<LE>()
				.map_err(|error| fastgmad_io_error!(while "reading entry CRC", error: error))?;

			let entry = GmaEntry::new(&conf.out, path, size)?;

			#[cfg(feature = "binary")]
			{
				total_size += entry.size as u64;
			}

			file_index.push(entry);
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
		r: &mut (impl BufRead + IoSkip),
		#[cfg(feature = "binary")] total_size: u64,
		file_index: &[GmaEntry],
	) -> Result<(), FastGmadError>;
}

struct StandardExtractGma;
impl ExtractGma for StandardExtractGma {
	fn write_entries(
		conf: &ExtractGmaConfig,
		mut r: &mut (impl BufRead + IoSkip),
		#[cfg(feature = "binary")] total_size: u64,
		file_index: &[GmaEntry],
	) -> Result<(), FastGmadError> {
		#[cfg(feature = "binary")]
		let mut progress = if !conf.noprogress {
			Some(crate::util::ProgressPrinter::new(total_size))
		} else {
			None
		};

		for GmaEntry { path, size } in file_index.iter() {
			let path = match path {
				Some(path) => path,
				None => {
					// Skip past the entry if we couldn't get a path for it
					r.skip(*size as u64)
						.map_err(|error| fastgmad_io_error!(while "skipping past GMA entry data", error: error))?;
					continue;
				}
			};

			if let Some(parent) = path.parent() {
				if parent != conf.out {
					std::fs::create_dir_all(parent)
						.map_err(|error| fastgmad_io_error!(while "creating directory for GMA entry", error: error, path: parent))?;
				}
			}

			let mut take = r.take(*size as u64);
			let mut w = File::create(path).map_err(|error| fastgmad_io_error!(while "creating file for GMA entry", error: error, path: path))?;

			std::io::copy(&mut take, &mut w).map_err(|error| fastgmad_io_error!(while "copying GMA entry data", error: error, path: path))?;

			w.flush()
				.map_err(|error| fastgmad_io_error!(while "flushing GMA entry file", error: error, path: path))?;

			r = take.into_inner();

			#[cfg(feature = "binary")]
			if let Some(progress) = &mut progress {
				progress.add_progress(*size as u64);
			}
		}

		Ok(())
	}
}

struct ParallelExtractGma;
impl ExtractGma for ParallelExtractGma {
	fn write_entries(
		conf: &ExtractGmaConfig,
		r: &mut (impl BufRead + IoSkip),
		#[cfg(feature = "binary")] total_size: u64,
		file_index: &[GmaEntry],
	) -> Result<(), FastGmadError> {
		#[cfg(feature = "binary")]
		let mut progress = if !conf.noprogress {
			Some(crate::util::ProgressPrinter::new(total_size))
		} else {
			None
		};

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

				let path = match path {
					Some(path) => path,
					None => {
						// Skip past the entry if we couldn't get a path for it
						r.skip(*size as u64)
							.map_err(|error| fastgmad_io_error!(while "skipping past GMA entry data", error: error))?;
						continue;
					}
				};

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
					take.read_to_end(&mut buf)
						.map_err(|error| fastgmad_io_error!(while "reading GMA entry data", error: error, path: path))?;
					r = take.into_inner();

					let memory_used = &memory_used;
					let error = &error;
					s.spawn(move || {
						let res = (move || {
							if let Some(parent) = path.parent() {
								if parent != conf.out {
									std::fs::create_dir_all(parent)
										.map_err(|error| fastgmad_io_error!(while "creating directory for GMA entry", error: error, path: parent))?;
								}
							}

							std::fs::write(path, buf)
								.map_err(|error| fastgmad_io_error!(while "writing GMA entry file", error: error, path: path))?;

							Ok::<_, FastGmadError>(())
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
							std::fs::create_dir_all(parent)
								.map_err(|error| fastgmad_io_error!(while "creating directory for GMA entry", error: error, path: parent))?;
						}
					}

					let mut take = r.take(*size as u64);
					let mut w =
						File::create(path).map_err(|error| fastgmad_io_error!(while "creating file for GMA entry", error: error, path: path))?;
					std::io::copy(&mut take, &mut w).map_err(|error| fastgmad_io_error!(while "copying GMA entry data", error: error, path: path))?;
					w.flush()
						.map_err(|error| fastgmad_io_error!(while "flushing GMA entry file", error: error, path: path))?;
					r = take.into_inner();
				}

				#[cfg(feature = "binary")]
				if let Some(progress) = &mut progress {
					progress.add_progress(*size as u64);
				}
			}

			Ok::<_, FastGmadError>(())
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
	path: Option<PathBuf>,
	size: usize,
}
impl GmaEntry {
	fn new(base_path: &Path, path: Vec<u8>, size: i64) -> Result<Self, FastGmadError> {
		let path = {
			#[cfg(unix)]
			{
				use std::{ffi::OsString, os::unix::ffi::OsStringExt};
				let path = OsString::from_vec(path);
				Some(path)
			}
			#[cfg(windows)]
			{
				use std::{ffi::OsString, os::windows::ffi::OsStringExt};
				match crate::util::ansi_to_wide(&path) {
					Ok(path) => Some(OsString::from_wide(&path)),
					Err(err) => {
						log::info!(
							"warning: skipping GMA entry with incompatible file path: {:?} ({err})",
							String::from_utf8_lossy(&path),
							err
						);
						None
					}
				}
			}
			#[cfg(not(any(unix, windows)))]
			{
				match String::from_utf8(path) {
					Ok(path) => Some(PathBuf::from(path)),
					Err(err) => {
						log::info!(
							"warning: skipping GMA entry with non-UTF-8 file path: {:?}",
							String::from_utf8_lossy(err.as_bytes())
						);
						None
					}
				}
			}
		};

		let size = match usize::try_from(size) {
			Ok(size) => size,
			Err(_) => {
				let error = std::io::Error::new(
					std::io::ErrorKind::InvalidData,
					format!("Unsupported file size for this system ({size} bytes > max {} bytes)", usize::MAX),
				);
				if let Some(path) = path {
					return Err(fastgmad_io_error!(while "reading GMA entry size", error: error, path: path));
				} else {
					return Err(fastgmad_io_error!(while "reading GMA entry size", error: error));
				}
			}
		};

		let path = path.and_then(|path| {
			let path = Path::new(&path);
			if path.components().any(|c| matches!(c, Component::ParentDir | Component::Prefix(_))) {
				log::warn!("Skipping GMA entry with invalid file path: {:?}", path);
				None
			} else {
				Some(base_path.join(path))
			}
		});

		Ok(Self { path, size })
	}
}
