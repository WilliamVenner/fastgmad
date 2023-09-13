pub struct BinaryConfig {
	pub no_progress: bool,
}
impl Default for BinaryConfig {
	fn default() -> Self {
		Self { no_progress: false }
	}
}