use fastgmad::{
	create::{CreateGmaConfig, CreateGmadOut},
	extract::{ExtractGmaConfig, ExtractGmadIn},
	workshop::{WorkshopPublishConfig, WorkshopUpdateConfig},
	PrintHelp,
};
use std::{
	ffi::OsStr,
	fs::File,
	io::{BufReader, BufWriter, Write},
	path::{Path, PathBuf},
	time::Instant,
};

fn main() {
	log::set_boxed_logger({
		log::set_max_level(log::LevelFilter::Info);

		struct Logger(Instant);
		impl log::Log for Logger {
			fn enabled(&self, metadata: &log::Metadata) -> bool {
				metadata.level() <= log::Level::Info
			}

			fn log(&self, record: &log::Record) {
				let level = match record.level() {
					log::Level::Info => {
						eprintln!("[+{:?}] {}", self.0.elapsed(), record.args());
						return;
					}
					log::Level::Warn => "WARN: ",
					log::Level::Error => "ERROR: ",
					log::Level::Debug => "DEBUG: ",
					log::Level::Trace => "TRACE: ",
				};
				eprintln!("{level}{}", record.args());
			}

			fn flush(&self) {
				std::io::stderr().lock().flush().ok();
			}
		}
		Box::new(Logger(Instant::now()))
	})
	.unwrap();

	eprintln!(concat!(
		"fastgmad v",
		env!("CARGO_PKG_VERSION"),
		" by Billy\nhttps://github.com/WilliamVenner/fastgmad\n",
		"Prefer to use a GUI? Check out https://github.com/WilliamVenner/gmpublisher\n"
	));

	match bin() {
		Ok(()) => {}

		Err(FastGmadBinError::Error(err)) => {
			log::error!("{err:#?}\n");
			Err::<(), _>(err).unwrap();
		}

		Err(FastGmadBinError::PrintHelp(msg)) => {
			if let Some(msg) = msg {
				log::error!("{msg}\n");
			}

			eprintln!("{}", include_str!("usage.txt"));
		}
	}
}

fn bin() -> Result<(), FastGmadBinError> {
	let mut exit = || {
		log::info!("Finished");
		std::process::exit(0);
	};

	let mut args = std::env::args_os().skip(1);

	let cmd = args.next().ok_or(PrintHelp(None))?;
	let path = Path::new(&cmd);

	if path.is_dir() {
		// The first argument is a path to a directory
		// Create a GMA from it
		let out = path.with_extension("gma");
		create(
			CreateGmaConfig {
				folder: PathBuf::from(cmd),
				..Default::default()
			},
			CreateGmadOut::File(out),
			&mut exit,
		)
	} else if path.is_file() && path.extension() == Some(OsStr::new("gma")) {
		// The first argument is a path to a GMA
		// Extract it
		extract(
			ExtractGmaConfig {
				out: path.with_extension(""),
				..Default::default()
			},
			ExtractGmadIn::File(PathBuf::from(cmd)),
			&mut exit,
		)
	} else {
		match cmd.to_str() {
			Some("create") => {
				let (conf, out) = CreateGmaConfig::from_args()?;
				create(conf, out, &mut exit)
			}

			Some("extract") => {
				let (conf, r#in) = ExtractGmaConfig::from_args()?;
				extract(conf, r#in, &mut exit)
			}

			Some("publish") => publish(WorkshopPublishConfig::from_args()?),

			Some("update") => update(WorkshopUpdateConfig::from_args()?),

			_ => Err(FastGmadBinError::PrintHelp(None)),
		}
	}
}

fn create(conf: CreateGmaConfig, out: CreateGmadOut, exit: &mut impl FnMut()) -> Result<(), FastGmadBinError> {
	match out {
		CreateGmadOut::File(path) => {
			let mut w = BufWriter::new(File::create(path)?);
			fastgmad::create::seekable_create_gma_with_done_callback(&conf, &mut w, exit)?;
		}

		CreateGmadOut::Stdout => {
			let mut w = std::io::stdout().lock();
			if conf.max_io_threads.get() != 1 {
				log::warn!("Writing to stdout cannot take advantage of multithreading; ignoring -max-io-threads");
			}

			fastgmad::create::create_gma_with_done_callback(&conf, &mut w, exit)?;
		}
	}
	Ok(())
}

fn extract(conf: ExtractGmaConfig, r#in: ExtractGmadIn, exit: &mut impl FnMut()) -> Result<(), FastGmadBinError> {
	match r#in {
		ExtractGmadIn::File(path) => {
			let mut r = BufReader::new(File::open(path)?);
			fastgmad::extract::extract_gma_with_done_callback(&conf, &mut r, exit)?;
		}

		ExtractGmadIn::Stdin => {
			let mut r = std::io::stdin().lock();
			fastgmad::extract::extract_gma_with_done_callback(&conf, &mut r, exit)?;
		}
	}
	Ok(())
}

fn publish(conf: WorkshopPublishConfig) -> Result<(), FastGmadBinError> {
	let id = fastgmad::workshop::publish_gma(&conf)?;
	println!("{}", id.0);
	log::info!("\nPublished to https://steamcommunity.com/sharedfiles/filedetails/?id={}", id.0);
	Ok(())
}

fn update(conf: WorkshopUpdateConfig) -> Result<(), FastGmadBinError> {
	log::warn!(
		">> You are UPDATING the Workshop item https://steamcommunity.com/sharedfiles/filedetails/?id={} <<\n",
		conf.id
	);
	fastgmad::workshop::update_gma(&conf)?;
	println!("{}", conf.id);
	log::info!("\nUpdated https://steamcommunity.com/sharedfiles/filedetails/?id={}", conf.id);
	Ok(())
}

enum FastGmadBinError {
	PrintHelp(Option<&'static str>),
	Error(anyhow::Error),
}
impl From<anyhow::Error> for FastGmadBinError {
	fn from(e: anyhow::Error) -> Self {
		Self::Error(e)
	}
}
impl From<std::io::Error> for FastGmadBinError {
	#[track_caller]
	fn from(e: std::io::Error) -> Self {
		Self::Error(e.into())
	}
}
impl From<PrintHelp> for FastGmadBinError {
	fn from(e: PrintHelp) -> Self {
		Self::PrintHelp(e.0)
	}
}
