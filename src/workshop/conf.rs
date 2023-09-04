use crate::PrintHelp;
use std::path::PathBuf;

pub struct WorkshopPublishConfig {
	pub addon: PathBuf,
	pub icon: Option<PathBuf>,
}
impl WorkshopPublishConfig {
	pub fn from_args() -> Result<Self, PrintHelp> {
		let mut config = Self {
			addon: PathBuf::new(),
			icon: None,
		};

		let mut args = std::env::args_os().skip(2);
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

				_ => return Err(PrintHelp(Some("Unknown publishing argument"))),
			}
		}

		Ok(config)
	}
}

pub struct WorkshopUpdateConfig {
	pub id: u64,
	pub addon: PathBuf,
	pub icon: Option<PathBuf>,
	pub changes: Option<String>,
}
impl WorkshopUpdateConfig {
	pub fn from_args() -> Result<Self, PrintHelp> {
		let mut config = Self {
			id: 0,
			addon: PathBuf::new(),
			icon: None,
			changes: None,
		};

		let mut args = std::env::args_os().skip(2);
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

				_ => return Err(PrintHelp(Some("Unknown publishing argument"))),
			}
		}

		if config.id == 0 {
			return Err(PrintHelp(Some("-id was empty or missing")));
		}

		Ok(config)
	}
}
