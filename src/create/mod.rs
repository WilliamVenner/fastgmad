use std::path::Path;

pub mod parallel;
pub mod standard;
pub mod conf;

#[derive(serde::Deserialize)]
struct AddonJson {
	#[serde(skip)]
	json: String,
	title: String,
	ignore: Vec<String>,
}
impl AddonJson {
	fn read(path: &Path) -> Result<Self, std::io::Error> {
		let json = std::fs::read_to_string(path)?;

		let mut addon_json: AddonJson = serde_json::from_str(&json)
			.map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Failed to parse addon.json: {err}")))?;

		addon_json.json = json;

		Ok(addon_json)
	}
}
