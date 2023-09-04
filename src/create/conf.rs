use std::num::NonZeroUsize;
use crate::util::PrintHelp;

macro_rules! nonzero {
	($nonzero:ident::new($expr:expr)) => {
		match $nonzero::new($expr) {
			Some(nonzero) => nonzero,
			None => panic!()
		}
	};
}

#[derive(Debug)]
pub struct CreateGmadConfig {
	pub warn_invalid: bool,
	pub max_io_threads: NonZeroUsize,
	pub max_io_memory_usage: NonZeroUsize,
}
impl CreateGmadConfig {
	pub fn from_args() -> Result<Self, PrintHelp> {
		let mut config = Self::default();
		let mut args = std::env::args();
		while let Some(arg) = args.next() {
			match arg.as_str() {
				"-warninvalid" => config.warn_invalid = true,
				"-max-io-threads" => {
					config.max_io_threads = args.next().and_then(|arg| arg.parse().ok()).ok_or(PrintHelp)?;
				},
				"-max-io-memory-usage" => {
					config.max_io_memory_usage = args.next().and_then(|arg| arg.parse().ok()).ok_or(PrintHelp)?;
				},
				_ => return Err(PrintHelp)
			}
		}
		Ok(config)
	}
}
impl CreateGmadConfig {
	pub const DEFAULT: &Self = &Self::default();

	pub const fn default() -> Self {
		Self {
			warn_invalid: false,
			max_io_threads: nonzero!(NonZeroUsize::new(512)),
			max_io_memory_usage: nonzero!(NonZeroUsize::new(2147483648)), // 2 GiB
		}
	}
}