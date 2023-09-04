use std::{fs::File, io::BufWriter, path::Path};
use uuid::Uuid;

mod fastgmad {
	pub(super) use crate::*;
}

lazy_static::lazy_static! {
	static ref TEMP_FILE_PATH: &'static Path = {
		let path = std::env::temp_dir().join(format!("fastgmad-tests/create-gmad/{}.gma", Uuid::new_v4()));
		{
			let dir = path.parent().unwrap();
			if dir.is_dir() {
				std::fs::remove_dir_all(dir).unwrap();
			}
			std::fs::create_dir_all(dir).unwrap();
		}
		Box::leak(Box::from(path))
	};
}

fn temp_file() -> BufWriter<File> {
	BufWriter::new(File::create(&*TEMP_FILE_PATH).unwrap())
}

macro_rules! create_gma_tests {
	($($addon:expr => [$standard:ident, $parallel:ident]),*) => {
		$(
			#[test]
			fn $standard() {
				fastgmad::create::standard::create_gma(fastgmad::create::conf::CreateGmadConfig::DEFAULT, $addon, &mut temp_file()).unwrap();
			}

			#[test]
			fn $parallel() {
				fastgmad::create::parallel::create_gma(fastgmad::create::conf::CreateGmadConfig::DEFAULT, $addon, &mut temp_file()).unwrap();
			}
		)*
	};
}
create_gma_tests! {
	concat!(env!("CARGO_MANIFEST_DIR"), "/benches/data/wiremod") => [create_gma_wiremod_standard, create_gma_wiremod_parallel],
	concat!(env!("CARGO_MANIFEST_DIR"), "/benches/data/cs2weaponprops") => [create_gma_cs2weaponprops_standard, create_gma_cs2weaponprops_parallel]
}
