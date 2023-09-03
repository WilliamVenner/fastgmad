use criterion::{criterion_group, criterion_main, Criterion};
use std::{
	fs::File,
	io::BufWriter,
	process::{Command, Stdio},
};
use uuid::Uuid;

fn criterion_benchmark(c: &mut Criterion) {
	let wiremod_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/benches/data/wiremod");
	let cs2weaponprops_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/benches/data/cs2weaponprops");
	let gmad = if cfg!(windows) {
		concat!(env!("CARGO_MANIFEST_DIR"), "/benches/data/gmad/gmad.exe")
	} else if cfg!(target_os = "linux") {
		concat!(env!("CARGO_MANIFEST_DIR"), "/benches/data/gmad/gmad_linux")
	} else {
		unimplemented!();
	};

	let tmp = std::env::temp_dir().join("fastgmad-benches/create-gma");
	if tmp.is_dir() {
		std::fs::remove_dir_all(&tmp).unwrap();
	}
	std::fs::create_dir_all(&tmp).unwrap();

	let temp_file_path = || tmp.join(format!("{}.gma", Uuid::new_v4()));
	let temp_file = || BufWriter::new(File::create(temp_file_path()).unwrap());

	for (group_name, addon_dir) in [("Create Wiremod GMA", wiremod_dir), ("Create CS2 Weapon Props GMA", cs2weaponprops_dir)] {
		let mut group = c.benchmark_group(group_name);
		group.sample_size(10);
		group.sampling_mode(criterion::SamplingMode::Flat);
		group.bench_function("Standard", |b| {
			b.iter(|| {
				fastgmad::create::standard::create_gma(addon_dir, &mut temp_file()).unwrap();
			});
		});
		group.bench_function("Parallel", |b| {
			b.iter(|| {
				fastgmad::create::parallel::create_gma(addon_dir, &mut temp_file()).unwrap();
			});
		});
		group.bench_function("GMAD", |b| {
			b.iter(|| {
				let output = Command::new(gmad)
					.args(&["create", "-folder", addon_dir, "-out", temp_file_path().to_str().unwrap()])
					.stdin(Stdio::null())
					.stdout(Stdio::piped())
					.stderr(Stdio::piped())
					.output()
					.unwrap();

				if !output.status.success() {
					panic!(
						"gmad.exe exited with status {}\n\n======= STDOUT =======\n{}\n\n======= STDERR =======\n{}",
						output.status,
						String::from_utf8_lossy(&output.stdout),
						String::from_utf8_lossy(&output.stderr)
					);
				}
			});
		});
		group.finish();
	}

	std::fs::remove_dir_all(&tmp).unwrap();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
