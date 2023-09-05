//! Fast gmad and gmpublisher implementation

#![cfg_attr(not(feature = "binary"), warn(missing_docs))]

const GMA_MAGIC: &[u8] = b"GMAD";
const GMA_VERSION: u8 = 3;

#[macro_use]
mod util;

#[cfg(test)]
mod tests;

/// GMA creation
pub mod create;

/// GMA extraction
pub mod extract;

/// GMA file pattern whitelist
pub mod whitelist;

#[cfg(feature = "workshop")]
/// Workshop publishing
pub mod workshop;

#[cfg(feature = "binary")]
pub use util::PrintHelp;

// TODO error struct with file path context and stuff
