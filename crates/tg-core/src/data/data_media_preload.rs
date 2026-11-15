//! Preload controllers over real media records/leases. Transport scheduling,
//! image-loaded facts and guarded cache callbacks are required owner contracts.
use super::{
    data_auto_download::{self, Full},
    data_document::DocumentData,
    data_file_origin::FileOrigin,
    data_media_location::CacheKey,
    data_media_store::{Documents, Photos},
    data_peer::PeerData,
    data_peer_fields::PeerFields,
    data_photo::PhotoData,
    data_photo_media::PhotoMedia,
    data_store_keys::*,
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};
use tg_rpl::{Lifetime, Producer};
pub const DEFAULT_PRELOAD_PREFIX: i64 = 4 * 1024 * 1024;
pub const DOWNLOAD_PART_SIZE: i32 = 128 * 1024;
pub struct MediaPreload {
    done: Rc<dyn Fn()>,
}
impl MediaPreload {
    pub fn new(done: Rc<dyn Fn()>) -> Self {
        Self { done }
    }
    pub fn call_done(&self) {
        let callback = self.done.clone();
        callback();
    }
}
pub trait PhotoPreloadOwner {
    fn loaded(&self, photo: PhotoRecordKey, lease: &Rc<RefCell<PhotoMedia>>) -> bool;
    fn load(&self, photo: PhotoRecordKey, origin: FileOrigin);
    fn cancel(&self, photo: PhotoRecordKey);
    fn downloader_task_finished(&self) -> Producer<()>;
}
pub struct PhotoPreload {
    base: MediaPreload,
    key: PhotoRecordKey,
    lease: Rc<RefCell<PhotoMedia>>,
    owner: Rc<dyn PhotoPreloadOwner>,
    life: Lifetime,
}
impl PhotoPreload {
    pub fn should(
        photo: &PhotoData,
        context: &PeerData,
        fields: &PeerFields<'_>,
        auto: &Full,
    ) -> bool {
        !photo.cancelled() && data_auto_download::should_photo(auto, context, fields, photo)
    }
    pub fn new(
        photos: &Photos,
        key: PhotoRecordKey,
        origin: FileOrigin,
        owner: Rc<dyn PhotoPreloadOwner>,
        done: Rc<dyn Fn()>,
    ) -> Rc<Self> {
        let lease = photos.edit(key, |p| p.create_media_view());
        let this = Rc::new(Self {
            base: MediaPreload::new(done),
            key,
            lease,
            owner,
            life: Lifetime::new(),
        });
        this.start(origin);
        this
    }
    fn start(self: &Rc<Self>, origin: FileOrigin) {
        if self.owner.loaded(self.key, &self.lease) {
            self.base.call_done();
        } else {
            self.owner.load(self.key, origin);
            let weak = Rc::downgrade(self);
            self.owner.downloader_task_finished().start(
                move |_| {
                    if let Some(s) = weak.upgrade() {
                        if s.owner.loaded(s.key, &s.lease) {
                            s.base.call_done();
                        }
                    }
                },
                |_| {},
                || {},
                &self.life,
            );
        }
    }
    pub fn lease(&self) -> Rc<RefCell<PhotoMedia>> {
        self.lease.clone()
    }
}
impl Drop for PhotoPreload {
    fn drop(&mut self) {
        self.owner.cancel(self.key);
    }
}
#[derive(Clone, Debug)]
pub enum VideoPreloadEffect {
    CheckCache(CacheKey),
    AddToQueue,
    RemoveFromQueue,
    CancelAllRequests,
    PutIfEmpty {
        key: CacheKey,
        bytes: Vec<u8>,
        tag: u8,
    },
}
#[derive(Default)]
struct VideoState {
    parts: BTreeMap<u32, Vec<u8>>,
    requested: BTreeSet<i32>,
    next_offset: i32,
    finished: bool,
    failed: bool,
}
pub struct VideoPreload {
    base: MediaPreload,
    docs: Rc<Documents>,
    video: DocumentRecordKey,
    origin: FileOrigin,
    full: i64,
    location: super::data_image_location::StorageFileLocation,
    state: RefCell<VideoState>,
    effect: Rc<dyn Fn(VideoPreloadEffect)>,
}
impl VideoPreload {
    pub fn can(video: &DocumentData) -> bool {
        video.can_be_streamed()
            && video.video_preload_location().valid()
            && video.big_file_base_cache_key() != CacheKey::default()
    }
    pub fn choose_preload_prefix(video: &DocumentData) -> i64 {
        let n = video.video_preload_prefix();
        if n != 0 {
            i64::from(n)
        } else {
            DEFAULT_PRELOAD_PREFIX.min(video.size)
        }
    }
    pub fn new(
        docs: Rc<Documents>,
        video: DocumentRecordKey,
        origin: FileOrigin,
        effect: Rc<dyn Fn(VideoPreloadEffect)>,
        done: Rc<dyn Fn()>,
    ) -> Rc<Self> {
        let full = docs.read(video, |d| d.size).expect("document lifetime");
        let location = docs
            .read(video, DocumentData::video_preload_location)
            .unwrap();
        let this = Rc::new(Self {
            base: MediaPreload::new(done),
            location,
            docs,
            video,
            origin,
            full,
            state: RefCell::default(),
            effect,
        });
        if this.docs.read(video, Self::can).unwrap() {
            this.check();
        } else {
            this.base.call_done();
        }
        this
    }
    pub fn location(&self) -> &super::data_image_location::StorageFileLocation {
        &self.location
    }
    pub fn video(&self) -> DocumentRecordKey {
        self.video
    }
    pub fn origin(&self) -> &FileOrigin {
        &self.origin
    }
    fn check(&self) {
        let key = self
            .docs
            .read(self.video, DocumentData::big_file_base_cache_key)
            .unwrap();
        (self.effect)(VideoPreloadEffect::CheckCache(key));
    }
    /// Owner routes the weak cache callback through crl/on-main equivalent before this.
    pub fn cache_result(&self, result: &[u8]) {
        if result.is_empty() {
            self.load();
        } else {
            self.base.call_done();
        }
    }
    pub fn load(&self) {
        if !self.docs.read(self.video, Self::can).unwrap() {
            self.base.call_done();
            return;
        }
        let prefix = self
            .docs
            .read(self.video, Self::choose_preload_prefix)
            .unwrap();
        assert!(prefix > 0 && prefix <= self.docs.read(self.video, |d| d.size).unwrap());
        let parts = (prefix + i64::from(DOWNLOAD_PART_SIZE) - 1) / i64::from(DOWNLOAD_PART_SIZE);
        {
            let mut s = self.state.borrow_mut();
            for i in 0..parts {
                s.parts
                    .entry((i as u32) * (DOWNLOAD_PART_SIZE as u32))
                    .or_default();
            }
        }
        (self.effect)(VideoPreloadEffect::AddToQueue);
    }
    pub fn ready_to_request(&self) -> bool {
        let s = self.state.borrow();
        !s.failed && i64::from(s.next_offset) < s.parts.len() as i64 * i64::from(DOWNLOAD_PART_SIZE)
    }
    pub fn take_next_request_offset(&self) -> i64 {
        assert!(self.ready_to_request());
        let mut s = self.state.borrow_mut();
        let offset = s.next_offset;
        s.requested.insert(offset);
        s.next_offset = s.next_offset.wrapping_add(DOWNLOAD_PART_SIZE);
        i64::from(*s.requested.last().unwrap())
    }
    pub fn feed_part(&self, offset: i64, bytes: &[u8]) -> bool {
        let result = {
            let mut s = self.state.borrow_mut();
            assert!(offset < s.parts.len() as i64 * i64::from(DOWNLOAD_PART_SIZE));
            assert!(s.requested.remove(&(offset as i32)));
            assert!(bytes.len() <= DOWNLOAD_PART_SIZE as usize);
            s.parts.insert(offset as u32, bytes.to_vec());
            if (i64::from(s.next_offset) + i64::from(DOWNLOAD_PART_SIZE)
                >= s.parts.len() as i64 * i64::from(DOWNLOAD_PART_SIZE))
                && s.requested.is_empty()
            {
                s.finished = true;
                let mut bytes = serialize_complex_parts_map(&s.parts);
                if bytes.len() as i64 == self.full {
                    bytes.push(0);
                }
                Some(bytes)
            } else {
                None
            }
        };
        if let Some(result) = result {
            (self.effect)(VideoPreloadEffect::RemoveFromQueue);
            self.done(result);
        }
        true
    }
    fn done(&self, result: Vec<u8>) {
        let key = self
            .docs
            .read(self.video, DocumentData::big_file_base_cache_key)
            .unwrap();
        if !result.is_empty() && key != CacheKey::default() {
            assert!(result.len() < 10 * 1024 * 1024);
            (self.effect)(VideoPreloadEffect::PutIfEmpty {
                key,
                bytes: result,
                tag: 0,
            });
        }
        self.base.call_done();
    }
    pub fn cancel_on_fail(&self) {
        self.state.borrow_mut().failed = true;
        (self.effect)(VideoPreloadEffect::CancelAllRequests);
        self.done(vec![]);
    }
    pub fn set_web_file_size_hook(&self, _size: i64) -> bool {
        self.cancel_on_fail();
        false
    }
}
impl Drop for VideoPreload {
    fn drop(&mut self) {
        let s = self.state.get_mut();
        if !s.finished && !s.failed {
            (self.effect)(VideoPreloadEffect::CancelAllRequests);
        }
    }
}
/// Original Streaming::SerializeComplexPartsMap framing, explicit little endian.
pub fn serialize_complex_parts_map(parts: &BTreeMap<u32, Vec<u8>>) -> Vec<u8> {
    let mut out = vec![];
    out.extend_from_slice(&(parts.len() as i32).to_le_bytes());
    for (&offset, bytes) in parts {
        out.extend_from_slice(&(offset as i32).to_le_bytes());
        out.extend_from_slice(&(bytes.len() as i32).to_le_bytes());
        out.extend_from_slice(bytes);
    }
    out
}
