mod conf;
pub use conf::{WorkshopPublishConfig, WorkshopUpdateConfig};

use crate::{util::BufReadEx, GMA_MAGIC, GMA_VERSION};
use byteorder::ReadBytesExt;
use std::{
	borrow::Cow,
	fs::File,
	io::{BufReader, Read},
	path::Path,
	path::PathBuf,
	time::Duration,
};
use steamworks::{Client, PublishedFileId};
use uuid::Uuid;

const LEGAL_AGREEMENT_MESSAGE: &str = r#"
You must accept the Steam Workshop legal agreement before you can upload addons.
You can do this at https://steamcommunity.com/sharedfiles/workshoplegalagreement
Once you have accepted the agreement, you can upload addons.
"#;

const WORKSHOP_DEFAULT_ICON: &[u8] = include_bytes!("gmpublisher_default_icon.png");

const GMOD_APPID: steamworks::AppId = steamworks::AppId(4000);

#[derive(Clone, Copy)]
enum PublishKind<'a> {
	Create,
	Update { id: PublishedFileId, changes: Option<&'a str> },
}
fn workshop_upload(
	kind: PublishKind,
	addon: &Path,
	icon: Option<&Path>,
) -> Result<PublishedFileId, anyhow::Error> {
	log::info!("Initializing Steam...");
	let (client, single) = Client::init_app(4000)?;

	log::info!("Reading GMA metadata...");
	let mut metadata = GmaPublishingMetadata::try_read(addon)?;

	log::info!("Preparing content folder...");
	let content_path = ContentPath::new(addon)?;

	macro_rules! run_steam_api {
		($callback:ident => $code:expr) => {{
			let (tx, rx) = std::sync::mpsc::sync_channel(1);
			let $callback = move |result| {
				tx.send(result).ok();
			};
			$code;
			loop {
				match rx.recv_timeout(Duration::from_millis(50)) {
					Ok(res) => break res,
					Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
						return Err(steamworks::SteamError::RemoteDisconnect.into());
					}
					Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
						single.run_callbacks();
					}
				}
			}
		}};
	}

	let (file_id, mut legal_agreement_pending) = match &kind {
		PublishKind::Create => {
			log::info!("Creating new Workshop item...");
			run_steam_api!(callback => client.ugc().create_item(GMOD_APPID, steamworks::FileType::Community, callback))?
		},
		PublishKind::Update { id, .. } => (*id, false),
	};

	let icon = match icon {
		Some(icon) => Cow::Borrowed(icon),
		None => {
			log::info!("Preparing icon...");
			let default_icon_path = std::env::temp_dir().join("fastgmad-publish/gmpublisher_default_icon.png");
			std::fs::write(&default_icon_path, WORKSHOP_DEFAULT_ICON)?;
			Cow::Owned(default_icon_path)
		}
	};

	let res = (|| {
		log::info!("Preparing item upload...");
		let mut item_update = client.ugc().start_item_update(GMOD_APPID, file_id);
		// item_update = item_update.visibility(steamworks::PublishedFileVisibility::Private);

		item_update = item_update.tags({
			metadata.tags.push("Addon".to_string());

			if let Some(addon_type) = metadata.addon_type.take() {
				metadata.tags.push(addon_type);
			}

			core::mem::take(&mut metadata.tags)
		});

		let changes = match kind {
			PublishKind::Create => {
				item_update = item_update.title(&metadata.title);

				if let Some(description) = &metadata.description {
					item_update = item_update.description(description.as_str());
				}

				None
			}

			PublishKind::Update { changes, .. } => changes,
		};

		item_update = item_update.preview_path(&icon);
		item_update = item_update.content_path(&content_path.0);

		log::info!("Uploading item...");
		legal_agreement_pending |=
			run_steam_api!(callback => item_update.submit(changes, callback)).map(|(_id, legal_agreement_pending)| legal_agreement_pending)?;

		drop(content_path);

		Ok::<_, steamworks::SteamError>(())
	})();

	if legal_agreement_pending {
		log::info!("\n{}\n", LEGAL_AGREEMENT_MESSAGE.trim());
	}

	res.map(|_| file_id).map_err(|err| {
		if let PublishKind::Create = kind {
			// Delete the item
			let _ = (|| {
				run_steam_api!(callback => client.ugc().delete_item(file_id, callback))?;
				Ok::<_, anyhow::Error>(())
			})();
		}

		anyhow::Error::from(err)
	})
}

