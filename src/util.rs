use std::io::Write;

pub trait WriteEx: Write {
	fn write_nul_str(&mut self, bytes: &[u8]) -> Result<(), std::io::Error>;
}
impl<W: Write> WriteEx for W {
	fn write_nul_str(&mut self, bytes: &[u8]) -> Result<(), std::io::Error> {
		self.write_all(bytes)?; // Write the bytes
		self.write_all(&[0u8])?; // Write the null terminator
		Ok(())
	}
}

#[cfg(all(not(bench), not(debug_assertions), test))]
#[allow(unused)]
#[macro_export]
macro_rules! perf {
	([$name:literal] => $code:expr) => {{
		let now = std::time::Instant::now();
		let ret = $code;
		let elapsed = now.elapsed();
		println!(concat!('[', $name, "]: {:?}"), elapsed);
		ret
	}};
}

#[cfg(not(all(not(bench), not(debug_assertions), test)))]
#[allow(unused)]
#[macro_export]
macro_rules! perf {
	([$name:literal] => $code:expr) => {$code};
}