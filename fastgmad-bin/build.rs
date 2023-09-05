fn main() {
	println!("cargo:rerun-if-changed=build.rs");
	println!("cargo:rerun-if-changed=src/usage.txt");

	let usage = std::fs::read_to_string("src/usage.txt").unwrap();
	let readme = std::fs::read_to_string("../README.md").unwrap();
	let usage_start = readme.find("<!--BEGINUSAGE><!-->").unwrap() + "<!--BEGINUSAGE><!-->".len();
	let usage_end = readme.find("<!--ENDUSAGE><!-->").unwrap();
	let readme = format!("{}\n```\n{usage}\n```\n{}", &readme[..usage_start], &readme[usage_end..]);
	std::fs::write("../README.md", readme.as_bytes()).unwrap();
}
