use std::path::PathBuf;
use crate::create::CreateGmaConfig;

pub enum WorkshopPublishAddonSrc {
	/// Directly publish a .GMA
	Gma(PathBuf),

	/// Create a .GMA from an addon folder, then publish
	Folder(CreateGmaConfig),
}

/// Options for publishing a new addon to the Workshop
pub struct WorkshopPublishConfig {
	/// Path to the addon .GMA file
	pub addon: WorkshopPublishAddonSrc,

	/// Path to the addon icon file
	///
	/// When publishing, if `None`, a default will be provided by this library
	///
	/// When updating, if `None`, the addon's icon will not be updated
	pub icon: Option<PathBuf>,
}
impl Default for WorkshopPublishConfig {
	fn default() -> Self {
		Self {
			addon: WorkshopPublishAddonSrc::Gma(PathBuf::new()),
			icon: None,
		}
	}
}

/// Options for updating an existing addon on the Workshop
pub struct WorkshopUpdateConfig {
	/// The Workshop ID of the addon to update
	pub id: u64,

	/// Path to the addon .GMA file
	pub addon: PathBuf,

	/// Path to the addon icon file
	///
	/// If `None`, the addon's icon will not be updated
	pub icon: Option<PathBuf>,

	/// Changelog
	pub changes: Option<String>,
}
impl Default for WorkshopUpdateConfig {
	fn default() -> Self {
		Self {
			id: 0,
			addon: PathBuf::new(),
			icon: None,
			changes: None,
		}
	}
}