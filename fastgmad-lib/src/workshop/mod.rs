mod conf;
pub use conf::{WorkshopPublishConfig, WorkshopUpdateConfig};

mod fastgmad_publish {
	#[cfg(feature = "binary")]
	pub mod shared {
		include!("../../../fastgmad-publish/src/shared.rs");
	}

	#[cfg(not(feature = "binary"))]
	include!("../../../fastgmad-publish/src/lib.rs");
}
use fastgmad_publish::shared::{CompletedItemUpdate, ItemUpdate, ItemUpdateStatus, PublishStateInterface};

use crate::{util::BufReadEx, GMA_MAGIC, GMA_VERSION};
use byteorder::ReadBytesExt;
use std::{
	borrow::Cow,
	collections::BTreeSet,
	fs::File,
	io::{BufReader, Read},
	path::Path,
	path::PathBuf,
};
use uuid::Uuid;

#[cfg(feature = "binary")]
mod ctrlc_handling {
	// cargo build --package fastgmad-publish --features binary && cargo run --package fastgmad-bin -- publish -addon C:\Users\William\Documents\GitHub\fastgmad\fastgmad-lib\test_data\wiremod.gma

	struct CtrlCState {
		handles: usize,
		pressed: bool,
	}

	static CTRL_C_INSTALLED: std::sync::Once = std::sync::Once::new();
	static CTRL_C_STATE: std::sync::Mutex<CtrlCState> = std::sync::Mutex::new(CtrlCState { handles: 0, pressed: false });

	fn exit_ctrlc() {
		std::process::exit(if cfg!(windows) {
			-1073741510
		} else if cfg!(unix) {
			130
		} else {
			1
		});
	}

	pub struct CtrlCHandle;
	impl CtrlCHandle {
		pub fn get() -> Self {
			let mut state = CTRL_C_STATE.lock().unwrap();

			state.handles += 1;

			CTRL_C_INSTALLED.call_once(|| {
				ctrlc::set_handler(|| {
					let mut state = CTRL_C_STATE.lock().unwrap();
					if state.handles == 0 {
						exit_ctrlc();
					} else {
						if core::mem::replace(&mut state.pressed, true) {
							// Already pressed, exit immediately
							exit_ctrlc();
						}
						eprintln!();
						log::warn!("Aborting, please wait...");
					}
				})
				.ok();
			});

			drop(state);

			CtrlCHandle
		}

		pub fn check(&self, cleanup: impl FnOnce()) {
			if CTRL_C_STATE.lock().unwrap().pressed {
				cleanup();
				exit_ctrlc();
				log::warn!("Aborted by user");
			}
		}
	}
	impl Drop for CtrlCHandle {
		fn drop(&mut self) {
			CTRL_C_STATE.lock().unwrap().handles -= 1;
		}
	}
}

const LEGAL_AGREEMENT_MESSAGE: &str = r#"
You must accept the Steam Workshop legal agreement before you can make your addon public.
You can do this at https://steamcommunity.com/sharedfiles/workshoplegalagreement
Once you have accepted the agreement, you can set the visiblity of your addon to public.
"#;

#[cfg(feature = "binary")]
fn init_steam() -> Result<Box<dyn PublishStateInterface>, anyhow::Error> {
	unsafe {
		let lib = Box::leak(Box::new(libloading::Library::new(if cfg!(target_os = "linux") {
			"libfastgmad_publish.so"
		} else if cfg!(target_os = "macos") {
			"libfastgmad_publish.dylib"
		} else {
			"fastgmad_publish"
		})?));

		let fastgmad_publish_init: fn() -> Result<*mut dyn PublishStateInterface, fastgmad_publish::shared::PublishError> =
			*lib.get(b"fastgmad_publish_init")?;

		let interface = fastgmad_publish_init()?;

		Ok(Box::from_raw(interface) as Box<dyn PublishStateInterface>)
	}
}

#[cfg(not(feature = "binary"))]
fn init_steam() -> Result<Box<dyn PublishStateInterface>, anyhow::Error> {
	Ok(Box::new(std::rc::Rc::new(fastgmad_publish::PublishState::new()?)) as Box<dyn PublishStateInterface>)
}

