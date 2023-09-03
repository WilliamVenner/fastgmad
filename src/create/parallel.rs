use super::AddonJson;
use crate::util::WriteEx;
use crate::whitelist;
use rayon::prelude::*;
use std::{
	io::{Seek, SeekFrom, Write},
	path::{Path, PathBuf},
	sync::Arc,
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

pub fn create_gma_with_done_callback(dir: &str, w: &mut (impl Write + Seek), done_callback: fn()) -> Result<(), std::io::Error> {
	let addon_json = perf!(["addon.json"] => AddonJson::read(&Path::new(dir).join("addon.json"))?);

	let entries = perf!(["entry discovery"] => {
		let mut entries = Vec::new();
		let mut prev_offset = 0;
		for entry in walkdir::WalkDir::new(dir).follow_links(true).sort_by_file_name() {
			let entry = entry?;
			if !entry.file_type().is_file() {
				continue;
			}

			let path = entry.path();
			let relative_path = path
				.strip_prefix(Path::new(dir))
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
				return Err(std::io::Error::new(
					std::io::ErrorKind::InvalidData,
					format!(
						"File {} not in GMA whitelist - see https://wiki.facepunch.com/gmod/Workshop_Addon_Creation",
						relative_path
					),
				));
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
	let entries = Arc::new(entries);
	let entries_ref = entries.clone();
	perf!(["write entries"] => {
		rayon::spawn(move || {
			entries_ref.par_iter().for_each_init(
				|| tx.clone(),
				|tx, GmaFileEntry { offset, path, .. }| {
					tx.send((*offset, std::fs::read(path))).ok();
				},
			);
		});

		let contents_ptr = w.stream_position()?;
		while let Ok((offset, contents)) = rx.recv() {
			let contents = contents?;
			w.seek(SeekFrom::Start(contents_ptr + offset))?;
			w.write_all(&contents)?;
		}
		w.flush()?;
	});

	// Explicitly free memory here
	// We may exit the process in done_callback (thereby allowing the OS to free the memory),
	// so make sure the optimiser knows to free all the memory here.
	done_callback();
	drop(entries);
	drop(addon_json);

	Ok(())
}

pub fn create_gma(dir: &str, w: &mut (impl Write + Seek)) -> Result<(), std::io::Error> {
	create_gma_with_done_callback(dir, w, || ())
}
