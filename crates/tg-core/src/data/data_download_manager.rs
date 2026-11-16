//! Download queue and exact Qt_5_1 list codec. Real History/Session resolve and UI:16/18–20/29/5.
use super::{
    data_msg_id::{FullMsgId, MsgId},
    data_peer_id::*,
    data_store_keys::*,
};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::Arc,
};
use tg_base::{
    qt_stream::{Reader, Writer},
    timer::{Timer, TimerHost},
};
use tg_rpl::{EventStream, Producer, Variable};
pub const MAX_FILE_SIZE: i64 = 4000 * 1024 * 1024;
pub const MAX_RESOLVE_PER_ATTEMPT: usize = 100;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum DownloadType {
    #[default]
    Document = 0,
    Photo = 1,
}
pub type DownloadDate = i64;
pub fn date_from_download_date(date: i64) -> i32 {
    (date / 1000) as i32
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DownloadId {
    pub object_id: u64,
    pub type_: DownloadType,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DownloadProgress {
    pub ready: i64,
    pub total: i64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DownloadObject {
    pub item: MessageKey,
    pub document: Option<DocumentRecordKey>,
    pub photo: Option<PhotoRecordKey>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DownloadedId {
    pub download: DownloadId,
    pub started: DownloadDate,
    pub path: Option<Vec<u16>>,
    pub size: i64,
    pub item_id: FullMsgId,
    pub peer_access_hash: u64,
    pub object: Option<DownloadObject>,
}
impl DownloadedId {
    pub fn path_text(&self) -> String {
        String::from_utf16_lossy(self.path.as_deref().unwrap_or_default())
    }
}
pub fn serialize(downloaded: &[DownloadedId]) -> Vec<u8> {
    let mut w = Writer::default();
    w.i32(downloaded.len() as i32);
    for id in downloaded {
        w.u64(id.download.object_id);
        w.i32(id.download.type_ as i32);
        w.u64(id.started as u64);
        w.i32(id.size as u32 as i32);
        w.u64(id.item_id.peer.value);
        w.u64(id.item_id.msg.bare as u64);
        w.u64(id.peer_access_hash);
        w.nullable_utf16(id.path.as_deref())
    }
    w.0
}
pub fn deserialize(serialized: &[u8]) -> Vec<DownloadedId> {
    deserialize_checked(serialized).unwrap_or_default()
}
pub fn deserialize_checked(serialized: &[u8]) -> Option<Vec<DownloadedId>> {
    if serialized.is_empty() {
        return Some(vec![]);
    }
    let mut q = Reader::new(serialized);
    let count = q.i32()?;
    if !(1..=99_999).contains(&count) {
        return None;
    }
    let mut result = vec![];
    for _ in 0..count {
        let object_id = q.u64()?;
        let type_ = match q.i32()? {
            0 => DownloadType::Document,
            1 => DownloadType::Photo,
            _ => return None,
        };
        let started = q.u64()? as i64;
        let size = i64::from(q.i32()? as u32);
        let peer = PeerId::new(q.u64()?);
        let msg = MsgId::new(q.u64()? as i64);
        let peer_access_hash = q.u64()?;
        let path = q.nullable_utf16()?;
        if path.as_deref().unwrap_or_default().is_empty() || size <= 0 || size > MAX_FILE_SIZE {
            return None;
        }
        result.push(DownloadedId {
            download: DownloadId { object_id, type_ },
            started,
            size,
            path,
            item_id: FullMsgId::new(peer, msg),
            peer_access_hash,
            object: None,
        })
    }
    Some(result)
}
#[derive(Clone)]
pub struct DownloadingId {
    pub object: DownloadObject,
    pub started: DownloadDate,
    pub path: String,
    pub external_cancel: Option<Rc<dyn Fn()>>,
    pub ready: i64,
    pub total: i64,
    pub hidden_by_view: bool,
    pub done: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExternalLoadingState {
    pub ready: i64,
    pub total: i64,
    pub done: bool,
}
#[derive(Default)]
struct SessionData {
    downloaded: Vec<DownloadedId>,
    downloading: Vec<DownloadingId>,
    resolve_needed: usize,
    resolve_sent_total: usize,
    resolve_sent_requests: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    WritePostponed(SessionScopeId),
    Cancel(DownloadObject),
    Repaint(MessageKey),
    ResolveNext(SessionScopeId),
    GenerateEntry {
        scope: SessionScopeId,
        index: usize,
    },
    Regenerate(DownloadObject),
    DeleteFile {
        path: String,
        scope: SessionScopeId,
        document: Option<DocumentRecordKey>,
        item_id: FullMsgId,
    },
}
pub struct DownloadManager {
    sessions: RefCell<BTreeMap<SessionScopeId, SessionData>>,
    loading: RefCell<BTreeSet<MessageKey>>,
    loading_documents: RefCell<BTreeSet<DocumentRecordKey>>,
    done: RefCell<BTreeSet<MessageKey>>,
    last_base: Cell<i32>,
    last_added: Cell<i32>,
    now: Rc<dyn Fn() -> i32>,
    effect: Rc<dyn Fn(Effect)>,
    progress: Variable<DownloadProgress>,
    changes: EventStream<()>,
    added: EventStream<DownloadedId>,
    removed: EventStream<MessageKey>,
    resolve_done: Variable<bool>,
    clear_timer: Timer,
}
impl DownloadManager {
    pub fn new(
        now: Rc<dyn Fn() -> i32>,
        effect: Rc<dyn Fn(Effect)>,
        host: &Rc<TimerHost>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &std::rc::Weak<Self>| {
            let w = weak.clone();
            Self {
                sessions: RefCell::default(),
                loading: RefCell::default(),
                loading_documents: RefCell::default(),
                done: RefCell::default(),
                last_base: Cell::new(0),
                last_added: Cell::new(0),
                now,
                effect,
                progress: Variable::new(DownloadProgress::default()),
                changes: EventStream::new(),
                added: EventStream::new(),
                removed: EventStream::new(),
                resolve_done: Variable::new(false),
                clear_timer: Timer::in_host(
                    host,
                    Some(Rc::new(move || {
                        if let Some(s) = w.upgrade() {
                            s.clear_loading()
                        }
                    })),
                ),
            }
        })
    }
    pub fn empty(&self) -> bool {
        self.sessions
            .borrow()
            .values()
            .all(|d| d.downloaded.is_empty() && d.downloading.is_empty())
    }
    pub fn track_session(&self, scope: SessionScopeId, bytes: &[u8]) {
        let downloaded = deserialize(bytes);
        let needed = downloaded.len();
        self.sessions
            .borrow_mut()
            .entry(scope)
            .or_default()
            .downloaded = downloaded;
        self.sessions
            .borrow_mut()
            .get_mut(&scope)
            .unwrap()
            .resolve_needed = needed;
    }
    pub fn compute_next_start_date(&self) -> i64 {
        let now = (self.now)();
        if self.last_base.replace(now) != now {
            self.last_added.set(0)
        } else {
            self.last_added.set(self.last_added.get().wrapping_add(1))
        }
        i64::from(now)
            .wrapping_mul(1000)
            .wrapping_add(i64::from(self.last_added.get()))
    }
    pub fn loading_list(&self) -> Vec<DownloadingId> {
        self.sessions
            .borrow()
            .values()
            .flat_map(|s| s.downloading.clone())
            .collect()
    }
    pub fn loaded_snapshot(&self) -> Vec<DownloadedId> {
        self.sessions
            .borrow()
            .values()
            .flat_map(|s| s.downloaded.clone())
            .filter(|x| x.object.is_some())
            .collect()
    }
    pub fn loading_progress(&self) -> DownloadProgress {
        self.progress.current()
    }
    pub fn loading_progress_value(&self) -> Producer<DownloadProgress> {
        self.progress.value()
    }
    pub fn loading_list_changes(&self) -> Producer<()> {
        self.changes.events()
    }
    pub fn loaded_added(&self) -> Producer<DownloadedId> {
        self.added.events()
    }
    pub fn loaded_removed(&self) -> Producer<MessageKey> {
        self.removed.events()
    }
    pub fn loaded_resolve_done(&self) -> Producer<()> {
        self.resolve_done.value().filter(|v| *v).map(|_| ())
    }
    pub fn serialized(&self, scope: SessionScopeId) -> Vec<u8> {
        self.sessions
            .borrow()
            .get(&scope)
            .map_or_else(Vec::new, |s| serialize(&s.downloaded))
    }
    pub fn serializator(self: &Rc<Self>, scope: &SessionScope) -> impl Fn() -> Option<Vec<u8>> {
        let alive = scope.downgrade();
        let weak = Rc::downgrade(self);
        let id = scope.id();
        move || {
            alive.upgrade()?;
            Some(weak.upgrade()?.serialized(id))
        }
    }
    pub fn add_loading(
        &self,
        object: DownloadObject,
        path: String,
        total: i64,
        visible: bool,
    ) -> bool {
        assert!(object.document.is_some());
        let existing = self
            .sessions
            .borrow()
            .get(&object.item.scope())
            .expect("untracked session")
            .downloading
            .iter()
            .find(|d| d.object.item == object.item)
            .cloned();
        if let Some(old) = existing {
            if old.object.document == object.document && old.object.photo == object.photo {
                return false;
            }
            self.remove(object.item);
        }
        if path.is_empty() {
            return false;
        }
        let started = self.compute_next_start_date();
        {
            let mut s = self.sessions.borrow_mut();
            let d = s.get_mut(&object.item.scope()).unwrap();
            let shown = d.downloading.first().is_some_and(|d| !d.hidden_by_view);
            d.downloading.push(DownloadingId {
                object,
                started,
                path,
                external_cancel: None,
                ready: 0,
                total,
                hidden_by_view: !shown && visible,
                done: false,
            });
        }
        self.loading.borrow_mut().insert(object.item);
        self.loading_documents
            .borrow_mut()
            .insert(object.document.unwrap());
        let mut p = self.progress.current();
        p.total += total;
        self.progress.set(p);
        self.changes.fire(());
        self.clear_timer.cancel();
        true
    }
    pub fn add_loading_external(
        &self,
        object: DownloadObject,
        path: String,
        total: i64,
        cancel: Rc<dyn Fn()>,
    ) {
        assert!(object.document.is_some());
        let started = self.compute_next_start_date();
        self.sessions
            .borrow_mut()
            .get_mut(&object.item.scope())
            .expect("untracked session")
            .downloading
            .push(DownloadingId {
                object,
                started,
                path,
                external_cancel: Some(cancel),
                ready: 0,
                total,
                hidden_by_view: false,
                done: false,
            });
        self.loading.borrow_mut().insert(object.item);
        let mut p = self.progress.current();
        p.total += total;
        self.progress.set(p);
        self.changes.fire(());
        self.clear_timer.cancel();
    }
    pub fn loading_external_state(&self, item: MessageKey) -> Option<ExternalLoadingState> {
        let s = self.sessions.borrow();
        let d = s
            .get(&item.scope())?
            .downloading
            .iter()
            .find(|d| d.object.item == item)?;
        d.external_cancel.as_ref()?;
        Some(ExternalLoadingState {
            ready: d.ready,
            total: d.total,
            done: d.done,
        })
    }
    pub fn update_loading_external(&self, item: MessageKey, ready: i64, total: i64) {
        let delta = {
            let mut s = self.sessions.borrow_mut();
            let Some(d) = s
                .get_mut(&item.scope())
                .and_then(|s| s.downloading.iter_mut().find(|d| d.object.item == item))
            else {
                return;
            };
            if d.external_cancel.is_none() || d.done {
                return;
            }
            let delta = (ready - d.ready, total - d.total);
            d.ready = ready;
            d.total = total;
            delta
        };
        if delta == (0, 0) {
            return;
        }
        let p = self.progress.current();
        self.progress.set(DownloadProgress {
            ready: p.ready + delta.0,
            total: p.total + delta.1,
        });
        (self.effect)(Effect::Repaint(item));
    }
    pub fn item_visibilities_updated(
        &self,
        scope: SessionScopeId,
        visible: impl Fn(MessageKey) -> bool,
    ) {
        let list = self
            .sessions
            .borrow()
            .get(&scope)
            .map(|d| d.downloading.clone())
            .unwrap_or_default();
        if !list.first().is_some_and(|d| d.hidden_by_view) {
            return;
        }
        if list.iter().any(|d| !d.done && !visible(d.object.item)) {
            for d in &mut self
                .sessions
                .borrow_mut()
                .get_mut(&scope)
                .unwrap()
                .downloading
            {
                d.hidden_by_view = false
            }
            self.changes.fire(());
        }
    }
    fn position(&self, item: MessageKey) -> Option<usize> {
        self.sessions
            .borrow()
            .get(&item.scope())?
            .downloading
            .iter()
            .position(|d| d.object.item == item)
    }
    fn last_position(&self, scope: SessionScopeId) -> Option<(usize, MessageKey)> {
        let all = self.sessions.borrow();
        let data = all.get(&scope)?;
        data.downloading
            .len()
            .checked_sub(1)
            .map(|i| (i, data.downloading[i].object.item))
    }
    fn remove_at(&self, scope: SessionScopeId, index: usize) -> DownloadingId {
        let entry = self
            .sessions
            .borrow_mut()
            .get_mut(&scope)
            .unwrap()
            .downloading
            .remove(index);
        let item = entry.object.item;
        let p = self.progress.current();
        self.loading.borrow_mut().remove(&item);
        self.done.borrow_mut().remove(&item);
        if let Some(doc) = entry.object.document {
            self.loading_documents.borrow_mut().remove(&doc);
        }
        self.changes.fire(());
        self.progress.set(DownloadProgress {
            ready: p.ready - entry.ready,
            total: p.total - entry.total,
        });
        if self.loading.borrow().is_empty() && !self.done.borrow().is_empty() {
            self.clear_timer.call_once(5000)
        }
        entry
    }
    fn cancel_at(&self, scope: SessionScopeId, index: usize, is_admin_log: bool) {
        let entry = self.remove_at(scope, index);
        if let Some(cancel) = entry.external_cancel {
            cancel()
        } else if !is_admin_log {
            (self.effect)(Effect::Cancel(entry.object))
        }
    }
    pub fn remove(&self, item: MessageKey) {
        if let Some(i) = self.position(item) {
            self.remove_at(item.scope(), i);
        }
    }
    pub fn cancel(&self, item: MessageKey, is_admin_log: bool) {
        if let Some(i) = self.position(item) {
            self.cancel_at(item.scope(), i, is_admin_log)
        }
    }
    pub fn remove_loading_external(&self, item: MessageKey) {
        if self.loading_external_state(item).is_some() {
            self.remove(item)
        }
    }
    pub fn cancel_loading_external(&self, item: MessageKey) {
        if self.loading_external_state(item).is_some() {
            self.cancel(item, false)
        }
    }
    pub fn loading_in_progress(&self, scope: Option<SessionScopeId>) -> bool {
        self.loading_list()
            .iter()
            .any(|e| !e.done && scope.is_none_or(|s| s == e.object.item.scope()))
    }
    pub fn clear_loading(&self) {
        assert!(self.loading.borrow().is_empty());
        let scopes = self.sessions.borrow().keys().copied().collect::<Vec<_>>();
        for scope in scopes {
            while let Some((i, _)) = self.last_position(scope) {
                self.remove_at(scope, i);
            }
        }
    }
    pub fn clear_if_finished(&self) {
        if self.clear_timer.is_active() {
            self.clear_timer.cancel();
            self.clear_loading()
        }
    }
    pub fn add_loaded(&self, mut id: DownloadedId, document_size: i64) {
        if id.size <= 0 || id.size > MAX_FILE_SIZE {
            return;
        }
        let object = id.object.expect("resolved loaded object");
        let scope = object.item.scope();
        id.item_id = object.item.id();
        self.sessions
            .borrow_mut()
            .get_mut(&scope)
            .expect("untracked")
            .downloaded
            .push(id.clone());
        self.added.fire(id);
        (self.effect)(Effect::WritePostponed(scope));
        if let Some(document) = self
            .loading_list()
            .iter()
            .find(|e| e.object.item == object.item)
            .and_then(|e| e.object.document)
        {
            self.loading_documents.borrow_mut().remove(&document);
        }
        self.finish_progress(object.item, document_size);
    }
    fn finish_progress(&self, item: MessageKey, size: i64) {
        if !self.loading.borrow_mut().remove(&item) {
            return;
        }
        let delta = {
            let mut s = self.sessions.borrow_mut();
            let e = s
                .get_mut(&item.scope())
                .unwrap()
                .downloading
                .iter_mut()
                .find(|e| e.object.item == item)
                .unwrap();
            let delta = (size - e.ready, size - e.total);
            e.ready = size;
            e.total = size;
            e.done = true;
            delta
        };
        self.done.borrow_mut().insert(item);
        let p = self.progress.current();
        self.progress.set(DownloadProgress {
            ready: p.ready + delta.0,
            total: p.total + delta.1,
        });
        self.changes.fire(());
        if self.loading.borrow().is_empty() {
            self.clear_timer.call_once(5000)
        }
    }
    pub fn finish_loading_external(&self, item: MessageKey, id: DownloadedId) {
        let Some(state) = self.loading_external_state(item) else {
            return;
        };
        if !state.done {
            self.finish_progress(item, state.total);
            (self.effect)(Effect::Repaint(item));
        }
        self.add_loaded(id, state.total);
    }
    #[allow(clippy::too_many_arguments)]
    pub fn check_progress(
        &self,
        item: MessageKey,
        contains_media: bool,
        is_admin_log: bool,
        loading: bool,
        offset: i64,
        size: i64,
        completed: Option<DownloadedId>,
    ) {
        let entry = self
            .loading_list()
            .into_iter()
            .find(|d| d.object.item == item)
            .expect("not loading");
        if entry.external_cancel.is_some() {
            return;
        }
        if !contains_media {
            self.cancel(item, is_admin_log)
        } else if let Some(id) = completed {
            if self.loading.borrow().contains(&item) {
                self.add_loaded(id, size)
            }
        } else if !loading {
            self.remove(item)
        } else {
            let delta = {
                let mut s = self.sessions.borrow_mut();
                let e = s
                    .get_mut(&item.scope())
                    .unwrap()
                    .downloading
                    .iter_mut()
                    .find(|e| e.object.item == item)
                    .unwrap();
                let r = (offset - e.ready, size - e.total);
                e.ready = offset;
                e.total = size;
                r
            };
            if delta != (0, 0) {
                let p = self.progress.current();
                self.progress.set(DownloadProgress {
                    ready: p.ready + delta.0,
                    total: p.total + delta.1,
                });
            }
        }
    }
    pub fn delete_files(&self, items: &[MessageKey], is_admin_log: impl Fn(MessageKey) -> bool) {
        let mut files = BTreeMap::new();
        let mut sessions = BTreeSet::new();
        for &item in items {
            self.cancel(item, is_admin_log(item));
            let removed = {
                let mut all = self.sessions.borrow_mut();
                let Some(data) = all.get_mut(&item.scope()) else {
                    continue;
                };
                let mut removed = Vec::new();
                data.downloaded.retain(|id| {
                    if id.object.is_some_and(|o| o.item == item) {
                        removed.push(id.clone());
                        false
                    } else {
                        true
                    }
                });
                removed
            };
            if removed.is_empty() {
                continue;
            }
            for id in removed {
                files
                    .entry(id.path.clone().unwrap_or_default())
                    .or_insert(Effect::DeleteFile {
                        path: id.path_text(),
                        scope: item.scope(),
                        document: id.object.and_then(|o| o.document),
                        item_id: item.id(),
                    });
            }
            sessions.insert(item.scope());
            self.removed.fire(item);
        }
        for scope in sessions {
            (self.effect)(Effect::WritePostponed(scope));
        }
        for (_, effect) in files {
            (self.effect)(effect);
        }
    }
    pub fn untrack(&self, scope: SessionScopeId) {
        while let Some((i, _)) = self.last_position(scope) {
            self.remove_at(scope, i);
        }
        self.sessions.borrow_mut().remove(&scope);
    }
}
pub type Reply = Arc<dyn Fn(Result<Vec<i32>, tg_mtproto::mtproto_response::Error>) + Send + Sync>;
pub fn send_request<R: tg_scheme::tl::Function>(
    sender: &tg_mtproto::sender::Sender,
    id: i32,
    request: R,
    reply: Reply,
) where
    R::Response: tg_scheme::tl::Serialize + Send + 'static,
{
    let fail = reply.clone();
    sender
        .request(request)
        .override_id(id)
        .done(move |r| reply(Ok(tg_scheme::tl::serialize(&r))))
        .fail(move |e| fail(Err(e.clone())))
        .send();
}
#[derive(Clone)]
pub enum ResolveRequest {
    Channel(tg_scheme::ChannelsGetMessages),
    Messages(tg_scheme::MessagesGetMessages),
}
impl DownloadManager {
    /// Actual QFileInfo and message-store lookups are supplied at this owner boundary.
    pub fn resolve(
        &self,
        scope: SessionScopeId,
        self_peer: PeerId,
        mut file_size: impl FnMut(&DownloadedId) -> Option<i64>,
        mut message_loaded: impl FnMut(FullMsgId) -> bool,
    ) -> Vec<ResolveRequest> {
        let mut groups: BTreeMap<PeerId, (u64, Vec<tg_scheme::InputMessage>)> = BTreeMap::new();
        {
            let mut all = self.sessions.borrow_mut();
            let data = all.get_mut(&scope).expect("untracked");
            if data.resolve_sent_total >= data.resolve_needed || data.resolve_sent_total >= 100 {
                drop(all);
                self.check_full_resolve_done();
                return vec![];
            }
            let from = data.resolve_needed - data.resolve_sent_total;
            for i in (0..from).rev() {
                let id = &mut data.downloaded[i];
                if file_size(id) != Some(id.size) {
                    id.path = None
                } else if !message_loaded(id.item_id)
                    && super::data_msg_id::is_server_msg_id(id.item_id.msg)
                {
                    let peer = if peer_is_channel(id.item_id.peer) {
                        id.item_id.peer
                    } else {
                        self_peer
                    };
                    let g = groups.entry(peer).or_default();
                    if peer_is_channel(id.item_id.peer) && g.0 == 0 {
                        g.0 = id.peer_access_hash
                    }
                    g.1.push(tg_scheme::mtp_input_message_id(id.item_id.msg.bare as i32));
                }
                data.resolve_sent_total += 1;
                if data.resolve_sent_total >= 100 {
                    break;
                }
            }
            data.resolve_sent_requests += groups.len();
        }
        let requests = groups
            .into_iter()
            .map(|(p, (hash, ids))| {
                if peer_is_channel(p) {
                    ResolveRequest::Channel(tg_scheme::ChannelsGetMessages::new(
                        tg_scheme::mtp_input_channel(peer_to_bare_mtp_int(p), hash),
                        ids,
                    ))
                } else {
                    ResolveRequest::Messages(tg_scheme::MessagesGetMessages::new(ids))
                }
            })
            .collect::<Vec<_>>();
        self.check_full_resolve_done();
        requests
    }
    pub fn resolve_request_finished(&self, scope: SessionScopeId) -> bool {
        let mut s = self.sessions.borrow_mut();
        let d = s.get_mut(&scope).unwrap();
        assert!(d.resolve_sent_requests > 0);
        d.resolve_sent_requests -= 1;
        d.resolve_sent_requests == 0
    }
    pub fn resolve_requests_finished(
        &self,
        scope: SessionScopeId,
        mut lookup_matching_object: impl FnMut(&DownloadedId) -> Option<DownloadObject>,
    ) {
        loop {
            let next = {
                let mut s = self.sessions.borrow_mut();
                let d = s.get_mut(&scope).unwrap();
                if d.resolve_sent_requests != 0 || d.resolve_sent_total == 0 {
                    None
                } else {
                    d.resolve_needed -= 1;
                    d.resolve_sent_total -= 1;
                    let i = d.resolve_needed;
                    Some((i, d.downloaded[i].clone()))
                }
            };
            let Some((i, id)) = next else { break };
            if id.path.as_deref().unwrap_or_default().is_empty() {
                self.sessions
                    .borrow_mut()
                    .get_mut(&scope)
                    .unwrap()
                    .downloaded
                    .remove(i);
                continue;
            }
            if let Some(object) = lookup_matching_object(&id) {
                self.resolved_entry(scope, i, object)
            } else {
                (self.effect)(Effect::GenerateEntry { scope, index: i })
            }
        }
        (self.effect)(Effect::ResolveNext(scope));
        self.check_full_resolve_done();
    }
    pub fn resolved_entry(&self, scope: SessionScopeId, index: usize, object: DownloadObject) {
        assert_eq!(scope, object.item.scope());
        let value = {
            let mut s = self.sessions.borrow_mut();
            let id = &mut s.get_mut(&scope).unwrap().downloaded[index];
            id.object = Some(object);
            id.clone()
        };
        self.added.fire(value);
    }
    pub fn check_full_resolve_done(&self) {
        if self.resolve_done.current() {
            return;
        }
        if self
            .sessions
            .borrow()
            .values()
            .all(|d| d.resolve_sent_total >= d.resolve_needed && d.resolve_sent_requests == 0)
        {
            self.resolve_done.set(true)
        }
    }
    pub fn delete_all(&self, is_admin_log: impl Fn(MessageKey) -> bool) {
        let scopes = self.sessions.borrow().keys().copied().collect::<Vec<_>>();
        let mut sessions = BTreeSet::new();
        let mut files = BTreeMap::new();
        for scope in scopes {
            while let Some((i, item)) = self.last_position(scope) {
                self.cancel_at(scope, i, is_admin_log(item));
            }
            let downloaded = std::mem::take(
                &mut self
                    .sessions
                    .borrow_mut()
                    .get_mut(&scope)
                    .unwrap()
                    .downloaded,
            );
            if !downloaded.is_empty() {
                sessions.insert(scope);
            }
            for id in downloaded {
                files
                    .entry(id.path.clone().unwrap_or_default())
                    .or_insert(Effect::DeleteFile {
                        path: id.path_text(),
                        scope,
                        document: id.object.and_then(|o| o.document),
                        item_id: id.item_id,
                    });
                if let Some(object) = id.object {
                    self.removed.fire(object.item);
                }
            }
        }
        // Source writes once here and once again in finishFilesDelete.
        for _ in 0..2 {
            for &scope in &sessions {
                (self.effect)(Effect::WritePostponed(scope));
            }
        }
        for (_, effect) in files {
            (self.effect)(effect);
        }
    }
    pub fn loading_stop(
        &self,
        only: Option<SessionScopeId>,
        is_admin: impl Fn(MessageKey) -> bool,
    ) {
        let scopes = self
            .sessions
            .borrow()
            .keys()
            .copied()
            .filter(|s| only.is_none_or(|wanted| wanted == *s))
            .collect::<Vec<_>>();
        for scope in scopes {
            while let Some((i, item)) = self.last_position(scope) {
                self.cancel_at(scope, i, is_admin(item));
            }
        }
    }
    pub fn loaded_has_non_cloud_file(&self, is_history_entry: impl Fn(MessageKey) -> bool) -> bool {
        self.loaded_snapshot()
            .iter()
            .any(|id| id.object.is_some_and(|o| !is_history_entry(o.item)))
    }
    /// Each detached entry gets a real replacement key from the future History owner.
    pub fn detached(&self, scope: SessionScopeId, index: usize, replacement: MessageKey) {
        let (old, now, removed) = {
            let mut s = self.sessions.borrow_mut();
            let d = s.get_mut(&scope).unwrap();
            let o = d.downloaded[index].object.as_mut().unwrap();
            let old = o.item;
            assert_eq!(replacement.scope(), scope);
            o.item = replacement;
            let now = d.downloaded[index].clone();
            let removed = !d
                .downloaded
                .iter()
                .any(|id| id.object.is_some_and(|o| o.item == old));
            (old, now, removed)
        };
        if removed {
            self.removed.fire(old)
        }
        self.added.fire(now);
    }
    pub fn changed(&self, item: MessageKey, mut contains: impl FnMut(DownloadObject) -> bool) {
        let objects = self
            .loaded_snapshot()
            .into_iter()
            .filter_map(|id| id.object.filter(|o| o.item == item))
            .collect::<Vec<_>>();
        for object in objects {
            if !contains(object) {
                (self.effect)(Effect::Regenerate(object))
            }
        }
    }
    pub fn removed(&self, item: MessageKey, is_admin_log: bool) {
        self.changed(item, |_| false);
        if self.loading_list().iter().any(|id| id.object.item == item) {
            self.cancel(item, is_admin_log)
        }
    }
}
/// The original IV marker and directory whitelist, checked without deleting anything.
pub fn rich_export_folder_to_remove(file: &std::path::Path) -> Option<std::path::PathBuf> {
    use std::io::Read;
    if !file.extension()?.to_str()?.eq_ignore_ascii_case("html") {
        return None;
    }
    let full = if file.is_absolute() {
        file.to_owned()
    } else {
        std::env::current_dir().ok()?.join(file)
    };
    let folder = full.parent()?;
    if folder.file_name() != full.file_stem() {
        return None;
    }
    let mut head = vec![];
    std::fs::File::open(&full)
        .ok()?
        .take(2048)
        .read_to_end(&mut head)
        .ok()?;
    if !String::from_utf8_lossy(&head).contains("Telegram Desktop rich message export") {
        return None;
    }
    for e in std::fs::read_dir(folder).ok()? {
        let name = e.ok()?.file_name();
        if name != full.file_name()? && name != "media" {
            return None;
        }
    }
    Some(folder.to_owned())
}
pub fn delete_download_file(file: &std::path::Path) -> std::io::Result<()> {
    if let Some(folder) = rich_export_folder_to_remove(file) {
        std::fs::remove_dir_all(folder)
    } else {
        std::fs::remove_file(file)
    }
}
impl DownloadManager {
    pub fn check_file_loader(
        &self,
        item: MessageKey,
        contains_media: bool,
        is_admin_log: bool,
        loader: &crate::storage::file_download::FileLoader,
        completed: Option<DownloadedId>,
    ) {
        assert_eq!(item.scope(), loader.session());
        self.check_progress(
            item,
            contains_media,
            is_admin_log,
            !loader.finished() && !loader.cancelled(),
            loader.current_offset(),
            loader.full_size(),
            completed,
        )
    }
    pub fn check_document(&self, document: DocumentRecordKey, check: impl FnOnce(MessageKey)) {
        if !self.loading_documents.borrow().contains(&document) {
            return;
        }
        if let Some(item) = self
            .loading_list()
            .iter()
            .find(|d| d.object.document == Some(document))
            .map(|d| d.object.item)
        {
            check(item)
        }
    }
}
pub fn peer_access_hash(peers: &super::data_session::Peers, peer: PeerKey) -> u64 {
    if let Ok(k) = UserKey::try_from(peer) {
        peers.read_user(k, |u| u.access_hash())
    } else if let Ok(k) = ChannelKey::try_from(peer) {
        peers.read_channel(k, |c| c.access_hash())
    } else {
        0
    }
}