#[derive(Clone, Copy)]
enum PublishKind<'a> {
	Create,
	Update { id: u64, changes: Option<&'a str> },
}
fn workshop_upload(#[cfg(feature = "binary")] noprogress: bool, kind: PublishKind, addon: &Path, icon: Option<&Path>) -> Result<u64, anyhow::Error> {
	// For some reason we need to manually check the icon file size
	// Steam just hangs forever if the icon is invalid
	if let Some(icon) = icon {
		log::info!("Checking icon...");

		let icon_size = std::fs::metadata(icon)
			.map_err(|err| anyhow::anyhow!("Failed to read icon: {err}"))?
			.len();

		if icon_size < WORKSHOP_ICON_MIN_SIZE {
			return Err(anyhow::anyhow!(
				"Icon is too small ({icon_size} bytes), must be at least {WORKSHOP_ICON_MAX_SIZE} bytes"
			));
		} else if icon_size > WORKSHOP_ICON_MAX_SIZE {
			return Err(anyhow::anyhow!(
				"Icon is too large ({icon_size} bytes), must be at most {WORKSHOP_ICON_MAX_SIZE} bytes"
			));
		}
	}

	log::info!("Initializing Steam...");
	let steam = init_steam()?;

	log::info!("Reading GMA metadata...");
	let mut metadata = GmaPublishingMetadata::try_read(addon)?;

	log::info!("Preparing content folder...");
	let content_path = ContentPath::new(addon)?;

	let mut created_item = None;
	let file_id;
	let mut legal_agreement_pending;
	match &kind {
		PublishKind::Create => {
			log::info!("Creating new Workshop item...");

			#[cfg(feature = "binary")]
			let ctrlc_handle = ctrlc_handling::CtrlCHandle::get();

			let mut created = steam.create_item()?;

			#[cfg(feature = "binary")]
			{
				ctrlc_handle.check(|| {
					created.delete();
				});
			}
			#[cfg(not(feature = "binary"))]
			{
				// HACK! Suppress unused `mut` warning
				created = created;
			}

			file_id = created.file_id();
			legal_agreement_pending = created.legal_agreement_pending();
			created_item = Some((created, {
				#[cfg(feature = "binary")]
				{
					ctrlc_handle
				}
				#[cfg(not(feature = "binary"))]
				{
					()
				}
			}));
		}
		PublishKind::Update { id, .. } => {
			file_id = *id;
			legal_agreement_pending = false;
		}
	}

	let icon = match (icon, &kind) {
		(Some(icon), _) => Some(Cow::Borrowed(icon)),

		(None, PublishKind::Create) => {
			log::info!("Preparing icon...");
			let default_icon_path = std::env::temp_dir().join("fastgmad-publish/gmpublisher_default_icon.png");
			std::fs::write(&default_icon_path, WORKSHOP_DEFAULT_ICON)?;
			Some(Cow::Owned(default_icon_path))
		}

		_ => None,
	};

	let res = (|| {
		log::info!("Preparing item {file_id} upload...");

		// Add "Addon" and the addon type to the tags
		let tags = {
			let mut tags = BTreeSet::from_iter(metadata.tags.into_iter());

			tags.insert("Addon".to_string());

			if let Some(addon_type) = metadata.addon_type.take() {
				tags.insert(addon_type);
			}

			tags.into_iter().collect::<Vec<_>>()
		};

		let details = match &kind {
			PublishKind::Create => ItemUpdate {
				file_id,
				content_path: &content_path.0,
				preview_path: icon.as_deref(),
				description: metadata.description.as_deref(),
				title: Some(metadata.title.as_str()),
				tags: &tags,
				change_note: None,
			},

			PublishKind::Update { changes, .. } => ItemUpdate {
				file_id,
				title: None,                   // do not update the title
				description: None,             // do not update the description
				preview_path: icon.as_deref(), // will be None for PublishKind::Update unless it was supplied
				content_path: &content_path.0,
				tags: &tags,
				change_note: *changes,
			},
		};

		log::info!("Uploading item...");

		let res = {
			let mut status = None;

			let mut progress_callback;
			let mut tick_callback;

			#[cfg(feature = "binary")]
			{
				progress_callback = {
					let mut total = None;
					let mut progress_printer = None;

					move |new_status, new_progress, new_total| {
						let new_status = if new_status != ItemUpdateStatus::Invalid {
							Some(new_status)
						} else {
							None
						};
						let did_status_change = core::mem::replace(&mut status, new_status) != new_status;

						let new_total = std::num::NonZeroU64::new(new_total);
						let did_total_change = core::mem::replace(&mut total, new_total) != new_total;

						if did_status_change {
							if let Some(new_status) = new_status {
								progress_printer = None; // Reset progress printer so we can print
								log::info!("{}", update_status_str(&new_status));
							}
						}
						if did_status_change || did_total_change {
							progress_printer = match (noprogress, new_status, new_total) {
								(false, Some(_), Some(new_total)) => Some(crate::util::ProgressPrinter::new(new_total.get())),
								_ => None,
							};
						}

						if let Some(progress_printer) = &mut progress_printer {
							progress_printer.set_progress(new_progress);
						}
					}
				};

				tick_callback = || {
					if let Some((created_item, ctrlc_handle)) = created_item.as_mut() {
						ctrlc_handle.check(|| {
							// If a CTRL+C occurs, delete the newly created item
							created_item.delete();
						});
					}
				};
			}

			#[cfg(not(feature = "binary"))]
			{
				progress_callback = move |new_status, _new_progress, _new_total| {
					let new_status = if new_status != ItemUpdateStatus::Invalid {
						Some(new_status)
					} else {
						None
					};

					let did_status_change = core::mem::replace(&mut status, new_status) != new_status;
					if did_status_change {
						if let Some(new_status) = new_status {
							log::info!("{}", update_status_str(&new_status));
						}
					}
				};

				tick_callback = || ();
			}

			steam.start_item_update(details, &mut tick_callback, &mut progress_callback)
		};

		legal_agreement_pending |= res.map(|CompletedItemUpdate { legal_agreement_pending }| legal_agreement_pending)?;

		// Everything OK!
		// Make sure we don't delete the newly created item
		if let Some((mut created_item, _)) = created_item {
			created_item.mark_as_successful();
		}

		drop(content_path);

		Ok::<_, anyhow::Error>(file_id)
	})();

	if legal_agreement_pending {
		log::info!("\n{}\n", LEGAL_AGREEMENT_MESSAGE.trim());
	}

	res
}

