//! Semantic click phases; real ClickHandler/QVariant/dialog adapters remain5/6/18/29.
use super::{data_msg_id::FullMsgId, data_store_keys::*};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FileClickHandler {
    context: FullMsgId,
}
impl FileClickHandler {
    pub fn new(context: FullMsgId) -> Self {
        Self { context }
    }
    pub fn context(&self) -> FullMsgId {
        self.context
    }
    pub fn set_message_id(&mut self, context: FullMsgId) {
        self.context = context
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentClickHandler {
    pub document: DocumentRecordKey,
    pub file: FileClickHandler,
}
impl DocumentClickHandler {
    pub fn new(document: DocumentRecordKey, context: FullMsgId) -> Self {
        Self {
            document,
            file: FileClickHandler::new(context),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhotoClickHandler {
    pub photo: PhotoRecordKey,
    pub peer: Option<PeerKey>,
    pub file: FileClickHandler,
}
impl PhotoClickHandler {
    pub fn new(photo: PhotoRecordKey, context: FullMsgId, peer: Option<PeerKey>) -> Self {
        if let Some(p) = peer {
            assert_eq!(p.scope(), photo.scope())
        }
        Self {
            photo,
            peer,
            file: FileClickHandler::new(context),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SaveMode {
    #[default]
    ToCacheOrFile,
    ToFile,
    ToNewFile,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveDecision {
    Ignore,
    SaveToCache,
    QueueFileSave,
    Started,
    ChooseFilename,
    SaveChosenFilename,
}
pub fn save(is_null: bool, forbidden: bool, save_to_cache: bool, mode: SaveMode) -> SaveDecision {
    if is_null || forbidden {
        SaveDecision::Ignore
    } else if mode == SaveMode::ToCacheOrFile && save_to_cache {
        SaveDecision::SaveToCache
    } else {
        SaveDecision::QueueFileSave
    }
}
pub fn queued_save(mode: SaveMode, saved_from_data: bool) -> SaveDecision {
    if mode != SaveMode::ToNewFile && saved_from_data {
        SaveDecision::Started
    } else {
        SaveDecision::ChooseFilename
    }
}
pub fn filename_chosen(path: &str) -> SaveDecision {
    if path.is_empty() {
        SaveDecision::Ignore
    } else {
        SaveDecision::SaveChosenFilename
    }
}
pub fn should_track(loading: bool, loading_path: &str, message_found: bool) -> bool {
    loading && !loading_path.is_empty() && message_found
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancelDecision {
    Ignore,
    UploadCallback,
    CancelMedia,
}
pub fn cancel(is_null: bool, uploading: bool, callback: bool) -> CancelDecision {
    if is_null {
        CancelDecision::Ignore
    } else if uploading && callback {
        CancelDecision::UploadCallback
    } else {
        CancelDecision::CancelMedia
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenWithDecision {
    Ignore,
    OpenWith,
    SaveToFile,
}
pub fn open_with(is_null: bool, path: &str) -> OpenWithDecision {
    if is_null {
        OpenWithDecision::Ignore
    } else if path.is_empty() {
        OpenWithDecision::SaveToFile
    } else {
        OpenWithDecision::OpenWith
    }
}
pub fn open(context: FullMsgId, callback: impl FnOnce(FullMsgId)) {
    callback(context)
}
pub fn photo_save(is_null: bool) -> bool {
    !is_null
} // true: owner clears Large failure then calls load(context).
