use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::Arc,
};
use tg_base::{
    qt_stream::{Reader, Writer},
    timer::{ManualClock, TimerHost},
};
use tg_core::data::{
    components::top_peers as top, data_emoji_statuses as emoji, data_peer_id::*,
    data_store_keys::*, data_types::EmojiStatusId,
};
use tg_rpl::Lifetime;
use tg_scheme as tl;
fn user(scope: &SessionScope, n: u64) -> PeerKey {
    scope.peer_key(peer_from_user(n))
}
fn facts() -> Option<top::IncrementFacts> {
    Some(top::IncrementFacts {
        is_bot: false,
        bot_info_present: false,
        supports_guest_chat: false,
    })
}
struct Codec<'a>(&'a SessionScope);
// Test-only peer codec: the original full peer format is not claimed by this
// test. Only TopPeers' framing/rating/rollback around its owner codec is tested.
impl top::PeerCodec for Codec<'_> {
    fn write_peer(&self, p: PeerKey, w: &mut Writer) {
        w.u64(p.id().value);
    }
    fn read_peer(&self, _version: u32, r: &mut Reader<'_>) -> Option<PeerKey> {
        Some(self.0.peer_key(PeerId::new(r.u64()?)))
    }
}
fn model(
    scope: &SessionScope,
    kind: top::TopPeerType,
) -> (Rc<top::TopPeers>, Rc<RefCell<Vec<top::Effect>>>) {
    let out = Rc::new(RefCell::new(Vec::new()));
    let o = out.clone();
    (
        Rc::new(top::TopPeers::new(
            scope,
            kind,
            Rc::new(move |e| o.borrow_mut().push(e)),
        )),
        out,
    )
}
#[test]
fn top_ratings_ties_persistence_and_reentrant_updates() {
    let scope = SessionScope::new();
    let (p, e) = model(&scope, top::TopPeerType::Chat);
    assert!(matches!(e.borrow()[0], top::Effect::LoadAfterChats));
    assert_eq!(p.session(), scope.id());
    let life = Lifetime::new();
    let weak = Rc::downgrade(&p);
    let calls = Rc::new(Cell::new(0));
    let n = calls.clone();
    p.updates().start(
        move |_| {
            let p = weak.upgrade().unwrap();
            let _ = p.list();
            n.set(n.get() + 1);
        },
        |_| {},
        || {},
        &life,
    );
    p.increment(user(&scope, 1), 1, 60, facts());
    p.increment(user(&scope, 2), 1, 60, facts());
    assert_eq!(p.list(), [user(&scope, 2), user(&scope, 1)]);
    let before = calls.get();
    p.increment(user(&scope, 2), 2, 60, facts());
    assert_eq!(calls.get(), before);
    assert!(matches!(
        e.borrow().last(),
        Some(top::Effect::ReadSearchSuggestions) | Some(top::Effect::WriteSearchSuggestionsDelayed)
    ));
    assert!(p.ratings()[0].rating > 2.);
    let b = p.serialize(7002009, &Codec(&scope));
    let (other, _) = model(&scope, top::TopPeerType::BotApp);
    other.apply_local(&b, &Codec(&scope));
    assert_eq!(other.list(), p.list());
    assert!((other.ratings()[0].rating - p.ratings()[0].rating).abs() < 0.000001);
    assert!(!other.disabled());
    other.apply_local(&b[..b.len() - 1], &Codec(&scope));
    assert!(other.list().is_empty());
    assert!(!other.disabled());
    other.apply_local(&[], &Codec(&scope));
    assert!(other.list().is_empty());
}
#[test]
fn top_increment_eligibility_disabled_and_source_reset_latch() {
    let s = SessionScope::new();
    let (p, e) = model(&s, top::TopPeerType::BotGuestChat);
    p.increment(user(&s, 1), 1, 60, facts());
    assert!(p.list().is_empty());
    let bot = Some(top::IncrementFacts {
        is_bot: true,
        bot_info_present: true,
        supports_guest_chat: true,
    });
    p.increment(user(&s, 1), 1, 60, bot);
    assert_eq!(p.list().len(), 1);
    p.toggle_disabled(true);
    p.increment(user(&s, 2), 2, 60, bot);
    assert!(p.list().is_empty());
    assert!(p.disabled());
    p.toggle_disabled(false);
    p.increment(user(&s, 1), 2, 60, bot);
    e.borrow_mut().clear();
    p.remove(user(&s, 1), tl::mtp_input_peer_user(1, 2));
    assert!(p.list().is_empty());
    assert!(e.borrow().iter().any(|x| matches!(
        x,
        top::Effect::Send {
            request: top::Request::Reset(_),
            ..
        }
    )));
    e.borrow_mut().clear();
    p.reload(1_000_000);
    assert!(!e
        .borrow()
        .iter()
        .any(|x| matches!(x, top::Effect::Send { .. })));
    assert!(!top::can_increment_peer(top::TopPeerType::BotApp, bot));
    assert!(!top::can_increment_peer(top::TopPeerType::Chat, None));
}
#[test]
fn top_requests_reply_order_throttle_and_local_skip() {
    let s = SessionScope::new();
    let (p, e) = model(&s, top::TopPeerType::Chat);
    p.reload(10);
    p.reload(20);
    assert_eq!(
        e.borrow()
            .iter()
            .filter(|x| matches!(x, top::Effect::Send { .. }))
            .count(),
        1
    );
    let result = tl::mtp_contacts_top_peers(
        vec![
            tl::mtp_top_peer_category_peers(
                top::type_to_category(top::TopPeerType::BotApp),
                0,
                vec![],
            ),
            tl::mtp_top_peer_category_peers(
                top::type_to_category(top::TopPeerType::Chat),
                2,
                vec![
                    tl::mtp_top_peer(tl::mtp_peer_user(1), 2.),
                    tl::mtp_top_peer(tl::mtp_peer_user(2), 1.),
                ],
            ),
        ],
        vec![],
        vec![],
    );
    e.borrow_mut().clear();
    p.handle_reply(
        top::Reply::Loaded {
            result,
            outer_msg_id: 100u64 << 32,
        },
        1000,
        |id| s.peer_key(id),
    );
    assert!(matches!(e.borrow()[0], top::Effect::ProcessUsers(_)));
    assert!(matches!(e.borrow()[1], top::Effect::ProcessChats(_)));
    assert!(matches!(e.borrow()[2], top::Effect::UnexpectedCategory));
    assert_eq!(p.list().len(), 2);
    p.apply_local(&[], &Codec(&s));
    assert_eq!(p.list().len(), 2);
    p.increment(user(&s, 3), 100, 60, facts());
    assert_eq!(p.list().len(), 2);
    e.borrow_mut().clear();
    p.reload(10_999);
    assert!(e.borrow().is_empty());
    p.reload(11_000);
    assert!(matches!(
        e.borrow()[0],
        top::Effect::Send {
            request: top::Request::Get(_),
            ..
        }
    ));
    p.handle_reply(top::Reply::Failed, 20_000, |id| s.peer_key(id));
    p.reload(30_000);
    assert!(e.borrow().len() >= 2);
    p.handle_reply(
        top::Reply::Loaded {
            result: tl::mtp_contacts_top_peers_disabled(),
            outer_msg_id: 200u64 << 32,
        },
        31_000,
        |id| s.peer_key(id),
    );
    assert!(p.disabled());
    assert!(p.list().is_empty());
    p.handle_reply(
        top::Reply::Loaded {
            result: tl::mtp_contacts_top_peers_not_modified(),
            outer_msg_id: 201u64 << 32,
        },
        32_000,
        |id| s.peer_key(id),
    );
    assert!(p.disabled());
}
#[test]
fn top_hash_only_first_64_and_type_flags() {
    let s = SessionScope::new();
    let (p, _) = model(&s, top::TopPeerType::Chat);
    for n in 1..=70 {
        p.increment(user(&s, n), 1, 60, facts());
    }
    assert_eq!(p.list().len(), 70);
    let mut hash = 0;
    for n in (7..=70).rev() {
        top::hash_update(&mut hash, n);
    }
    assert_eq!(p.count_hash(), hash);
    assert_eq!(top::type_to_get_flags(top::TopPeerType::BotApp), 1 << 16);
    assert_eq!(
        top::type_to_get_flags(top::TopPeerType::BotGuestChat),
        1 << 17
    );
    assert_eq!(top::deserialize_rating(u64::MAX), 1_000_000.);
}
type EmojiFixture = (
    SessionScope,
    Rc<emoji::EmojiStatuses>,
    Rc<TimerHost>,
    Arc<ManualClock>,
    Rc<Cell<i32>>,
    Rc<RefCell<Vec<emoji::Effect>>>,
);
fn emoji_model() -> EmojiFixture {
    let scope = SessionScope::new();
    let clock = Arc::new(ManualClock::new(0));
    let host = TimerHost::new(clock.clone());
    let now = Rc::new(Cell::new(100));
    let n = now.clone();
    let effects = Rc::new(RefCell::new(Vec::new()));
    let e = effects.clone();
    let service = emoji::EmojiStatuses::new(
        &scope,
        &host,
        Rc::new(move || n.get()),
        Rc::new(move |effect| e.borrow_mut().push(effect)),
    );
    (scope, service, host, clock, now, effects)
}
fn status(id: u64) -> tl::EmojiStatus {
    tl::mtp_emoji_status(Default::default(), id, None)
}
fn reply(kind: emoji::Type, hash: u64, list: Vec<tl::EmojiStatus>) -> emoji::Reply {
    emoji::Reply::Statuses(kind, tl::mtp_account_emoji_statuses(hash, list))
}
#[test]
fn emoji_constructor_hash_failures_lists_and_periodic_refresh() {
    let (scope, s, host, clock, _, e) = emoji_model();
    assert_eq!(s.owner(), scope.id());
    assert_eq!(s.session(), scope.id());
    assert_eq!(e.borrow().len(), 2);
    assert!(matches!(
        e.borrow()[0],
        emoji::Effect::Send {
            request: emoji::Request::Default(_),
            ..
        }
    ));
    assert!(matches!(
        e.borrow()[1],
        emoji::Effect::Send {
            request: emoji::Request::Colored(_),
            ..
        }
    ));
    s.refresh_default();
    assert_eq!(e.borrow().len(), 2);
    let life = Lifetime::new();
    let count = Rc::new(Cell::new(0));
    let c = count.clone();
    let weak = Rc::downgrade(&s);
    s.default_updates().start(
        move |_| {
            assert_eq!(weak.upgrade().unwrap().list(emoji::Type::Default).len(), 1);
            c.set(c.get() + 1);
        },
        |_| {},
        || {},
        &life,
    );
    s.handle_reply(
        reply(
            emoji::Type::Default,
            123,
            vec![status(5), tl::mtp_emoji_status_empty()],
        ),
        |_| panic!(),
    );
    assert_eq!(count.get(), 1);
    assert!(e
        .borrow()
        .iter()
        .any(|v| matches!(v, emoji::Effect::InvalidStatus)));
    s.refresh_default();
    assert!(
        matches!(e.borrow().last(),Some(emoji::Effect::Send{request:emoji::Request::Default(r),..}) if r.hash==123)
    );
    s.handle_reply(emoji::Reply::Failed(emoji::Type::Default), |_| panic!());
    s.refresh_default();
    assert!(
        matches!(e.borrow().last(),Some(emoji::Effect::Send{request:emoji::Request::Default(r),..}) if r.hash==0)
    );
    clock.advance(3_600_000);
    host.process();
    assert!(e.borrow().iter().any(|v| matches!(
        v,
        emoji::Effect::Send {
            request: emoji::Request::ChannelDefault(_),
            ..
        }
    )));
}
#[test]
fn emoji_recent_delayed_requests_coalesce_without_sleep() {
    let (_scope, s, host, clock, _, e) = emoji_model();
    e.borrow_mut().clear();
    s.refresh_recent_delayed();
    s.refresh_recent_delayed();
    clock.advance(9999);
    host.process();
    assert!(e.borrow().is_empty());
    clock.advance(1);
    host.process();
    assert_eq!(e.borrow().len(), 1);
    s.refresh_recent_delayed();
    s.handle_reply(
        reply(emoji::Type::Recent, 42, vec![status(7)]),
        |_| panic!(),
    );
    assert_eq!(s.list(emoji::Type::Recent)[0].document_id, 7);
    s.refresh_recent_delayed();
    clock.advance(10_000);
    host.process();
    assert!(
        matches!(e.borrow().last(),Some(emoji::Effect::Send{request:emoji::Request::Recent(r),..}) if r.hash==42)
    );
}
#[test]
fn emoji_colored_processes_all_documents_then_collectibles() {
    let (_scope, s, _, _, _, e) = emoji_model();
    e.borrow_mut().clear();
    let sticker = tl::mtp_messages_sticker_set(
        tl::mtp_sticker_set(
            Default::default(),
            None,
            1,
            2,
            b"s".to_vec(),
            b"s".to_vec(),
            None,
            None,
            None,
            None,
            0,
            0,
        ),
        vec![],
        vec![],
        vec![tl::mtp_document_empty(3), tl::mtp_document_empty(4)],
    );
    let mut processed = Vec::new();
    s.handle_reply(emoji::Reply::Stickers(emoji::Type::Colored, sticker), |d| {
        let tl::Document::DocumentEmpty(d) = d else {
            panic!()
        };
        processed.push(d.id);
        d.id
    });
    assert_eq!(processed, [3, 4]);
    assert_eq!(
        s.list(emoji::Type::Colored)
            .iter()
            .map(|id| id.document_id)
            .collect::<Vec<_>>(),
        [3, 4]
    );
    assert!(matches!(
        e.borrow().last(),
        Some(emoji::Effect::Send {
            request: emoji::Request::Collectibles(_),
            ..
        })
    ));
    s.handle_reply(
        reply(emoji::Type::Collectibles, 5, vec![status(8)]),
        |_| panic!(),
    );
    assert_eq!(s.list(emoji::Type::Collectibles).len(), 1);
    s.refresh_channel_colored();
    s.handle_reply(
        emoji::Reply::Stickers(
            emoji::Type::ChannelColored,
            tl::mtp_messages_sticker_set_not_modified(),
        ),
        |_| panic!(),
    );
    s.refresh_channel_default();
    s.handle_reply(
        reply(emoji::Type::ChannelDefault, 1, vec![status(9)]),
        |_| panic!(),
    );
    assert_eq!(s.list(emoji::Type::ChannelDefault)[0].document_id, 9);
}
#[test]
fn emoji_collectible_first_payload_keeps_shared_identity() {
    let (_scope, s, _, _, _, _) = emoji_model();
    let mut c = emoji::EmojiStatusCollectible {
        id: 7,
        document_id: 8,
        title: "first".into(),
        ..Default::default()
    };
    assert!(c.is_present());
    let a = s.intern_collectible(c.clone());
    c.title = "second".into();
    let b = s.intern_collectible(c);
    assert_eq!(a, b);
    assert_eq!(s.collectible_info(7).unwrap().title, "first");
    assert!(s.collectible_info(99).is_none());
    assert!(s
        .parse(&tl::mtp_input_emoji_status_collectible(
            Default::default(),
            7,
            None
        ))
        .id
        .collectible
        .is_none());
    let wire = emoji::status_to_mtp(&a, 200);
    assert!(
        matches!(wire,tl::EmojiStatus::InputEmojiStatusCollectible(d) if d.collectible_id==7&&d.until==Some(200))
    );
    let doc = EmojiStatusId {
        document_id: 9,
        collectible: None,
    };
    let parsed = s.parse(&emoji::status_to_mtp(&doc, 300));
    assert_eq!(parsed.id, doc);
    assert_eq!(parsed.until, 300);
    assert!(!s
        .parse(&emoji::status_to_mtp(&EmojiStatusId::default(), 0))
        .id
        .is_present());
}
#[test]
fn emoji_set_cancels_before_mutation_and_preserves_local_on_failure() {
    let (scope, s, _, _, _, e) = emoji_model();
    e.borrow_mut().clear();
    let peer = user(&scope, 1);
    let u = UserKey::try_from(peer).unwrap();
    s.set_self(
        u,
        EmojiStatusId {
            document_id: 3,
            collectible: None,
        },
        200,
    );
    s.set_self(u, EmojiStatusId::default(), 0);
    let e = e.borrow();
    assert!(matches!(
        &e[0],
        emoji::Effect::SetPeerStatus { until: 200, .. }
    ));
    assert!(matches!(
        &e[1],
        emoji::Effect::Send {
            request: emoji::Request::SetSelf(_),
            ..
        }
    ));
    assert!(matches!(&e[2], emoji::Effect::Cancel(_)));
    assert!(matches!(
        &e[3],
        emoji::Effect::SetPeerStatus { until: 0, .. }
    ));
    drop(e);
    s.handle_reply(emoji::Reply::SetFinished(peer), |_| panic!());
    s.set(
        scope.peer_key(peer_from_channel(2)),
        false,
        Some(tl::mtp_input_channel(2, 5)),
        EmojiStatusId::default(),
        0,
    );
}
#[test]
fn emoji_automatic_clear_cap_remove_and_virtual_expiration() {
    let (scope, s, host, clock, now, e) = emoji_model();
    e.borrow_mut().clear();
    let a = user(&scope, 1);
    let b = user(&scope, 2);
    s.register_automatic_clear(a, 100 + 100_000);
    assert_eq!(s.clearing_remaining_time(), 21_600_000);
    s.register_automatic_clear(b, 110);
    assert_eq!(s.clearing_remaining_time(), 10_000);
    s.register_automatic_clear(b, 110);
    assert_eq!(s.automatic_clear_at(b), 110);
    now.set(110);
    clock.advance(10_000);
    host.process();
    assert_eq!(s.automatic_clear_at(b), 0);
    assert_eq!(s.automatic_clear_at(a), 100_100);
    assert!(e
        .borrow()
        .iter()
        .any(|v| matches!(v,emoji::Effect::SetPeerStatus{peer,until:0,..} if *peer==b)));
    s.register_automatic_clear(a, 0);
    assert_eq!(s.clearing_remaining_time(), -1);
    s.register_automatic_clear(a, 109);
    assert_eq!(s.automatic_clear_at(a), 0);
}
#[test]
fn emoji_clearing_reentrancy_merges_new_registration() {
    let scope = SessionScope::new();
    let clock = Arc::new(ManualClock::new(0));
    let host = TimerHost::new(clock);
    let slot = Rc::new(RefCell::new(Weak::<emoji::EmojiStatuses>::new()));
    let v = slot.clone();
    let peer = user(&scope, 1);
    let s = emoji::EmojiStatuses::new(
        &scope,
        &host,
        Rc::new(|| 100),
        Rc::new(move |e| {
            if matches!(e, emoji::Effect::SetPeerStatus { .. }) {
                v.borrow()
                    .upgrade()
                    .unwrap()
                    .register_automatic_clear(peer, 200);
            }
        }),
    );
    *slot.borrow_mut() = Rc::downgrade(&s);
    s.register_automatic_clear(peer, 99);
    assert_eq!(s.automatic_clear_at(peer), 200);
    assert_eq!(s.clearing_remaining_time(), -1); // source cancels from its taken map's minWait=0
}
#[test]
fn emoji_collectible_tl_color_and_until_parse() {
    let (_scope, s, _, _, _, _) = emoji_model();
    let wire = tl::mtp_emoji_status_collectible(
        Default::default(),
        5,
        6,
        b"title".to_vec(),
        b"slug".to_vec(),
        7,
        0x123456,
        0x789abc,
        -1,
        0,
        Some(200),
    );
    let result = s.parse(&wire);
    assert_eq!(result.until, 200);
    let info = s.collectible_info(5).unwrap();
    assert_eq!(info.center_color, Some(0xff123456));
    assert_eq!(info.pattern_color, Some(u32::MAX));
    assert_eq!(info.text_color, Some(0xff000000));
    assert_eq!(result.id.collectible.unwrap().document_id, 6);
    let life = Lifetime::new();
    for producer in [
        s.recent_updates(),
        s.channel_default_updates(),
        s.collectibles_updates(),
    ] {
        producer.start(|_| {}, |_| {}, || {}, &life);
    }
}
#[test]
fn real_sender_adapters_deliver_typed_replies_without_network() {
    use std::sync::Mutex;
    use tg_mtproto::{
        mtp_instance::{get_next_request_id, Fields, Instance, Mode},
        mtproto_dc_options::Environment,
        mtproto_response::Response,
        sender::Sender,
    };
    let instance = Instance::new(Mode::Normal, Fields::new(Environment::Test));
    let sender = Sender::new(instance.clone());
    let top_replies = Arc::new(Mutex::new(Vec::new()));
    let queue = top_replies.clone();
    let id = get_next_request_id();
    top::send_request(
        &sender,
        id,
        top::Request::Get(tl::ContactsGetTopPeers::new(Default::default(), 0, 64, 0)),
        Arc::new(move |r| queue.lock().unwrap().push(r)),
    );
    assert!(sender.pending(id));
    instance.process_callback(&Response {
        request_id: id,
        outer_msg_id: 123u64 << 32,
        reply: tg_tl::serialize(&tl::mtp_contacts_top_peers_not_modified()),
    });
    assert!(
        matches!(&top_replies.lock().unwrap()[0],top::Reply::Loaded{outer_msg_id,..} if *outer_msg_id==123u64<<32)
    );
    let id = get_next_request_id();
    let queue = top_replies.clone();
    top::send_request(
        &sender,
        id,
        top::Request::Toggle(tl::ContactsToggleTopPeers::new(true)),
        Arc::new(move |r| queue.lock().unwrap().push(r)),
    );
    instance.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&true),
    });
    assert!(matches!(
        top_replies.lock().unwrap().last(),
        Some(top::Reply::ToggleDone)
    ));
    let replies = Arc::new(Mutex::new(Vec::new()));
    let queue = replies.clone();
    let id = get_next_request_id();
    emoji::send_request(
        &sender,
        id,
        Some(emoji::Type::Recent),
        None,
        emoji::Request::Recent(tl::AccountGetRecentEmojiStatuses::new(17)),
        Arc::new(move |r| queue.lock().unwrap().push(r)),
    );
    assert!(sender.pending(id));
    instance.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&tl::mtp_account_emoji_statuses_not_modified()),
    });
    assert!(matches!(
        &replies.lock().unwrap()[0],
        emoji::Reply::Statuses(emoji::Type::Recent, _)
    ));
    let cancelled = get_next_request_id();
    assert!(emoji::send_effect(
        &sender,
        &emoji::Effect::Send {
            id: cancelled,
            kind: Some(emoji::Type::Recent),
            peer: None,
            request: emoji::Request::Recent(tl::AccountGetRecentEmojiStatuses::new(0))
        },
        Arc::new(|_| panic!("cancelled callback"))
    ));
    assert!(sender.pending(cancelled));
    assert!(emoji::send_effect(
        &sender,
        &emoji::Effect::Cancel(cancelled),
        Arc::new(|_| panic!())
    ));
    assert!(!sender.pending(cancelled));
    assert!(!emoji::send_effect(
        &sender,
        &emoji::Effect::InvalidStatus,
        Arc::new(|_| panic!())
    ));
    drop(sender);
    instance.shutdown();
}
#[test]
fn emoji_binds_real_peers_changes_and_clear_effects() {
    use tg_core::data::{data_changes::Changes, data_session::Peers};
    let scope = SessionScope::new();
    let peers = Rc::new(Peers::new(&scope, Changes::new(&scope)));
    let peer = peers.ensure_peer(peer_from_user(1));
    let clock = Arc::new(ManualClock::new(0));
    let host = TimerHost::new(clock.clone());
    let now = Rc::new(Cell::new(100));
    let n = now.clone();
    let p = peers.clone();
    let service = emoji::EmojiStatuses::new(
        &scope,
        &host,
        Rc::new(move || n.get()),
        Rc::new(move |e| {
            emoji::apply_peer_status_effect(&p, &e);
        }),
    );
    let _life = service.bind_peer_effects(&peers);
    let id = EmojiStatusId {
        document_id: 123,
        collectible: None,
    };
    service.set_self(UserKey::try_from(peer).unwrap(), id.clone(), 110);
    assert_eq!(peers.read(peer, |p| p.emoji_status_id().clone()), id);
    assert_eq!(service.automatic_clear_at(peer), 110);
    now.set(110);
    clock.advance(10_000);
    host.process();
    assert!(!peers.read(peer, |p| p.emoji_status_id().is_present()));
    assert_eq!(service.automatic_clear_at(peer), 0);
}
