use shared::{CompletedItemUpdate, CreatedItemInterface, ItemUpdate, ItemUpdateStatus, PublishError, PublishStateInterface};
use std::rc::Rc;
use std::time::Duration;

pub mod shared;

const GMOD_APP_ID: steamworks::AppId = steamworks::AppId(4000);

impl From<steamworks::UpdateStatus> for ItemUpdateStatus {
	fn from(value: steamworks::UpdateStatus) -> Self {
		match value {
			steamworks::UpdateStatus::Invalid => ItemUpdateStatus::Invalid,
			steamworks::UpdateStatus::PreparingConfig => ItemUpdateStatus::PreparingConfig,
			steamworks::UpdateStatus::PreparingContent => ItemUpdateStatus::PreparingContent,
			steamworks::UpdateStatus::UploadingContent => ItemUpdateStatus::UploadingContent,
			steamworks::UpdateStatus::UploadingPreviewFile => ItemUpdateStatus::UploadingPreviewFile,
			steamworks::UpdateStatus::CommittingChanges => ItemUpdateStatus::CommittingChanges,
		}
	}
}

macro_rules! run_steam_api {
	($callback:ident => $single:expr => $code:expr) => {{
		let (tx, rx) = std::sync::mpsc::sync_channel(1);
		let $callback = move |result| {
			tx.send(result).ok();
		};
		$code;
		loop {
			match rx.recv_timeout(Duration::from_millis(50)) {
				Ok(res) => break res,
				Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break Err(steamworks::SteamError::RemoteDisconnect),
				Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
					$single.run_callbacks();
				}
			}
		}
	}};
}

pub struct PublishState {
	client: steamworks::Client,
	single: steamworks::SingleClient,
}
impl PublishState {
	pub fn new() -> Result<Self, steamworks::SteamError> {
		let (client, single) = steamworks::Client::init_app(GMOD_APP_ID)?;
		Ok(Self { client, single })
	}
}

pub struct CreatedItem {
	keep_on_drop: bool,
	state: Rc<PublishState>,
	file_id: u64,
	legal_agreement_pending: bool,
}
impl CreatedItemInterface for CreatedItem {
	fn mark_as_successful(&mut self) {
		self.keep_on_drop = true;
	}

	fn file_id(&self) -> u64 {
		self.file_id
	}

	fn legal_agreement_pending(&self) -> bool {
		self.legal_agreement_pending
	}
}
impl Drop for CreatedItem {
	fn drop(&mut self) {
		if !self.keep_on_drop {
			// Delete the item
			run_steam_api!(callback => self.state.single => self.state.client.ugc().delete_item(steamworks::PublishedFileId(self.file_id), callback))
				.ok();
		}
	}
}

impl PublishStateInterface for Rc<PublishState> {
	fn create_item(&self) -> Result<Box<dyn CreatedItemInterface>, PublishError> {
		run_steam_api!(callback => self.single => self.client.ugc().create_item(GMOD_APP_ID, steamworks::FileType::Community, callback))
			.map(|(file_id, legal_agreement_pending)| {
				Box::new(CreatedItem {
					state: self.clone(),
					keep_on_drop: false,
					file_id: file_id.0,
					legal_agreement_pending,
				}) as Box<dyn CreatedItemInterface>
			})
			.map_err(|e| PublishError::from(Box::new(e) as Box<_>))
	}

	fn start_item_update(
		&self,
		details: ItemUpdate,
		mut progress_callback: Box<dyn FnMut(ItemUpdateStatus, u64, u64)>,
	) -> Result<CompletedItemUpdate, PublishError> {
		let mut update = self
			.client
			.ugc()
			.start_item_update(GMOD_APP_ID, steamworks::PublishedFileId(details.file_id));

		update = update.content_path(details.content_path);
		update = update.tags(details.tags.to_vec());
		if let Some(description) = details.description {
			update = update.description(description);
		}
		if let Some(preview_path) = details.preview_path {
			update = update.preview_path(preview_path);
		}
		if let Some(title) = details.title {
			update = update.title(title);
		}

		let (tx, rx) = std::sync::mpsc::sync_channel(1);

		let update: steamworks::UpdateWatchHandle<steamworks::ClientManager> = update.submit(details.change_note, move |res| {
			tx.send(res).ok();
		});

		let res = loop {
			let (status, progress, total) = update.progress();
			progress_callback(ItemUpdateStatus::from(status), progress, total);

			match rx.recv_timeout(Duration::from_millis(50)) {
				Ok(res) => break res,
				Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break Err(steamworks::SteamError::RemoteDisconnect),
				Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
					self.single.run_callbacks();
				}
			}
		};

		res.map(|(_, legal_agreement_pending)| CompletedItemUpdate { legal_agreement_pending })
			.map_err(|e| PublishError::from(Box::new(e) as Box<_>))
	}
}

#[cfg(feature = "binary")]
#[no_mangle]
pub fn fastgmad_publish_init() -> Result<*mut dyn PublishStateInterface, Box<dyn std::error::Error>> {
	PublishState::new()
		.map(|state| Box::into_raw(Box::new(Rc::new(state)) as Box<dyn PublishStateInterface>))
		.map_err(Into::into)
}
