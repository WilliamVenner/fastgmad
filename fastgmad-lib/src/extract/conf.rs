use std::{num::NonZeroUsize, path::PathBuf};

macro_rules! nonzero {
	($nonzero:ident::new($expr:expr)) => {
		match $nonzero::new($expr) {
			Some(nonzero) => nonzero,
			None => panic!(),
		}
	};
}

/// Options for .GMA extraction
#[derive(Debug)]
pub struct ExtractGmaConfig {
	/// The folder to extract the .GMA to
	pub out: PathBuf,

	/// The maximum number of threads to use for I/O
	pub max_io_threads: NonZeroUsize,

	/// The maximum amount of memory to use for I/O
	pub max_io_memory_usage: NonZeroUsize,

	#[cfg(feature = "binary")]
	pub noprogress: bool,
}
impl ExtractGmaConfig {
	#[cfg(feature = "binary")]
	pub fn from_args() -> Result<(Self, ExtractGmadIn), crate::util::PrintHelp> {
		use crate::util::PrintHelp;

		let mut config = Self::default();
		let mut r#in = None;
		let mut args = std::env::args_os().skip(2);
		while let Some(arg) = args.next() {
			match arg.to_str().ok_or(PrintHelp(Some("Unknown GMAD extraction argument")))? {
				"-max-io-threads" => {
					config.max_io_threads = args
						.next()
						.ok_or(PrintHelp(Some("Expected value for -max-io-threads")))?
						.to_str()
						.and_then(|v| v.parse().ok())
						.ok_or(PrintHelp(Some("Expected integer greater than zero for -max-io-threads")))?;
				}
				"-max-io-memory-usage" => {
					config.max_io_memory_usage = args
						.next()
						.ok_or(PrintHelp(Some("Expected value for -max-io-memory-usage")))?
						.to_str()
						.and_then(|v| v.parse().ok())
						.ok_or(PrintHelp(Some("Expected integer greater than zero for -max-io-memory-usage")))?;
				}
				"-out" => {
					config.out = PathBuf::from(
						args.next()
							.filter(|out| !out.is_empty())
							.ok_or(PrintHelp(Some("Expected a value after -out")))?,
					);
				}
				"-stdin" => {
					r#in = Some(ExtractGmadIn::Stdin);
				}
				"-file" => {
					r#in = Some(ExtractGmadIn::File(
						args.next()
							.filter(|r#in| !r#in.is_empty())
							.map(PathBuf::from)
							.ok_or(PrintHelp(Some("Expected a value after -folder")))?,
					));
				}
				"-noprogress" => {
					config.noprogress = true;
				}
				_ => return Err(PrintHelp(Some("Unknown GMAD extraction argument"))),
			}
		}

		let r#in = r#in.ok_or(PrintHelp(Some("Please provide an input path")))?;

		if config.out.as_os_str().is_empty() {
			if let ExtractGmadIn::File(path) = &r#in {
				let mut dir = path.to_owned();
				dir.set_extension("");
				if !dir.exists() || dir.is_dir() {
					config.out = dir;
				}
			} else {
				return Err(PrintHelp(Some("Please provide an output path")));
			}
		}

		Ok((config, r#in))
	}
}
impl Default for ExtractGmaConfig {
	fn default() -> Self {
		Self {
			out: PathBuf::new(),
			max_io_threads: std::thread::available_parallelism().unwrap_or_else(|_| nonzero!(NonZeroUsize::new(1))),
			max_io_memory_usage: nonzero!(NonZeroUsize::new(2147483648)), // 2 GiB

			#[cfg(feature = "binary")]
			noprogress: false,
		}
	}
}

#[cfg(feature = "binary")]
pub enum ExtractGmadIn {
	Stdin,
	File(PathBuf),
}
