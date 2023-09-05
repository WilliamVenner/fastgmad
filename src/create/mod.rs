use std::path::Path;

pub mod parallel;
pub mod standard;

mod conf;
pub use conf::CreateGmadConfig;

#[cfg(feature = "binary")]
pub use conf::CreateGmadOut;

#[derive(serde::Deserialize)]
struct AddonJson {
	#[serde(skip)]
	json: String,
	title: String,
	ignore: Vec<String>,
}
impl AddonJson {
	fn read(path: &Path) -> Result<Self, std::io::Error> {
		let json = std::fs::read_to_string(path).map_err(|err| std::io::Error::new(err.kind(), "Failed to read addon.json"))?;

		let mut addon_json: AddonJson = serde_json::from_str(&json)
			.map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Failed to parse addon.json: {err}")))?;

		addon_json.json = json;

		Ok(addon_json)
	}
}
