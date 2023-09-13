use crate::conf::ParalellismConfig;
use std::path::PathBuf;

/// Options for .GMA creation
pub struct CreateGmaConfig {
	/// The folder to create a .GMA from
	pub folder: PathBuf,

	/// Whether to warn about invalid files or to throw an error
	pub warn_invalid: bool,

	/// Paralellism options
	pub parallelism: ParalellismConfig,
}
impl Default for CreateGmaConfig {
	fn default() -> Self {
		Self {
			folder: PathBuf::new(),
			warn_invalid: false,
			parallelism: ParalellismConfig::default(),
		}
	}
}
