//! mtpFileLoader's own state. DownloadMtprotoTask transport, CDN validation,
//! reference refresh and shared priorities belong to download_manager_mtproto3/4.
//! The owner feeds real haveSentRequests after removing a completed request.
use super::file_download::{FailureReason, FileLoader};
use crate::data::{data_file_origin::FileOrigin, data_image_location::DownloadLocation};
pub const DOWNLOAD_PART_SIZE: i64 = 128 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskEffect {
    AddToQueue { priority: i32 },
    RemoveFromQueue,
    CancelAllRequests,
}
pub struct MtpFileLoader {
    pub file: FileLoader,
    pub location: DownloadLocation,
    pub origin: FileOrigin,
    last_complete: bool,
    next_request_offset: i64,
    effects: Vec<TaskEffect>,
}
impl MtpFileLoader {
    pub fn new(file: FileLoader, location: DownloadLocation, origin: FileOrigin) -> Self {
        Self {
            file,
            location,
            origin,
            last_complete: false,
            next_request_offset: 0,
            effects: Vec::new(),
        }
    }
    pub fn file_origin(&self) -> &FileOrigin {
        &self.origin
    }
    pub fn obj_id(&self) -> u64 {
        match &self.location {
            DownloadLocation::Storage(s) => s.id,
            DownloadLocation::AudioAlbumThumb { document_id } => *document_id,
            _ => 0,
        }
    }
    pub fn ready_to_request(&self, have_sent_requests: bool) -> bool {
        !self.file.finished()
            && !self.last_complete
            && (self.file.full_size() != 0 || !have_sent_requests)
            && (self.file.full_size() == 0 || self.next_request_offset < self.file.load_size())
    }
    pub fn take_next_request_offset(&mut self, have_sent_requests: bool) -> Option<i64> {
        if !self.ready_to_request(have_sent_requests) {
            return None;
        }
        let result = self.next_request_offset;
        self.next_request_offset = self.next_request_offset.checked_add(DOWNLOAD_PART_SIZE)?;
        Some(result)
    }
    pub fn feed_part(&mut self, offset: i64, bytes: &[u8], have_sent_requests: bool) -> bool {
        if !self.file.write_result_part(offset, bytes) {
            return false;
        }
        if bytes.is_empty() || !bytes.len().is_multiple_of(1024) {
            self.last_complete = true;
        }
        if !have_sent_requests
            && (self.last_complete
                || (self.file.full_size() != 0
                    && self.next_request_offset >= self.file.load_size()))
        {
            self.effects.push(TaskEffect::RemoveFromQueue);
            self.file.finalize_result()
        } else {
            self.file.notify_about_progress();
            true
        }
    }
    pub fn cancel_on_fail(&mut self) {
        self.cancel(FailureReason::OtherFailure);
    }
    pub fn set_web_file_size_hook(&mut self, size: i64) -> bool {
        if size >= 0 && (self.file.full_size() == 0 || self.file.full_size() == size) {
            self.file.set_sizes(size);
            true
        } else {
            self.cancel_on_fail();
            false
        }
    }
    pub fn start_loading(&mut self) {
        self.effects.push(TaskEffect::AddToQueue { priority: 0 });
    }
    pub fn start_loading_with_partial(&mut self, data: &[u8], have_sent_requests: bool) -> bool {
        if !data.starts_with(b"partial:") {
            return false;
        }
        let used = (data.len() - 8) / (DOWNLOAD_PART_SIZE as usize) * (DOWNLOAD_PART_SIZE as usize);
        if used > 0 {
            self.next_request_offset = used as i64;
            self.feed_part(0, &data[8..8 + used], have_sent_requests);
        }
        self.start_loading();
        true
    }
    pub fn cancel(&mut self, reason: FailureReason) {
        self.effects.push(TaskEffect::CancelAllRequests);
        self.file.cancel(reason);
    }
    pub fn take_effects(&mut self) -> Vec<TaskEffect> {
        std::mem::take(&mut self.effects)
    }
}
