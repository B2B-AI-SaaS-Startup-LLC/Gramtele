//! Uploader's transport/preparation state. PreparedUpload is a scalar/encoded
//! projection of the real FilePrepareResult, not a fake image/session model.
use crate::data::data_msg_id::FullMsgId;
use md5::{Digest, Md5};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs::File,
    io::Read,
    path::PathBuf,
};
use tg_mtproto::{mtp_instance::get_next_request_id, mtproto_response::Error};
use tg_scheme as tl;
pub fn thumbnail_format(mime: &str) -> &'static str {
    if crate::core::mime_type::is_mime_sticker(mime) {
        "WEBP"
    } else {
        "JPG"
    }
}
pub const USE_BIG_FILES_FROM: i64 = 30 * 1024 * 1024;
pub const MAX_UPLOAD_PER_SESSION: usize = 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UploadKind {
    Photo,
    File,
    ThemeFile,
    Audio,
    Round,
    SecondaryFile,
    Secure,
}
impl UploadKind {
    pub fn is_document(self) -> bool {
        matches!(
            self,
            Self::File | Self::ThemeFile | Self::Audio | Self::Round
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preparation {
    Transcode,
    Archive,
}
#[derive(Clone)]
pub enum Content {
    Bytes(Vec<u8>),
    Path(PathBuf),
}
/// Only fields consumed by this machine. Real options/edit/History/thumbnail
/// objects remain owned by the caller under `item`, not manufactured here.
#[derive(Clone)]
pub struct PreparedUpload {
    pub id: u64,
    pub kind: UploadKind,
    pub filename: String,
    pub size: i64,
    pub content: Content,
    pub parts: Vec<Vec<u8>>,
    pub parts_id: u64,
    pub parts_name: String,
    pub parts_md5: Vec<u8>,
    pub file_md5: Vec<u8>,
    pub parts_size: i64,
    pub preparing: Option<Preparation>,
    pub video_cover: Option<Box<PreparedUpload>>,
    pub attached_stickers: Vec<tl::InputDocument>,
    pub force_file: bool,
    pub preparation_lease: Option<std::sync::Arc<super::localimageloader::FilePrepareResult>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartLayout {
    pub size: i32,
    pub count: u16,
}
pub fn part_layout(size: i64) -> PartLayout {
    let count = |part: i32| ((size + i64::from(part) - 1) / i64::from(part)) as u16;
    if size < 1024 * 1024 && count(32768) <= 4000 {
        return PartLayout {
            size: 32768,
            count: count(32768),
        };
    }
    if size <= 32 * 1024 * 1024 && count(65536) <= 4000 {
        return PartLayout {
            size: 65536,
            count: count(65536),
        };
    }
    for part in [131072, 262144, 524288] {
        if count(part) <= 4000 || part == 524288 {
            return PartLayout {
                size: part,
                count: count(part),
            };
        }
    }
    unreachable!()
}
pub struct Entry {
    pub item: FullMsgId,
    pub file: PreparedUpload,
    pub sent_size: i64,
    pub parts_sent: u16,
    pub parts_waiting: u16,
    pub doc_sent_size: i64,
    pub doc_parts_sent: u16,
    pub doc_parts_waiting: u16,
    pub layout: PartLayout,
    pub preparing: bool,
    md5: Md5,
    doc_file: Option<File>,
}
impl Entry {
    pub fn new(item: FullMsgId, mut file: PreparedUpload) -> Self {
        if !file.kind.is_document() {
            file.parts_id = file.id;
        }
        let layout = if file.kind.is_document() {
            part_layout(file.size)
        } else {
            PartLayout { size: 0, count: 0 }
        };
        let preparing = file.preparing.is_some();
        Self {
            item,
            file,
            sent_size: 0,
            parts_sent: 0,
            parts_waiting: 0,
            doc_sent_size: 0,
            doc_parts_sent: 0,
            doc_parts_waiting: 0,
            layout,
            preparing,
            md5: Md5::new(),
            doc_file: None,
        }
    }
    fn read_doc_part(&mut self) -> Vec<u8> {
        let n = self.layout.size as usize;
        let bytes = match &self.file.content {
            Content::Bytes(b) if !b.is_empty() => {
                let offset = usize::from(self.doc_parts_sent) * n;
                b.get(offset..b.len().min(offset.saturating_add(n)))
                    .unwrap_or_default()
                    .to_vec()
            }
            Content::Bytes(_) | Content::Path(_) => {
                if self.doc_file.is_none() {
                    let Content::Path(path) = &self.file.content else {
                        return Vec::new();
                    };
                    let Ok(file) = File::open(path) else {
                        return Vec::new();
                    };
                    self.doc_file = Some(file);
                }
                let mut bytes = vec![0; n];
                let mut read = 0;
                while read < n {
                    match self.doc_file.as_mut().unwrap().read(&mut bytes[read..]) {
                        Ok(0) => break,
                        Ok(n) => read += n,
                        Err(_) => return Vec::new(),
                    }
                }
                bytes.truncate(read);
                bytes
            }
        };
        // Source feeds MD5 before validating part length.
        if self.file.kind.is_document() && self.file.size <= USE_BIG_FILES_FROM {
            self.md5.update(&bytes);
        }
        if bytes.is_empty()
            || bytes.len() > n
            || (bytes.len() < n && self.doc_parts_sent.wrapping_add(1) != self.layout.count)
        {
            Vec::new()
        } else {
            bytes
        }
    }
    fn complete(&self) -> bool {
        usize::from(self.parts_sent) >= self.file.parts.len()
            && self.doc_parts_sent >= self.layout.count
            && self.parts_waiting == 0
            && self.doc_parts_waiting == 0
    }
}
#[derive(Clone, Debug)]
pub struct Request {
    pub item: FullMsgId,
    pub sent: i64,
    pub bytes: Vec<u8>,
    pub queued: usize,
    pub part: u16,
    pub dc_index: usize,
    pub doc_part: bool,
    pub big_part: bool,
    pub non_premium_delayed: bool,
}
#[derive(Clone)]
pub enum UploadRequest {
    Small(tl::UploadSaveFilePart),
    Big(tl::UploadSaveBigFilePart),
}
#[derive(Clone)]
pub struct UploadedMedia {
    pub id: u64,
    pub item: FullMsgId,
    pub file: tl::InputFile,
    pub thumb: Option<tl::InputFile>,
    pub kind: UploadKind,
    pub video_cover: Option<tl::InputPhoto>,
    pub attached_stickers: Vec<tl::InputDocument>,
    pub force_file: bool,
}
#[derive(Clone)]
pub enum Effect {
    Send {
        id: i32,
        dc: i32,
        request: UploadRequest,
    },
    Cancel(i32),
    StopSession(i32),
    BeginUpload {
        item: FullMsgId,
        id: u64,
        kind: UploadKind,
        size: i64,
        preparing: bool,
    },
    Progress {
        item: FullMsgId,
        id: u64,
        kind: UploadKind,
        offset: i64,
        size: i64,
    },
    Failed {
        item: FullMsgId,
        id: u64,
        kind: UploadKind,
    },
    Ready(UploadedMedia),
    NeedCoverId {
        video: UploadedMedia,
        cover: Box<PreparedUpload>,
    },
    UploadCover {
        video: FullMsgId,
        cover: UploadedMedia,
    },
    SecureReady {
        item: FullMsgId,
        id: u64,
        parts: i32,
    },
    NonPremiumDelay(FullMsgId),
    ContinuePart(FullMsgId),
    ContinueFailure(FullMsgId),
    ContinueCancelAll,
    ContinuePreparation,
    /// Keeps original Entry's prepared file alive through its synchronous ready callback.
    ReleasePreparation(std::sync::Arc<super::localimageloader::FilePrepareResult>),
    ContinueFinish {
        send: bool,
    },
    ContinueSending {
        used: BTreeSet<usize>,
        dc: usize,
    },
    StartPreparation {
        item: FullMsgId,
        kind: Preparation,
    },
    CancelPreparation(FullMsgId),
    PreparationFinished {
        id: u64,
        size: i64,
    },
    PreparationProgress {
        item: FullMsgId,
        id: u64,
        progress: f64,
    },
    RemoveTemporary(PathBuf),
}
#[derive(Default)]
struct PrepareQueue {
    list: VecDeque<FullMsgId>,
    running: Option<FullMsgId>,
}
#[derive(Default)]
pub struct Uploader {
    pub queue: Vec<Entry>,
    pub requests: BTreeMap<i32, Request>,
    pub sent_per_dc: Vec<usize>,
    pub fast: BTreeSet<usize>,
    pub latest_added: i64,
    pub latest_removed: i64,
    pub pending: VecDeque<Request>,
    pub paused: Option<FullMsgId>,
    next_timer: Option<i64>,
    stop_timer: Option<i64>,
    archive: PrepareQueue,
    transcode: PrepareQueue,
    effects: Vec<Effect>,
    deferred_send: bool,
    video_to_cover: BTreeMap<FullMsgId, FullMsgId>,
    waiting_cover: BTreeMap<FullMsgId, UploadedMedia>,
}
#[derive(Clone, Copy)]
enum SendResult {
    Success,
    Failed,
    Full,
}
impl Uploader {
    pub fn current_upload_id(&self) -> FullMsgId {
        self.queue
            .first()
            .map_or_else(FullMsgId::default, |e| e.item)
    }
    pub fn upload(&mut self, item: FullMsgId, file: PreparedUpload, now: i64) {
        self.effects.push(Effect::BeginUpload {
            item,
            id: file.id,
            kind: file.kind,
            size: if file.kind.is_document() {
                file.size
            } else {
                file.parts_size
            },
            preparing: file.preparing.is_some(),
        });
        let preparing = file.preparing;
        self.queue.push(Entry::new(item, file));
        if let Some(kind) = preparing {
            let q = if kind == Preparation::Archive {
                &mut self.archive
            } else {
                &mut self.transcode
            };
            q.list.push_back(item);
            self.maybe_start_preparation(kind);
        } else if self.next_timer.is_none() {
            self.maybe_send(now);
        }
    }
    fn maybe_start_preparation(&mut self, kind: Preparation) {
        let q = if kind == Preparation::Archive {
            &mut self.archive
        } else {
            &mut self.transcode
        };
        if q.running.is_some() {
            return;
        }
        while let Some(item) = q.list.pop_front() {
            if self.queue.iter().any(|e| e.item == item && e.preparing) {
                q.running = Some(item);
                self.effects.push(Effect::StartPreparation { item, kind });
                return;
            }
        }
    }
    pub fn update_prepare_progress(&mut self, item: FullMsgId, progress: f64) {
        if let Some(e) = self.queue.iter().find(|e| e.item == item && e.preparing) {
            self.effects.push(Effect::PreparationProgress {
                item,
                id: e.file.id,
                progress,
            });
        }
    }
    /// Worker callback after actual archive/transcode work. No fake preparer is used.
    pub fn finish_preparation(
        &mut self,
        item: FullMsgId,
        kind: Preparation,
        result: Option<Content>,
        now: i64,
    ) {
        let q = if kind == Preparation::Archive {
            &mut self.archive
        } else {
            &mut self.transcode
        };
        q.running = None;
        let size = match &result {
            Some(Content::Bytes(b)) => b.len() as i64,
            Some(Content::Path(p)) => std::fs::metadata(p).map_or(0, |m| m.len() as i64),
            None => 0,
        };
        if let Some(e) = self
            .queue
            .iter_mut()
            .find(|e| e.item == item && e.preparing)
        {
            if size > 0 {
                if let (Some(prepared), Some(content)) = (&e.file.preparation_lease, &result) {
                    prepared.materialize(content, size);
                }
                e.file.content = result.unwrap();
                e.file.size = size;
                e.layout = part_layout(size);
                e.preparing = false;
                e.file.preparing = None;
                self.effects.push(Effect::PreparationFinished {
                    id: e.file.id,
                    size,
                });
                self.effects.push(Effect::PreparationProgress {
                    item,
                    id: e.file.id,
                    progress: 1.,
                });
                self.effects.push(Effect::ContinuePreparation);
            } else {
                self.failed(item, now);
            }
        } else if let Some(Content::Path(path)) = result {
            self.effects.push(Effect::RemoveTemporary(path));
        }
        self.maybe_start_preparation(Preparation::Transcode);
        self.maybe_start_preparation(Preparation::Archive);
    }
    pub fn can_add_dc_index(&self) -> bool {
        self.sent_per_dc.len() < 8 && self.sent_per_dc.len() == self.fast.len()
    }
    pub fn choose_dc_index(&mut self, used: &BTreeSet<usize>, now: i64) -> Option<usize> {
        if let Some(i) = self
            .sent_per_dc
            .iter()
            .enumerate()
            .position(|(i, n)| *n == 0 && !used.contains(&i))
        {
            return Some(i);
        }
        if self.can_add_dc_index() {
            let i = self.sent_per_dc.len();
            self.sent_per_dc.push(0);
            self.fast.clear();
            self.latest_added = now;
            return Some(i);
        }
        self.sent_per_dc
            .iter()
            .enumerate()
            .filter(|(i, _)| !used.contains(i))
            .min_by_key(|(_, n)| **n)
            .map(|(i, _)| i)
    }
    fn choose_entry(&self) -> Option<usize> {
        if let Some(r) = self.pending.front() {
            return self.queue.iter().position(|e| e.item == r.item);
        }
        self.queue.iter().position(|e| {
            !e.preparing
                && (usize::from(e.parts_sent) < e.file.parts.len()
                    || e.doc_parts_sent < e.layout.count)
        })
    }
    fn send_request(&mut self, mut request: Request, file_id: u64, count: u16, now: i64) {
        let id = get_next_request_id();
        request.queued = self.sent_per_dc[request.dc_index];
        self.sent_per_dc[request.dc_index] += request.bytes.len();
        request.sent = now;
        let rpc = if request.big_part {
            UploadRequest::Big(tl::UploadSaveBigFilePart::new(
                file_id,
                i32::from(request.part),
                i32::from(count),
                request.bytes.clone(),
            ))
        } else {
            UploadRequest::Small(tl::UploadSaveFilePart::new(
                file_id,
                i32::from(request.part),
                request.bytes.clone(),
            ))
        };
        self.effects.push(Effect::Send {
            id,
            dc: tg_mtproto::facade::upload_dc_id(request.dc_index as i32),
            request: rpc,
        });
        self.requests.insert(id, request);
    }
    fn send_part(&mut self, index: usize, dc: usize, now: i64) -> SendResult {
        let e = &mut self.queue[index];
        let item = e.item;
        if let Some(mut r) = self.pending.pop_front() {
            assert_eq!(r.item, item);
            r.dc_index = dc;
            let id = if r.doc_part {
                e.file.id
            } else {
                e.file.parts_id
            };
            let count = e.layout.count;
            self.send_request(r, id, count, now);
            return SendResult::Success;
        }
        let (bytes, part, doc_part, big_part, file_id, count) =
            if usize::from(e.parts_sent) < e.file.parts.len() {
                let bytes = e.file.parts[usize::from(e.parts_sent)].clone();
                if self.sent_per_dc[dc] + bytes.len() >= MAX_UPLOAD_PER_SESSION {
                    return SendResult::Full;
                }
                let part = e.parts_sent;
                e.parts_sent = e.parts_sent.wrapping_add(1);
                e.parts_waiting = e.parts_waiting.wrapping_add(1);
                // Pinned file_upload.cpp:843 does not initialize Request.part for sliced uploads.
                // https://github.com/AyuGram/AyuGramDesktop/blob/0f2a31bb420de98f331990432265b131b8d836b5/Telegram/SourceFiles/storage/file_upload.cpp#L843
                let rpc_id = e.file.parts_id;
                let request = Request {
                    item,
                    sent: now,
                    bytes: bytes.clone(),
                    queued: 0,
                    part: 0,
                    dc_index: dc,
                    doc_part: false,
                    big_part: false,
                    non_premium_delayed: false,
                };
                let id = get_next_request_id();
                let queued = self.sent_per_dc[dc];
                self.sent_per_dc[dc] += bytes.len();
                let mut request = request;
                request.queued = queued;
                self.requests.insert(id, request);
                self.effects.push(Effect::Send {
                    id,
                    dc: tg_mtproto::facade::upload_dc_id(dc as i32),
                    request: UploadRequest::Small(tl::UploadSaveFilePart::new(
                        rpc_id,
                        i32::from(part),
                        bytes,
                    )),
                });
                return SendResult::Success;
            } else {
                if self.sent_per_dc[dc] + e.layout.size as usize > MAX_UPLOAD_PER_SESSION {
                    return SendResult::Full;
                }
                let bytes = e.read_doc_part();
                if bytes.is_empty() {
                    self.failed(item, now);
                    return SendResult::Failed;
                }
                let part = e.doc_parts_sent;
                e.doc_parts_sent = e.doc_parts_sent.wrapping_add(1);
                e.doc_parts_waiting = e.doc_parts_waiting.wrapping_add(1);
                (
                    bytes,
                    part,
                    true,
                    e.file.size > USE_BIG_FILES_FROM,
                    e.file.id,
                    e.layout.count,
                )
            };
        self.send_request(
            Request {
                item,
                sent: now,
                bytes,
                queued: 0,
                part,
                dc_index: dc,
                doc_part,
                big_part,
                non_premium_delayed: false,
            },
            file_id,
            count,
            now,
        );
        SendResult::Success
    }
    pub fn maybe_send(&mut self, now: i64) {
        if self.queue.is_empty() {
            self.stop_timer.get_or_insert(now + 15000);
            self.paused = None;
            return;
        }
        if self.paused.is_some() {
            return;
        }
        self.stop_timer = None;
        self.send_loop(BTreeSet::new(), None, now);
    }
    fn send_loop(&mut self, mut used: BTreeSet<usize>, mut resume: Option<usize>, now: i64) {
        while let Some(dc) = resume.take().or_else(|| self.choose_dc_index(&used, now)) {
            let Some(index) = self.choose_entry() else {
                return;
            };
            match self.send_part(index, dc, now) {
                SendResult::Full => return,
                SendResult::Success => {}
                SendResult::Failed => {
                    self.effects.push(Effect::ContinueSending { used, dc });
                    return;
                }
            }
            if self.sent_per_dc[dc] >= 512 * 1024 {
                used.insert(dc);
            }
        }
        self.next_timer = if used.is_empty() {
            None
        } else {
            Some(now + 200)
        };
    }
    pub fn pause(&mut self, item: FullMsgId) {
        self.paused = Some(item);
    }
    pub fn unpause(&mut self, now: i64) {
        self.paused = None;
        self.maybe_send(now);
    }
    fn finish_request(&mut self, id: i32) -> Option<Request> {
        let r = self.requests.remove(&id)?;
        self.sent_per_dc[r.dc_index] -= r.bytes.len();
        Some(r)
    }
    pub fn part_loaded(&mut self, id: i32, ok: bool, now: i64) {
        let Some(r) = self.finish_request(id) else {
            return;
        };
        if !ok {
            self.failed(r.item, now);
            return;
        }
        if now - r.sent >= 1000 {
            self.fast.clear();
            if now - r.sent >= 8000
                && now - self.latest_removed >= 8000
                && self.sent_per_dc.len() > 1
            {
                self.remove_dc_index();
                self.latest_removed = now;
            }
        } else if r.sent > self.latest_added && r.queued + r.bytes.len() >= 512 * 1024 {
            self.fast.insert(r.dc_index);
        }
        let Some(e) = self.queue.iter_mut().find(|e| e.item == r.item) else {
            return;
        };
        if r.doc_part {
            e.doc_parts_waiting = e.doc_parts_waiting.wrapping_sub(1);
            e.doc_sent_size += r.bytes.len() as i64;
        } else {
            e.parts_waiting = e.parts_waiting.wrapping_sub(1);
            e.sent_size += r.bytes.len() as i64;
        }
        self.effects.push(Effect::Progress {
            item: r.item,
            id: e.file.id,
            kind: e.file.kind,
            offset: if e.file.kind.is_document() {
                e.doc_sent_size.min(e.file.size)
            } else {
                e.sent_size
            },
            size: if e.file.kind.is_document() {
                e.file.size
            } else {
                e.file.parts_size
            },
        });
        if r.non_premium_delayed {
            self.effects.push(Effect::NonPremiumDelay(r.item));
        }
        self.effects.push(Effect::ContinuePart(r.item));
    }
    pub fn continue_part(&mut self, item: FullMsgId, now: i64) {
        if self.queue.first().is_some_and(|e| e.item == item) {
            self.continue_finish(true, now);
        } else {
            self.maybe_send(now);
        }
    }
    fn continue_finish(&mut self, send: bool, now: i64) {
        if self.queue.first().is_some_and(Entry::complete) {
            self.finish_front();
            self.effects.push(Effect::ContinueFinish { send });
        } else if send {
            self.maybe_send(now);
        }
    }
    /// Dispatch notifications after releasing the machine borrow, then apply
    /// this continuation and drain its effects before the next outer effect.
    /// This preserves synchronous C++ callback reentrancy without holding a borrow.
    pub fn apply_continuation(&mut self, effect: &Effect, now: i64) -> bool {
        match effect {
            Effect::ContinuePart(item) => self.continue_part(*item, now),
            Effect::ContinueFailure(item) => self.continue_failure(*item, now),
            Effect::ContinueCancelAll => self.cancel_all(now),
            Effect::ReleasePreparation(_) => {}
            Effect::ContinuePreparation => {
                if self.next_timer.is_none() {
                    self.maybe_send(now);
                }
            }
            Effect::ContinueFinish { send } => self.continue_finish(*send, now),
            Effect::ContinueSending { used, dc } => self.send_loop(used.clone(), Some(*dc), now),
            _ => return false,
        }
        true
    }

    pub fn part_failed(&mut self, id: i32, _error: &Error, now: i64) {
        if let Some(r) = self.finish_request(id) {
            self.failed(r.item, now);
        }
    }
    pub fn non_premium_delayed(&mut self, id: i32) {
        if let Some(r) = self.requests.get_mut(&id) {
            r.non_premium_delayed = true;
        }
    }
    pub fn remove_dc_index(&mut self) {
        let dc = self.sent_per_dc.len() - 1;
        assert!(dc > 0);
        let ids = self
            .requests
            .iter()
            .filter(|(_, r)| r.dc_index == dc)
            .map(|(&id, _)| id)
            .collect::<Vec<_>>();
        for id in ids {
            let r = self.finish_request(id).unwrap();
            self.effects.push(Effect::Cancel(id));
            self.pending.push_back(r);
        }
        assert_eq!(self.sent_per_dc[dc], 0);
        self.sent_per_dc.pop();
        self.fast.remove(&dc);
        self.effects
            .push(Effect::StopSession(tg_mtproto::facade::upload_dc_id(
                dc as i32,
            )));
    }
    fn finish_front(&mut self) {
        let mut e = self.queue.remove(0);
        let lease = e.file.preparation_lease.take();
        let kind = e.file.kind;
        let count = if kind.is_document() {
            i32::from(e.layout.count)
        } else {
            e.file.parts.len() as i32
        };
        if kind == UploadKind::Secure {
            self.effects.push(Effect::SecureReady {
                item: e.item,
                id: e.file.id,
                parts: count,
            });
            if let Some(lease) = lease {
                self.effects.push(Effect::ReleasePreparation(lease));
            }
            return;
        }
        let mut name = e.file.filename.clone();
        if kind == UploadKind::Photo && !crate::core::mime_type::qt_ascii_ci_suffix(&name, ".jpg") {
            name.push_str(".jpg");
        }
        let md5 = if kind.is_document() {
            e.md5
                .finalize()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
                .into_bytes()
        } else {
            e.file.file_md5.clone()
        };
        let file = if kind.is_document() && e.file.size > USE_BIG_FILES_FROM {
            tl::mtp_input_file_big(e.file.id, count, name.into_bytes())
        } else {
            tl::mtp_input_file(e.file.id, count, name.into_bytes(), md5)
        };
        let thumb = if kind.is_document() && !e.file.parts.is_empty() {
            Some(tl::mtp_input_file(
                e.file.parts_id,
                e.file.parts.len() as i32,
                e.file.parts_name.into_bytes(),
                e.file.parts_md5,
            ))
        } else {
            None
        };
        let ready = UploadedMedia {
            id: e.file.id,
            item: e.item,
            file,
            thumb,
            kind,
            video_cover: None,
            attached_stickers: e.file.attached_stickers,
            force_file: kind.is_document() && e.file.force_file,
        };
        if kind == UploadKind::Photo && self.waiting_cover.contains_key(&e.item) {
            let video = self.waiting_cover[&e.item].item;
            self.effects.push(Effect::UploadCover {
                video,
                cover: ready,
            });
        } else if let Some(cover) = e.file.video_cover {
            self.effects.push(Effect::NeedCoverId {
                video: ready,
                cover,
            });
        } else {
            self.effects.push(Effect::Ready(ready));
        }
        if let Some(lease) = lease {
            self.effects.push(Effect::ReleasePreparation(lease));
        }
    }
    pub fn upload_video_cover(
        &mut self,
        video: UploadedMedia,
        cover_item: FullMsgId,
        cover: PreparedUpload,
        now: i64,
    ) {
        self.video_to_cover.insert(video.item, cover_item);
        self.waiting_cover.insert(cover_item, video);
        self.upload(cover_item, cover, now);
    }
    pub fn cover_uploaded(
        &mut self,
        video: FullMsgId,
        cover: FullMsgId,
        result: tl::MessageMedia,
        now: i64,
    ) {
        let photo = match result {
            tl::MessageMedia::MessageMediaPhoto(d) => d.photo.clone(),
            _ => None,
        };
        let Some(tl::Photo::Photo(photo)) = photo else {
            self.failed(cover, now);
            return;
        };
        if let Some(key) = self.video_to_cover.remove(&video) {
            if let Some(mut data) = self.waiting_cover.remove(&key) {
                data.video_cover = Some(tl::mtp_input_photo(
                    photo.id,
                    photo.access_hash,
                    photo.file_reference.clone(),
                ));
                self.effects.push(Effect::Ready(data));
            }
        }
    }

    pub fn failed(&mut self, item: FullMsgId, _now: i64) {
        if let Some(i) = self.queue.iter().position(|e| e.item == item) {
            let mut e = self.queue.remove(i);
            let lease = e.file.preparation_lease.take();
            if e.preparing {
                self.effects.push(Effect::CancelPreparation(item));
            }
            self.effects.push(Effect::Failed {
                item,
                id: e.file.id,
                kind: e.file.kind,
            });
            if let Some(lease) = lease {
                self.effects.push(Effect::ReleasePreparation(lease));
            }
        } else if let Some(cover) = self.video_to_cover.remove(&item) {
            if let Some(video) = self.waiting_cover.remove(&cover) {
                self.effects.push(Effect::Failed {
                    item: video.item,
                    id: video.id,
                    kind: video.kind,
                });
            }
            self.failed(cover, _now);
        } else if let Some(video) = self.waiting_cover.remove(&item) {
            self.video_to_cover.remove(&video.item);
            self.effects.push(Effect::Failed {
                item: video.item,
                id: video.id,
                kind: video.kind,
            });
        }
        self.effects.push(Effect::ContinueFailure(item));
    }
    pub fn continue_failure(&mut self, item: FullMsgId, now: i64) {
        self.cancel_requests(item);
        self.continue_finish(false, now);
        self.deferred_send = true;
    }

    pub fn cancel(&mut self, item: FullMsgId, now: i64) {
        self.failed(item, now);
    }
    pub fn cancel_requests(&mut self, item: FullMsgId) {
        let ids = self
            .requests
            .iter()
            .filter(|(_, r)| r.item == item)
            .map(|(&id, _)| id)
            .collect::<Vec<_>>();
        for id in ids {
            self.finish_request(id);
            self.effects.push(Effect::Cancel(id));
        }
        self.pending.retain(|r| r.item != item);
    }
    pub fn cancel_all_requests(&mut self) {
        for id in std::mem::take(&mut self.requests).into_keys() {
            self.effects.push(Effect::Cancel(id));
        }
        self.sent_per_dc.fill(0);
    }
    pub fn stop_sessions(&mut self, now: i64) {
        if self.sent_per_dc.iter().any(|n| *n != 0) {
            self.stop_timer = Some(now + 15000);
        } else {
            for i in 0..self.sent_per_dc.len() {
                self.effects
                    .push(Effect::StopSession(tg_mtproto::facade::upload_dc_id(
                        i as i32,
                    )));
            }
            self.sent_per_dc.clear();
            self.fast.clear();
        }
    }
    pub fn clear(&mut self, now: i64) {
        for e in &self.queue {
            if e.preparing {
                self.effects.push(Effect::CancelPreparation(e.item));
            }
        }
        self.transcode.list.clear();
        self.archive.list.clear();
        self.queue.clear();
        self.cancel_all_requests();
        self.stop_sessions(now);
        self.stop_timer = None;
    }
    pub fn cancel_all(&mut self, now: i64) {
        if let Some(item) = self.queue.first().map(|e| e.item) {
            self.failed(item, now);
            self.effects.push(Effect::ContinueCancelAll);
        } else {
            self.clear(now);
            self.unpause(now);
        }
    }
    pub fn advance_to(&mut self, now: i64) {
        if self.next_timer.is_some_and(|at| at <= now) {
            self.next_timer = None;
            self.maybe_send(now);
        }
        if self.stop_timer.is_some_and(|at| at <= now) {
            self.stop_timer = None;
            self.stop_sessions(now);
        }
        if std::mem::take(&mut self.deferred_send) {
            self.maybe_send(now);
        }
    }
    pub fn has_deferred_work(&self) -> bool {
        self.deferred_send
    }
    pub fn next_deadline(&self) -> Option<i64> {
        self.next_timer.into_iter().chain(self.stop_timer).min()
    }
    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadFileProgress {
    pub full_id: FullMsgId,
    pub offset: i64,
    pub size: i64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadSecureDone {
    pub full_id: FullMsgId,
    pub file_id: u64,
    pub parts_count: i32,
}
#[derive(Default)]
pub struct UploadEvents {
    photo_ready: tg_rpl::EventStream<UploadedMedia>,
    document_ready: tg_rpl::EventStream<UploadedMedia>,
    secondary_ready: tg_rpl::EventStream<UploadedMedia>,
    secure_ready: tg_rpl::EventStream<UploadSecureDone>,
    photo_progress: tg_rpl::EventStream<FullMsgId>,
    document_progress: tg_rpl::EventStream<FullMsgId>,
    secondary_progress: tg_rpl::EventStream<UploadFileProgress>,
    secure_progress: tg_rpl::EventStream<UploadFileProgress>,
    photo_failed: tg_rpl::EventStream<FullMsgId>,
    document_failed: tg_rpl::EventStream<FullMsgId>,
    secondary_failed: tg_rpl::EventStream<FullMsgId>,
    secure_failed: tg_rpl::EventStream<FullMsgId>,
    non_premium: tg_rpl::EventStream<FullMsgId>,
}
impl UploadEvents {
    pub fn photo_ready(&self) -> tg_rpl::Producer<UploadedMedia> {
        self.photo_ready.events()
    }
    pub fn document_ready(&self) -> tg_rpl::Producer<UploadedMedia> {
        self.document_ready.events()
    }
    pub fn secondary_file_ready(&self) -> tg_rpl::Producer<UploadedMedia> {
        self.secondary_ready.events()
    }
    pub fn secure_ready(&self) -> tg_rpl::Producer<UploadSecureDone> {
        self.secure_ready.events()
    }
    pub fn photo_progress(&self) -> tg_rpl::Producer<FullMsgId> {
        self.photo_progress.events()
    }
    pub fn document_progress(&self) -> tg_rpl::Producer<FullMsgId> {
        self.document_progress.events()
    }
    pub fn secondary_file_progress(&self) -> tg_rpl::Producer<UploadFileProgress> {
        self.secondary_progress.events()
    }
    pub fn secure_progress(&self) -> tg_rpl::Producer<UploadFileProgress> {
        self.secure_progress.events()
    }
    pub fn photo_failed(&self) -> tg_rpl::Producer<FullMsgId> {
        self.photo_failed.events()
    }
    pub fn document_failed(&self) -> tg_rpl::Producer<FullMsgId> {
        self.document_failed.events()
    }
    pub fn secondary_file_failed(&self) -> tg_rpl::Producer<FullMsgId> {
        self.secondary_failed.events()
    }
    pub fn secure_failed(&self) -> tg_rpl::Producer<FullMsgId> {
        self.secure_failed.events()
    }
    pub fn non_premium_delays(&self) -> tg_rpl::Producer<FullMsgId> {
        self.non_premium.events()
    }
    pub fn apply(&self, effect: &Effect) -> bool {
        match effect {
            Effect::Ready(v) => match v.kind {
                UploadKind::Photo => self.photo_ready.fire(v.clone()),
                UploadKind::SecondaryFile => self.secondary_ready.fire(v.clone()),
                _ => self.document_ready.fire(v.clone()),
            },
            Effect::SecureReady { item, id, parts } => self.secure_ready.fire(UploadSecureDone {
                full_id: *item,
                file_id: *id,
                parts_count: *parts,
            }),
            Effect::Progress {
                item,
                kind,
                offset,
                size,
                ..
            } => {
                let v = UploadFileProgress {
                    full_id: *item,
                    offset: *offset,
                    size: *size,
                };
                match kind {
                    UploadKind::Photo => self.photo_progress.fire(*item),
                    UploadKind::SecondaryFile => self.secondary_progress.fire(v),
                    UploadKind::Secure => self.secure_progress.fire(v),
                    _ => self.document_progress.fire(*item),
                }
            }
            Effect::Failed { item, kind, .. } => match kind {
                UploadKind::Photo => self.photo_failed.fire(*item),
                UploadKind::SecondaryFile => self.secondary_failed.fire(*item),
                UploadKind::Secure => self.secure_failed.fire(*item),
                _ => self.document_failed.fire(*item),
            },
            Effect::NonPremiumDelay(item) => self.non_premium.fire(*item),
            Effect::PreparationProgress { item, .. } => self.document_progress.fire(*item),
            _ => return false,
        }
        true
    }
}

/// Apply before publishing UploadEvents, inside the real store's edit closure.
/// Cache/location, thumbnails and History side effects retain their own owners.
pub fn apply_document_upload_effect(
    effect: &Effect,
    document: &mut crate::data::data_document::DocumentData,
) -> bool {
    use crate::data::data_media_types::{FileStatus, UploadState};
    match effect {
        Effect::BeginUpload {
            id,
            kind,
            size,
            preparing,
            ..
        } if *id == document.id() && kind.is_document() => {
            let mut state = UploadState::new(*size);
            state.preparing = *preparing;
            document.uploading_data = Some(state);
        }
        Effect::Progress {
            id, kind, offset, ..
        } if *id == document.id() && kind.is_document() => {
            if let Some(state) = &mut document.uploading_data {
                state.offset = (*offset).min(state.size);
            }
        }
        Effect::PreparationProgress { id, progress, .. } if *id == document.id() => {
            if let Some(state) = &mut document.uploading_data {
                state.prepare_progress = *progress;
            }
        }
        Effect::PreparationFinished { id, size } if *id == document.id() => {
            document.size = *size;
            if let Some(state) = &mut document.uploading_data {
                state.size = *size;
                state.offset = 0;
                state.preparing = false;
                state.prepare_progress = 1.;
            }
        }
        Effect::Failed { id, kind, .. } if *id == document.id() && kind.is_document() => {
            if let Some(state) = &mut document.uploading_data {
                state.preparing = false;
                document.status = FileStatus::UploadFailed;
            }
        }
        _ => return false,
    }
    true
}
pub fn apply_photo_upload_effect(
    effect: &Effect,
    photo: &mut crate::data::data_photo::PhotoData,
) -> bool {
    match effect {
        Effect::BeginUpload {
            id,
            kind: UploadKind::Photo,
            size,
            ..
        } if *id == photo.id() => {
            photo.uploading_data = Some(crate::data::data_media_types::UploadState::new(*size));
        }
        Effect::Progress {
            id,
            kind: UploadKind::Photo,
            size,
            offset,
            ..
        } if *id == photo.id() => {
            if let Some(state) = &mut photo.uploading_data {
                state.size = *size;
                state.offset = *offset;
            }
        }
        _ => return false,
    }
    true
}
