const HELP: &str = concat!(
	r#"
Drag & Drop
-----------
Drag & drop a .gma onto fastgmad to extract it
Drag & drop a folder onto fastgmad to convert it to .gma

Creating GMAs
-------------
fastgmad create -folder path/to/folder -out path/to/gma.gma
fastgmad create -folder path/to/folder -out path/to/gma.gma
fastgmad create -folder path/to/folder
fastgmad create -folder path/to/folder -stdout

Extracting GMAs
---------------
fastgmad extract -file path/to/gma.gma -out path/to/folder
fastgmad extract -file path/to/gma.gma
fastgmad extract -stdin -out path/to/folder

Publishing GMAs
---------------
>> Adding an icon is OPTIONAL for publishing a new Workshop addon. A default icon will be provided for you if you don't add one.
Accepted Icon Formats: JPG, PNG, GIF
Icon Max Size: 1 MB
Recommended Icon Dimensions: 512x512

fastgmad publish -addon path/to/gma.gma -icon path/to/icon
fastgmad update -id 1337 -addon path/to/gma.gma
fastgmad update -id 1337 -addon path/to/gma.gma -icon path/to/icon
fastgmad update -id 1337 -addon path/to/gma.gma -changes "fixed something"
fastgmad update -id 1337 -addon path/to/gma.gma -changes "fixed something" -icon path/to/icon

Additional flags
----------------
-max-io-threads <integer> - The maximum number of threads to use for reading and writing files. Defaults to the number of logical cores on the system.
-max-io-memory-usage <integer> - The maximum amount of memory to use for reading and writing files in parallel. Defaults to 2 GiB.
-warninvalid - Warns rather than errors if the GMA contains invalid files. Off by default.

Notes
-----
- CRC checking and computation is not a feature. Implementing this would slow down the program for no benefit and it is virtually unused and redundant in Garry's Mod.
"#
);

use fastgmad::{
	create::{CreateGmadConfig, CreateGmadOut},
	extract::{ExtractGmadConfig, ExtractGmadIn},
	workshop::{WorkshopPublishConfig, WorkshopUpdateConfig},
	PrintHelp,
};
use std::{
	ffi::OsStr,
	fs::File,
	io::{BufReader, BufWriter},
	path::{Path, PathBuf},
	time::Instant,
};

fn main() {
	eprintln!(concat!(
		"fastgmad v",
		env!("CARGO_PKG_VERSION"),
		" by Billy\nhttps://github.com/WilliamVenner/fastgmad\n",
		"Prefer to use a GUI? Check out https://github.com/WilliamVenner/gmpublisher\n"
	));
	match bin() {
		Ok(()) => {}

		Err(FastGmadBinError::Error(err)) => {
			eprintln!("{err:#?}\n");
			Err::<(), _>(err).unwrap();
		}

		Err(FastGmadBinError::PrintHelp(msg)) => {
			if let Some(msg) = msg {
				eprintln!("{msg}\n");
			}

			eprintln!("{}", HELP.trim());
		}
	}
}

fn bin() -> Result<(), FastGmadBinError> {
	let start = Instant::now();
	let mut exit = || {
		eprintln!("Finished in {:?}", start.elapsed());
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
			CreateGmadConfig {
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
			ExtractGmadConfig {
				out: path.with_extension(""),
				..Default::default()
			},
			ExtractGmadIn::File(PathBuf::from(cmd)),
			&mut exit,
		)
	} else {
		match cmd.to_str() {
			Some("create") => {
				let (conf, out) = CreateGmadConfig::from_args()?;
				create(conf, out, &mut exit)
			}

			Some("extract") => {
				let (conf, r#in) = ExtractGmadConfig::from_args()?;
				extract(conf, r#in, &mut exit)
			}

			Some("publish") => publish(WorkshopPublishConfig::from_args()?),

			Some("update") => update(WorkshopUpdateConfig::from_args()?),

			_ => Err(FastGmadBinError::PrintHelp(None)),
		}
	}
}

fn create(conf: CreateGmadConfig, out: CreateGmadOut, exit: &mut impl FnMut()) -> Result<(), FastGmadBinError> {
	match out {
		CreateGmadOut::File(path) => {
			let mut w = BufWriter::new(File::create(path)?);
			if conf.max_io_threads.get() == 1 {
				fastgmad::create::standard::create_gma_with_done_callback(&conf, &mut w, exit)?;
			} else {
				fastgmad::create::parallel::create_gma_with_done_callback(&conf, &mut w, exit)?;
			}
		}

		CreateGmadOut::Stdout => {
			let mut w = std::io::stdout().lock();
			if conf.max_io_threads.get() != 1 {
				eprintln!("warning: writing to stdout cannot take advantage of multithreading; ignoring -max-io-threads");
			}

			fastgmad::create::standard::create_gma_with_done_callback(&conf, &mut w, exit)?;
		}
	}
	Ok(())
}

fn extract(conf: ExtractGmadConfig, r#in: ExtractGmadIn, exit: &mut impl FnMut()) -> Result<(), FastGmadBinError> {
	match r#in {
		ExtractGmadIn::File(path) => {
			let mut r = BufReader::new(File::open(path)?);
			if conf.max_io_threads.get() == 1 {
				fastgmad::extract::standard::extract_gma_with_done_callback(&conf, &mut r, exit)?;
			} else {
				fastgmad::extract::parallel::extract_gma_with_done_callback(&conf, &mut r, exit)?;
			}
		}

		ExtractGmadIn::Stdin => {
			let mut r = std::io::stdin().lock();
			if conf.max_io_threads.get() == 1 {
				fastgmad::extract::standard::extract_gma_with_done_callback(&conf, &mut r, exit)?;
			} else {
				fastgmad::extract::parallel::extract_gma_with_done_callback(&conf, &mut r, exit)?;
			}
		}
	}
	Ok(())
}

fn publish(conf: WorkshopPublishConfig) -> Result<(), FastGmadBinError> {
	let id = fastgmad::workshop::publish_gma(&conf)?;
	println!("{}", id.0);
	eprintln!("\nPublished to https://steamcommunity.com/sharedfiles/filedetails/?id={}", id.0);
	Ok(())
}

fn update(conf: WorkshopUpdateConfig) -> Result<(), FastGmadBinError> {
	eprintln!(">> You are UPDATING the Workshop item https://steamcommunity.com/sharedfiles/filedetails/?id={} <<\n", conf.id);
	fastgmad::workshop::update_gma(&conf)?;
	println!("{}", conf.id);
	eprintln!("\nUpdated https://steamcommunity.com/sharedfiles/filedetails/?id={}", conf.id);
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
