//! Actual search requests, result mutation phases and cached slice controllers.
//! HistoryItem/Histories scheduling remains a required key-based owner18–20/26.
use super::{
    data_messages::{LoadDirection, MessagePosition},
    data_msg_id::*,
    data_peer_id::*,
    data_session::Peers,
    data_sparse_ids::{
        self, SparseIdsAroundData, SparseIdsMergedSlice, SparseIdsSlice, SparseIdsSliceBuilder,
    },
    data_store_keys::*,
};
use crate::storage::{
    storage_shared_media::{SharedMediaType, SharedMediaTypesMask},
    storage_sparse_ids_list::{MsgRange, SparseIdsList, SparseIdsListQuery},
};
use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
    collections::BTreeMap,
    rc::{Rc, Weak},
    sync::Arc,
};
use tg_base::timer::{Timer, TimerHost};
use tg_rpl::{EventStream, Lifetime, Producer};
use tg_scheme as tl;
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchResult {
    pub message_ids: Vec<MsgId>,
    pub no_skip_range: MsgRange,
    pub full_count: i32,
}
pub type HistoryResult = SearchResult;
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GlobalMediaResult {
    pub message_ids: Vec<MessagePosition>,
    pub offset_position: MessagePosition,
    pub offset_rate: i32,
    pub full_count: i32,
    pub filtered_count: i32,
}
pub fn prepare_search_filter(kind: Option<SharedMediaType>) -> tl::MessagesFilter {
    use SharedMediaType::*;
    match kind {
        Some(Photo) => tl::mtp_input_messages_filter_photos(),
        Some(Video) => tl::mtp_input_messages_filter_video(),
        Some(PhotoVideo) => tl::mtp_input_messages_filter_photo_video(),
        Some(MusicFile) => tl::mtp_input_messages_filter_music(),
        Some(File) => tl::mtp_input_messages_filter_document(),
        Some(VoiceFile) => tl::mtp_input_messages_filter_voice(),
        Some(RoundVoiceFile) => tl::mtp_input_messages_filter_round_voice(),
        Some(RoundFile) => tl::mtp_input_messages_filter_round_video(),
        Some(GIF) => tl::mtp_input_messages_filter_gif(),
        Some(Link) => tl::mtp_input_messages_filter_url(),
        Some(ChatPhoto) => tl::mtp_input_messages_filter_chat_photos(),
        Some(Pinned) => tl::mtp_input_messages_filter_pinned(),
        Some(Poll) => tl::mtp_input_messages_filter_poll(),
        None => tl::mtp_input_messages_filter_empty(),
    }
}
/// Actual source calls only; no replacement HistoryItem/History model.
pub trait SearchOwner {
    fn input_peer(&self, peer: PeerKey) -> tl::InputPeer;
    fn process_users(&self, users: &[tl::User]);
    fn process_chats(&self, chats: &[tl::Chat]);
    fn process_topics(&self, peer: PeerKey, topics: &[tl::ForumTopic]);
    fn channel_pts_received(&self, channel: ChannelKey, pts: i32);
    fn add_existing_message(&self, message: &tl::Message) -> Option<MessageKey>;
    fn message_position(&self, message: MessageKey) -> MessagePosition;
    fn shared_media_types(&self, message: MessageKey) -> SharedMediaTypesMask;
    fn allows_forward(&self, message: MessageKey) -> bool;
    fn topic_root_id(&self, message: MessageKey) -> MsgId;
    fn sublist_peer_id(&self, message: MessageKey) -> PeerId;
    /// Destroyed message key is still LIVE during this event, as Changes contract.
    fn item_removed(&self) -> Producer<MessageKey>;
    fn history_cleared(&self) -> Producer<PeerKey>;
    /// Histories::RequestType::History scheduling, not a fake immediate server.
    fn send_history_request(&self, peer: PeerKey, request: tl::MessagesSearch) -> i32;
    fn cancel_history_request(&self, id: i32);
    fn finish_history_request(&self, id: i32);
}
fn offsets(id: MsgId, dir: LoadDirection, limit: i32) -> (i32, i32) {
    let n = id.bare + if dir == LoadDirection::After { 1 } else { 0 };
    (
        n.clamp(0, 0x3fffffff) as i32,
        match dir {
            LoadDirection::Before => 0,
            LoadDirection::Around => -limit / 2,
            LoadDirection::After => -limit,
        },
    )
}
#[allow(clippy::too_many_arguments)]
pub fn prepare_search_request(
    owner: &dyn SearchOwner,
    peers: &Peers,
    peer: PeerKey,
    topic: MsgId,
    monoforum: PeerId,
    kind: Option<SharedMediaType>,
    query: &str,
    id: MsgId,
    dir: LoadDirection,
) -> Option<tl::MessagesSearch> {
    if query.is_empty() && kind.is_none() {
        return None;
    }
    let limit = if id.is_zero() { 0 } else { 100 };
    let (offset_id, add_offset) = offsets(id, dir, limit);
    Some(tl::MessagesSearch {
        flags: tl::tl::Flags::from_raw(
            (if topic.is_zero() { 0 } else { 2 }) | (if monoforum.is_zero() { 0 } else { 4 }),
        ),
        peer: owner.input_peer(peer),
        q: query.as_bytes().to_vec(),
        from_id: None,
        saved_peer_id: (!monoforum.is_zero())
            .then(|| owner.input_peer(peers.ensure_peer(monoforum))),
        saved_reaction: None,
        top_msg_id: (!topic.is_zero()).then_some(topic.bare as i32),
        filter: prepare_search_filter(kind),
        min_date: 0,
        max_date: 0,
        offset_id,
        add_offset,
        limit,
        max_id: 0,
        min_id: 0,
        hash: 0,
    })
}
pub fn prepare_history_request(
    owner: &dyn SearchOwner,
    peer: PeerKey,
    id: MsgId,
    dir: LoadDirection,
) -> tl::MessagesGetHistory {
    let (offset_id, add_offset) = offsets(id, dir, 50);
    tl::MessagesGetHistory {
        peer: owner.input_peer(peer),
        offset_id,
        offset_date: 0,
        add_offset,
        limit: 50,
        max_id: 0,
        min_id: 0,
        hash: 0,
    }
}
pub fn prepare_global_media_request(
    owner: &dyn SearchOwner,
    peers: &Peers,
    rate: i32,
    position: MessagePosition,
    kind: Option<SharedMediaType>,
    query: &str,
) -> Option<tl::MessagesSearchGlobal> {
    if query.is_empty() && kind.is_none() {
        return None;
    }
    Some(tl::MessagesSearchGlobal {
        flags: tl::tl::Flags::from_raw(1),
        folder_id: Some(0),
        community: None,
        q: query.as_bytes().to_vec(),
        filter: prepare_search_filter(kind),
        min_date: 0,
        max_date: 0,
        offset_rate: rate,
        offset_peer: if position.full_id.peer.is_zero() {
            tl::mtp_input_peer_empty()
        } else {
            owner.input_peer(peers.ensure_peer(position.full_id.peer))
        },
        offset_id: position.full_id.msg.bare as i32,
        limit: if position.full_id.peer.is_zero() {
            0
        } else {
            100
        },
    })
}
struct Batch<'a> {
    messages: &'a [tl::Message],
    topics: &'a [tl::ForumTopic],
    pts: Option<i32>,
    full_count: i32,
    rate: i32,
}
fn batch<'a>(owner: &dyn SearchOwner, data: &'a tl::MessagesMessages) -> Option<Batch<'a>> {
    match data {
        tl::MessagesMessages::MessagesMessagesNotModified(_) => None,
        tl::MessagesMessages::MessagesMessages(d) => {
            owner.process_users(&d.users);
            owner.process_chats(&d.chats);
            Some(Batch {
                messages: &d.messages,
                topics: &d.topics,
                pts: None,
                full_count: d.messages.len() as i32,
                rate: 0,
            })
        }
        tl::MessagesMessages::MessagesMessagesSlice(d) => {
            owner.process_users(&d.users);
            owner.process_chats(&d.chats);
            Some(Batch {
                messages: &d.messages,
                topics: &d.topics,
                pts: None,
                full_count: d.count,
                rate: d.next_rate.unwrap_or(0),
            })
        }
        tl::MessagesMessages::MessagesChannelMessages(d) => {
            owner.process_users(&d.users);
            owner.process_chats(&d.chats);
            Some(Batch {
                messages: &d.messages,
                topics: &d.topics,
                pts: Some(d.pts),
                full_count: d.count,
                rate: 0,
            })
        }
    }
}
pub fn parse_search_result(
    owner: &dyn SearchOwner,
    peer: PeerKey,
    kind: Option<SharedMediaType>,
    id: MsgId,
    dir: LoadDirection,
    data: &tl::MessagesMessages,
) -> SearchResult {
    let mut result = SearchResult {
        no_skip_range: MsgRange {
            from: id.bare,
            till: id.bare,
        },
        ..Default::default()
    };
    let Some(batch) = batch(owner, data) else {
        return result;
    };
    if let (Some(pts), Ok(channel)) = (batch.pts, ChannelKey::try_from(peer)) {
        owner.channel_pts_received(channel, pts);
    }
    owner.process_topics(peer, batch.topics);
    result.full_count = batch.full_count;
    for message in batch.messages {
        if let Some(key) = owner.add_existing_message(message) {
            assert_eq!(key.scope(), peer.scope());
            let item_id = key.id().msg;
            if kind.is_none_or(|t| owner.shared_media_types(key).test(t)) {
                result.message_ids.push(item_id);
            }
            result.no_skip_range.from = result.no_skip_range.from.min(item_id.bare);
            result.no_skip_range.till = result.no_skip_range.till.max(item_id.bare);
        }
    }
    if !id.is_zero() && result.message_ids.is_empty() {
        result.no_skip_range = match dir {
            LoadDirection::Before => MsgRange {
                from: 0,
                till: result.no_skip_range.till,
            },
            LoadDirection::Around => MsgRange {
                from: 0,
                till: SERVER_MAX_MSG_ID.bare,
            },
            LoadDirection::After => MsgRange {
                from: result.no_skip_range.from,
                till: SERVER_MAX_MSG_ID.bare,
            },
        };
    }
    result
}
pub fn parse_history_result(
    owner: &dyn SearchOwner,
    peer: PeerKey,
    id: MsgId,
    dir: LoadDirection,
    data: &tl::MessagesMessages,
) -> HistoryResult {
    parse_search_result(owner, peer, None, id, dir, data)
}
pub fn parse_global_media_result(
    owner: &dyn SearchOwner,
    data: &tl::MessagesMessages,
    only_forwardable: bool,
) -> GlobalMediaResult {
    // Original dereferences null messages for messagesNotModified: diagnose it.
    let batch = batch(owner, data).expect("messagesNotModified in ParseGlobalMediaResult");
    let mut result = GlobalMediaResult {
        full_count: batch.full_count,
        offset_rate: batch.rate,
        ..Default::default()
    };
    for message in batch.messages {
        if let Some(key) = owner.add_existing_message(message) {
            result.offset_position = owner.message_position(key);
            if only_forwardable && !owner.allows_forward(key) {
                result.filtered_count += 1;
            } else {
                result.message_ids.push(owner.message_position(key));
            }
        }
    }
    result
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    pub peer_id: PeerId,
    pub topic_root_id: MsgId,
    pub monoforum_peer_id: PeerId,
    pub migrated_peer_id: PeerId,
    pub type_: Option<SharedMediaType>,
    pub query: String,
}
impl Ord for Query {
    fn cmp(&self, b: &Self) -> Ordering {
        (
            self.peer_id,
            self.topic_root_id,
            self.monoforum_peer_id,
            self.migrated_peer_id,
            self.type_.map_or(13, |t| t as u8),
        )
            .cmp(&(
                b.peer_id,
                b.topic_root_id,
                b.monoforum_peer_id,
                b.migrated_peer_id,
                b.type_.map_or(13, |t| t as u8),
            ))
            .then_with(|| self.query.encode_utf16().cmp(b.query.encode_utf16()))
    }
}
impl PartialOrd for Query {
    fn partial_cmp(&self, b: &Self) -> Option<Ordering> {
        Some(self.cmp(b))
    }
}
#[derive(Default)]
pub struct SavedState {
    pub query: Query,
    pub peer_list: Rc<SparseIdsList>,
    pub migrated_list: Option<Rc<SparseIdsList>>,
}
struct Data {
    active: Cell<bool>,
    peer: PeerKey,
    list: RefCell<Rc<SparseIdsList>>,
    requests: RefCell<BTreeMap<SparseIdsAroundData, i32>>,
    owner: Rc<dyn SearchOwner>,
}
impl Data {
    fn new(peer: PeerKey, owner: Rc<dyn SearchOwner>) -> Rc<Self> {
        Rc::new(Self {
            active: Cell::new(true),
            peer,
            list: RefCell::new(Rc::default()),
            requests: RefCell::default(),
            owner,
        })
    }
    fn cancel(&self) {
        self.active.set(false);
        let ids = std::mem::take(&mut *self.requests.borrow_mut());
        for (_, id) in ids {
            self.owner.cancel_history_request(id);
        }
    }
}
impl Drop for Data {
    fn drop(&mut self) {
        self.cancel();
    }
}
struct CacheEntry {
    peer: Rc<Data>,
    migrated: Option<Rc<Data>>,
}
impl CacheEntry {
    fn cancel(&self) {
        self.peer.cancel();
        if let Some(m) = &self.migrated {
            m.cancel();
        }
    }
}
pub struct SearchController {
    peers: Rc<Peers>,
    owner: Rc<dyn SearchOwner>,
    cache: RefCell<BTreeMap<Query, CacheEntry>>,
    current: RefCell<Option<Query>>,
}
impl SearchController {
    pub fn new(peers: Rc<Peers>, owner: Rc<dyn SearchOwner>) -> Rc<Self> {
        Rc::new(Self {
            peers,
            owner,
            cache: RefCell::default(),
            current: RefCell::default(),
        })
    }
    fn entry(&self, q: &Query) -> CacheEntry {
        CacheEntry {
            peer: Data::new(self.peers.ensure_peer(q.peer_id), self.owner.clone()),
            migrated: (!q.migrated_peer_id.is_zero()).then(|| {
                Data::new(
                    self.peers.ensure_peer(q.migrated_peer_id),
                    self.owner.clone(),
                )
            }),
        }
    }
    pub fn has_in_cache(&self, q: &Query) -> bool {
        q.query.is_empty() || self.cache.borrow().contains_key(q)
    }
    pub fn set_query(&self, q: Query) {
        if q.query.is_empty() {
            let old = std::mem::take(&mut *self.cache.borrow_mut());
            for entry in old.values() {
                entry.cancel();
            }
            drop(old);
        }
        let missing = !self.cache.borrow().contains_key(&q);
        if missing {
            let entry = self.entry(&q);
            self.cache.borrow_mut().insert(q.clone(), entry);
        }
        *self.current.borrow_mut() = Some(q);
    }
    pub fn query(&self) -> Query {
        self.current
            .borrow()
            .clone()
            .expect("setQuery before query")
    }
    pub fn ids_slice(
        self: &Rc<Self>,
        around: MsgId,
        before: i32,
        after: i32,
    ) -> Producer<SparseIdsMergedSlice> {
        let q = self.query();
        let key = data_sparse_ids::Key::new(
            q.peer_id,
            q.topic_root_id,
            q.monoforum_peer_id,
            q.migrated_peer_id,
            around,
        );
        let weak = Rc::downgrade(self);
        SparseIdsMergedSlice::create_viewer(
            key,
            before,
            after,
            Rc::new(move |peer, topic, mono, id, b, a| {
                weak.upgrade().map_or_else(tg_rpl::never, |s| {
                    s.simple_ids_slice(peer, topic, mono, id, &q, b, a)
                })
            }),
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn simple_ids_slice(
        self: &Rc<Self>,
        peer: PeerId,
        topic: MsgId,
        mono: PeerId,
        around: MsgId,
        q: &Query,
        before: i32,
        after: i32,
    ) -> Producer<SparseIdsSlice> {
        assert!(!peer.is_zero());
        assert!(is_server_msg_id(around) || around.is_zero());
        assert!(!around.is_zero() || (before == 0 && after == 0));
        assert!(
            q.peer_id == peer && q.topic_root_id == topic && q.monoforum_peer_id == mono
                || q.migrated_peer_id == peer && topic.is_zero() && mono.is_zero()
        );
        let data = {
            let cache = self.cache.borrow();
            let Some(entry) = cache.get(q) else {
                return tg_rpl::never();
            };
            if peer == q.peer_id {
                entry.peer.clone()
            } else {
                entry.migrated.clone().expect("migrated data")
            }
        };
        let weak = Rc::downgrade(self);
        let q = q.clone();
        let owner = self.owner.clone();
        Producer::new(move |consumer| {
            let life = Lifetime::new();
            let builder = Rc::new(SparseIdsSliceBuilder::new(around, before, after));
            let b = builder.clone();
            let c = consumer.clone();
            let push: Rc<dyn Fn()> = Rc::new(move || {
                c.put_next(b.snapshot());
            });
            let w = weak.clone();
            let query = q.clone();
            let d = data.clone();
            builder.insufficient_around().start(
                move |key| {
                    if let Some(s) = w.upgrade() {
                        s.request_more(key, &query, &d);
                    }
                },
                |_| {},
                || {},
                &life,
            );
            let list = data.list.borrow().clone();
            let b = builder.clone();
            let p = push.clone();
            list.slice_updated().start(
                move |u| {
                    if b.apply_update(&u) {
                        p();
                    }
                },
                |_| {},
                || {},
                &life,
            );
            let b = builder.clone();
            let p = push.clone();
            let o = owner.clone();
            owner.item_removed().start(
                move |key| {
                    if key.id().peer == peer
                        && (topic.is_zero() || o.topic_root_id(key) == topic)
                        && (mono.is_zero() || o.sublist_peer_id(key) == mono)
                        && b.remove_one(key.id().msg)
                    {
                        p();
                    }
                },
                |_| {},
                || {},
                &life,
            );
            let b = builder.clone();
            let p = push.clone();
            owner.history_cleared().start(
                move |key| {
                    if key.id() == peer && b.remove_all() {
                        p();
                    }
                },
                |_| {},
                || {},
                &life,
            );
            let b = builder.clone();
            let p = push.clone();
            list.query(SparseIdsListQuery {
                around_id: around.bare,
                limit_before: before,
                limit_after: after,
            })
            .start(
                move |r| {
                    if b.apply_initial(&r) {
                        p();
                    }
                },
                |_| {},
                move || builder.check_insufficient(),
                &life,
            );
            life
        })
    }
    fn request_more(&self, key: SparseIdsAroundData, q: &Query, data: &Rc<Data>) {
        assert!(
            data.active.get(),
            "viewer must end before cached Data destruction"
        );
        if data.requests.borrow().contains_key(&key) {
            return;
        }
        let Some(request) = prepare_search_request(
            &*self.owner,
            &self.peers,
            data.peer,
            q.topic_root_id,
            q.monoforum_peer_id,
            q.type_,
            &q.query,
            key.around_id,
            key.direction,
        ) else {
            return;
        };
        let id = self.owner.send_history_request(data.peer, request);
        data.requests.borrow_mut().insert(key, id);
    }
    pub fn handle_reply(&self, id: i32, result: Result<tl::MessagesMessages, ()>) {
        let found = {
            let cache = self.cache.borrow();
            cache.iter().find_map(|(q, e)| {
                std::iter::once(&e.peer)
                    .chain(e.migrated.as_ref())
                    .find_map(|d| {
                        let key = d
                            .requests
                            .borrow()
                            .iter()
                            .find_map(|(k, v)| (*v == id).then_some(*k));
                        key.map(|k| (q.clone(), d.clone(), k))
                    })
            })
        };
        let Some((q, data, key)) = found else {
            return;
        };
        if let Ok(result) = result {
            data.requests.borrow_mut().remove(&key);
            self.owner.cancel_history_request(id);
            let parsed = parse_search_result(
                &*self.owner,
                data.peer,
                q.type_,
                key.around_id,
                key.direction,
                &result,
            );
            let list = data.list.borrow().clone();
            list.add_slice(
                parsed.message_ids.into_iter().map(|id| id.bare).collect(),
                parsed.no_skip_range,
                Some(parsed.full_count),
            );
        }
        // Failure deliberately retains the request key/lifetime, blocking retries.
        self.owner.finish_history_request(id);
    }
    pub fn save_state(&self) -> SavedState {
        let Some(q) = self.current.borrow().clone() else {
            return SavedState::default();
        };
        let (peer, migrated) = {
            let cache = self.cache.borrow();
            let e = &cache[&q];
            (e.peer.clone(), e.migrated.clone())
        };
        let peer_list = std::mem::take(&mut *peer.list.borrow_mut());
        let migrated_list = migrated.map(|d| std::mem::take(&mut *d.list.borrow_mut()));
        SavedState {
            query: q,
            peer_list,
            migrated_list,
        }
    }
    pub fn restore_state(&self, state: SavedState) {
        if state.query.peer_id.is_zero() {
            return;
        }
        let q = state.query;
        let missing = !self.cache.borrow().contains_key(&q);
        if missing {
            let entry = self.entry(&q);
            self.cache.borrow_mut().insert(q.clone(), entry);
        }
        let (old_peer, old_migrated) = {
            let mut cache = self.cache.borrow_mut();
            let e = cache.get_mut(&q).unwrap();
            let replacement = Data::new(e.peer.peer, self.owner.clone());
            *replacement.list.borrow_mut() = state.peer_list;
            let old = std::mem::replace(&mut e.peer, replacement);
            let migrated = if let Some(list) = state.migrated_list {
                let m = e
                    .migrated
                    .as_mut()
                    .expect("migrated state without migrated peer");
                let replacement = Data::new(m.peer, self.owner.clone());
                *replacement.list.borrow_mut() = list;
                Some(std::mem::replace(m, replacement))
            } else {
                None
            };
            (old, migrated)
        };
        old_peer.cancel();
        if let Some(m) = old_migrated {
            m.cancel();
        }
        *self.current.borrow_mut() = Some(q);
    }
}
impl Drop for SearchController {
    fn drop(&mut self) {
        let old = std::mem::take(self.cache.get_mut());
        for entry in old.values() {
            entry.cancel();
        }
    }
}
pub struct DelayedSearchController {
    controller: Rc<SearchController>,
    next: RefCell<Query>,
    timer: Timer,
    changes: EventStream<String>,
}
impl DelayedSearchController {
    pub fn new(controller: Rc<SearchController>, host: Rc<TimerHost>) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let w = weak.clone();
            Self {
                controller,
                next: RefCell::default(),
                changes: EventStream::new(),
                timer: Timer::in_host(
                    &host,
                    Some(Rc::new(move || {
                        if let Some(s) = w.upgrade() {
                            let q = s.next.borrow().clone();
                            s.set_query_fast(q);
                        }
                    })),
                ),
            }
        })
    }
    pub fn current_query(&self) -> Query {
        self.controller.query()
    }
    pub fn set_query(&self, q: Query) {
        self.set_query_delayed(q, 200);
    }
    pub fn set_query_delayed(&self, q: Query, delay: i64) {
        if self.current_query() == q {
            self.timer.cancel();
            return;
        }
        if self.controller.has_in_cache(&q) {
            self.set_query_fast(q);
        } else {
            *self.next.borrow_mut() = q;
            self.timer.call_once(delay);
        }
    }
    pub fn set_query_fast(&self, q: Query) {
        self.controller.set_query(q.clone());
        self.changes.fire(q.query);
    }
    pub fn current_query_value(&self) -> Producer<String> {
        self.changes
            .events_starting_with(self.current_query().query)
    }
    pub fn ids_slice(
        &self,
        around: MsgId,
        before: i32,
        after: i32,
    ) -> Producer<SparseIdsMergedSlice> {
        self.controller.ids_slice(around, before, after)
    }
    pub fn save_state(&self) -> SavedState {
        self.controller.save_state()
    }
    pub fn restore_state(&self, s: SavedState) {
        self.controller.restore_state(s);
    }
}
/// API half of Histories::sendRequest; actual scheduling/finish/cancel owner18–20.
pub fn send_search_request(
    sender: &tg_mtproto::sender::Sender,
    id: i32,
    request: tl::MessagesSearch,
    reply: Arc<dyn Fn(Result<tl::MessagesMessages, ()>) + Send + Sync>,
) {
    let fail = reply.clone();
    sender
        .request(request)
        .override_id(id)
        .done(move |r| reply(Ok(r)))
        .fail(move |_| fail(Err(())))
        .send();
}
