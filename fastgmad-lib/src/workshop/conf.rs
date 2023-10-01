use crate::create::CreateGmaConfig;
use std::path::PathBuf;

/// Options for publishing a new addon to the Workshop
pub struct WorkshopPublishConfig {
	/// Path to the addon .GMA file or directory
	pub addon: PathBuf,

	/// Path to the addon icon file
	///
	/// If `None`, a default will be provided by this library
	pub icon: Option<PathBuf>,

	/// Options for creating a .GMA file (if `addon` is a directory)
	pub create_config: Option<CreateGmaConfig>,

	#[cfg(feature = "binary")]
	pub noprogress: bool,
}

/// Options for updating an existing addon on the Workshop
pub struct WorkshopUpdateConfig {
	/// The Workshop ID of the addon to update
	pub id: u64,

	/// Path to the addon .GMA file or directory
	pub addon: PathBuf,

	/// Path to the addon icon file
	///
	/// If `None`, the addon's icon will not be updated
	pub icon: Option<PathBuf>,

	/// Changelog
	pub changes: Option<String>,

	/// Options for creating a .GMA file (if `addon` is a directory)
	pub create_config: Option<CreateGmaConfig>,

	#[cfg(feature = "binary")]
	pub noprogress: bool,
}

#[cfg(feature = "binary")]
mod binary {
	use super::*;
	use crate::util::PrintHelp;
	use std::{ffi::OsString, path::Path};

	impl WorkshopPublishConfig {
		pub fn from_args() -> Result<Self, PrintHelp> {
			let mut config = Self {
				addon: PathBuf::new(),
				icon: None,
				noprogress: false,
				create_config: None,
			};

			let mut args = std::env::args_os().skip(2);
			let mut create_args = Vec::new();
			while let Some(arg) = args.next() {
				match arg.to_str().ok_or(PrintHelp(Some("Unknown publishing argument")))? {
					"-icon" => {
						config.icon = Some(
							args.next()
								.filter(|icon| !icon.is_empty())
								.map(PathBuf::from)
								.ok_or(PrintHelp(Some("Expected a value after -icon")))?,
						);
					}

					"-addon" => {
						config.addon = args
							.next()
							.filter(|addon| !addon.is_empty())
							.map(PathBuf::from)
							.ok_or(PrintHelp(Some("Expected a value after -addon")))?;
					}

					"-noprogress" => {
						config.noprogress = true;
					}

					_ => create_args.push(arg),
				}
			}

			config.create_config = consume_create_args(&config.addon, create_args)?;

			Ok(config)
		}
	}

	impl WorkshopUpdateConfig {
		pub fn from_args() -> Result<Self, PrintHelp> {
			let mut config = Self {
				id: 0,
				addon: PathBuf::new(),
				icon: None,
				changes: None,
				noprogress: false,
				create_config: None,
			};

			let mut args = std::env::args_os().skip(2);
			let mut create_args = Vec::new();
			while let Some(arg) = args.next() {
				match arg.to_str().ok_or(PrintHelp(Some("Unknown publishing argument")))? {
					"-id" => {
						config.id = args
							.next()
							.ok_or(PrintHelp(Some("Expected a value after -id")))?
							.to_str()
							.ok_or(PrintHelp(Some("-id was not valid UTF-8")))?
							.parse()
							.map_err(|_| PrintHelp(Some("-id was not a valid integer")))?;
					}

					"-icon" => {
						config.icon = Some(
							args.next()
								.filter(|addon| !addon.is_empty())
								.map(PathBuf::from)
								.ok_or(PrintHelp(Some("Expected a value after -icon")))?,
						);
					}

					"-addon" => {
						config.addon = args
							.next()
							.filter(|addon| !addon.is_empty())
							.map(PathBuf::from)
							.ok_or(PrintHelp(Some("Expected a value after -addon")))?;
					}

					"-changes" => {
						config.changes = Some(
							args.next()
								.filter(|changes| !changes.is_empty())
								.ok_or(PrintHelp(Some("Expected a value after -changes")))?
								.to_str()
								.ok_or(PrintHelp(Some("-changes was not valid UTF-8")))?
								.to_owned(),
						);
					}

					"-noprogress" => {
						config.noprogress = true;
					}

					_ => create_args.push(arg),
				}
			}

			if config.id == 0 {
				return Err(PrintHelp(Some("-id was empty or missing")));
			}

			config.create_config = consume_create_args(&config.addon, create_args)?;

			Ok(config)
		}
	}

	fn consume_create_args(addon: &Path, mut create_args: Vec<OsString>) -> Result<Option<CreateGmaConfig>, PrintHelp> {
		if addon.is_dir() {
			// Don't let the user provide any output arguments
			if create_args.iter().any(|arg| matches!(arg.to_str(), Some("-out") | Some("-stdout"))) {
				return Err(PrintHelp(Some("Unknown publishing argument")));
			}

			create_args.push(OsString::from("-stdout"));
			create_args.push(OsString::from("-folder"));
			create_args.push(addon.as_os_str().to_owned());

			Ok(Some(CreateGmaConfig::from_args(create_args.into_iter())?.0))
		} else if !create_args.is_empty() {
			Err(PrintHelp(Some("Unknown publishing argument")))
		} else {
			Ok(None)
		}
	}
}
