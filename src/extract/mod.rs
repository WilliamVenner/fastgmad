use std::{
	borrow::Cow,
	path::{Component, Path, PathBuf},
};

pub mod parallel;
pub mod standard;

mod conf;
pub use conf::ExtractGmadConfig;

#[cfg(feature = "binary")]
pub use conf::ExtractGmadIn;

#[derive(serde::Serialize)]
struct StubAddonJson<'a> {
	title: Cow<'a, str>,
	description: Cow<'a, str>,
}

struct GmaEntry<Size> {
	path: PathBuf,
	size: Size,
}
impl<Size> GmaEntry<Size>
where
	Size: TryFrom<i64>,
{
	fn try_new(base_path: &Path, path: Vec<u8>, size: i64) -> Option<Self> {
		let size = match Size::try_from(size) {
			Ok(size) => size,
			Err(_) => {
				log::warn!("Skipping GMA entry with unsupported file size ({size} bytes): {path:?}");
				return None;
			}
		};

		let path = match String::from_utf8(path) {
			Ok(path) => path,
			Err(err) => {
				log::info!(
					"warning: skipping GMA entry with non-UTF-8 file path: {:?}",
					String::from_utf8_lossy(err.as_bytes())
				);
				return None;
			}
		};

		let path = {
			let path = Path::new(&path);
			if path.components().any(|c| matches!(c, Component::ParentDir | Component::Prefix(_))) {
				log::warn!("Skipping GMA entry with invalid file path: {:?}", path);
				return None;
			}
			base_path.join(path)
		};

		Some(Self { path, size })
	}
}
