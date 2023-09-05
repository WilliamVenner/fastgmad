const GMA_MAGIC: &[u8] = b"GMAD";
const GMA_VERSION: u8 = 3;

#[macro_use]
mod util;

#[cfg(test)]
mod tests;

pub mod create;
pub mod extract;
pub mod whitelist;

#[cfg(feature = "workshop")]
pub mod workshop;

#[cfg(feature = "binary")]
pub use util::PrintHelp;

// TODO error struct with file path context and stuff
