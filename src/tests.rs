mod fastgmad {
	pub(super) use crate::*;
}

use fastgmad::{create::CreateGmaConfig, extract::ExtractGmaConfig};
use lazy_static::lazy_static;
use std::{
	fs::{File, OpenOptions},
	io::{BufReader, BufWriter, Cursor, Read, Seek, SeekFrom},
	num::NonZeroUsize,
	path::{Path, PathBuf},
};
use uuid::Uuid;
use zip::ZipArchive;

struct WiremodTestData {
	gmad_gma: PathBuf,
	addon_dir: PathBuf,
}
impl WiremodTestData {
	fn init() -> Self {
		let addon_dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/test_data/wiremod"));
		let gmad_gma = addon_dir.with_extension("gma");
		if !addon_dir.is_dir() {
			// Download Wiremod and unzip it
			let zip = sysreq::get("https://github.com/wiremod/wire/archive/refs/tags/v20201205.zip").unwrap();
			let mut zip = ZipArchive::new(Cursor::new(zip)).unwrap();

			std::fs::create_dir_all(&addon_dir).unwrap();

			for i in 0..zip.len() {
				let mut file = zip.by_index(i).unwrap();
				if !file.is_file() {
					continue;
				}
				let path = file.enclosed_name().unwrap();
				let path = path.strip_prefix(path.components().next().unwrap()).unwrap();
				let file_path = addon_dir.join(path);
				std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
				std::io::copy(&mut file, &mut File::create(file_path).unwrap()).unwrap();
			}
		}
		if !gmad_gma.is_file() {
			std::fs::create_dir_all(gmad_gma.parent().unwrap()).unwrap();

			// Create a GMA file using gmad.exe
			let gma = sysreq::get("https://github.com/wiremod/wire/releases/download/v20201205/wiremod.gma").unwrap();
			std::fs::write(&gmad_gma, gma).unwrap();
		}
		Self { gmad_gma, addon_dir }
	}
}
lazy_static! {
	static ref WIREMOD_TEST_DATA: WiremodTestData = WiremodTestData::init();
	static ref GMA_TEMP_DIR: &'static Path = {
		let dir = std::env::temp_dir().join("fastgmad-tests");

		if dir.is_dir() {
			std::fs::remove_dir_all(&dir).unwrap();
		}
		std::fs::create_dir_all(&dir).unwrap();

		Box::leak(Box::from(dir))
	};
}

#[test]
fn test_extract_wiremod_parallel() {
	let wiremod_test_data = &*WIREMOD_TEST_DATA;

	let mut config = ExtractGmaConfig::default();
	config.out = PathBuf::from(GMA_TEMP_DIR.join(Uuid::new_v4().to_string()));
	config.max_io_threads = config.max_io_threads.max(NonZeroUsize::new(2).unwrap()); // force parallel
	fastgmad::extract::extract_gma(&config, &mut BufReader::new(File::open(&wiremod_test_data.gmad_gma).unwrap())).unwrap();

	verify_extracted_wiremod(&config.out, wiremod_test_data);
}

#[test]
fn test_extract_wiremod_standard() {
	let wiremod_test_data = &*WIREMOD_TEST_DATA;

	let mut config = ExtractGmaConfig::default();
	config.out = PathBuf::from(GMA_TEMP_DIR.join(Uuid::new_v4().to_string()));
	config.max_io_threads = NonZeroUsize::new(1).unwrap(); // force series
	fastgmad::extract::extract_gma(&config, &mut BufReader::new(File::open(&wiremod_test_data.gmad_gma).unwrap())).unwrap();

	verify_extracted_wiremod(&config.out, wiremod_test_data);
}

