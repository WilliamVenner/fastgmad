use std::{fs::File, io::BufWriter, path::PathBuf};
use uuid::Uuid;

fn temp_file_path() -> PathBuf {
	let path = std::env::temp_dir().join(format!("fastgmad-tests/create-gmad/{}.gma", Uuid::new_v4()));
	std::fs::create_dir_all(path.parent().unwrap()).unwrap();
	path
}

fn temp_file() -> BufWriter<File> {
	BufWriter::new(File::create(temp_file_path()).unwrap())
}

macro_rules! create_gma_tests {
	($($addon:expr => [$standard:ident, $parallel:ident]),*) => {
		$(
			#[test]
			fn $standard() {
				crate::create::standard::create_gma($addon, &mut temp_file()).unwrap();
			}

			#[test]
			fn $parallel() {
				crate::create::parallel::create_gma($addon, &mut temp_file()).unwrap();
			}
		)*
	};
}
create_gma_tests! {
	concat!(env!("CARGO_MANIFEST_DIR"), "/benches/data/wiremod") => [create_gma_wiremod_standard, create_gma_wiremod_parallel],
	concat!(env!("CARGO_MANIFEST_DIR"), "/benches/data/cs2weaponprops") => [create_gma_cs2weaponprops_standard, create_gma_cs2weaponprops_parallel]
}
