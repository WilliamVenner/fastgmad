#![allow(clippy::unnecessary_literal_unwrap)]

use fastgmad::{
	bin_prelude::*,
	create::{CreateGmaConfig, CreateGmadOut},
	error::{FastGmadError, FastGmadErrorKind},
	extract::{ExtractGmaConfig, ExtractGmadIn},
	workshop::{WorkshopPublishConfig, WorkshopUpdateConfig},
};
use std::{
	ffi::OsStr,
	fs::File,
	io::{BufReader, BufWriter, Write},
	path::{Path, PathBuf},
	time::Instant,
};

fn main() {
	log::set_logger({
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
		Box::leak(Box::new(Logger(Instant::now())))
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

		Err(FastGmadBinError::FastGmadError(FastGmadError {
			kind: FastGmadErrorKind::Libloading(err),
			..
		})) => {
			eprintln!();
			log::error!("Error loading shared libraries for Workshop publishing: {err}");
			if cfg!(target_os = "windows") {
				log::error!("fastgmad comes with two additional DLLs: steam_api64.dll and fastgmad_publish.dll");
				log::error!("Make sure these DLL files are present in the same directory as fastgmad, otherwise Workshop publishing will not work");
			} else if cfg!(target_os = "linux") {
				log::error!("fastgmad comes with two additional shared libraries: libsteam_api.so and libfastgmad_publish.so");
				log::error!("Make sure these shared libraries are present in the same directory & dynamic linker search path as fastgmad, otherwise Workshop publishing will not work");
			} else if cfg!(target_os = "macos") {
				log::error!("fastgmad comes with two additional shared libraries: libsteam_api.dylib and libfastgmad_publish.dylib");
				log::error!(
					"Make sure these shared libraries are present in the same directory as fastgmad, otherwise Workshop publishing will not work"
				);
			} else {
				log::error!("fastgmad comes with two additional shared libraries");
				log::error!("Make sure these shared libraries are present in the same directory & dynamic linker search path as fastgmad, otherwise Workshop publishing will not work");
			}
			log::error!("Additionally, it is not recommended to install fastgmad directly in the bin directory of Garry's Mod, as Garry's Mod itself may use a different version of the Steam API and updates can break this. If you have done this, and replaced files in the process, you may have broken your game and will need to verify integrity cache.");
			eprintln!();
			Err(err).unwrap()
		}

		Err(FastGmadBinError::FastGmadError(err)) => {
			eprintln!();
			log::error!("{err}\n");
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
			log::info!("Opening output file...");
			let mut w = BufWriter::new(File::create(&path).map_err(|error| FastGmadError {
				kind: FastGmadErrorKind::PathIoError { path, error },
				context: Some("opening output file".to_string()),
			})?);
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
			log::info!("Opening input file...");
			let mut r = BufReader::new(File::open(&path).map_err(|error| FastGmadError {
				kind: FastGmadErrorKind::PathIoError { path, error },
				context: Some("opening input file".to_string()),
			})?);
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
	// TODO allow both creation+publishing in a single command
	let id = fastgmad::workshop::publish_gma(&conf)?;
	println!("{}", id);
	log::info!("Published to https://steamcommunity.com/sharedfiles/filedetails/?id={}", id);
	Ok(())
}

fn update(conf: WorkshopUpdateConfig) -> Result<(), FastGmadBinError> {
	log::warn!(
		">> You are UPDATING the Workshop item https://steamcommunity.com/sharedfiles/filedetails/?id={} <<\n",
		conf.id
	);
	fastgmad::workshop::update_gma(&conf)?;
	println!("{}", conf.id);
	log::info!("Updated https://steamcommunity.com/sharedfiles/filedetails/?id={}", conf.id);
	Ok(())
}

enum FastGmadBinError {
	FastGmadError(FastGmadError),
	PrintHelp(Option<&'static str>),
}
impl From<FastGmadError> for FastGmadBinError {
	fn from(e: FastGmadError) -> Self {
		Self::FastGmadError(e)
	}
}
impl From<PrintHelp> for FastGmadBinError {
	fn from(e: PrintHelp) -> Self {
		Self::PrintHelp(e.0)
	}
}
