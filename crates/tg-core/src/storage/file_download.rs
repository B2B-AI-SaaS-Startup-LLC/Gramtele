//! FileLoader byte/file state. Mutation returns ordered effects; the real owner
//! applies cache/Session/platform notifications after releasing its store borrow.
use crate::data::{
    data_image_location::DownloadLocation, data_media_location::CacheKey,
    data_store_keys::SessionScopeId,
};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
};
pub const MAX_FILE_IN_MEMORY: i64 = 10 * 1024 * 1024;
pub const MAX_STICKER_BYTES_SIZE: i64 = 2 * 1024 * 1024;
pub const MAX_WALLPAPER_IN_MEMORY: i64 = MAX_FILE_IN_MEMORY;
pub const MAX_ANIMATION_IN_MEMORY: i64 = MAX_FILE_IN_MEMORY;
pub const MAX_WALLPAPER_DIMENSION: i32 = 4096;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadToCacheSetting {
    FileOnly,
    CacheAsWell,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadFromCloudSetting {
    CloudOrLocal = 0,
    LocalOnly = 1,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum LocationType {
    #[default]
    Unknown = 0,
    Document = 0x4e45abe9,
    Audio = 0x74dc404d,
    Video = 0x3d0364ec,
    Secure = 0xcbc7ee28,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FailureReason {
    #[default]
    NoFailure,
    FileWriteFailure,
    OtherFailure,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Error {
    pub failure_reason: FailureReason,
    pub started: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalStatus {
    NotTried,
    NotFound,
    Loading,
    Loaded,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    ReadCache {
        key: CacheKey,
        decode_image: bool,
    },
    StartLoading {
        partial: Option<Vec<u8>>,
    },
    CancelRequests,
    Progress,
    Done,
    Failed(Error),
    PostprocessDownloaded(PathBuf),
    WriteFileLocation {
        key: (u64, u64),
        path: PathBuf,
    },
    CachePut {
        key: CacheKey,
        bytes: Vec<u8>,
        tag: u8,
    },
    NotifyDownloaderTaskFinished,
    ClearFileNameAfterUpdate,
}
#[derive(Clone, Debug)]
pub struct LoaderConfig {
    pub session: SessionScopeId,
    pub filename: PathBuf,
    pub load_size: i64,
    pub full_size: i64,
    pub location_type: LocationType,
    pub to_cache: LoadToCacheSetting,
    pub from_cloud: LoadFromCloudSetting,
    pub auto_loading: bool,
    pub cache_tag: u8,
}
pub struct FileLoader {
    config: LoaderConfig,
    pub(crate) finished: bool,
    pub(crate) cancelled: bool,
    pub(crate) local_status: LocalStatus,
    data: Vec<u8>,
    file: Option<File>,
    file_readable: bool,
    skipped_bytes: i64,
    reported_offset: Option<i64>,
    effects: Vec<Effect>,
    pub cache_key: CacheKey,
    pub file_location_key: Option<(u64, u64)>,
}
impl FileLoader {
    pub fn new(
        config: LoaderConfig,
        cache_key: CacheKey,
        file_location_key: Option<(u64, u64)>,
    ) -> Option<Self> {
        if config.load_size < 0
            || config.full_size < 0
            || config.load_size > config.full_size
            || (config.filename.as_os_str().is_empty() && config.full_size > MAX_FILE_IN_MEMORY)
        {
            return None;
        }
        Some(Self {
            config,
            cache_key,
            file_location_key,
            finished: false,
            cancelled: false,
            local_status: LocalStatus::NotTried,
            data: Vec::new(),
            file: None,
            file_readable: false,
            skipped_bytes: 0,
            reported_offset: None,
            effects: Vec::new(),
        })
    }
    pub fn session(&self) -> SessionScopeId {
        self.config.session
    }
    pub fn finished(&self) -> bool {
        self.finished
    }
    pub fn cancelled(&self) -> bool {
        self.cancelled
    }
    pub fn bytes(&self) -> &[u8] {
        &self.data
    }
    pub fn file_name(&self) -> &std::path::Path {
        &self.config.filename
    }
    pub fn full_size(&self) -> i64 {
        self.config.full_size
    }
    pub fn load_size(&self) -> i64 {
        self.config.load_size
    }
    pub fn auto_loading(&self) -> bool {
        self.config.auto_loading
    }
    pub fn is_file_only(&self) -> bool {
        self.config.to_cache == LoadToCacheSetting::FileOnly
    }
    pub fn loading_local(&self) -> bool {
        self.local_status == LocalStatus::Loading
    }
    pub fn local_status(&self) -> LocalStatus {
        self.local_status
    }
    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }
    pub fn notify_about_progress(&mut self) {
        self.effects.push(Effect::Progress);
    }
    pub(crate) fn set_reported_offset(&mut self, offset: i64) {
        self.reported_offset = Some(offset);
    }
    pub fn current_offset(&self) -> i64 {
        if let Some(offset) = self.reported_offset {
            return offset;
        }
        self.file
            .as_ref()
            .and_then(|f| f.metadata().ok())
            .map_or(self.data.len() as i64, |m| m.len() as i64)
            - self.skipped_bytes
    }
    pub fn current_progress(&self) -> f64 {
        if self.finished {
            1.
        } else if self.config.load_size == 0 {
            0.
        } else {
            (self.current_offset() as f64 / self.config.load_size as f64).clamp(0., 1.)
        }
    }
    pub fn set_file_name(&mut self, name: PathBuf) -> bool {
        if self.config.to_cache != LoadToCacheSetting::CacheAsWell
            || !self.config.filename.as_os_str().is_empty()
        {
            return name.as_os_str().is_empty() || name == self.config.filename;
        }
        self.config.filename = name;
        true
    }
    pub fn permit_load_from_cloud(&mut self) {
        self.config.from_cloud = LoadFromCloudSetting::CloudOrLocal;
    }
    pub fn increase_load_size(&mut self, size: i64, auto_loading: bool) -> bool {
        if size <= self.config.load_size || size > self.config.full_size {
            return false;
        }
        self.config.load_size = size;
        self.config.auto_loading = auto_loading;
        true
    }
    pub(crate) fn set_sizes(&mut self, size: i64) {
        self.config.full_size = size;
        self.config.load_size = size;
    }
    fn open_write(&mut self) -> bool {
        if self.file.is_some() {
            return true;
        }
        match OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&self.config.filename)
        {
            Ok(f) => {
                self.file = Some(f);
                self.file_readable = false;
                true
            }
            Err(_) => {
                self.cancel(FailureReason::FileWriteFailure);
                false
            }
        }
    }
    fn check_for_open(&mut self) -> bool {
        self.config.filename.as_os_str().is_empty()
            || self.config.to_cache != LoadToCacheSetting::FileOnly
            || self.open_write()
    }
    fn try_load_local(&mut self) -> bool {
        match self.local_status {
            LocalStatus::NotFound | LocalStatus::Loaded => false,
            LocalStatus::Loading => true,
            LocalStatus::NotTried => {
                if self.config.to_cache == LoadToCacheSetting::CacheAsWell && self.cache_key.valid()
                {
                    self.effects.push(Effect::ReadCache {
                        key: self.cache_key,
                        decode_image: self.config.location_type != LocationType::Audio,
                    });
                    self.notify_about_progress();
                    self.local_status = LocalStatus::Loading;
                    true
                } else {
                    self.local_status = LocalStatus::NotFound;
                    false
                }
            }
        }
    }
    pub fn start(&mut self) {
        if self.finished || self.try_load_local() {
            return;
        }
        if self.config.from_cloud == LoadFromCloudSetting::LocalOnly {
            self.cancel(FailureReason::NoFailure);
            return;
        }
        if self.check_for_open() {
            self.effects.push(Effect::StartLoading { partial: None });
        }
    }
    pub fn local_loaded(&mut self, data: &[u8]) {
        if data.is_empty() {
            self.local_status = LocalStatus::NotFound;
            self.start();
            return;
        }
        let partial = data.starts_with(b"partial:");
        if partial && (data.len() as i64) < self.config.load_size + 8 {
            self.local_status = LocalStatus::NotFound;
            if self.check_for_open() {
                self.effects.push(Effect::StartLoading {
                    partial: Some(data.to_vec()),
                });
            }
            return;
        }
        self.finish_with_bytes(if partial { &data[8..] } else { data });
    }
    pub fn finish_with_bytes(&mut self, data: &[u8]) {
        self.data = data.to_vec();
        self.local_status = LocalStatus::Loaded;
        if !self.config.filename.as_os_str().is_empty()
            && self.config.to_cache == LoadToCacheSetting::CacheAsWell
        {
            if !self.open_write() {
                return;
            }
            if !self.write_file(0, data) {
                return;
            }
        }
        self.finished = true;
        self.close_file();
        self.effects.push(Effect::Done);
        self.effects.push(Effect::NotifyDownloaderTaskFinished);
    }
    fn close_file(&mut self) {
        if self.file.take().is_some() {
            self.effects
                .push(Effect::PostprocessDownloaded(self.config.filename.clone()));
        }
    }
    pub fn cancel(&mut self, reason: FailureReason) {
        let started = self.current_offset() > 0;
        self.effects.push(Effect::CancelRequests);
        self.cancelled = true;
        self.finished = true;
        if self.file.take().is_some() {
            let _ = std::fs::remove_file(&self.config.filename);
        }
        self.data.clear();
        self.effects.push(if reason == FailureReason::NoFailure {
            Effect::Done
        } else {
            Effect::Failed(Error {
                failure_reason: reason,
                started,
            })
        });
        self.effects.push(Effect::ClearFileNameAfterUpdate);
    }
    /// Source cancel clears the name only after fire_done/fire_error, guarded
    /// by the owner lifetime. Apply this after dispatching the preceding update.
    pub fn apply_post_update(&mut self, effect: &Effect) -> bool {
        if matches!(effect, Effect::ClearFileNameAfterUpdate) {
            self.config.filename.clear();
            true
        } else {
            false
        }
    }
    fn write_file(&mut self, offset: u64, bytes: &[u8]) -> bool {
        let result = self
            .file
            .as_mut()
            .is_some_and(|f| f.seek(SeekFrom::Start(offset)).is_ok() && f.write_all(bytes).is_ok());
        if !result {
            self.cancel(FailureReason::FileWriteFailure);
        }
        result
    }
    pub fn write_result_part(&mut self, offset: i64, bytes: &[u8]) -> bool {
        if self.finished || offset < 0 {
            return false;
        }
        if bytes.is_empty() {
            return true;
        }
        let Some(end) = usize::try_from(offset)
            .ok()
            .and_then(|v| v.checked_add(bytes.len()))
        else {
            return false;
        };
        let len = self
            .file
            .as_ref()
            .and_then(|f| f.metadata().ok())
            .map_or(self.data.len() as i64, |m| m.len() as i64);
        if self.file.is_some() {
            if offset < len {
                self.skipped_bytes -= bytes.len() as i64;
            } else if offset > len {
                self.skipped_bytes += offset - len;
            }
            return self.write_file(offset as u64, bytes);
        }
        if self
            .data
            .try_reserve(end.saturating_sub(self.data.len()))
            .is_err()
        {
            self.cancel(FailureReason::OtherFailure);
            return false;
        }
        if offset > len {
            self.skipped_bytes += offset - len;
        } else if offset < len {
            self.skipped_bytes -= bytes.len() as i64;
        }
        if end > self.data.len() {
            self.data.resize(end, 0);
        }
        self.data[offset as usize..end].copy_from_slice(bytes);
        true
    }
    pub fn read_loaded_part_back(&mut self, offset: i64, size: usize) -> Vec<u8> {
        if offset < 0 || size == 0 {
            return Vec::new();
        }
        if self.file.is_some() && !self.file_readable {
            self.file.take();
            match OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(&self.config.filename)
            {
                Ok(file) => {
                    self.file = Some(file);
                    self.file_readable = true;
                }
                Err(_) => {
                    self.cancel(FailureReason::FileWriteFailure);
                    return Vec::new();
                }
            }
        }
        if let Some(file) = &mut self.file {
            let mut bytes = vec![0; size];
            if file.seek(SeekFrom::Start(offset as u64)).is_err()
                || file.read_exact(&mut bytes).is_err()
            {
                return Vec::new();
            }
            bytes
        } else {
            usize::try_from(offset)
                .ok()
                .and_then(|start| {
                    start
                        .checked_add(size)
                        .and_then(|end| self.data.get(start..end))
                })
                .unwrap_or_default()
                .to_vec()
        }
    }
    pub fn finalize_result(&mut self) -> bool {
        if self.finished {
            return false;
        }
        if !self.config.filename.as_os_str().is_empty()
            && self.config.to_cache == LoadToCacheSetting::CacheAsWell
        {
            if !self.open_write() {
                return false;
            }
            let data = self.data.clone();
            if !self.write_file(0, &data) {
                return false;
            }
        }
        self.finished = true;
        self.close_file();
        if self.local_status == LocalStatus::NotFound {
            if let Some(key) = self
                .file_location_key
                .filter(|_| !self.config.filename.as_os_str().is_empty())
            {
                self.effects.push(Effect::WriteFileLocation {
                    key,
                    path: self.config.filename.clone(),
                });
            }
            if self.config.to_cache == LoadToCacheSetting::CacheAsWell
                && (self.data.len() as i64) <= MAX_FILE_IN_MEMORY
                && self.cache_key.valid()
            {
                let mut bytes = if self.config.full_size == 0
                    || self.data.len() as i64 == self.config.full_size
                {
                    Vec::new()
                } else {
                    b"partial:".to_vec()
                };
                bytes.extend_from_slice(&self.data);
                self.effects.push(Effect::CachePut {
                    key: self.cache_key,
                    bytes,
                    tag: self.config.cache_tag,
                });
            }
        }
        self.effects.push(Effect::Done);
        self.effects.push(Effect::NotifyDownloaderTaskFinished);
        true
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoaderKind {
    Mtproto,
    Web,
    Memory,
}
pub fn loader_kind(location: &DownloadLocation) -> LoaderKind {
    match location {
        DownloadLocation::Url(_) => LoaderKind::Web,
        DownloadLocation::InMemory(_) => LoaderKind::Memory,
        _ => LoaderKind::Mtproto,
    }
}

pub struct FromMemoryLoader {
    pub file: FileLoader,
    data: Vec<u8>,
}
impl FromMemoryLoader {
    pub fn start_loading(&mut self) {
        self.file.finish_with_bytes(&self.data);
    }
}
#[allow(clippy::large_enum_variant)]
pub enum FileDownload {
    Mtproto(super::file_download_mtproto::MtpFileLoader),
    Web(super::file_download_web::WebFileLoader),
    Memory(FromMemoryLoader),
}
/// Constructor selection only; the owner handles StartLoading/ReadCache effects
/// and supplies the real DownloadMtprotoTask/Reader/Session integration.
pub fn create_file_loader(
    mut config: LoaderConfig,
    location: DownloadLocation,
    mut origin: crate::data::data_file_origin::FileOrigin,
) -> Option<FileDownload> {
    match &location {
        DownloadLocation::Url(url) => {
            // Source's first webFileLoader constructor ignores its `to` argument.
            config.filename.clear();
            config.full_size = 0;
            config.load_size = 0;
            config.location_type = LocationType::Unknown;
            config.to_cache = LoadToCacheSetting::CacheAsWell;
            let file = FileLoader::new(config, location.cache_key(), None)?;
            Some(FileDownload::Web(
                super::file_download_web::WebFileLoader::new(
                    file,
                    url.clone(),
                    super::file_download_web::WebRequestType::FullLoad,
                ),
            ))
        }
        DownloadLocation::InMemory(data) => {
            config.from_cloud = LoadFromCloudSetting::CloudOrLocal;
            let file = FileLoader::new(config, CacheKey::default(), None)?;
            Some(FileDownload::Memory(FromMemoryLoader {
                file,
                data: data.clone(),
            }))
        }
        _ => {
            let location_key = if let DownloadLocation::Storage(s) = &location {
                (config.location_type != LocationType::Unknown).then_some((
                    (u64::from(config.location_type as u32) << 32) | u64::from(s.dc_id),
                    s.id,
                ))
            } else {
                config.filename.clear();
                config.location_type = LocationType::Unknown;
                config.to_cache = LoadToCacheSetting::CacheAsWell;
                origin = crate::data::data_file_origin::FileOrigin::None;
                None
            };
            let file = FileLoader::new(config, location.cache_key(), location_key)?;
            Some(FileDownload::Mtproto(
                super::file_download_mtproto::MtpFileLoader::new(file, location, origin),
            ))
        }
    }
}

/// Owner-side RPL endpoint, deliberately separate from mutable FileLoader.
/// Drain effects under the borrow, then dispatch after releasing it.
#[derive(Default)]
pub struct LoaderUpdates {
    stream: tg_rpl::EventStream<(), Error>,
}
impl LoaderUpdates {
    pub fn updates(&self) -> tg_rpl::Producer<(), Error> {
        self.stream.events()
    }
    pub fn apply(&self, effect: &Effect) -> bool {
        match effect {
            Effect::Progress => self.stream.fire(()),
            Effect::Done => self.stream.fire_done(),
            Effect::Failed(error) => self.stream.fire_error(*error),
            _ => return false,
        }
        true
    }
}
pub fn apply_cache_put(effect: &Effect, cache: &tg_cache::cache::Database) -> bool {
    if let Effect::CachePut { key, bytes, tag } = effect {
        cache.put(
            *key,
            tg_cache::cache::TaggedValue::new(bytes.clone(), *tag),
            None,
        );
        true
    } else {
        false
    }
}
/// Encoded cache phase; image decoding remains5. The real owner's callback must
/// post to its MainRef and check the loader lifetime before calling local_loaded.
pub fn read_local_cache(
    cache: &tg_cache::cache::Database,
    key: CacheKey,
    done: impl FnOnce(Vec<u8>) + Send + 'static,
) {
    cache.get(key, done);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_starts_write_only_and_reopens_read_write_without_truncation() {
        let scope = crate::data::data_store_keys::SessionScope::new();
        let mut random = [0; 16];
        tg_base::random::random_fill(&mut random);
        let path = std::env::temp_dir().join(format!(
            "gramtele-loader-mode-{:x}",
            u128::from_le_bytes(random)
        ));
        let config = LoaderConfig {
            session: scope.id(),
            filename: path.clone(),
            load_size: 3,
            full_size: 3,
            location_type: LocationType::Document,
            to_cache: LoadToCacheSetting::FileOnly,
            from_cloud: LoadFromCloudSetting::CloudOrLocal,
            auto_loading: false,
            cache_tag: 0,
        };
        let mut loader = FileLoader::new(config, CacheKey::default(), None).unwrap();
        loader.start();
        assert!(loader.write_result_part(0, b"abc"));
        assert!(loader.file.as_mut().unwrap().read(&mut [0; 1]).is_err());
        assert_eq!(loader.read_loaded_part_back(0, 3), b"abc");
        assert!(loader.file_readable);
        assert_eq!(loader.file.as_mut().unwrap().metadata().unwrap().len(), 3);
        loader.cancel(FailureReason::NoFailure);
        assert!(!path.exists());
    }
}
