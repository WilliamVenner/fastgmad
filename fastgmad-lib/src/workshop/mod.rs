mod conf;
pub use conf::{WorkshopPublishConfig, WorkshopUpdateConfig, WorkshopPublishAddonSrc};

mod fastgmad_publish {
	#[cfg(feature = "binary")]
	pub mod shared {
		include!("../../../fastgmad-publish/src/shared.rs");
	}

	#[cfg(not(feature = "binary"))]
	include!("../../../fastgmad-publish/src/lib.rs");
}
use fastgmad_publish::shared::{CompletedItemUpdate, CreatedItemInterface, ItemUpdate, ItemUpdateStatus, PublishStateInterface};

use crate::{
	error::{fastgmad_error, fastgmad_io_error, FastGmadError},
	util::BufReadEx,
	GMA_MAGIC, GMA_VERSION,
};
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
fn init_steam() -> Result<Box<dyn PublishStateInterface>, FastGmadError> {
	unsafe {
		let lib = Box::leak(Box::new(
			libloading::Library::new(if cfg!(target_os = "linux") {
				"libfastgmad_publish.so"
			} else if cfg!(target_os = "macos") {
				"libfastgmad_publish.dylib"
			} else {
				"fastgmad_publish"
			})
			.map_err(|err| fastgmad_error!(error: Libloading(err)))?,
		));

		let fastgmad_publish_init: fn() -> Result<*mut dyn PublishStateInterface, fastgmad_publish::shared::PublishError> =
			*lib.get(b"fastgmad_publish_init").map_err(|err| fastgmad_error!(error: Libloading(err)))?;

		let interface = fastgmad_publish_init().map_err(|err| fastgmad_error!(while "initializing Steam", error: SteamError(err.to_string())))?;

		Ok(Box::from_raw(interface) as Box<dyn PublishStateInterface>)
	}
}

#[cfg(not(feature = "binary"))]
fn init_steam() -> Result<Box<dyn PublishStateInterface>, FastGmadError> {
	Ok(Box::new(std::rc::Rc::new(
		fastgmad_publish::PublishState::new().map_err(|err| fastgmad_error!(while "initializing Steam", error: SteamError(err.to_string())))?,
	)) as Box<dyn PublishStateInterface>)
}

#[derive(Clone, Copy)]
enum PublishKind<'a> {
	Create,
	Update { id: u64, changes: Option<&'a str> },
}
fn workshop_upload(#[cfg(feature = "binary")] noprogress: bool, kind: PublishKind, addon: &Path, icon: Option<&Path>) -> Result<u64, FastGmadError> {
	let mut created_item: Option<Box<dyn CreatedItemInterface>> = None;

	// For some reason we need to manually check the icon file size
	// Steam just hangs forever if the icon is invalid
	if let Some(icon) = icon {
		log::info!("Checking icon...");

		let icon_size = std::fs::metadata(icon)
			.map_err(|error| fastgmad_io_error!(while "reading icon metadata", error: error))?
			.len();

		if icon_size < WORKSHOP_ICON_MIN_SIZE {
			return Err(fastgmad_error!(error: IconTooSmall));
		} else if icon_size > WORKSHOP_ICON_MAX_SIZE {
			return Err(fastgmad_error!(error: IconTooLarge));
		}
	}

	log::info!("Initializing Steam...");
	let steam = init_steam()?;

	log::info!("Reading GMA metadata...");
	let mut metadata = GmaPublishingMetadata::try_read(addon)?;

	#[cfg(feature = "binary")]
	let ctrlc_handle = ctrlc_handling::CtrlCHandle::get();

	log::info!("Preparing content folder...");
	let content_path = ContentPath::new(addon)?;

	let file_id;
	let mut legal_agreement_pending;
	match &kind {
		PublishKind::Create => {
			log::info!("Creating new Workshop item...");

			let created = steam
				.create_item()
				.map_err(|error| fastgmad_error!(error: SteamError(error.to_string())))?;

			file_id = created.file_id();
			legal_agreement_pending = created.legal_agreement_pending();

			created_item = Some(created);
		}
		PublishKind::Update { id, .. } => {
			file_id = *id;
			legal_agreement_pending = false;
		}
	}

	#[cfg(feature = "binary")]
	let ctrlc_check = {
		let ctrlc_check = |content_path: &ContentPath, created_item: &mut Option<Box<dyn CreatedItemInterface>>| {
			ctrlc_handle.check(|| {
				content_path.delete();

				if let Some(created_item) = created_item {
					created_item.delete();
				}
			})
		};

		// Do a quick check before we start uploading to Steam...
		ctrlc_check(&content_path, &mut created_item);

		ctrlc_check
	};

	let icon = match (icon, &kind) {
		(Some(icon), _) => Some(Cow::Borrowed(icon)),

		(None, PublishKind::Create) => {
			log::info!("Preparing icon...");

			let default_icon_path = std::env::temp_dir().join("fastgmad-publish/gmpublisher_default_icon.png");

			std::fs::write(&default_icon_path, WORKSHOP_DEFAULT_ICON)
				.map_err(|error| fastgmad_io_error!(while "writing default icon", error: error))?;

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
								(false, Some(_), Some(new_total)) => Some(ProgressPrinter::new(new_total.get())),
								_ => None,
							};
						}

						if let Some(progress_printer) = &mut progress_printer {
							progress_printer.set_progress(new_progress);
						}
					}
				};

				tick_callback = || {
					ctrlc_check(&content_path, &mut created_item);
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

		legal_agreement_pending |= res
			.map(|CompletedItemUpdate { legal_agreement_pending }| legal_agreement_pending)
			.map_err(|error| fastgmad_error!(error: SteamError(error.to_string())))?;

		// Everything OK!
		// Make sure we don't delete the newly created item
		if let Some(mut created_item) = created_item {
			created_item.mark_as_successful();
		}

		drop(content_path);

		Ok::<_, FastGmadError>(file_id)
	})();

	if legal_agreement_pending {
		log::info!("\n{}\n", LEGAL_AGREEMENT_MESSAGE.trim());
	}

	res
}