/// Publishes a GMA to the Steam Workshop
pub fn publish_gma(conf: &WorkshopPublishConfig) -> Result<u64, anyhow::Error> {
	workshop_upload(
		#[cfg(feature = "binary")]
		conf.noprogress,
		PublishKind::Create,
		&conf.addon,
		conf.icon.as_deref(),
	)
}

/// Updates a GMA on the Steam Workshop
pub fn update_gma(conf: &WorkshopUpdateConfig) -> Result<(), anyhow::Error> {
	workshop_upload(
		#[cfg(feature = "binary")]
		conf.noprogress,
		PublishKind::Update {
			id: conf.id,
			changes: conf.changes.as_deref(),
		},
		&conf.addon,
		conf.icon.as_deref(),
	)
	.map(|_| ())
}

struct ContentPath(PathBuf);
impl ContentPath {
	fn new(gma_path: &Path) -> Result<Self, anyhow::Error> {
		let dir = std::env::temp_dir().join(format!("fastgmad-publish/{}", Uuid::new_v4()));

		std::fs::create_dir_all(&dir)?;

		let temp_gma_path = dir.join("fastgmad.gma");

		let symlink_result = {
			#[cfg(windows)]
			{
				let res = std::os::windows::fs::symlink_file(gma_path, &temp_gma_path);
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
				std::os::unix::fs::symlink(gma_path, &temp_gma_path)
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

fn update_status_str(status: &ItemUpdateStatus) -> &'static str {
	match status {
		ItemUpdateStatus::PreparingConfig => "Preparing config...",
		ItemUpdateStatus::PreparingContent => "Preparing content...",
		ItemUpdateStatus::UploadingContent => "Uploading content...",
		ItemUpdateStatus::UploadingPreviewFile => "Uploading preview file...",
		ItemUpdateStatus::CommittingChanges => "Committing changes...",
		ItemUpdateStatus::Invalid => "",
	}
}

const WORKSHOP_DEFAULT_ICON: &[u8] = include_bytes!("gmpublisher_default_icon.png");
const WORKSHOP_ICON_MAX_SIZE: u64 = 1048576;
const WORKSHOP_ICON_MIN_SIZE: u64 = 16;
