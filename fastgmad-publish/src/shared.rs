use std::path::Path;

#[repr(C)]
#[derive(Debug)]
pub struct PublishError(Box<dyn std::error::Error + Send + Sync + 'static>);
impl std::fmt::Display for PublishError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		self.0.fmt(f)
	}
}
impl std::error::Error for PublishError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		self.0.source()
	}
}
impl From<Box<dyn std::error::Error + Send + Sync + 'static>> for PublishError {
	fn from(value: Box<dyn std::error::Error + Send + Sync + 'static>) -> Self {
		Self(value)
	}
}

pub trait PublishStateInterface {
	fn create_item(&self) -> Result<Box<dyn CreatedItemInterface>, PublishError>;
	fn start_item_update(
		&self,
		details: ItemUpdate,
		tick_callback: &mut dyn FnMut(),
		progress_callback: &mut dyn FnMut(ItemUpdateStatus, u64, u64),
	) -> Result<CompletedItemUpdate, PublishError>;
}

pub trait CreatedItemInterface {
	fn mark_as_successful(&mut self);
	fn file_id(&self) -> u64;
	fn legal_agreement_pending(&self) -> bool;
	fn delete(&mut self);
}

#[repr(C)]
pub struct ItemUpdate<'a> {
	pub file_id: u64,
	pub content_path: &'a Path,
	pub preview_path: Option<&'a Path>,
	pub description: Option<&'a str>,
	pub title: Option<&'a str>,
	pub tags: &'a [String],
	pub change_note: Option<&'a str>,
}

#[repr(C)]
pub struct CompletedItemUpdate {
	pub legal_agreement_pending: bool,
}

#[repr(u8)]
#[allow(dead_code)]
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum ItemUpdateStatus {
	Invalid = 0,
	CommittingChanges,
	PreparingConfig,
	PreparingContent,
	UploadingContent,
	UploadingPreviewFile,
}
