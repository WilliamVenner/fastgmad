//! Fast gmad and gmpublish implementation
//!
//! # Feature flags
//!
//! `workshop` - Workshop publishing support
//!
//! `binary` - Recommended if you're using fastgmad in a binary as this enables some binary-related helpers.

#![cfg_attr(not(feature = "binary"), warn(missing_docs))]
#![allow(clippy::unnecessary_literal_unwrap)]

#[cfg(all(feature = "binary", feature = "workshop"))]
compile_error!("Cannot enable both `binary` and `workshop` features (`binary` implies `workshop`)");

const GMA_MAGIC: &[u8] = b"GMAD";
const GMA_VERSION: u8 = 3;

mod util;

/// FastGMAD errors
pub mod error;

#[cfg(test)]
mod tests;

/// GMA creation
pub mod create;

/// GMA extraction
pub mod extract;

/// GMA file pattern whitelist
pub mod whitelist;

#[cfg(any(feature = "binary", feature = "workshop"))]
/// Workshop publishing
pub mod workshop;

#[cfg(feature = "binary")]
pub mod bin_prelude {
	pub use crate::util::PrintHelp;
	pub use libloading;
	pub use log;
}