/// Publishes a GMA to the Steam Workshop
pub fn publish_gma(conf: &WorkshopPublishConfig) -> Result<u64, FastGmadError> {
	workshop_upload(
		#[cfg(feature = "binary")]
		conf.noprogress,
		PublishKind::Create,
		&conf.addon,
		conf.icon.as_deref(),
	)
}

/// Updates a GMA on the Steam Workshop
pub fn update_gma(conf: &WorkshopUpdateConfig) -> Result<(), FastGmadError> {
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
	fn new(gma_path: &Path) -> Result<Self, FastGmadError> {
		let dir = std::env::temp_dir().join(format!("fastgmad-publish/{}", Uuid::new_v4()));

		std::fs::create_dir_all(&dir).map_err(|error| fastgmad_io_error!(while "creating content path directory", error: error, path: dir))?;

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
				Err(FastGmadError::IoError(std::io::Error::new(
					std::io::ErrorKind::Other,
					"Unsupported platform",
				)))
			}
		};

		if symlink_result.is_err() {
			std::fs::copy(gma_path, &temp_gma_path)
				.map_err(|error| fastgmad_io_error!(while "copying .gma to temporary directory", error: error, paths: (gma_path, temp_gma_path)))?;
		}

		Ok(Self(dir))
	}

	fn delete(&self) {
		std::fs::remove_dir_all(&self.0).ok();
	}
}
impl Drop for ContentPath {
	fn drop(&mut self) {
		self.delete();
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
	fn try_read(path: &Path) -> Result<Self, FastGmadError> {
		let mut metadata = Self::default();

		let mut r = BufReader::new(File::open(path).map_err(|error| fastgmad_io_error!(while "opening GMA file", error: error, path: path))?);

		{
			let mut magic = [0u8; 4];
			let res = r.read_exact(&mut magic);
			if let Err(error) = res {
				if error.kind() != std::io::ErrorKind::UnexpectedEof {
					return Err(fastgmad_io_error!(while "reading GMA magic bytes", error: error));
				}
			}
			if magic != GMA_MAGIC {
				return Err(fastgmad_io_error!(error: std::io::Error::new(std::io::ErrorKind::InvalidData, "File is not in GMA format")));
			}
		}

		let version = r
			.read_u8()
			.map_err(|error| fastgmad_io_error!(while "reading version byte", error: error))?;
		if version != GMA_VERSION {
			log::warn!("File is in GMA version {version}, expected version {GMA_VERSION}, reading anyway...");
		}

		// SteamID (unused)
		r.read_exact(&mut [0u8; 8])
			.map_err(|error| fastgmad_io_error!(while "reading SteamID", error: error))?;

		// Timestamp
		r.read_exact(&mut [0u8; 8])
			.map_err(|error| fastgmad_io_error!(while "reading timestamp", error: error))?;

		if version > 1 {
			// Required content
			let mut buf = Vec::new();
			loop {
				buf.clear();
				let content = r
					.read_nul_str(&mut buf)
					.map_err(|error| fastgmad_io_error!(while "reading required content", error: error))?;
				if content.is_empty() {
					break;
				}
			}
		}

		// Addon name
		metadata.title = {
			let mut buf = Vec::new();
			r.read_nul_str(&mut buf)
				.map_err(|error| fastgmad_io_error!(while "reading addon name", error: error))?;

			if buf.last() == Some(&0) {
				buf.pop();
			}

			String::from_utf8(buf).map_err(
				|error| fastgmad_io_error!(while "decoding addon name", error: std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
			)?
		};

		// addon.json
		{
			let mut buf = Vec::new();
			r.read_nul_str(&mut buf)
				.map_err(|error| fastgmad_io_error!(while "reading addon description", error: error))?;

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
				metadata.description = Some(String::from_utf8(buf).map_err(
					|error| fastgmad_io_error!(while "decoding addon description", error: std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
				)?);
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
