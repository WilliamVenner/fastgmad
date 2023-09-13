pub use libloading;
pub use log;

mod args;
mod conf;
mod progress;
pub use self::args::{ArgConsumed, ArgsConsumer};
pub(crate) use self::progress::ProgressPrinter;

use self::{args::BinaryArg, conf::BinaryConfig};
use crate::{
	conf::ParalellismConfig,
	create::CreateGmaConfig,
	workshop::{WorkshopPublishAddonSrc, WorkshopPublishConfig, WorkshopUpdateConfig}, extract::ExtractGmaConfig,
};
use std::{borrow::Cow, ffi::OsString, path::PathBuf};

pub enum ConfError {
	PrintHelp(Option<Cow<'static, str>>),
	UnknownArg(OsString),
}
impl ConfError {
	pub fn print_help(str: impl Into<Cow<'static, str>>) -> Self {
		Self::PrintHelp(Some(str.into()))
	}
}

impl ArgsConsumer for BinaryConfig {
	fn consume_arg(&mut self, arg: BinaryArg<'_>) -> Result<ArgConsumed, ConfError> {
		match arg.to_str() {
			Some("-noprogress") => {
				self.no_progress = true;
			}

			_ => return Ok(ArgConsumed::UnknownArg),
		}
		Ok(ArgConsumed::Consumed)
	}
}

impl ArgsConsumer for ParalellismConfig {
	fn consume_arg(&mut self, arg: BinaryArg<'_>) -> Result<ArgConsumed, ConfError> {
		match arg.to_str() {
			Some("-max-io-threads") => {
				self.max_io_threads = arg
					.value()?
					.to_str()
					.and_then(|v| v.parse().ok())
					.ok_or(ConfError::print_help("Expected integer greater than zero for -max-io-threads"))?;
			}
			Some("-max-io-memory-usage") => {
				self.max_io_memory_usage = arg
					.value()?
					.to_str()
					.and_then(|v| v.parse().ok())
					.ok_or(ConfError::print_help("Expected integer greater than zero for -max-io-memory-usage"))?;
			}
			_ => return Ok(ArgConsumed::UnknownArg),
		}
		Ok(ArgConsumed::Consumed)
	}
}

impl ArgsConsumer for WorkshopPublishConfig {
	fn consume_arg(&mut self, arg: BinaryArg<'_>) -> Result<ArgConsumed, ConfError> {
		match arg.to_str() {
			Some("-icon") => {
				self.icon = Some(PathBuf::from(arg.value()?));
			}

			Some("-addon") => {
				self.addon = WorkshopPublishAddonSrc::Gma(PathBuf::from(arg.value()?));
			}

			Some("-folder") => {}

			_ => return Ok(ArgConsumed::UnknownArg),
		}
		Ok(ArgConsumed::Consumed)
	}
}

impl ArgsConsumer for WorkshopUpdateConfig {
	fn consume_arg(&mut self, arg: BinaryArg<'_>) -> Result<ArgConsumed, ConfError> {
		match arg.to_str() {
			Some("-id") => {
				self.id = arg
					.value()?
					.to_str()
					.ok_or(ConfError::print_help("-id was not valid UTF-8"))?
					.parse()
					.map_err(|_| ConfError::print_help("-id was not a valid integer"))?;
			}

			Some("-changes") => {
				self.changes = Some(
					arg.value()?
						.to_str()
						.ok_or(ConfError::print_help("-changes was not valid UTF-8"))?
						.to_owned(),
				);
			}

			_ => return Ok(ArgConsumed::UnknownArg),
		}
		Ok(ArgConsumed::Consumed)
	}

	fn validate(self) -> Result<Self, ConfError> {
		if self.id == 0 {
			return Err(ConfError::print_help("-id missing"));
		}

		Ok(self)
	}
}

impl ArgsConsumer for CreateGmaConfig {
	fn consume_arg(&mut self, arg: BinaryArg<'_>) -> Result<ArgConsumed, ConfError> {
		match arg.to_str() {
			Some("-warninvalid") => {
				self.warn_invalid = true;
			}
			Some("-folder") => {
				self.folder = PathBuf::from(arg.value()?);
			}
			_ => return Ok(ArgConsumed::UnknownArg),
		}
		Ok(ArgConsumed::Consumed)
	}
}

impl ArgsConsumer for ExtractGmaConfig {
	fn consume_arg(&mut self, arg: BinaryArg<'_>) -> Result<ArgConsumed, ConfError> {
		Ok(ArgConsumed::Consumed)
	}
}