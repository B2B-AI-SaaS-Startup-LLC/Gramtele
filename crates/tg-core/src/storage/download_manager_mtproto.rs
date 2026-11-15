//! DC queue/task machines from download_manager_mtproto. Clocks are explicit
//! milliseconds; effects are applied by the owner after releasing its borrow.
use super::{
    file_download::{Effect as FileEffect, FailureReason},
    file_download_mtproto::{MtpFileLoader, TaskEffect, DOWNLOAD_PART_SIZE},
};
use crate::data::{
    data_file_origin::{FileLocationId, FileOrigin, UpdatedFileReferences},
    data_image_location::{DownloadLocation, StorageFileType},
};
use std::collections::{BTreeMap, VecDeque};
use tg_mtproto::{
    mtp_instance::get_next_request_id, mtproto_auth_key::CtrState, mtproto_response::Error,
};
use tg_scheme as tl;
pub const PART_SIZE: i32 = DOWNLOAD_PART_SIZE as i32;
pub const MAX_WAITED: i32 = 16 * PART_SIZE;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaskId(pub u64);
#[derive(Default)]
pub struct Queue {
    pub tasks: Vec<(TaskId, i32)>,
}
impl Queue {
    pub fn enqueue(&mut self, id: TaskId, priority: i32) {
        let pos = self
            .tasks
            .iter()
            .position(|t| t.1 <= priority)
            .unwrap_or(self.tasks.len());
        let index = if let Some(i) = self.tasks.iter().position(|t| t.0 == id) {
            self.tasks[i].1 = priority;
            i
        } else {
            self.tasks.push((id, priority));
            self.tasks.len() - 1
        };
        if pos < index {
            self.tasks[pos..=index].rotate_right(1);
        } else if pos > index + 1 {
            self.tasks[index..pos].rotate_left(1);
        }
    }
    pub fn remove(&mut self, id: TaskId) {
        self.tasks.retain(|t| t.0 != id);
    }
    pub fn reset_generation(&mut self) {
        if let Some(start) = self.tasks.iter().position(|t| t.1 == 0) {
            for t in &mut self.tasks[start..] {
                if t.1 != 0 {
                    assert_eq!(t.1, -1, "source Queue::resetGeneration invariant");
                    break;
                }
                t.1 = -1;
            }
        }
    }
    pub fn next_task(&self, only_highest: bool, ready: impl Fn(TaskId) -> bool) -> Option<TaskId> {
        let highest = self.tasks.first()?.1;
        self.tasks
            .iter()
            .take_while(|t| !only_highest || highest <= 0 || t.1 == highest)
            .find(|t| ready(t.0))
            .map(|t| t.0)
    }
}
#[derive(Clone, Debug)]
pub struct SessionBalance {
    pub requested: i32,
    pub successes: i32,
    pub max_waited: i32,
}
impl Default for SessionBalance {
    fn default() -> Self {
        Self {
            requested: 0,
            successes: 0,
            max_waited: 4 * PART_SIZE,
        }
    }
}
#[derive(Clone, Debug)]
pub struct DcBalance {
    pub sessions: Vec<SessionBalance>,
    pub last_remove: i64,
    pub remove_index: usize,
    pub remove_times: i32,
    pub timeouts: i32,
    pub total_requested: i32,
}
impl Default for DcBalance {
    fn default() -> Self {
        Self {
            sessions: vec![SessionBalance::default()],
            last_remove: 0,
            remove_index: 0,
            remove_times: 0,
            timeouts: 0,
            total_requested: 0,
        }
    }
}
#[derive(Clone, Debug)]
pub struct RequestData {
    pub offset: i64,
    pub session: usize,
    pub amount: i32,
    pub sent: i64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CdnHash {
    pub offset: i64,
    pub limit: i32,
    pub hash: Vec<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HashResult {
    NoHash,
    Invalid,
    Good,
}
/// `limit` is retained but deliberately NOT checked by the original hash function.
pub fn check_cdn_file_hash(
    hashes: &BTreeMap<i64, CdnHash>,
    offset: i64,
    bytes: &[u8],
) -> HashResult {
    let Some(h) = hashes.get(&offset) else {
        return HashResult::NoHash;
    };
    if tg_base::openssl_help::sha256(bytes) == h.hash {
        HashResult::Good
    } else {
        HashResult::Invalid
    }
}
pub fn decrypt_cdn(bytes: &mut [u8], key: &[u8; 32], iv: &[u8; 16], offset: i64) {
    let mut iv = *iv;
    iv[12..].copy_from_slice(&((offset >> 4) as u32).to_be_bytes());
    CtrState::new(key, &iv).apply(bytes);
}
#[derive(Clone)]
pub enum DownloadRequest {
    File {
        location: DownloadLocation,
        offset: i64,
    },
    Cdn {
        token: Vec<u8>,
        offset: i64,
    },
    Hashes {
        token: Vec<u8>,
        offset: i64,
    },
    Reupload {
        token: Vec<u8>,
        request_token: Vec<u8>,
    },
}
#[derive(Clone)]
pub enum Effect {
    Send {
        task: TaskId,
        id: i32,
        dc: i32,
        request: DownloadRequest,
    },
    Cancel(i32),
    KillSession(i32),
    StopSession(i32),
    RefreshFileReference {
        task: TaskId,
        id: i32,
        origin: FileOrigin,
        current: Vec<u8>,
    },
    File {
        task: TaskId,
        effect: FileEffect,
    },
    Continue {
        dc: i32,
    },
    ContinueHashes {
        task: TaskId,
        checked: bool,
    },
    TaskFinished,
    NonPremiumDelay(u64),
}
pub struct DownloadMtprotoTask {
    pub loader: MtpFileLoader,
    pub dc: i32,
    pub sent: BTreeMap<i32, RequestData>,
    by_offset: BTreeMap<i64, i32>,
    pub cdn_dc: i32,
    cdn_token: Vec<u8>,
    cdn_key: Vec<u8>,
    cdn_iv: Vec<u8>,
    pub hashes: BTreeMap<i64, CdnHash>,
    unchecked: BTreeMap<i64, (RequestData, Vec<u8>)>,
    hashes_request: i32,
}
impl DownloadMtprotoTask {
    pub fn new(loader: MtpFileLoader, dc: i32) -> Self {
        Self {
            loader,
            dc,
            sent: BTreeMap::new(),
            by_offset: BTreeMap::new(),
            cdn_dc: 0,
            cdn_token: Vec::new(),
            cdn_key: Vec::new(),
            cdn_iv: Vec::new(),
            hashes: BTreeMap::new(),
            unchecked: BTreeMap::new(),
            hashes_request: 0,
        }
    }
    pub fn have_sent_requests(&self) -> bool {
        !self.sent.is_empty() || !self.unchecked.is_empty()
    }
    pub fn have_sent_request_for_offset(&self, offset: i64) -> bool {
        self.by_offset.contains_key(&offset) || self.unchecked.contains_key(&offset)
    }
    pub fn object_id(&self) -> u64 {
        if let DownloadLocation::Storage(s) = &self.loader.location {
            s.id
        } else {
            0
        }
    }
    pub fn location(&self) -> &DownloadLocation {
        &self.loader.location
    }
}
#[derive(Default)]
pub struct DownloadManagerMtproto {
    pub queues: BTreeMap<i32, Queue>,
    pub balances: BTreeMap<i32, DcBalance>,
    pub tasks: BTreeMap<TaskId, DownloadMtprotoTask>,
    next_task: u64,
    reset_at: Option<i64>,
    kill_when: BTreeMap<i32, i64>,
    kill_timer: Option<i64>,
    deferred: VecDeque<Work>,
    effects: Vec<Effect>,
}
#[derive(Clone, Copy)]
enum Work {
    Timeout(i32, usize),
    Hashes(TaskId),
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Finish {
    Success,
    Redirect,
    Cancel,
}
impl DownloadManagerMtproto {
    pub fn insert(&mut self, task: DownloadMtprotoTask) -> TaskId {
        self.next_task += 1;
        let id = TaskId(self.next_task);
        self.tasks.insert(id, task);
        id
    }
    pub fn enqueue(&mut self, id: TaskId, priority: i32, now: i64) {
        let dc = self.tasks[&id].dc;
        self.queues.entry(dc).or_default().enqueue(id, priority);
        self.reset_at.get_or_insert(now + 200);
        self.pump(dc, now);
    }
    pub fn remove(&mut self, id: TaskId, now: i64) {
        let Some(t) = self.tasks.get(&id) else {
            return;
        };
        let dc = t.dc;
        self.queues.entry(dc).or_default().remove(id);
        self.pump(dc, now);
    }
    pub fn choose_session_index(&self, dc: i32) -> usize {
        self.balances[&dc]
            .sessions
            .iter()
            .enumerate()
            .min_by_key(|(_, s)| s.requested)
            .unwrap()
            .0
    }
    pub fn pump_all(&mut self, now: i64) {
        let dcs = self
            .queues
            .iter()
            .filter(|(_, q)| !q.tasks.is_empty())
            .map(|(&dc, _)| dc)
            .collect::<Vec<_>>();
        for dc in dcs {
            self.pump(dc, now);
        }
    }
    pub fn pump(&mut self, dc: i32, now: i64) {
        self.balances.entry(dc).or_default();
        loop {
            let b = &self.balances[&dc];
            let (index, s) = b
                .sessions
                .iter()
                .enumerate()
                .min_by_key(|(_, s)| {
                    if s.requested < s.max_waited {
                        s.requested
                    } else {
                        MAX_WAITED
                    }
                })
                .unwrap();
            if s.requested + PART_SIZE > s.max_waited {
                break;
            }
            let Some(task) = self.queues.get(&dc).and_then(|q| {
                q.next_task(b.total_requested > 0, |id| {
                    self.tasks
                        .get(&id)
                        .is_some_and(|t| t.loader.ready_to_request(t.have_sent_requests()))
                })
            }) else {
                break;
            };
            let t = self.tasks.get_mut(&task).unwrap();
            let offset = t
                .loader
                .take_next_request_offset(t.have_sent_requests())
                .unwrap();
            self.make_request(
                task,
                RequestData {
                    offset,
                    session: index,
                    amount: 0,
                    sent: now,
                },
                now,
            );
        }
    }
    fn amount(&mut self, dc: i32, index: usize, delta: i32, now: i64) -> i32 {
        let b = self.balances.entry(dc).or_default();
        b.sessions[index].requested += delta;
        b.total_requested += delta;
        let result = b.sessions[index].requested;
        if delta > 0 {
            self.kill_when.remove(&dc);
            if self.kill_when.is_empty() {
                self.kill_timer = None;
            }
        } else if b.sessions.iter().all(|s| s.requested <= 0) {
            self.kill_when.entry(dc).or_insert(now + 15000);
            self.kill_timer.get_or_insert(now + 15005);
        }
        result
    }
    fn send(
        &mut self,
        task: TaskId,
        mut data: RequestData,
        request: DownloadRequest,
        route_dc: i32,
        now: i64,
    ) -> i32 {
        let id = get_next_request_id();
        let dc = self.tasks[&task].dc;
        data.amount = self.amount(dc, data.session, PART_SIZE, now);
        data.sent = now;
        let route = tg_mtproto::facade::download_dc_id(route_dc, data.session as i32);
        let t = self.tasks.get_mut(&task).unwrap();
        assert!(t.by_offset.insert(data.offset, id).is_none());
        t.sent.insert(id, data);
        self.effects.push(Effect::Send {
            task,
            id,
            dc: route,
            request,
        });
        id
    }
    fn make_request(&mut self, task: TaskId, data: RequestData, now: i64) {
        let t = &self.tasks[&task];
        let (dc, request) = if t.cdn_dc != 0 {
            (
                t.cdn_dc,
                DownloadRequest::Cdn {
                    token: t.cdn_token.clone(),
                    offset: data.offset,
                },
            )
        } else {
            (
                t.dc,
                DownloadRequest::File {
                    location: t.loader.location.clone(),
                    offset: data.offset,
                },
            )
        };
        self.send(task, data, request, dc, now);
    }
    fn finish(&mut self, task: TaskId, id: i32, why: Finish, now: i64) -> Option<RequestData> {
        let t = self.tasks.get_mut(&task)?;
        let data = t.sent.remove(&id)?;
        if t.hashes_request == id {
            t.hashes_request = 0;
        }
        t.by_offset.remove(&data.offset);
        let dc = t.dc;
        self.amount(dc, data.session, -PART_SIZE, now);
        if why == Finish::Success {
            self.request_succeeded(dc, &data, now);
        }
        Some(data)
    }
    pub fn request_succeeded(&mut self, dc: i32, data: &RequestData, now: i64) {
        let b = self.balances.get_mut(&dc).unwrap();
        let s = &mut b.sessions[data.session];
        if data.sent <= b.last_remove || data.amount > s.max_waited {
            return;
        }
        if now - data.sent >= 8000 {
            self.deferred.push_back(Work::Timeout(dc, data.session));
            return;
        }
        if data.amount == s.max_waited && s.max_waited < MAX_WAITED {
            s.max_waited = (s.max_waited + PART_SIZE).min(MAX_WAITED);
        }
        s.successes = (s.successes + 1).min(192);
        if b.sessions
            .iter()
            .any(|s| s.successes < (b.remove_times + 1) * 3)
        {
            return;
        }
        for s in &mut b.sessions {
            s.successes = 0;
        }
        if b.timeouts > 0 {
            b.timeouts -= 1;
            return;
        }
        if b.sessions.len() == 8 {
            return;
        }
        if b.last_remove != 0 && now < b.last_remove + (i64::from(b.remove_times) + 1) * 8000 {
            return;
        }
        b.sessions.push(SessionBalance::default());
    }
    pub fn session_timed_out(&mut self, dc: i32, index: usize, now: i64) {
        let Some(b) = self.balances.get_mut(&dc) else {
            return;
        };
        if index >= b.sessions.len() {
            return;
        }
        for s in &mut b.sessions {
            s.successes = 0;
        }
        if b.sessions.len() == 1 {
            return;
        }
        b.timeouts += 1;
        if b.timeouts < 4 {
            return;
        }
        b.timeouts = 0;
        self.remove_session(dc, now);
    }
    fn remove_session(&mut self, dc: i32, now: i64) {
        let b = self.balances.get_mut(&dc).unwrap();
        let index = b.sessions.len() - 1;
        if b.remove_index == index {
            b.remove_times = (b.remove_times + 1).min(64);
        } else {
            b.remove_index = index;
            b.remove_times = 1;
        }
        b.sessions[index].requested += MAX_WAITED * 8;
        let ids = self
            .queues
            .get(&dc)
            .map(|q| q.tasks.iter().map(|t| t.0).collect::<Vec<_>>())
            .unwrap_or_default();
        for task in ids {
            let requests = self.tasks[&task]
                .sent
                .iter()
                .filter(|(_, d)| d.session == index)
                .map(|(&id, _)| id)
                .collect::<Vec<_>>();
            let new = self.choose_session_index(dc);
            for (d, _) in self.tasks.get_mut(&task).unwrap().unchecked.values_mut() {
                if d.session == index {
                    d.session = new;
                }
            }
            for id in requests {
                let hashes = self.tasks[&task].hashes_request == id;
                self.effects.push(Effect::Cancel(id));
                if let Some(mut data) = self.finish(task, id, Finish::Cancel, now) {
                    if hashes {
                        if !self.tasks[&task].unchecked.is_empty() {
                            self.deferred.push_back(Work::Hashes(task));
                        }
                    } else {
                        data.session = self.choose_session_index(dc);
                        self.make_request(task, data, now);
                    }
                }
            }
        }
        let b = self.balances.get_mut(&dc).unwrap();
        assert_eq!(b.sessions[index].requested, MAX_WAITED * 8);
        b.sessions.pop();
        b.last_remove = now;
        self.effects
            .push(Effect::KillSession(tg_mtproto::facade::download_dc_id(
                dc,
                index as i32,
            )));
    }
    pub fn cancel_request_for_offset(&mut self, task: TaskId, offset: i64, now: i64) {
        if let Some(id) = self.tasks[&task].by_offset.get(&offset).copied() {
            self.cancel_request(task, id, now);
        }
        self.tasks.get_mut(&task).unwrap().unchecked.remove(&offset);
    }
    fn cancel_request(&mut self, task: TaskId, id: i32, now: i64) {
        let hashes = self.tasks[&task].hashes_request == id;
        self.effects.push(Effect::Cancel(id));
        self.finish(task, id, Finish::Cancel, now);
        if hashes && !self.tasks[&task].unchecked.is_empty() {
            self.deferred.push_back(Work::Hashes(task));
        }
    }
    pub fn cancel_all_requests(&mut self, task: TaskId, now: i64) {
        let ids = self.tasks[&task].sent.keys().copied().collect::<Vec<_>>();
        for id in ids {
            self.cancel_request(task, id, now);
        }
        self.tasks.get_mut(&task).unwrap().unchecked.clear();
    }
    pub fn close_task(&mut self, task: TaskId, now: i64) {
        if !self.tasks.contains_key(&task) {
            return;
        }
        if !self.tasks[&task].loader.file.finished() {
            self.tasks
                .get_mut(&task)
                .unwrap()
                .loader
                .cancel(FailureReason::NoFailure);
            self.drain_loader(task, now);
        }
        self.cancel_all_requests(task, now);
        self.remove(task, now);
        self.tasks.remove(&task);
    }
    fn fail(&mut self, task: TaskId, now: i64) {
        self.tasks.get_mut(&task).unwrap().loader.cancel_on_fail();
        self.drain_loader(task, now);
    }
    pub fn start_task(&mut self, task: TaskId, now: i64) {
        self.tasks.get_mut(&task).unwrap().loader.file.start();
        self.drain_loader(task, now);
    }
    pub fn local_loaded(&mut self, task: TaskId, bytes: &[u8], now: i64) {
        if let Some(t) = self.tasks.get_mut(&task) {
            t.loader.file.local_loaded(bytes);
            self.drain_loader(task, now);
        }
    }
    pub fn drain_loader(&mut self, task: TaskId, now: i64) {
        loop {
            let task_effects = self.tasks.get_mut(&task).unwrap().loader.take_effects();
            let file_effects = self
                .tasks
                .get_mut(&task)
                .unwrap()
                .loader
                .file
                .take_effects();
            if task_effects.is_empty() && file_effects.is_empty() {
                break;
            }
            for e in task_effects {
                match e {
                    TaskEffect::AddToQueue { priority } => self.enqueue(task, priority, now),
                    TaskEffect::RemoveFromQueue => self.remove(task, now),
                    TaskEffect::CancelAllRequests => self.cancel_all_requests(task, now),
                }
            }
            for effect in file_effects {
                match effect {
                    FileEffect::StartLoading { partial } => {
                        let t = self.tasks.get_mut(&task).unwrap();
                        if let Some(data) = partial {
                            t.loader
                                .start_loading_with_partial(&data, t.have_sent_requests());
                        } else {
                            t.loader.start_loading();
                        }
                    }
                    FileEffect::CancelRequests => self.cancel_all_requests(task, now),
                    FileEffect::NotifyDownloaderTaskFinished => {
                        self.effects.push(Effect::TaskFinished)
                    }
                    effect => self.effects.push(Effect::File { task, effect }),
                }
            }
        }
    }
    fn feed(&mut self, task: TaskId, data: RequestData, bytes: &[u8], now: i64) {
        let t = self.tasks.get_mut(&task).unwrap();
        t.loader
            .feed_part(data.offset, bytes, t.have_sent_requests());
        self.drain_loader(task, now);
    }
    pub fn normal_part_loaded(&mut self, task: TaskId, id: i32, result: tl::UploadFile, now: i64) {
        let Some(data) = self.finish(task, id, Finish::Success, now) else {
            return;
        };
        let dc = self.tasks[&task].dc;
        match result {
            tl::UploadFile::UploadFile(d) => self.feed(task, data, &d.bytes, now),
            tl::UploadFile::UploadFileCdnRedirect(d) => self.change_cdn_params(
                task,
                data,
                d.dc_id,
                d.file_token.clone(),
                d.encryption_key.clone(),
                d.encryption_iv.clone(),
                tl_hashes(&d.file_hashes),
                now,
            ),
        }
        self.effects.push(Effect::Continue { dc });
    }
    pub fn web_part_loaded(&mut self, task: TaskId, id: i32, result: tl::UploadWebFile, now: i64) {
        let Some(data) = self.finish(task, id, Finish::Success, now) else {
            return;
        };
        let dc = self.tasks[&task].dc;
        let d = result.c_upload_web_file();
        if self
            .tasks
            .get_mut(&task)
            .unwrap()
            .loader
            .set_web_file_size_hook(i64::from(d.size))
        {
            self.feed(task, data, &d.bytes, now);
        } else {
            self.drain_loader(task, now);
        }
        self.effects.push(Effect::Continue { dc });
    }
    pub fn cdn_part_loaded(&mut self, task: TaskId, id: i32, result: tl::UploadCdnFile, now: i64) {
        if !self
            .tasks
            .get(&task)
            .is_some_and(|t| t.sent.contains_key(&id))
        {
            return;
        }
        match result {
            tl::UploadCdnFile::UploadCdnFileReuploadNeeded(d) => {
                let data = self.finish(task, id, Finish::Redirect, now).unwrap();
                let t = &self.tasks[&task];
                self.send(
                    task,
                    data,
                    DownloadRequest::Reupload {
                        token: t.cdn_token.clone(),
                        request_token: d.request_token.clone(),
                    },
                    t.dc,
                    now,
                );
            }
            tl::UploadCdnFile::UploadCdnFile(d) => {
                let data = self.finish(task, id, Finish::Success, now).unwrap();
                let t = &self.tasks[&task];
                let dc = t.dc;
                let mut bytes = d.bytes.clone();
                decrypt_cdn(
                    &mut bytes,
                    t.cdn_key.as_slice().try_into().unwrap(),
                    t.cdn_iv.as_slice().try_into().unwrap(),
                    data.offset,
                );
                match check_cdn_file_hash(&t.hashes, data.offset, &bytes) {
                    HashResult::NoHash => {
                        self.tasks
                            .get_mut(&task)
                            .unwrap()
                            .unchecked
                            .entry(data.offset)
                            .or_insert((data, bytes));
                        self.request_more_hashes(task, now);
                    }
                    HashResult::Invalid => self.fail(task, now),
                    HashResult::Good => self.feed(task, data, &bytes, now),
                }
                self.effects.push(Effect::Continue { dc });
            }
        }
    }
    fn add_hashes(&mut self, task: TaskId, hashes: Vec<CdnHash>) {
        for h in hashes {
            self.tasks
                .get_mut(&task)
                .unwrap()
                .hashes
                .entry(h.offset)
                .or_insert(h);
        }
    }
    fn request_more_hashes(&mut self, task: TaskId, now: i64) {
        let Some(t) = self.tasks.get(&task) else {
            return;
        };
        if t.hashes_request != 0 {
            return;
        }
        let Some((data, _)) = t.unchecked.values().next() else {
            return;
        };
        let data = data.clone();
        let dc = t.dc;
        let token = t.cdn_token.clone();
        let id = self.send(
            task,
            data,
            DownloadRequest::Hashes {
                token,
                offset: t.unchecked.keys().next().copied().unwrap(),
            },
            dc,
            now,
        );
        self.tasks.get_mut(&task).unwrap().hashes_request = id;
    }
    pub fn reupload_done(&mut self, task: TaskId, id: i32, hashes: Vec<CdnHash>, now: i64) {
        if let Some(data) = self.finish(task, id, Finish::Redirect, now) {
            self.add_hashes(task, hashes);
            self.make_request(task, data, now);
        }
    }
    pub fn hashes_done(&mut self, task: TaskId, id: i32, hashes: Vec<CdnHash>, now: i64) {
        if self.tasks.get(&task).is_none_or(|t| t.hashes_request != id) {
            return;
        }
        let Some(data) = self.finish(task, id, Finish::Redirect, now) else {
            return;
        };
        self.add_hashes(task, hashes);
        let _ = data;
        self.continue_hashes(task, false, now);
    }
    /// One verified feed per callback boundary: the owner may destroy/cancel
    /// the loader in progress callbacks before the remaining unchecked parts.
    pub fn continue_hashes(&mut self, task: TaskId, checked: bool, now: i64) {
        let Some(t) = self.tasks.get(&task) else {
            return;
        };
        if t.loader.file.finished() {
            return;
        }
        let offsets = t.unchecked.keys().copied().collect::<Vec<_>>();
        for offset in offsets {
            let t = &self.tasks[&task];
            let (_, bytes) = &t.unchecked[&offset];
            match check_cdn_file_hash(&t.hashes, offset, bytes) {
                HashResult::NoHash => {}
                HashResult::Invalid => {
                    self.fail(task, now);
                    return;
                }
                HashResult::Good => {
                    let (data, bytes) = self
                        .tasks
                        .get_mut(&task)
                        .unwrap()
                        .unchecked
                        .remove(&offset)
                        .unwrap();
                    self.feed(task, data, &bytes, now);
                    self.effects.push(Effect::ContinueHashes {
                        task,
                        checked: true,
                    });
                    return;
                }
            }
        }
        if !checked {
            self.fail(task, now);
            return;
        }
        self.request_more_hashes(task, now);
    }
    pub fn apply_continuation(&mut self, effect: &Effect, now: i64) -> bool {
        match *effect {
            Effect::Continue { dc } => self.pump(dc, now),
            Effect::ContinueHashes { task, checked } => self.continue_hashes(task, checked, now),
            _ => return false,
        }
        true
    }
    #[allow(clippy::too_many_arguments)]
    pub fn change_cdn_params(
        &mut self,
        task: TaskId,
        data: RequestData,
        dc: i32,
        token: Vec<u8>,
        key: Vec<u8>,
        iv: Vec<u8>,
        hashes: Vec<CdnHash>,
        now: i64,
    ) {
        if dc != 0 && (key.len() != 32 || iv.len() != 16) {
            self.fail(task, now);
            return;
        }
        let t = self.tasks.get_mut(&task).unwrap();
        let resend = t.cdn_dc != dc || t.cdn_token != token || t.cdn_key != key || t.cdn_iv != iv;
        t.cdn_dc = dc;
        t.cdn_token = token;
        t.cdn_key = key;
        t.cdn_iv = iv;
        self.add_hashes(task, hashes);
        if resend {
            let ids = self.tasks[&task].sent.keys().copied().collect::<Vec<_>>();
            let mut requests = Vec::new();
            for id in ids {
                self.effects.push(Effect::Cancel(id));
                if let Some(d) = self.finish(task, id, Finish::Redirect, now) {
                    requests.push(d);
                }
            }
            for d in requests {
                self.make_request(task, d, now);
            }
        }
        self.make_request(task, data, now);
    }
    pub fn part_failed(
        &mut self,
        task: TaskId,
        id: i32,
        error: &Error,
        cdn: bool,
        reference: Option<&[u8]>,
        now: i64,
    ) -> bool {
        if !self
            .tasks
            .get(&task)
            .is_some_and(|t| t.sent.contains_key(&id))
        {
            return true;
        }
        if error.is_default_handled_error() {
            return false;
        }
        if let Some(reference) =
            reference.filter(|_| error.code == 400 && error.kind.starts_with("FILE_REFERENCE_"))
        {
            self.effects.push(Effect::RefreshFileReference {
                task,
                id,
                origin: self.tasks[&task].loader.origin.clone(),
                current: reference.to_vec(),
            });
            return true;
        }
        if cdn
            && matches!(
                error.kind.as_str(),
                "FILE_TOKEN_INVALID" | "REQUEST_TOKEN_INVALID"
            )
        {
            let data = self.finish(task, id, Finish::Redirect, now).unwrap();
            self.change_cdn_params(
                task,
                data,
                0,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                now,
            );
            return true;
        }
        self.fail(task, now);
        true
    }
    pub fn refresh_file_reference_from(
        &mut self,
        task: TaskId,
        id: i32,
        updates: &UpdatedFileReferences,
        current: &[u8],
        now: i64,
    ) {
        let Some(t) = self.tasks.get_mut(&task) else {
            return;
        };
        if !t.sent.contains_key(&id) {
            return;
        }
        let DownloadLocation::Storage(s) = &mut t.loader.location else {
            self.fail(task, now);
            return;
        };
        let key = match s.kind {
            StorageFileType::Document => Some(FileLocationId::Document(s.id)),
            StorageFileType::Photo => Some(FileLocationId::Photo(s.id)),
            _ => None,
        };
        if let Some(bytes) = key.and_then(|k| updates.data.get(&k)) {
            if !bytes.is_empty() {
                s.file_reference = bytes.clone();
            }
        }
        if s.file_reference == current {
            self.fail(task, now);
            return;
        }
        if let Some(data) = self.finish(task, id, Finish::Redirect, now) {
            self.make_request(task, data, now);
        }
    }
    pub fn non_premium_delayed(&mut self, id: i32) {
        for t in self.tasks.values() {
            if t.sent.contains_key(&id)
                && matches!(&t.loader.location,DownloadLocation::Storage(s) if s.kind==StorageFileType::Document && s.id!=0)
            {
                self.effects.push(Effect::NonPremiumDelay(t.object_id()));
            }
        }
    }
    pub fn advance_to(&mut self, now: i64) {
        if self.reset_at.is_some_and(|at| at <= now) {
            for q in self.queues.values_mut() {
                q.reset_generation();
            }
            self.reset_at = None;
        }
        if self.kill_timer.is_some_and(|at| at <= now) {
            self.kill_timer = None;
            let dcs = self
                .kill_when
                .iter()
                .filter(|(_, at)| **at <= now)
                .map(|(&dc, _)| dc)
                .collect::<Vec<_>>();
            for dc in dcs {
                self.stop_dc(dc);
                self.kill_when.remove(&dc);
            }
            if !self.kill_when.is_empty() {
                let left = self
                    .kill_when
                    .values()
                    .map(|at| at - now)
                    .min()
                    .unwrap()
                    .min(15000);
                self.kill_timer = Some(now + left);
            }
        }
        let work = std::mem::take(&mut self.deferred);
        for w in work {
            match w {
                Work::Timeout(dc, index) => self.session_timed_out(dc, index, now),
                Work::Hashes(task) => self.request_more_hashes(task, now),
            }
        }
    }
    fn stop_dc(&mut self, dc: i32) {
        if let Some(b) = self.balances.get_mut(&dc) {
            assert_eq!(b.total_requested, 0);
            let count = b.sessions.len();
            *b = DcBalance::default();
            b.sessions = vec![SessionBalance::default(); count];
            for i in 0..count {
                self.effects
                    .push(Effect::StopSession(tg_mtproto::facade::download_dc_id(
                        dc, i as i32,
                    )));
            }
        }
    }
    pub fn close(&mut self, now: i64) {
        let dcs = self
            .kill_when
            .iter()
            .filter(|(_, at)| **at <= now)
            .map(|(&dc, _)| dc)
            .collect::<Vec<_>>();
        for dc in dcs {
            self.stop_dc(dc);
            self.kill_when.remove(&dc);
        }
    }
    pub fn has_deferred_work(&self) -> bool {
        !self.deferred.is_empty()
    }
    pub fn next_deadline(&self) -> Option<i64> {
        self.reset_at.into_iter().chain(self.kill_timer).min()
    }
    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }
}
pub fn tl_hashes(hashes: &[tl::FileHash]) -> Vec<CdnHash> {
    hashes
        .iter()
        .map(|h| {
            let h = h.c_file_hash();
            CdnHash {
                offset: h.offset as i64,
                limit: h.limit,
                hash: h.hash.clone(),
            }
        })
        .collect()
}

impl DownloadManagerMtproto {
    /// Pinned cpp:135 passes raw GetDcIdShift (16+index), NOT index. Therefore
    /// instance-restart notices are ignored by its >=sessions.size() guard.
    /// https://github.com/AyuGram/AyuGramDesktop/blob/0f2a31bb420de98f331990432265b131b8d836b5/Telegram/SourceFiles/storage/download_manager_mtproto.cpp#L135
    pub fn restart_by_timeout(&mut self, shifted: i32, now: i64) {
        if tg_mtproto::facade::is_download_dc_id(shifted) {
            self.session_timed_out(
                tg_mtproto::core_types::bare_dc_id(shifted),
                tg_mtproto::core_types::get_dc_id_shift(shifted) as usize,
                now,
            );
        }
    }
}
#[derive(Default)]
pub struct DownloadEvents {
    finished: tg_rpl::EventStream<()>,
    non_premium: tg_rpl::EventStream<u64>,
}
impl DownloadEvents {
    pub fn task_finished(&self) -> tg_rpl::Producer<()> {
        self.finished.events()
    }
    pub fn non_premium_delays(&self) -> tg_rpl::Producer<u64> {
        self.non_premium.events()
    }
    pub fn apply(&self, effect: &Effect) -> bool {
        match *effect {
            Effect::TaskFinished => self.finished.fire(()),
            Effect::NonPremiumDelay(id) => self.non_premium.fire(id),
            _ => return false,
        }
        true
    }
}
