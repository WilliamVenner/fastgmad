use std::io::Write;

pub struct ProgressPrinter {
	progress: u64,
	progress_max: u64,
	backspaces: usize,
}
impl ProgressPrinter {
	const PROGRESS_BAR_LEN: usize = 30;

	pub fn new(progress_max: u64) -> Self {
		Self {
			progress_max,
			progress: Default::default(),
			backspaces: 0,
		}
	}

	pub fn add_progress(&mut self, add: u64) {
		self.set_progress(self.progress + add)
	}

	pub fn set_progress(&mut self, progress: u64) {
		self.progress = progress;

		if self.progress_max != 0 {
			let progress_pct = self.progress as f32 / self.progress_max as f32;

			let filled = ((progress_pct * Self::PROGRESS_BAR_LEN as f32) as usize).min(Self::PROGRESS_BAR_LEN);
			let outlined = Self::PROGRESS_BAR_LEN - filled;
			let (filled, outlined) = ("▮".repeat(filled), "▯".repeat(outlined));

			let progress_pct = format!("{filled}{outlined} {:.02}%", progress_pct * 100.0);
			let backspaces = core::mem::replace(&mut self.backspaces, progress_pct.len());

			let mut stderr = std::io::stderr().lock();
			stderr.write_all("\u{8}".repeat(backspaces).as_bytes()).ok();
			stderr.write_all(progress_pct.as_bytes()).ok();
			stderr.flush().ok();
		} else {
			let backspaces = core::mem::replace(&mut self.backspaces, 0);
			if backspaces > 0 {
				let mut stderr = std::io::stderr().lock();
				stderr.write_all("\u{8}".repeat(backspaces).as_bytes()).ok();
				stderr.flush().ok();
			}
		}
	}
}
impl Drop for ProgressPrinter {
	fn drop(&mut self) {
		let mut stderr = std::io::stderr().lock();
		stderr.write_all("\u{8}".repeat(self.backspaces).as_bytes()).ok();
		stderr.write_all(" ".repeat(self.backspaces).as_bytes()).ok();
		stderr.write_all("\u{8}".repeat(self.backspaces).as_bytes()).ok();
		stderr.flush().ok();
	}
}