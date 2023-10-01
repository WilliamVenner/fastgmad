use std::{num::NonZeroUsize, path::PathBuf};

macro_rules! nonzero {
	($nonzero:ident::new($expr:expr)) => {
		match $nonzero::new($expr) {
			Some(nonzero) => nonzero,
			None => panic!(),
		}
	};
}

/// Options for .GMA creation
#[derive(Debug)]
pub struct CreateGmaConfig {
	/// The folder to create a .GMA from
	pub folder: PathBuf,

	/// Whether to warn about invalid files or to throw an error
	pub warn_invalid: bool,

	/// The maximum number of threads to use for I/O
	pub max_io_threads: NonZeroUsize,

	/// The maximum amount of memory to use for I/O
	pub max_io_memory_usage: NonZeroUsize,

	#[cfg(feature = "binary")]
	pub noprogress: bool,
}
impl CreateGmaConfig {
	#[cfg(feature = "binary")]
	pub fn from_args(mut args: impl Iterator<Item = std::ffi::OsString>) -> Result<(Self, CreateGmadOut), crate::util::PrintHelp> {
		use crate::util::PrintHelp;

		let mut config = Self::default();
		let mut out = None;

		while let Some(arg) = args.next() {
			match arg.to_str().ok_or(PrintHelp(Some("Unknown GMAD creation argument")))? {
				"-warninvalid" => {
					config.warn_invalid = true;
				}
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
					out = Some(CreateGmadOut::File(PathBuf::from(
						args.next()
							.filter(|out| !out.is_empty())
							.ok_or(PrintHelp(Some("Expected a value after -out")))?,
					)));
				}
				"-stdout" => {
					out = Some(CreateGmadOut::Stdout);
				}
				"-folder" => {
					config.folder = args
						.next()
						.filter(|folder| !folder.is_empty())
						.map(PathBuf::from)
						.ok_or(PrintHelp(Some("Expected a value after -folder")))?;
				}
				"-noprogress" => {
					config.noprogress = true;
				}
				_ => return Err(PrintHelp(Some("Unknown GMAD creation argument"))),
			}
		}

		if config.folder.as_os_str().is_empty() {
			return Err(PrintHelp(Some("Please provide a folder to create a GMA from")));
		}

		Ok((config, out.ok_or(PrintHelp(Some("Please provide an output path for GMAD creation")))?))
	}
}
impl Default for CreateGmaConfig {
	fn default() -> Self {
		Self {
			folder: PathBuf::new(),
			warn_invalid: false,
			max_io_threads: std::thread::available_parallelism().unwrap_or_else(|_| nonzero!(NonZeroUsize::new(1))),
			max_io_memory_usage: nonzero!(NonZeroUsize::new(2147483648)), // 2 GiB

			#[cfg(feature = "binary")]
			noprogress: false,
		}
	}
}

#[cfg(feature = "binary")]
pub enum CreateGmadOut {
	Stdout,
	File(PathBuf),
}
