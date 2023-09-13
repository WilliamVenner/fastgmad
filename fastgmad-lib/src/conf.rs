use std::num::NonZeroUsize;

macro_rules! nonzero {
	($nonzero:ident::new($expr:expr)) => {
		match $nonzero::new($expr) {
			Some(nonzero) => nonzero,
			None => panic!(),
		}
	};
}

pub struct ParalellismConfig {
	/// The maximum number of threads to use for I/O
	pub max_io_threads: NonZeroUsize,

	/// The maximum amount of memory to use for I/O
	pub max_io_memory_usage: NonZeroUsize,
}
impl Default for ParalellismConfig {
	fn default() -> Self {
		Self {
			max_io_threads: std::thread::available_parallelism().unwrap_or_else(|_| nonzero!(NonZeroUsize::new(1))),
			max_io_memory_usage: nonzero!(NonZeroUsize::new(2147483648)), // 2 GiB
		}
	}
}