pub fn publish_gma(conf: &WorkshopPublishConfig) -> Result<PublishedFileId, anyhow::Error> {
	workshop_upload(PublishKind::Create, &conf.addon, conf.icon.as_deref())
}

pub fn update_gma(conf: &WorkshopUpdateConfig) -> Result<(), anyhow::Error> {
	workshop_upload(
		PublishKind::Update {
			id: PublishedFileId(conf.id),
			changes: conf.changes.as_deref(),
		},
		&conf.addon,
		conf.icon.as_deref(),
	)
	.map(|_| ())
}

// TODO ctrlc handler

struct ContentPath(PathBuf);
impl ContentPath {
	fn new(gma_path: &Path) -> Result<Self, anyhow::Error> {
		let dir = std::env::temp_dir().join(format!("fastgmad-publish/{}", Uuid::new_v4()));

		std::fs::create_dir_all(&dir)?;

		let temp_gma_path = dir.join("fastgmad.gma");

		let symlink_result = {
			#[cfg(windows)]
			{
				let res = std::os::windows::fs::symlink_file(&temp_gma_path, gma_path);
				match &res {
					Err(res) if res.kind() == std::io::ErrorKind::PermissionDenied => {
						log::warn!("Copying .gma to temporary directory for publishing. To skip this in future, run as administrator so that fastgmad can create symlinks.");
					}
					_ => {}
				}
				res
			}
			#[cfg(unix)]
			{
				std::os::unix::fs::symlink(&temp_gma_path, gma_path)
			}
			#[cfg(not(any(windows, unix)))]
			{
				Err(std::io::Error::new(std::io::ErrorKind::Other, "Unsupported platform"))
			}
		};

		if symlink_result.is_err() {
			std::fs::copy(gma_path, temp_gma_path)?;
		}

		Ok(Self(dir))
	}
}
impl Drop for ContentPath {
	fn drop(&mut self) {
		std::fs::remove_dir_all(&self.0).ok();
	}
}

#[derive(Default)]
struct GmaPublishingMetadata {
	title: String,
	addon_type: Option<String>,
	tags: Vec<String>,
	description: Option<String>,
}
impl GmaPublishingMetadata {
	fn try_read(path: &Path) -> Result<Self, anyhow::Error> {
		let mut metadata = Self::default();

		let mut r = BufReader::new(File::open(path)?);

		{
			let mut magic = [0u8; 4];
			r.read_exact(&mut magic)?;
			if magic != GMA_MAGIC {
				return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "File is not in GMA format").into());
			}
		}

		let version = r.read_u8()?;
		if version != GMA_VERSION {
			log::warn!("File is in GMA version {version}, expected version {GMA_VERSION}, reading anyway...");
		}

		// SteamID (unused)
		r.read_exact(&mut [0u8; 8])?;

		// Timestamp
		r.read_exact(&mut [0u8; 8])?;

		if version > 1 {
			// Required content
			let mut buf = Vec::new();
			loop {
				buf.clear();
				if r.read_nul_str(&mut buf)?.is_empty() {
					break;
				}
			}
		}

		// Addon name
		metadata.title = {
			let mut buf = Vec::new();
			r.read_nul_str(&mut buf)?;

			if buf.last() == Some(&0) {
				buf.pop();
			}

			String::from_utf8(buf)?
		};

		// addon.json
		{
			let mut buf = Vec::new();
			r.read_nul_str(&mut buf)?;

			if buf.last() == Some(&0) {
				buf.pop();
			}

			#[derive(serde::Deserialize)]
			struct AddonJson {
				r#type: Option<String>,

				#[serde(default)]
				tags: Vec<String>,
			}
			if let Ok(addon_json) = serde_json::from_slice::<AddonJson>(&buf) {
				metadata.tags = addon_json.tags;
				metadata.addon_type = addon_json.r#type;
			} else {
				metadata.description = Some(String::from_utf8(buf)?);
			}
		};

		Ok(metadata)
	}
}
