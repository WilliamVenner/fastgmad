mod fastgmad {
	pub(super) use crate::*;
}

use std::{fs::File, io::{BufWriter, BufReader}, path::{Path, PathBuf}};
use lazy_static::lazy_static;
use uuid::Uuid;
use fastgmad::{create::CreateGmadConfig, extract::ExtractGmadConfig};

macro_rules! create_gma_tests {
	($($addon:expr => [$standard:ident, $parallel:ident]),*) => {
		lazy_static! {
			static ref CREATE_GMA_TEMP_DIR: &'static Path = {
				let dir = std::env::temp_dir().join("fastgmad-tests/create-gmad");

				if dir.is_dir() {
					std::fs::remove_dir_all(&dir).unwrap();
				}
				std::fs::create_dir_all(&dir).unwrap();

				Box::leak(Box::from(dir))
			};
		}

		fn temp_gma_file() -> BufWriter<File> {
			BufWriter::new(File::create(&*CREATE_GMA_TEMP_DIR.join(format!("{}.gma", Uuid::new_v4()))).unwrap())
		}

		$(
			#[test]
			fn $standard() {
				let mut config = CreateGmadConfig::default();
				config.folder = PathBuf::from($addon);
				fastgmad::create::standard::create_gma(&config, &mut temp_gma_file()).unwrap();
			}

			#[test]
			fn $parallel() {
				let mut config = CreateGmadConfig::default();
				config.folder = PathBuf::from($addon);
				fastgmad::create::parallel::create_gma(&config, &mut temp_gma_file()).unwrap();
			}
		)*
	};
}
create_gma_tests! {
	concat!(env!("CARGO_MANIFEST_DIR"), "/benches/data/wiremod") => [create_gma_wiremod_standard, create_gma_wiremod_parallel],
	concat!(env!("CARGO_MANIFEST_DIR"), "/benches/data/cs2weaponprops") => [create_gma_cs2weaponprops_standard, create_gma_cs2weaponprops_parallel]
}

macro_rules! extract_gma_tests {
	($($addon:expr => [$standard:ident, $parallel:ident]),*) => {
		lazy_static! {
			static ref EXTRACT_GMA_TEMP_DIR: &'static Path = {
				let dir = std::env::temp_dir().join("fastgmad-tests/extract-gmad");

				if dir.is_dir() {
					std::fs::remove_dir_all(&dir).unwrap();
				}
				std::fs::create_dir_all(&dir).unwrap();

				Box::leak(Box::from(dir))
			};
		}

		$(
			#[test]
			fn $standard() {
				let mut config = ExtractGmadConfig::default();
				config.out = PathBuf::from(EXTRACT_GMA_TEMP_DIR.join(Uuid::new_v4().to_string()));
				fastgmad::extract::standard::extract_gma(&config, &mut BufReader::new(File::open($addon).unwrap())).unwrap();
			}

			#[test]
			fn $parallel() {
				let mut config = ExtractGmadConfig::default();
				config.out = PathBuf::from(EXTRACT_GMA_TEMP_DIR.join(Uuid::new_v4().to_string()));
				fastgmad::extract::parallel::extract_gma(&config, &mut BufReader::new(File::open($addon).unwrap())).unwrap();
			}
		)*
	};
}
extract_gma_tests! {
	concat!(env!("CARGO_MANIFEST_DIR"), "/benches/data/wiremod.gma") => [cextract_gma_wiremod_standard, extract_gma_wiremod_parallel],
	concat!(env!("CARGO_MANIFEST_DIR"), "/benches/data/cs2weaponprops.gma") => [extract_gma_cs2weaponprops_standard, extract_gma_cs2weaponprops_parallel]
}