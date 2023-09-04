use crate::{
	create::{conf::CreateGmadConfig, AddonJson},
	util::WriteEx,
	whitelist,
};
use std::{
	fs::File,
	io::{BufReader, Write},
	path::PathBuf,
	time::SystemTime,
};

struct GmaFileEntry {
	path: PathBuf,
	relative_path: String,
	size: u64,
}

pub fn create_gma_with_done_callback(conf: &CreateGmadConfig, w: &mut impl Write, done_callback: &mut dyn FnMut()) -> Result<(), anyhow::Error> {
	let addon_json = perf!(["addon.json"] => AddonJson::read(&conf.folder.join("addon.json"))?);

	let entries = perf!(["entry discovery"] => {
		let mut entries = Vec::new();
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

			entries.push(GmaFileEntry {
				path: path.to_owned(),
				relative_path,
				size: entry.metadata()?.len(),
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

	// Write entries
	perf!(["write entries"] => {
		for GmaFileEntry { path, .. } in entries.iter() {
			std::io::copy(&mut BufReader::new(File::open(path)?), w)?;
		}
	});

	// Explicitly free memory here
	// We may exit the process in done_callback (thereby allowing the OS to free the memory),
	// so make sure the optimiser knows to free all the memory here.
	done_callback();
	drop(entries);
	drop(addon_json);

	Ok(())
}

pub fn create_gma(conf: &CreateGmadConfig, w: &mut impl Write) -> Result<(), anyhow::Error> {
	create_gma_with_done_callback(conf, w, &mut || ())
}
