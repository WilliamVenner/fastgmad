use std::path::PathBuf;

/// An error that occurred in FastGMAD, with an optional context string
#[derive(Debug)]
pub struct FastGmadError {
	/// The kind of error that occurred
	pub kind: FastGmadErrorKind,

	/// An optional context string
	pub context: Option<String>
}
impl std::fmt::Display for FastGmadError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if let Some(context) = &self.context {
			write!(f, "{} while {}", self.kind, context)
		} else {
			write!(f, "{}", self.kind)
		}
	}
}
impl std::error::Error for FastGmadError {}

/// Errors that can occur in FastGMAD
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum FastGmadErrorKind {
	#[error("File {0} not in GMA whitelist - see https://wiki.facepunch.com/gmod/Workshop_Addon_Creation")]
	/// GMA entry not in whitelist
	EntryNotWhitelisted(String),

	#[error("JSON error ({0})")]
	/// serde_json error
	JsonError(#[from] serde_json::Error),

	#[error("I/O error ({0})")]
	/// I/O error
	IoError(std::io::Error),

	#[error("I/O error ({error}) (from path \"{path}\")")]
	/// I/O error (with an associated path)
	PathIoError {
		/// The path associated with the I/O error
		path: PathBuf,
		/// The I/O error
		error: std::io::Error,
	},

	#[error("I/O error ({error}) (from path \"{a}\" and path \"{b}\")")]
	/// I/O error (with two associated paths)
	DoublePathIoError {
		/// First path associated with the I/O error
		a: PathBuf,
		/// Second path associated with the I/O error
		b: PathBuf,
		/// The I/O error
		error: std::io::Error,
	},

	#[cfg(feature = "binary")]
	#[error("Shared library error ({0})")]
	/// Shared library error
	Libloading(#[from] libloading::Error),

	#[cfg(feature = "workshop")]
	#[error("Steam error ({0})")]
	/// Steam error during publishing
	SteamError(String),

	#[cfg(feature = "workshop")]
	#[error("Icon too large")]
	/// Icon too large
	IconTooLarge,

	#[cfg(feature = "workshop")]
	#[error("Icon too small")]
	/// Icon too small
	IconTooSmall,
}
impl From<(PathBuf, std::io::Error)> for FastGmadErrorKind {
	fn from((path, error): (PathBuf, std::io::Error)) -> Self {
		Self::PathIoError {
			path,
			error,
		}
	}
}

macro_rules! fastgmad_error {
	(
		while $while:expr,
		error: $kind:expr
	) => {
		crate::error::FastGmadError {
			kind: {
				#[allow(unused_imports)]
				use crate::error::FastGmadErrorKind::*;
				$kind.into()
			},
			context: Some($while.into())
		}
	};

	(error: $kind:expr) => {
		crate::error::FastGmadError {
			kind: {
				#[allow(unused_imports)]
				use crate::error::FastGmadErrorKind::*;
				$kind.into()
			},
			context: None
		}
	};
}
pub(crate) use fastgmad_error;

macro_rules! fastgmad_io_error {
	{
		while $while:expr,
		error: $error:expr
	} => {
		crate::error::FastGmadError {
			kind: crate::error::FastGmadErrorKind::IoError($error),
			context: Some($while.into())
		}
	};

	{
		error: $error:expr
	} => {
		crate::error::FastGmadError {
			kind: crate::error::FastGmadErrorKind::IoError($error),
			context: None
		}
	};

	{
		while $while:expr,
		error: $error:expr,
		path: $path:expr
	} => {
		crate::error::FastGmadError {
			kind: crate::error::FastGmadErrorKind::PathIoError { path: (&$path).into(), error: $error },
			context: Some($while.into())
		}
	};

	{
		error: $error:expr,
		path: $path:expr
	} => {
		crate::error::FastGmadError {
			kind: crate::error::FastGmadErrorKind::PathIoError { path: (&$path).into(), error: $error },
			context: None
		}
	};

	{
		while $while:expr,
		error: $error:expr,
		paths: ($path:expr, $path2:expr)
	} => {
		crate::error::FastGmadError {
			kind: crate::error::FastGmadErrorKind::DoublePathIoError { a: (&$path).into(), b: (&$path2).into(), error: $error },
			context: Some($while.into())
		}
	};

	{
		error: $error:expr,
		paths: ($path:expr, $path2:expr)
	} => {
		crate::error::FastGmadError {
			kind: crate::error::FastGmadErrorKind::DoublePathIoError { a: (&$path).into(), b: (&$path2).into(), error: $error },
			context: None
		}
	};
}
pub(crate) use fastgmad_io_error;