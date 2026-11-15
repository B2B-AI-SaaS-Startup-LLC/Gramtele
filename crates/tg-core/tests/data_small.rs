use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::{Rc, Weak},
    sync::{Arc, Mutex},
};
use tg_base::{
    qt_stream::{Reader, Writer},
    timer::{ManualClock, TimerHost},
};
use tg_core::{
    data::{
        components::{passkeys as p, recent_peers as recent, top_peers::PeerCodec},
        data_changes::Changes,
        data_file_origin::FileOrigin,
        data_messages::LoadDirection,
        data_msg_id::*,
        data_passkey_deserialize as format,
        data_peer_id::*,
        data_search_controller as search,
        data_session::Peers,
        data_store_keys::*,
        data_streaming as stream,
    },
    platform::platform_webauthn::RegisterResult,
    storage::storage_shared_media::SharedMediaType as Type,
};
use tg_rpl::{Lifetime, Producer};
use tg_runtime::MainQueue;
use tg_scheme as tl;
#[path = "support/data_small_owner.rs"]
mod search_owner;
fn collect<T: Clone + 'static>(p: Producer<T>, life: &Lifetime) -> Rc<RefCell<Vec<T>>> {
    let v = Rc::new(RefCell::new(vec![]));
    let out = v.clone();
    p.start(move |x| out.borrow_mut().push(x), |_| {}, || {}, life);
    v
}
fn clock() -> (ManualClock, Rc<TimerHost>) {
    let c = ManualClock::new(10000);
    let h = TimerHost::new(Arc::new(c.clone()));
    (c, h)
}
#[derive(Default)]
struct PasskeyOwner {
    requests: RefCell<Vec<(i32, p::Request)>>,
    log: RefCell<Vec<&'static str>>,
    supported: Cell<bool>,
    max: Cell<i32>,
    display: Cell<bool>,
}
impl p::PasskeysOwner for PasskeyOwner {
    fn webauthn_supported(&self) -> bool {
        self.log.borrow_mut().push("support");
        self.supported.get()
    }
    fn account_passkeys_max(&self) -> i32 {
        self.log.borrow_mut().push("max");
        self.max.get()
    }
    fn settings_display_passkeys(&self) -> bool {
        self.display.get()
    }
    fn send(&self, id: i32, r: p::Request) {
        self.requests.borrow_mut().push((id, r));
    }
}
fn passkey() -> tl::Passkey {
    tl::mtp_passkey(
        tl::tl::Flags::from_raw(0),
        b"id".to_vec(),
        b"device".to_vec(),
        4,
        None,
        None,
    )
}
fn passkeys(list: Vec<tl::Passkey>) -> tl::AccountPasskeys {
    tl::mtp_account_passkeys(list)
}
fn registration(json: &[u8]) -> tl::AccountPasskeyRegistrationOptions {
    tl::mtp_account_passkey_registration_options(tl::mtp_data_json(json.to_vec()))
}
#[test]
fn passkeys_registration_cache_boundary_concurrent_requests_invalid_json_and_fail() {
    let (c, h) = clock();
    let owner = Rc::new(PasskeyOwner::default());
    let model = Rc::new(p::Passkeys::new(h, owner.clone()));
    let done = Rc::new(Cell::new(0));
    let n = done.clone();
    let weak = Rc::downgrade(&model);
    let callback = Rc::new(move |_: &format::RegisterData| {
        assert!(weak.upgrade().unwrap().list().is_empty());
        n.set(n.get() + 1);
    });
    model.init_registration(callback.clone());
    model.init_registration(callback.clone());
    assert_eq!(owner.requests.borrow().len(), 2);
    let id = owner.requests.borrow()[0].0;
    model.handle_reply(
        id,
        p::Reply::Init(registration(br#"{"publicKey":{"timeout":10}}"#)),
    );
    assert_eq!(done.get(), 1);
    model.init_registration(callback.clone());
    assert_eq!(done.get(), 2);
    assert_eq!(owner.requests.borrow().len(), 2);
    c.advance(10);
    model.init_registration(callback.clone());
    assert_eq!(owner.requests.borrow().len(), 3);
    let id = owner.requests.borrow()[2].0;
    model.handle_reply(id, p::Reply::Init(registration(b"{}")));
    assert_eq!(done.get(), 2);
    model.init_registration(callback);
    let id = owner.requests.borrow().last().unwrap().0;
    model.handle_reply(id, p::Reply::Failed("bad".into()));
    assert_eq!(done.get(), 2);
}
#[test]
fn passkeys_list_eager_call_lazy_stream_timeout_failure_registration_order_and_delete() {
    let (c, h) = clock();
    let owner = Rc::new(PasskeyOwner::default());
    owner.supported.set(true);
    owner.max.set(1);
    owner.display.set(true);
    let m = Rc::new(p::Passkeys::new(h, owner.clone()));
    let life = Lifetime::new();
    let first = m.request_list();
    assert_eq!(owner.requests.borrow().len(), 1);
    let calls = collect(first, &life);
    assert!(calls.borrow().is_empty());
    let immediate = collect(m.request_list(), &life);
    assert_eq!(immediate.borrow().len(), 1);
    let id = owner.requests.borrow()[0].0;
    m.handle_reply(id, p::Reply::Failed("NO".into()));
    assert!(!m.list_known());
    c.advance(5000);
    let _r = m.request_list();
    assert_eq!(owner.requests.borrow().len(), 1);
    c.advance(1);
    let _r = m.request_list();
    let id = owner.requests.borrow().last().unwrap().0;
    m.handle_reply(id, p::Reply::List(passkeys(vec![passkey()])));
    assert!(m.list_known());
    assert!(!m.can_register());
    assert!(m.possible());
    owner.max.set(-1);
    assert!(m.can_register()); // source size_t/int comparison
    let observed = Rc::new(RefCell::new(vec![]));
    let v = observed.clone();
    let weak = Rc::downgrade(&m);
    m.request_list().start(
        move |_| {
            v.borrow_mut().push(weak.upgrade().unwrap().list().len());
        },
        |_| {},
        || {},
        &life,
    );
    let v = observed.clone();
    m.register_passkey(
        &RegisterResult::default(),
        Rc::new(move || v.borrow_mut().push(99)),
    );
    let id = owner.requests.borrow().last().unwrap().0;
    m.handle_reply(id, p::Reply::Register(passkey()));
    assert_eq!(&observed.borrow()[observed.borrow().len() - 2..], &[2, 99]);
    let done = Rc::new(Cell::new(false));
    let d = done.clone();
    let weak = Rc::downgrade(&m);
    m.delete_passkey(
        "id",
        Rc::new(move || {
            assert!(!weak.upgrade().unwrap().list_known());
            d.set(true);
        }),
        Rc::new(|_| panic!()),
    );
    let id = owner.requests.borrow().last().unwrap().0;
    m.handle_reply(id, p::Reply::Deleted);
    assert!(done.get());
    assert!(matches!(
        owner.requests.borrow().last().unwrap().1,
        p::Request::List(_)
    ));
    assert_eq!(m.list().len(), 2);
}
#[test]
fn passkeys_register_failure_drops_cache_delete_failure_and_live_config() {
    let (_, h) = clock();
    let o = Rc::new(PasskeyOwner::default());
    let m = p::Passkeys::new(h, o.clone());
    m.init_registration(Rc::new(|_| {}));
    let id = o.requests.borrow()[0].0;
    m.handle_reply(
        id,
        p::Reply::Init(registration(br#"{"publicKey":{"a":1}}"#)),
    );
    m.register_passkey(&RegisterResult::default(), Rc::new(|| panic!()));
    let id = o.requests.borrow().last().unwrap().0;
    m.handle_reply(id, p::Reply::Failed("REGISTER".into()));
    let before = o.requests.borrow().len();
    m.init_registration(Rc::new(|_| {}));
    assert_eq!(o.requests.borrow().len(), before + 1);
    let error = Rc::new(RefCell::new(String::new()));
    let e = error.clone();
    m.delete_passkey(
        "id",
        Rc::new(|| panic!()),
        Rc::new(move |v| *e.borrow_mut() = v),
    );
    let id = o.requests.borrow().last().unwrap().0;
    m.handle_reply(id, p::Reply::Failed("DELETE".into()));
    assert_eq!(*error.borrow(), "DELETE");
    o.log.borrow_mut().clear();
    assert!(!m.can_register());
    assert_eq!(*o.log.borrow(), ["max", "support"]);
    o.supported.set(true);
    o.max.set(2);
    assert!(m.can_register());
    assert!(!m.possible());
    o.display.set(true);
    assert!(m.possible());
}
#[derive(Default)]
struct RecentOwner {
    reads: Cell<usize>,
    writes: Cell<usize>,
    threads: RefCell<BTreeMap<recent::ThreadKey, Rc<()>>>,
}
impl recent::RecentPeersOwner for RecentOwner {
    fn read_search_suggestions(&self) {
        self.reads.set(self.reads.get() + 1);
    }
    fn write_search_suggestions_delayed(&self) {
        self.writes.set(self.writes.get() + 1);
    }
    fn thread_lifetime(&self, t: recent::ThreadKey) -> Weak<()> {
        Rc::downgrade(&self.threads.borrow()[&t])
    }
}
struct Codec<'a>(&'a SessionScope);
// Only the service frame is tested; this is NOT original Serialize::peer format.
impl PeerCodec for Codec<'_> {
    fn write_peer(&self, p: PeerKey, w: &mut Writer) {
        w.u64(p.id().value);
    }
    fn read_peer(&self, _: u32, r: &mut Reader<'_>) -> Option<PeerKey> {
        Some(self.0.peer_key(PeerId::new(r.u64()?)))
    }
}
fn key(s: &SessionScope, n: u64) -> PeerKey {
    s.peer_key(peer_from_user(n))
}
#[test]
fn recent_peers_unbounded_bump_first_noop_absent_remove_clear_and_reentry() {
    let s = SessionScope::new();
    let o = Rc::new(RecentOwner::default());
    let m = Rc::new(recent::RecentPeers::new(&s, o.clone()));
    let life = Lifetime::new();
    let weak = Rc::downgrade(&m);
    let count = Rc::new(Cell::new(0));
    let n = count.clone();
    m.updates().start(
        move |_| {
            let _ = weak.upgrade().unwrap().list();
            n.set(n.get() + 1);
        },
        |_| {},
        || {},
        &life,
    );
    for n in 1..=60 {
        m.bump(key(&s, n));
    }
    assert_eq!(m.list().len(), 60);
    let writes = o.writes.get();
    m.bump(key(&s, 60));
    assert_eq!(o.writes.get(), writes);
    m.remove(key(&s, 999));
    assert_eq!(o.writes.get(), writes + 1);
    assert_eq!(count.get(), 60);
    m.bump(key(&s, 1));
    assert_eq!(m.list()[0], key(&s, 1));
    m.remove(key(&s, 1));
    assert_eq!(m.list().len(), 59);
    m.clear();
    m.clear();
    assert!(m.list().is_empty());
    assert_eq!(count.get(), 64);
}
#[test]
fn recent_peers_frame_limit_corruption_duplicate_order_and_no_update_on_read() {
    let s = SessionScope::new();
    let o = Rc::new(RecentOwner::default());
    let m = recent::RecentPeers::new(&s, o.clone());
    for n in 1..=60 {
        m.bump(key(&s, n));
    }
    let data = m.serialize(7002009, &Codec(&s));
    assert_eq!(&data[..8], &[0, 106, 215, 153, 0, 0, 0, 48]);
    assert_eq!(data.len(), 8 + 48 * 8);
    let other = recent::RecentPeers::new(&s, o);
    let life = Lifetime::new();
    let events = collect(other.updates(), &life);
    other.apply_local(&data, &Codec(&s));
    assert_eq!(other.list(), m.list()[..48]);
    assert!(events.borrow().is_empty());
    other.apply_local(&data[..data.len() - 1], &Codec(&s));
    assert!(other.list().is_empty());
    other.apply_local(&[], &Codec(&s));
    assert!(other.serialize(1, &Codec(&s)).is_empty());
    let mut w = Writer::default();
    w.i32(1);
    w.i32(2);
    w.u64(key(&s, 1).id().value);
    w.u64(key(&s, 1).id().value);
    other.apply_local(&w.0, &Codec(&s));
    assert_eq!(other.list(), [key(&s, 1); 2]);
    other.remove(key(&s, 1));
    assert_eq!(other.list(), [key(&s, 1)]);
}
#[test]
fn recent_open_weak_identity_no_revive_cap_reorder_remove_and_sublist() {
    let s = SessionScope::new();
    let o = Rc::new(RecentOwner::default());
    let m = recent::RecentPeers::new(&s, o.clone());
    for n in 1..=40 {
        let t = recent::ThreadKey::History(key(&s, n));
        o.threads.borrow_mut().insert(t, Rc::new(()));
        m.chat_open_push(t);
    }
    assert_eq!(m.collect_chat_open_history().len(), 32);
    let t = recent::ThreadKey::Topic(TopicKey {
        history: key(&s, 1),
        root_id: 7.into(),
    });
    o.threads.borrow_mut().insert(t, Rc::new(()));
    m.chat_open_push(t);
    o.threads.borrow_mut().remove(&t);
    o.threads.borrow_mut().insert(t, Rc::new(()));
    assert!(!m.collect_chat_open_history().contains(&t));
    m.chat_open_push(t);
    assert_eq!(m.collect_chat_open_history()[0], t);
    let sub = recent::ThreadKey::Sublist(SublistKey::new(key(&s, 1), key(&s, 2)));
    o.threads.borrow_mut().insert(sub, Rc::new(()));
    m.chat_open_push(sub);
    m.chat_open_push(t);
    assert_eq!(m.collect_chat_open_history()[0], t);
    m.chat_open_remove(t);
    assert!(!m.collect_chat_open_history().contains(&t));
}
struct StreamReader {
    remote: bool,
}
struct StreamDocument {
    _reader: Rc<StreamReader>,
    qualities: RefCell<Vec<stream::QualityDescriptor>>,
    drops: Rc<Cell<usize>>,
}
impl Drop for StreamDocument {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
#[derive(Default)]
struct StreamBackend {
    requests: Cell<usize>,
    deny: Cell<bool>,
    drops: Rc<Cell<usize>>,
    qualities: RefCell<Vec<(DocumentRecordKey, i64, i32)>>,
}
impl stream::StreamingBackend for StreamBackend {
    type Reader = StreamReader;
    type Document = StreamDocument;
    fn create_reader(
        &self,
        _: stream::MediaKey,
        _: FileOrigin,
        force: bool,
    ) -> Option<Rc<StreamReader>> {
        self.requests.set(self.requests.get() + 1);
        (!self.deny.get()).then(|| Rc::new(StreamReader { remote: force }))
    }
    fn is_remote_loader(&self, r: &StreamReader) -> bool {
        r.remote
    }
    fn create_document(
        &self,
        _: stream::MediaKey,
        r: Rc<StreamReader>,
        q: Vec<stream::QualityDescriptor>,
    ) -> Rc<StreamDocument> {
        Rc::new(StreamDocument {
            _reader: r,
            qualities: RefCell::new(q),
            drops: self.drops.clone(),
        })
    }
    fn set_other_qualities(&self, d: &StreamDocument, q: Vec<stream::QualityDescriptor>) {
        *d.qualities.borrow_mut() = q;
    }
    fn resolve_qualities(&self, _: DocumentRecordKey, _: MessageKey) -> Vec<DocumentRecordKey> {
        self.qualities.borrow().iter().map(|(k, _, _)| *k).collect()
    }
    fn quality_data(&self, quality: DocumentRecordKey) -> (i64, i32) {
        let v = self.qualities.borrow();
        let (_, size, height) = v.iter().find(|(k, _, _)| *k == quality).unwrap();
        (*size, *height)
    }
}
fn photos(s: &SessionScope) -> (stream::MediaKey, stream::MediaKey) {
    let p = tg_core::data::data_media_store::Photos::new(s);
    (
        stream::MediaKey::Photo(p.ensure_photo(1)),
        stream::MediaKey::Photo(p.ensure_photo(2)),
    )
}
#[test]
fn streaming_shares_weak_readers_force_remote_and_loader_failure() {
    let s = SessionScope::new();
    let (_, h) = clock();
    let b = Rc::new(StreamBackend::default());
    let m = stream::Streaming::new(&s, b.clone(), h);
    let (key, other) = photos(&s);
    let r = m.shared_reader(key, FileOrigin::default(), false).unwrap();
    assert!(Rc::ptr_eq(
        &r,
        &m.shared_reader(key, FileOrigin::default(), false).unwrap()
    ));
    b.deny.set(true);
    assert!(m.shared_reader(key, FileOrigin::default(), true).is_none());
    assert!(Rc::ptr_eq(
        &r,
        &m.shared_reader(key, FileOrigin::default(), false).unwrap()
    ));
    b.deny.set(false);
    let remote = m.shared_reader(key, FileOrigin::default(), true).unwrap();
    assert!(!Rc::ptr_eq(&r, &remote));
    assert!(Rc::ptr_eq(
        &remote,
        &m.shared_reader(key, FileOrigin::default(), true).unwrap()
    ));
    drop(r);
    drop(remote);
    assert!(m
        .shared_reader(other, FileOrigin::default(), false)
        .is_some());
    assert!(m.shared_reader(key, FileOrigin::default(), false).is_some());
}
#[test]
fn streaming_keepalive_extends_deadline_no_early_rearm_and_releases_on_drop() {
    let s = SessionScope::new();
    let (c, h) = clock();
    let b = Rc::new(StreamBackend::default());
    let m = stream::Streaming::new(&s, b.clone(), h.clone());
    let (key, other) = photos(&s);
    m.keep_alive(other);
    let d = m.shared_document(key, FileOrigin::default()).unwrap();
    let weak = Rc::downgrade(&d);
    m.keep_alive(key);
    drop(d);
    c.advance(3000);
    m.keep_alive(key);
    c.advance(2000);
    h.process();
    assert!(weak.upgrade().is_some());
    c.advance(2999);
    h.process();
    assert!(weak.upgrade().is_some());
    c.advance(1);
    h.process();
    assert!(weak.upgrade().is_none());
    assert_eq!(b.drops.get(), 1);
    let d = m.shared_document(key, FileOrigin::default()).unwrap();
    m.keep_alive(key);
    drop(d);
    drop(m);
    assert_eq!(b.drops.get(), 2);
}
fn search_fixture() -> (
    SessionScope,
    Rc<search_owner::Owner>,
    Rc<search::SearchController>,
) {
    let s = SessionScope::new();
    let owner = Rc::new(search_owner::Owner::new(&s));
    let peers = Rc::new(Peers::new(&s, Changes::with_queue(&s, MainQueue::new())));
    let c = search::SearchController::new(peers, owner.clone());
    (s, owner, c)
}
fn query() -> search::Query {
    search::Query {
        peer_id: peer_from_user(5),
        query: "a".into(),
        ..Default::default()
    }
}
fn messages(ids: &[i32]) -> tl::MessagesMessages {
    tl::mtp_messages_messages(
        ids.iter()
            .map(|id| tl::mtp_message_empty(tl::tl::Flags::from_raw(0), *id, None))
            .collect(),
        vec![],
        vec![],
        vec![],
    )
}
#[test]
fn search_parsing_source_filter_expands_range_before_after_and_not_modified() {
    let (s, o, _) = search_fixture();
    o.included.borrow_mut().insert(7);
    let p = key(&s, 5);
    let r = search::parse_search_result(
        &*o,
        p,
        Some(Type::Photo),
        10.into(),
        LoadDirection::Around,
        &messages(&[3, 7, 21, 0]),
    );
    assert_eq!(r.message_ids, [MsgId::new(7)]);
    assert_eq!((r.no_skip_range.from, r.no_skip_range.till), (3, 21));
    assert_eq!(r.full_count, 4);
    assert_eq!(&o.log.borrow()[..3], &["users", "chats", "topics"]);
    for (d, a, b) in [
        (LoadDirection::Before, 0, 21),
        (LoadDirection::Around, 0, SERVER_MAX_MSG_ID.bare),
        (LoadDirection::After, 3, SERVER_MAX_MSG_ID.bare),
    ] {
        o.included.borrow_mut().clear();
        let r = search::parse_search_result(
            &*o,
            p,
            Some(Type::Photo),
            10.into(),
            d,
            &messages(&[3, 21]),
        );
        assert_eq!((r.no_skip_range.from, r.no_skip_range.till), (a, b));
    }
    let r = search::parse_history_result(
        &*o,
        p,
        10.into(),
        LoadDirection::After,
        &tl::mtp_messages_messages_not_modified(99),
    );
    assert_eq!(r.full_count, 0);
    assert_eq!((r.no_skip_range.from, r.no_skip_range.till), (10, 10));
}
#[test]
fn search_global_filtered_offset_channel_pts_and_not_modified_precondition() {
    let (s, o, _) = search_fixture();
    o.forward.set(false);
    let r = search::parse_global_media_result(&*o, &messages(&[2, 3]), true);
    assert_eq!(r.filtered_count, 2);
    assert!(r.message_ids.is_empty());
    assert_eq!(r.offset_position.full_id.msg, 3.into());
    o.forward.set(true);
    assert_eq!(
        search::parse_global_media_result(&*o, &messages(&[2, 3]), true)
            .message_ids
            .len(),
        2
    );
    let result = tl::mtp_messages_channel_messages(
        tl::tl::Flags::from_raw(0),
        42,
        10,
        None,
        vec![],
        vec![],
        vec![],
        vec![],
    );
    let r = search::parse_search_result(
        &*o,
        s.peer_key(peer_from_channel(1)),
        None,
        0.into(),
        LoadDirection::Around,
        &result,
    );
    assert_eq!(r.full_count, 10);
    assert!(o.log.borrow().contains(&"pts:42".into()));
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        search::parse_global_media_result(&*o, &tl::mtp_messages_messages_not_modified(2), false)
    }))
    .is_err());
}
#[test]
fn search_cache_viewer_real_storage_removal_filters_and_request_failure_latch() {
    let (s, o, c) = search_fixture();
    c.set_query(query());
    let life = Lifetime::new();
    let out = collect(c.ids_slice(10.into(), 1, 1), &life);
    assert!(!o.requests.borrow().is_empty());
    let id = o.requests.borrow()[0].0;
    c.handle_reply(id, Err(()));
    let before = o.requests.borrow().len();
    let _second = collect(c.ids_slice(10.into(), 1, 1), &life);
    assert_eq!(o.requests.borrow().len(), before);
    assert!(out.borrow().is_empty());
    life.destroy();
    let mut next = query();
    next.query = "success".into();
    next.topic_root_id = 7.into();
    c.set_query(next);
    let life = Lifetime::new();
    let out = collect(c.ids_slice(10.into(), 1, 1), &life);
    let id = o.requests.borrow().last().unwrap().0;
    c.handle_reply(id, Ok(messages(&[9, 10, 11])));
    assert!(!out.borrow().is_empty());
    assert_eq!(out.borrow().last().unwrap().size(), 3);
    let k = o
        .keys
        .borrow()
        .get(FullMsgId::new(peer_from_user(5), 10.into()))
        .unwrap();
    o.removed.fire(k);
    assert_eq!(out.borrow().last().unwrap().size(), 3);
    o.topic.set(7.into());
    o.mono.set(peer_from_user(8));
    o.removed.fire(k);
    assert_eq!(out.borrow().last().unwrap().size(), 2);
    o.cleared.fire(key(&s, 5));
    assert_eq!(out.borrow().last().unwrap().size(), 0);
    assert!(o.log.borrow().contains(&format!("finish:{id}")));
    assert!(o.log.borrow().contains(&format!("cancel:{id}")));
}
#[test]
fn search_save_restore_cancel_empty_cache_and_query_order() {
    let (_scope, o, c) = search_fixture();
    assert!(c.has_in_cache(&search::Query::default()));
    let state = c.save_state();
    c.restore_state(state);
    c.set_query(query());
    let life = Lifetime::new();
    let _out = collect(c.ids_slice(10.into(), 1, 1), &life);
    let id = o.requests.borrow()[0].0;
    c.handle_reply(id, Ok(messages(&[9, 10, 11])));
    life.destroy();
    let saved = c.save_state();
    assert_eq!(saved.query, query());
    let mut q = query();
    q.query = "b".into();
    c.set_query(q);
    c.restore_state(saved);
    assert_eq!(c.query(), query());
    let restored_life = Lifetime::new();
    let restored = collect(c.ids_slice(10.into(), 1, 1), &restored_life);
    assert_eq!(restored.borrow().last().unwrap().size(), 3);
    restored_life.destroy();
    c.set_query(search::Query {
        peer_id: peer_from_user(5),
        ..Default::default()
    });
    assert!(!c.has_in_cache(&query()));
    let mut a = query();
    a.query = "\u{10000}".into();
    let mut b = a.clone();
    b.query = "\u{e000}".into();
    assert!(a < b);
    a.query = b.query.clone();
    a.type_ = Some(Type::Photo);
    assert!(a < b); // kCount sorts last
}
#[test]
fn delayed_search_virtual_debounce_cached_fast_path_keeps_old_timer_and_cancel() {
    let (_scope, _owner, c) = search_fixture();
    c.set_query(query());
    let (clock, host) = clock();
    let d = search::DelayedSearchController::new(c, host.clone());
    let life = Lifetime::new();
    let seen = collect(d.current_query_value(), &life);
    let mut q = query();
    q.query = "b".into();
    d.set_query(q.clone());
    clock.advance(199);
    host.process();
    assert_eq!(d.current_query(), query());
    d.set_query(query());
    clock.advance(1);
    host.process();
    assert_eq!(d.current_query(), query());
    d.set_query_delayed(q.clone(), 200);
    d.set_query(search::Query {
        peer_id: peer_from_user(5),
        ..Default::default()
    });
    clock.advance(200);
    host.process();
    assert_eq!(d.current_query(), q);
    assert_eq!(*seen.borrow(), ["a", "", "b"]);
    let saved = d.save_state();
    d.restore_state(saved);
}
#[test]
fn search_request_defaults_topic_mono_global_and_empty_query() {
    let (s, o, _) = search_fixture();
    let peers = Peers::new(&s, Changes::with_queue(&s, MainQueue::new()));
    let p = peers.ensure_peer(peer_from_user(5));
    assert!(search::prepare_search_request(
        &*o,
        &peers,
        p,
        0.into(),
        PeerId::default(),
        None,
        "",
        0.into(),
        LoadDirection::Around
    )
    .is_none());
    let r = search::prepare_search_request(
        &*o,
        &peers,
        p,
        7.into(),
        peer_from_user(8),
        Some(Type::Pinned),
        "",
        5.into(),
        LoadDirection::After,
    )
    .unwrap();
    assert_eq!(r.flags.value(), 6);
    assert_eq!(r.top_msg_id, Some(7));
    assert!(r.saved_peer_id.is_some());
    assert_eq!((r.offset_id, r.add_offset, r.limit), (6, -100, 100));
    assert!(
        search::prepare_global_media_request(&*o, &peers, 0, Default::default(), None, "")
            .is_none()
    );
    let r = search::prepare_global_media_request(
        &*o,
        &peers,
        2,
        Default::default(),
        Some(Type::Photo),
        "",
    )
    .unwrap();
    assert_eq!(r.folder_id, Some(0));
    assert_eq!(r.limit, 0);
    assert_eq!(r.offset_rate, 2);
    assert!(matches!(r.offset_peer, tl::InputPeer::InputPeerEmpty(_)));
}
#[test]
fn actual_sender_passkeys_and_search_replies_no_network() {
    use tg_mtproto::{
        mtp_instance::{get_next_request_id, Fields, Instance, Mode},
        mtproto_dc_options::Environment,
        mtproto_response::Response,
        sender::Sender,
    };
    let instance = Instance::new(Mode::Normal, Fields::new(Environment::Test));
    let sender = Sender::new(instance.clone());
    let replies = Arc::new(Mutex::new(vec![]));
    let r = replies.clone();
    let id = get_next_request_id();
    p::send_request(
        &sender,
        id,
        p::Request::List(tl::AccountGetPasskeys {}),
        Arc::new(move |id, rp| r.lock().unwrap().push((id, rp))),
    );
    instance.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&passkeys(vec![passkey()])),
    });
    assert!(matches!(replies.lock().unwrap()[0].1, p::Reply::List(_)));
    let done = Arc::new(Mutex::new(vec![]));
    let out = done.clone();
    let (s, o, _) = search_fixture();
    let q = search::prepare_history_request(&*o, key(&s, 5), 0.into(), LoadDirection::Around);
    let id = get_next_request_id();
    search::send_search_request(
        &sender,
        id,
        tl::MessagesSearch {
            flags: tl::tl::Flags::from_raw(0),
            peer: q.peer,
            q: vec![],
            from_id: None,
            saved_peer_id: None,
            saved_reaction: None,
            top_msg_id: None,
            filter: search::prepare_search_filter(None),
            min_date: 0,
            max_date: 0,
            offset_id: 0,
            add_offset: 0,
            limit: 0,
            max_id: 0,
            min_id: 0,
            hash: 0,
        },
        Arc::new(move |r| out.lock().unwrap().push(r)),
    );
    instance.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&messages(&[])),
    });
    assert!(done.lock().unwrap()[0].is_ok());
}
struct NoFiles;
impl tg_core::core::file_location::FileLocations for NoFiles {
    fn inspect(
        &self,
        _: &tg_core::core::file_location::FileLocation,
    ) -> tg_core::core::file_location::FileInfo {
        panic!("unexpected filesystem access")
    }
    fn read(&self, _: (u64, u64)) -> tg_core::core::file_location::FileLocation {
        Default::default()
    }
}
#[test]
fn streaming_quality_metadata_updates_only_nonempty_and_shares_reader() {
    let s = SessionScope::new();
    let (_, h) = clock();
    let docs = tg_core::data::data_media_store::Documents::new(&s, Rc::new(NoFiles));
    let q = docs.ensure_document(1);
    let original = docs.ensure_document(2);
    let mut messages = MessageKeys::new(&s);
    let context = messages.insert(FullMsgId::new(peer_from_user(5), 1.into()));
    let b = Rc::new(StreamBackend::default());
    let m = stream::Streaming::new(&s, b.clone(), h);
    let d = m
        .shared_quality_document(q, original, Some(context), FileOrigin::default())
        .unwrap();
    assert!(d.qualities.borrow().is_empty());
    let quality = stream::QualityDescriptor {
        size_in_bytes: 123,
        height: 720,
    };
    let alt = docs.ensure_document(3);
    let zero = docs.ensure_document(4);
    b.qualities
        .borrow_mut()
        .extend([(q, 100, 999), (zero, 500, 0), (alt, 123, 720)]);
    let other = m
        .shared_quality_document(q, original, Some(context), FileOrigin::default())
        .unwrap();
    assert!(Rc::ptr_eq(&d, &other));
    assert_eq!(*d.qualities.borrow(), [quality]);
    b.qualities.borrow_mut().clear();
    assert!(Rc::ptr_eq(
        &d,
        &m.shared_document(stream::MediaKey::Document(q), FileOrigin::default())
            .unwrap()
    ));
    assert_eq!(*d.qualities.borrow(), [quality]);
    assert_eq!(b.requests.get(), 1);
}
#[test]
fn passkey_login_real_sender_parse_invalid_handle_dc_switch_done_and_fail() {
    use tg_core::platform::platform_webauthn::LoginResult;
    use tg_mtproto::{
        mtp_instance::{Fields, Instance, Mode},
        mtproto_auth_key::{AuthKey, KeyType},
        mtproto_dc_options::Environment,
        mtproto_response::Response,
        sender::Sender,
    };
    let mut fields = Fields::new(Environment::Test);
    fields.main_dc_id = 1;
    fields.keys.push(Arc::new(AuthKey::new(
        KeyType::ReadFromFile,
        1,
        [7; 256],
        0,
    )));
    let instance = Instance::new(Mode::Normal, fields);
    let sender = Sender::new(instance.clone());
    let log = Arc::new(Mutex::new(vec![]));
    let v = log.clone();
    let id = p::init_passkey_login(
        &sender,
        123,
        b"api-hash-fixture".to_vec(),
        Arc::new(move |data| v.lock().unwrap().push(data.timeout)),
    );
    instance.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&tl::mtp_auth_passkey_login_options(tl::mtp_data_json(
            br#"{"publicKey":{"timeout":10}}"#.to_vec(),
        ))),
    });
    assert_eq!(*log.lock().unwrap(), [10]);
    let v = log.clone();
    let id = p::init_passkey_login(
        &sender,
        123,
        vec![],
        Arc::new(move |data| v.lock().unwrap().push(data.timeout)),
    );
    instance.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&tl::mtp_auth_passkey_login_options(tl::mtp_data_json(
            b"{}".to_vec(),
        ))),
    });
    assert_eq!(log.lock().unwrap().len(), 1);
    let errors = Arc::new(Mutex::new(vec![]));
    let e = errors.clone();
    assert!(p::finish_passkey_login(
        &sender,
        10001,
        &LoginResult::default(),
        Arc::new(|_| panic!()),
        Arc::new(move |v| e.lock().unwrap().push(v))
    )
    .is_none());
    assert_eq!(instance.main_dc_id(), 1);
    assert_eq!(errors.lock().unwrap()[0], "PASSKEY_HANDLE_INVALID");
    let done = Arc::new(Mutex::new(0));
    let d = done.clone();
    let id = p::finish_passkey_login(
        &sender,
        10001,
        &LoginResult {
            user_handle: b"2:".to_vec(),
            ..Default::default()
        },
        Arc::new(move |_| *d.lock().unwrap() += 1),
        Arc::new(|_| panic!()),
    )
    .unwrap();
    assert_eq!(instance.main_dc_id(), 2);
    instance.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&tl::mtp_auth_authorization_sign_up_required(
            tl::tl::Flags::from_raw(0),
            None,
        )),
    });
    assert_eq!(*done.lock().unwrap(), 1);
    let e = errors.clone();
    let id = p::finish_passkey_login(
        &sender,
        10001,
        &LoginResult {
            user_handle: b"3:u".to_vec(),
            ..Default::default()
        },
        Arc::new(|_| panic!()),
        Arc::new(move |v| e.lock().unwrap().push(v)),
    )
    .unwrap();
    instance.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&tl::mtp_rpc_error(400, b"PASSKEY_BAD".to_vec())),
    });
    assert_eq!(errors.lock().unwrap().last().unwrap(), "PASSKEY_BAD");
}
#[test]
fn search_migrated_requests_saved_lists_and_cancellation_on_cache_clear() {
    let (_scope, o, c) = search_fixture();
    let mut q = query();
    q.migrated_peer_id = peer_from_chat(8);
    c.set_query(q);
    let life = Lifetime::new();
    let _out = collect(c.ids_slice(MsgId::new(-1), 0, 0), &life);
    assert_eq!(o.requests.borrow().len(), 2);
    assert!(o
        .requests
        .borrow()
        .iter()
        .any(|(_, r)| matches!(r.peer, tl::InputPeer::InputPeerChat(_))));
    life.destroy();
    let saved = c.save_state();
    assert!(saved.migrated_list.is_some());
    c.restore_state(saved);
    assert_eq!(
        o.log
            .borrow()
            .iter()
            .filter(|s| s.starts_with("cancel:"))
            .count(),
        2
    );
    c.set_query(search::Query {
        peer_id: peer_from_user(5),
        ..Default::default()
    });
}
