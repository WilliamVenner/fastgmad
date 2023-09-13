use super::ConfError;
use std::{
	env::ArgsOs,
	ffi::{OsStr, OsString},
	iter::{Peekable, Skip},
	ops::Deref,
};

pub trait ArgsConsumer: Sized {
	fn consume_arg(&mut self, arg: BinaryArg<'_>) -> Result<ArgConsumed, ConfError>;

	fn validate(self) -> Result<Self, ConfError> {
		Ok(self)
	}
}
pub enum ArgConsumed {
	Consumed,
	UnknownArg,
}

type ArgsIter = Peekable<Skip<ArgsOs>>;

pub struct BinaryArgs {
	iter: ArgsIter,
}
impl BinaryArgs {
	pub fn new() -> Self {
		Self {
			iter: std::env::args_os().skip(2).peekable(),
		}
	}

	pub fn next(&mut self) -> Option<BinaryArg<'_>> {
		self.iter.next().map(|arg| BinaryArg { iter: &mut self.iter, arg })
	}
}

pub struct BinaryArg<'a> {
	iter: &'a mut ArgsIter,
	arg: OsString,
}
impl BinaryArg<'_> {
	pub fn value(mut self) -> Result<OsString, ConfError> {
		match self.iter.next().filter(|value| !value.as_os_str().is_empty()) {
			Some(value) => Ok(value),
			None => Err(ConfError::print_help(format!("Expected a value after {}", self.arg.to_string_lossy()))),
		}
	}
}
impl Deref for BinaryArg<'_> {
	type Target = OsStr;

	fn deref(&self) -> &Self::Target {
		&self.arg
	}
}
