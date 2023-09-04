const HELP: &str = concat!(
	r#"
Usage:

	Drag'n'drop a .gma onto the gmad.exe to extract it
	Drag'n'drop a folder onto the gmad.exe to convert it to a .gma

	fastgmad create -folder path/to/folder -out path/to/gma.gma
"#
);

use fastgmad::{
	create::conf::{CreateGmadConfig, CreateGmadOut},
	PrintHelp,
};
use std::{fs::File, time::Instant};

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

fn bin() -> Result<(), FastGmadBinError> {
	let start = Instant::now();
	let mut exit = || {
		eprintln!("Finished in {:?}", start.elapsed());
		std::process::exit(0);
	};

	let mut args = std::env::args().skip(1);
	let cmd = args.next().ok_or(PrintHelp(None))?;
	match cmd.as_str() {
		"create" => {
			let (conf, out) = CreateGmadConfig::from_args()?;
			let conf = &*Box::leak(Box::new(conf));
			match out {
				CreateGmadOut::File(path) => {
					let mut w = File::create(path)?;
					if conf.max_io_threads.get() == 1 {
						fastgmad::create::standard::create_gma_with_done_callback(conf, &mut w, &mut exit)?;
					} else {
						fastgmad::create::parallel::create_gma_with_done_callback(conf, &mut w, &mut exit)?;
					}
				}

				CreateGmadOut::Stdout => {
					let mut w = std::io::stdout().lock();
					if conf.max_io_threads.get() == 1 {
						fastgmad::create::standard::create_gma_with_done_callback(conf, &mut w, &mut exit)?;
					} else {
						todo!();
						// fastgmad::create::parallel::create_gma_with_done_callback(conf, &mut $w, exit)?;
					}
				}
			}
		}

		"extract" => {}

		_ => return Err(FastGmadBinError::PrintHelp(None)),
	}
	Ok(())
}

fn main() {
	eprintln!(concat!(
		"fastgmad v",
		env!("CARGO_PKG_VERSION"),
		" by Billy\nhttps://github.com/WilliamVenner/fastgmad\n"
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
