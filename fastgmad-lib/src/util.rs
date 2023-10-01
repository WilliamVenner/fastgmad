use std::{
	io::{BufRead, Write, Seek, SeekFrom, StdinLock, BufReader},
	path::Path, fs::File,
};

pub fn is_hidden_file(path: &Path) -> Result<bool, std::io::Error> {
	let hidden;

	#[cfg(unix)]
	{
		if let Some(file_name) = path.file_name() {
			use std::os::unix::prelude::OsStrExt;
			hidden = file_name.as_bytes().starts_with(b".");
		} else {
			hidden = false;
		}
	}

	#[cfg(windows)]
	{
		use std::os::windows::fs::MetadataExt;
		const HIDDEN: u32 = 0x00000002;
		hidden = std::fs::metadata(path)?.file_attributes() & HIDDEN != 0;
	}

	Ok(hidden)
}

pub trait WriteEx: Write {
	fn write_nul_str(&mut self, bytes: &[u8]) -> Result<(), std::io::Error>;
}
impl<W: Write> WriteEx for W {
	fn write_nul_str(&mut self, bytes: &[u8]) -> Result<(), std::io::Error> {
		self.write_all(bytes)?; // Write the bytes
		self.write_all(&[0u8])?; // Write the null terminator
		Ok(())
	}
}

pub trait BufReadEx: BufRead {
	fn skip_until(&mut self, delim: u8) -> Result<usize, std::io::Error> {
		// https://github.com/rust-lang/rust/pull/98943
		let mut read = 0;
		loop {
			let (done, used) = {
				let available = match self.fill_buf() {
					Ok(n) => n,
					Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
					Err(e) => return Err(e),
				};
				match memchr::memchr(delim, available) {
					Some(i) => (true, i + 1),
					None => (false, available.len()),
				}
			};
			self.consume(used);
			read += used;
			if done || used == 0 {
				return Ok(read);
			}
		}
	}

	fn read_nul_str<'a>(&mut self, buf: &'a mut Vec<u8>) -> Result<&'a mut [u8], std::io::Error>;
	fn skip_nul_str(&mut self) -> Result<(), std::io::Error>;
}
impl<R: BufRead> BufReadEx for R {
	fn read_nul_str<'a>(&mut self, buf: &'a mut Vec<u8>) -> Result<&'a mut [u8], std::io::Error> {
		let read = self.read_until(0u8, buf)?;
		Ok(&mut buf[0..read.saturating_sub(1)])
	}

	fn skip_nul_str(&mut self) -> Result<(), std::io::Error> {
		self.skip_until(0).map(|_| ())
	}
}

pub trait IoSkip {
	fn skip(&mut self, bytes: u64) -> Result<(), std::io::Error>;
}
impl IoSkip for File {
	fn skip(&mut self, bytes: u64) -> Result<(), std::io::Error> {
		let pos = self.stream_position()?;
		self.seek(SeekFrom::Start(pos + bytes))?;
		Ok(())
	}
}
impl IoSkip for BufReader<File> {
	fn skip(&mut self, bytes: u64) -> Result<(), std::io::Error> {
		let pos = self.stream_position()?;
		self.seek(SeekFrom::Start(pos + bytes))?;
		Ok(())
	}
}
impl IoSkip for StdinLock<'_> {
	fn skip(&mut self, bytes: u64) -> Result<(), std::io::Error> {
		let mut consumed = 0;
		while consumed < bytes {
			let buffered = self.fill_buf()?;
			let buffered = buffered.len();
			let consume = (bytes - consumed).min(buffered as u64);
			self.consume(consume as _);
			consumed += consume;
		}
		Ok(())
	}
}

#[cfg(feature = "binary")]
mod binary {
	use super::*;

	pub struct PrintHelp(pub Option<&'static str>);

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
}
#[cfg(feature = "binary")]
pub use binary::*;
