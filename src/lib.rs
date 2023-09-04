const GMA_MAGIC: &[u8] = b"GMAD";
const GMA_VERSION: u8 = 3;

#[macro_use]
mod util;

#[cfg(test)]
mod tests {
	mod create_gma;
}

pub mod create;
pub mod extract;
pub mod publish;
pub mod whitelist;

pub use util::PrintHelp;