#[test]
fn test_create_wiremod_parallel() {
	let wiremod_test_data = &*WIREMOD_TEST_DATA;

	let out_path = GMA_TEMP_DIR.join(Uuid::new_v4().to_string()).with_extension("gma");
	let mut gma_file = OpenOptions::new()
		.create(true)
		.write(true)
		.truncate(true)
		.read(true)
		.open(&out_path)
		.unwrap();

	let mut config = CreateGmaConfig::default();
	config.folder = wiremod_test_data.addon_dir.clone();
	config.max_io_threads = config.max_io_threads.max(NonZeroUsize::new(2).unwrap()); // force parallel

	fastgmad::create::seekable_create_gma(&config, &mut BufWriter::new(&mut gma_file)).unwrap();
	gma_file.seek(SeekFrom::Start(0)).unwrap();

	let mut config = ExtractGmaConfig::default();
	config.out = PathBuf::from(GMA_TEMP_DIR.join(Uuid::new_v4().to_string()));
	config.max_io_threads = config.max_io_threads.max(NonZeroUsize::new(2).unwrap()); // force parallel
	fastgmad::extract::extract_gma(&config, &mut BufReader::new(gma_file)).unwrap();

	verify_extracted_wiremod(&config.out, wiremod_test_data);
}

#[test]
fn test_create_wiremod_standard() {
	let wiremod_test_data = &*WIREMOD_TEST_DATA;

	let out_path = GMA_TEMP_DIR.join(Uuid::new_v4().to_string()).with_extension("gma");
	let mut gma_file = OpenOptions::new()
		.create(true)
		.write(true)
		.truncate(true)
		.read(true)
		.open(&out_path)
		.unwrap();

	let mut config = CreateGmaConfig::default();
	config.folder = wiremod_test_data.addon_dir.clone();
	config.max_io_threads = NonZeroUsize::new(1).unwrap(); // force series

	fastgmad::create::create_gma(&config, &mut BufWriter::new(&mut gma_file)).unwrap();
	gma_file.seek(SeekFrom::Start(0)).unwrap();

	let mut config = ExtractGmaConfig::default();
	config.out = PathBuf::from(GMA_TEMP_DIR.join(Uuid::new_v4().to_string()));
	config.max_io_threads = NonZeroUsize::new(1).unwrap(); // force series
	fastgmad::extract::extract_gma(&config, &mut BufReader::new(gma_file)).unwrap();

	verify_extracted_wiremod(&config.out, wiremod_test_data);
}

fn verify_extracted_wiremod(dir: &Path, wiremod_test_data: &WiremodTestData) {
	#[derive(serde::Deserialize)]
	struct AddonJson {
		ignore: Vec<String>,
	}

	let addon_json_path = wiremod_test_data.addon_dir.join("addon.json");
	let ignore = serde_json::from_reader::<_, AddonJson>(BufReader::new(File::open(&addon_json_path).unwrap()))
		.unwrap()
		.ignore;

	for unpacked_entry in walkdir::WalkDir::new(&wiremod_test_data.addon_dir) {
		let unpacked_entry = unpacked_entry.unwrap();
		if !unpacked_entry.file_type().is_file() {
			continue;
		}

		let unpacked_entry = unpacked_entry.into_path();
		if unpacked_entry == addon_json_path {
			continue;
		}

		let relative_path = unpacked_entry.strip_prefix(&wiremod_test_data.addon_dir).unwrap();
		let relative_path_str = relative_path.to_str().unwrap();
		if fastgmad::whitelist::is_ignored(relative_path_str, &ignore) {
			continue;
		}
		assert!(fastgmad::whitelist::check(relative_path_str), "{:?} is not whitelisted", relative_path);

		let packed_entry = dir.join(relative_path);

		assert_eq!(
			std::fs::metadata(&unpacked_entry).unwrap().len(),
			std::fs::metadata(&packed_entry).unwrap().len(),
			"{unpacked_entry:?} size does not match unpacked"
		);

		let mut entry = File::open(&unpacked_entry).unwrap();
		let mut packed_entry = File::open(&packed_entry).unwrap();
		let mut a = [0u8; 1024];
		let mut b = [0u8; 1024];
		loop {
			let read = entry.read(&mut a).unwrap();
			if read == 0 {
				break;
			}

			packed_entry.read_exact(&mut b[..read]).unwrap();

			assert_eq!(&a[..read], &b[..read], "{entry:?} does not match unpacked");
		}
	}
}
