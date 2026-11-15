//! Streaming reader contract by its actual calls; concrete Reader is stage7.
use super::{
    file_download::{FailureReason, FileLoader},
    file_download_mtproto::DOWNLOAD_PART_SIZE,
};
use crate::data::data_file_origin::FileOrigin;
pub const REQUEST_PARTS_COUNT: i32 = 32;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReaderEffect {
    LoadForDownloader { offset: i64 },
    ContinueDownloaderFromMainThread,
    DoneForDownloader { offset: i64 },
    CancelForDownloader,
}
pub struct StreamedFileDownloader {
    pub file: FileLoader,
    pub object_id: u64,
    pub origin: FileOrigin,
    part_is_saved: Vec<bool>,
    next_part_index: usize,
    parts_requested: i32,
    parts_saved: usize,
    effects: Vec<ReaderEffect>,
}
impl StreamedFileDownloader {
    pub fn new(file: FileLoader, object_id: u64, origin: FileOrigin) -> Option<Self> {
        let count = usize::try_from(
            file.full_size().checked_add(DOWNLOAD_PART_SIZE - 1)? / DOWNLOAD_PART_SIZE,
        )
        .ok()?;
        let mut part_is_saved = Vec::new();
        part_is_saved.try_reserve(count).ok()?;
        part_is_saved.resize(count, false);
        Some(Self {
            file,
            object_id,
            origin,
            part_is_saved,
            next_part_index: 0,
            parts_requested: 0,
            parts_saved: 0,
            effects: Vec::new(),
        })
    }
    pub fn obj_id(&self) -> u64 {
        self.object_id
    }
    pub fn file_origin(&self) -> &FileOrigin {
        &self.origin
    }
    pub fn start_loading(&mut self) {
        self.request_parts();
    }
    fn request_parts(&mut self) {
        while !self.file.finished()
            && self.next_part_index < self.part_is_saved.len()
            && self.parts_requested < REQUEST_PARTS_COUNT
        {
            let next = self.part_is_saved[self.next_part_index..]
                .iter()
                .position(|v| !*v)
                .map(|i| i + self.next_part_index);
            if let Some(i) = next {
                self.next_part_index = i + 1;
                self.effects.push(ReaderEffect::LoadForDownloader {
                    offset: i as i64 * DOWNLOAD_PART_SIZE,
                });
                self.parts_requested += 1;
            } else {
                self.next_part_index = self.part_is_saved.len();
            }
        }
        self.effects
            .push(ReaderEffect::ContinueDownloaderFromMainThread);
    }
    pub fn read_loaded_part(&mut self, offset: i64) -> Vec<u8> {
        if offset < 0 || offset >= self.file.full_size() || offset % DOWNLOAD_PART_SIZE != 0 {
            return Vec::new();
        }
        if self.part_is_saved[offset as usize / DOWNLOAD_PART_SIZE as usize] {
            self.file
                .read_loaded_part_back(offset, DOWNLOAD_PART_SIZE as usize)
        } else {
            Vec::new()
        }
    }
    pub fn cancel(&mut self, reason: FailureReason) {
        self.parts_requested = 0;
        self.next_part_index = 0;
        self.effects.push(ReaderEffect::CancelForDownloader);
        self.file.cancel(reason);
    }
    pub fn reader_failed(&mut self) {
        self.cancel(FailureReason::OtherFailure);
    }
    pub fn save_part(&mut self, offset: i64, bytes: &[u8]) -> bool {
        if self.file.finished() || self.file.cancelled() {
            return true;
        }
        if offset < 0 || offset >= self.file.full_size() || offset % DOWNLOAD_PART_SIZE != 0 {
            return false;
        }
        let index = offset as usize / DOWNLOAD_PART_SIZE as usize;
        if self.part_is_saved[index] {
            return true;
        }
        self.part_is_saved[index] = true;
        self.parts_saved += 1;
        if index < self.next_part_index {
            self.parts_requested -= 1;
        }
        if !self.file.write_result_part(offset, bytes) {
            return false;
        }
        self.effects
            .push(ReaderEffect::DoneForDownloader { offset });
        if self.parts_saved == self.part_is_saved.len() {
            self.file.finalize_result();
        } else {
            self.request_parts();
            self.file.notify_about_progress();
        }
        true
    }
    /// Must be applied while the real reader is still owned; Source destructor
    /// cancels the reader even after successful completion.
    pub fn close(&mut self) {
        if !self.file.finished() {
            self.cancel(FailureReason::NoFailure);
        } else {
            self.effects.push(ReaderEffect::CancelForDownloader);
        }
    }
    pub fn take_effects(&mut self) -> Vec<ReaderEffect> {
        std::mem::take(&mut self.effects)
    }
}
