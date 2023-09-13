use std::path::PathBuf;
use crate::conf::ParalellismConfig;

/// An entry extracted from a .GMA
pub struct ExtractedGmaEntry {
	/// Relative path to the entry
	pub path: String,

	/// The entry contents
	pub data: Vec<u8>,
}

pub enum ExtractGmaDestination {
	Directory(PathBuf),
	Callback(Box<dyn FnMut(ExtractedGmaEntry)>),
}

/// Options for .GMA extraction
pub struct ExtractGmaConfig {
	pub out: ExtractGmaDestination,

	/// Parallelism options
	pub parallelism: ParalellismConfig,
}
impl Default for ExtractGmaConfig {
	fn default() -> Self {
		Self {
			out: ExtractGmaDestination::Directory(PathBuf::new()),
			parallelism: Default::default()
		}
	}
}
