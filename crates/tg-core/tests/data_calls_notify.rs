use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::Arc,
};
use tg_base::timer::{ManualClock, TimerHost};
use tg_core::{
    core::{
        file_location::{FileInfo, FileLocation, FileLocations},
        mime_type::{MimeDatabase, MimeType},
    },
    data::{
        data_changes::Changes,
        data_file_origin::FileOrigin,
        data_group_call as gc, data_media_preload as preload,
        data_media_store::{Documents, Photos},
        data_peer::{LoadedStatus, PeerMutation},
        data_peer_id::*,
        data_photo_media::PhotoMedia,
        data_session::Peers,
        data_statistics_chart::*,
        data_store_keys::*,
        notify::{data_notify_settings as notify, data_peer_notify_settings::*},
    },
};
use tg_rpl::{EventStream, Lifetime, Producer};
use tg_runtime::MainQueue;
use tg_scheme as tl;
struct NoFiles;
impl FileLocations for NoFiles {
    fn inspect(&self, _: &FileLocation) -> FileInfo {
        panic!("file access outside this model test")
    }
    fn read(&self, _: (u64, u64)) -> FileLocation {
        FileLocation::default()
    }
}
struct NoMime;
impl MimeDatabase for NoMime {
    fn for_name(&self, _: &str) -> MimeType {
        panic!("MIME registry outside this attribute test")
    }
}
struct Owner {
    me: UserKey,
    clock: ManualClock,
    group: RefCell<Vec<gc::Effect>>,
    notify: RefCell<Vec<notify::Effect>>,
    joined: Cell<bool>,
    topics: RefCell<BTreeMap<TopicKey, PeerNotifySettings>>,
    histories: RefCell<BTreeMap<PeerKey, bool>>,
    destroyed: EventStream<TopicKey>,
    ringtones: EventStream<()>,
}
impl gc::GroupCallOwner for Owner {
    fn self_user(&self) -> UserKey {
        self.me
    }
    fn unix_now(&self) -> i32 {
        1000 + (self.clock_time() / 1000) as i32
    }
    fn in_call(&self, _: GroupCallKey) -> bool {
        self.joined.get()
    }
    fn can_manage(&self, _: PeerKey) -> bool {
        true
    }
    fn input_peer(&self, p: PeerKey) -> tl::InputPeer {
        input(p)
    }
    fn effect(&self, e: gc::Effect) {
        self.group.borrow_mut().push(e);
    }
}
impl Owner {
    fn clock_time(&self) -> i64 {
        use tg_base::timer::Clock;
        self.clock.now()
    }
    fn now(&self) -> i32 {
        1000 + (self.clock_time() / 1000) as i32
    }
}
impl notify::NotifyOwner for Owner {
    fn self_user(&self) -> UserKey {
        self.me
    }
    fn unix_now(&self) -> i32 {
        self.now()
    }
    fn user_inaccessible(&self, _: UserKey) -> bool {
        false
    }
    fn input_peer(&self, p: PeerKey) -> tl::InputPeer {
        input(p)
    }
    fn input_channel(&self, c: ChannelKey) -> tl::InputChannel {
        tl::mtp_input_channel(peer_to_channel(c.peer().id()).bare, 7)
    }
    fn topic_loaded(
        &self,
        p: PeerKey,
        root: tg_core::data::data_msg_id::MsgId,
    ) -> Option<TopicKey> {
        let t = TopicKey {
            history: p,
            root_id: root,
        };
        self.topics.borrow().contains_key(&t).then_some(t)
    }
    fn topic_settings(&self, t: TopicKey) -> PeerNotifySettings {
        self.topics.borrow()[&t].clone()
    }
    fn set_topic_settings(&self, t: TopicKey, s: PeerNotifySettings) {
        self.topics.borrow_mut().insert(t, s);
    }
    fn topic_destroyed(&self, t: TopicKey) -> Producer<()> {
        self.destroyed.events().filter(move |p| *p == t).map(|_| ())
    }
    fn forum_topics(&self, p: PeerKey) -> Vec<TopicKey> {
        self.topics
            .borrow()
            .keys()
            .filter(|t| t.history == p)
            .copied()
            .collect()
    }
    fn community_histories(&self, _: ChannelKey) -> Vec<PeerKey> {
        self.histories.borrow().keys().copied().collect()
    }
    fn history_muted(&self, p: PeerKey) -> Option<bool> {
        self.histories.borrow().get(&p).copied()
    }
    fn ringtone_list_updates(&self) -> Producer<()> {
        self.ringtones.events()
    }
    fn effect(&self, e: notify::Effect) {
        if let notify::Effect::SetHistoryMuted { peer, muted } = &e {
            self.histories.borrow_mut().insert(*peer, *muted);
        }
        self.notify.borrow_mut().push(e);
    }
}
fn input(p: PeerKey) -> tl::InputPeer {
    let id = p.id();
    if peer_is_user(id) {
        tl::mtp_input_peer_user(peer_to_user(id).bare, 7)
    } else if peer_is_chat(id) {
        tl::mtp_input_peer_chat(peer_to_chat(id).bare)
    } else {
        tl::mtp_input_peer_channel(peer_to_channel(id).bare, 7)
    }
}
struct Fixture {
    scope: SessionScope,
    queue: MainQueue,
    clock: ManualClock,
    host: Rc<TimerHost>,
    peers: Rc<Peers>,
    changes: Changes,
    docs: Rc<Documents>,
    owner: Rc<Owner>,
}
impl Fixture {
    fn new() -> Self {
        let scope = SessionScope::new();
        let queue = MainQueue::new();
        let clock = ManualClock::new(10000);
        let host = TimerHost::new(Arc::new(clock.clone()));
        let changes = Changes::with_queue(&scope, queue.clone());
        let peers = Rc::new(Peers::new(&scope, changes.clone()));
        let me = UserKey::try_from(peers.ensure_peer(peer_from_user(99))).unwrap();
        let owner = Rc::new(Owner {
            me,
            clock: clock.clone(),
            group: RefCell::default(),
            notify: RefCell::default(),
            joined: Cell::new(false),
            topics: RefCell::default(),
            histories: RefCell::default(),
            destroyed: EventStream::new(),
            ringtones: EventStream::new(),
        });
        let docs = Rc::new(Documents::new(&scope, Rc::new(NoFiles)));
        Self {
            scope,
            queue,
            clock,
            host,
            peers,
            changes,
            docs,
            owner,
        }
    }
    fn peer(&self, id: PeerId) -> PeerKey {
        let p = self.peers.ensure_peer(id);
        self.peers.update(p, |p| {
            p.set_loaded_status(LoadedStatus::Normal);
            PeerMutation::quiet(())
        });
        p
    }
    fn call(&self, origin: gc::GroupCallOrigin) -> Rc<gc::GroupCall> {
        let peer = self.peer(peer_from_chat(7));
        let call = self.scope.group_call_key(42);
        self.peers
            .set_group_call(peer, Some(&tl::mtp_input_group_call(42, 8)), 0, false);
        gc::GroupCall::new(
            call,
            peer,
            8,
            50,
            false,
            origin,
            self.peers.clone(),
            self.owner.clone(),
            self.host.clone(),
        )
    }
    fn notify(&self) -> Rc<notify::NotifySettings> {
        notify::NotifySettings::new(
            self.peers.clone(),
            self.docs.clone(),
            self.owner.clone(),
            self.queue.clone(),
            self.host.clone(),
        )
    }
}
fn collect<T: Clone + 'static>(p: Producer<T>, life: &Lifetime) -> Rc<RefCell<Vec<T>>> {
    let out = Rc::new(RefCell::new(vec![]));
    let v = out.clone();
    p.start(move |x| v.borrow_mut().push(x), |_| {}, || {}, life);
    out
}
fn participant(id: u64, flags: u64, ssrc: i32, volume: Option<i32>) -> tl::GroupCallParticipant {
    tl::GroupCallParticipant::GroupCallParticipant(
        tl::GroupCallParticipantData {
            flags: tl::tl::Flags::from_raw(flags),
            peer: tl::mtp_peer_user(id),
            date: id as i32,
            active_date: None,
            source: ssrc,
            volume,
            about: None,
            raise_hand_rating: None,
            video: None,
            presentation: None,
            paid_stars_total: None,
        }
        .into(),
    )
}
fn call_fields(version: i32, flags: u64, count: i32) -> tl::GroupCallData {
    tl::GroupCallData {
        flags: tl::tl::Flags::from_raw(flags),
        id: 42,
        access_hash: 8,
        participants_count: count,
        title: Some(b"Call".to_vec()),
        stream_dc_id: None,
        record_start_date: Some(123),
        schedule_date: Some(456),
        unmuted_video_count: None,
        unmuted_video_limit: 9,
        version,
        invite_link: Some(b"invite".to_vec()),
        send_paid_messages_stars: Some(12),
        default_send_as: Some(tl::mtp_peer_user(99)),
    }
}
fn call_update(version: i32) -> tl::Update {
    tl::Update::UpdateGroupCall(
        tl::UpdateGroupCallData {
            flags: Default::default(),
            peer: None,
            call: tl::GroupCall::GroupCall(call_fields(version, 0, 0).into()),
        }
        .into(),
    )
}
fn participants_update(version: i32, list: Vec<tl::GroupCallParticipant>) -> tl::Update {
    tl::Update::UpdateGroupCallParticipants(
        tl::UpdateGroupCallParticipantsData {
            call: tl::mtp_input_group_call(42, 8),
            participants: list,
            version,
        }
        .into(),
    )
}
fn full(version: i32, count: i32, list: Vec<tl::GroupCallParticipant>) -> tl::PhoneGroupCall {
    tl::PhoneGroupCall::PhoneGroupCall(
        tl::PhoneGroupCallData {
            call: tl::GroupCall::GroupCall(call_fields(version, 0, count).into()),
            participants: list,
            participants_next_offset: b"next".to_vec(),
            chats: vec![],
            users: vec![],
        }
        .into(),
    )
}
fn page(count: i32, list: Vec<tl::GroupCallParticipant>) -> tl::PhoneGroupParticipants {
    tl::PhoneGroupParticipants::PhoneGroupParticipants(
        tl::PhoneGroupParticipantsData {
            count,
            participants: list,
            next_offset: vec![],
            chats: vec![],
            users: vec![],
            version: 1,
        }
        .into(),
    )
}
#[test]
fn group_constructor_values_call_fields_and_conference_local_override() {
    let f = Fixture::new();
    let c = f.call(gc::GroupCallOrigin::Conference);
    let life = Lifetime::new();
    let loaded = collect(c.loaded_value(), &life);
    let titles = collect(c.title_value(), &life);
    let _ = collect(c.record_start_date_value(), &life);
    let _ = collect(c.schedule_date_value(), &life);
    let _ = collect(c.schedule_start_subscribed_value(), &life);
    let _ = collect(c.messages_enabled_value(), &life);
    let _ = collect(c.messages_min_price_value(), &life);
    let _ = collect(c.send_as_value(), &life);
    let _ = collect(c.full_count_value(), &life);
    let _ = collect(c.record_start_date_changes(), &life);
    let _ = collect(c.schedule_date_changes(), &life);
    assert_eq!(c.key(), f.scope.group_call_key(42));
    assert_eq!(c.session(), f.scope.id());
    assert_eq!(c.id(), 42);
    assert_eq!(c.origin(), gc::GroupCallOrigin::Conference);
    assert!(c.blockchain_may_be_empty());
    assert!(!c.can_manage());
    assert!(matches!(f.owner.group.borrow()[0], gc::Effect::Register(_)));
    let d = call_fields(
        0,
        (1 << 14) | (1 << 15) | (1 << 17) | (1 << 11) | (1 << 8),
        3,
    );
    c.apply_call_fields(&d);
    assert_eq!(*loaded.borrow(), vec![false, true]);
    assert!(c.loaded());
    assert!(c.creator());
    assert!(c.can_manage());
    assert!(!c.can_change_join_muted());
    assert!(!c.join_muted());
    assert!(c.messages_enabled());
    c.set_messages_enabled_locally(false);
    c.apply_call_fields(&call_fields(2, d.flags.value(), 3));
    assert!(!c.messages_enabled());
    assert!(c.can_change_messages_enabled());
    assert_eq!(c.record_start_date(), 123);
    assert_eq!(c.schedule_date(), 456);
    assert!(c.schedule_start_subscribed());
    assert_eq!(c.messages_min_price(), 12);
    assert_eq!(c.unmuted_video_limit(), 9);
    assert!(c.record_video());
    assert_eq!(c.conference_invite_link(), "invite");
    assert_eq!(c.resolve_send_as(), f.owner.me.peer());
    assert!(c.joined_to_top());
    c.set_title("local".into());
    assert_eq!(titles.borrow().last().unwrap(), "local");
    c.set_join_muted_locally(true);
    assert!(c.join_muted());
    assert!(!c.rtmp());
    assert!(!c.listeners_hidden());
    assert!(!c.blockchain_may_be_empty());
    assert_eq!(*collect(c.loaded_value(), &life).borrow(), vec![true]);
    c.save_send_as(f.owner.me.peer());
    assert!(matches!(
        f.owner.group.borrow().last(),
        Some(gc::Effect::Send {
            request: gc::Request::SaveSendAs(_),
            ..
        })
    ));
    drop(c);
    assert!(f
        .owner
        .group
        .borrow()
        .iter()
        .any(|e| matches!(e, gc::Effect::Unregister(_))));
}
#[test]
fn participant_order_min_volume_left_missing_and_events_release_borrow() {
    let f = Fixture::new();
    let c = f.call(gc::GroupCallOrigin::Group);
    c.apply_call_fields(&call_fields(1, 0, 5));
    let life = Lifetime::new();
    let seen = collect(c.participant_updated(), &life);
    let weak = Rc::downgrade(&c);
    c.participant_updated().start(
        move |_| {
            assert!(weak.upgrade().unwrap().participants().len() <= 3);
        },
        |_| {},
        || {},
        &life,
    );
    c.apply_participants_slice(
        &[
            participant(3, 0, 30, Some(3000)),
            participant(1, 0, 10, Some(1000)),
            participant(2, 0, 20, None),
        ],
        gc::ApplySliceSource::SliceLoaded,
    );
    assert_eq!(
        c.participants()
            .iter()
            .map(|p| peer_to_user(p.peer.id()).bare)
            .collect::<Vec<_>>(),
        vec![3, 1, 2]
    );
    let p = c.participants()[0].peer;
    c.apply_participants_slice(
        &[participant(3, (1 << 8) | (1 << 9), 31, Some(9000))],
        gc::ApplySliceSource::UpdateReceived,
    );
    let v = c.participant_by_peer(p).unwrap();
    assert_eq!(v.volume, 3000);
    assert!(!v.muted_by_me);
    assert!(!v.only_min_loaded);
    assert_eq!(c.participant_peer_by_audio_ssrc(30), None);
    assert_eq!(c.participant_peer_by_audio_ssrc(31), Some(p));
    assert_eq!(c.full_count(), 5);
    c.apply_participants_slice(
        &[participant(8, 1 << 1, 80, None)],
        gc::ApplySliceSource::UpdateReceived,
    );
    assert_eq!(c.full_count(), 4);
    c.apply_participants_slice(
        &[participant(3, 1 << 1, 31, None)],
        gc::ApplySliceSource::UpdateReceived,
    );
    assert_eq!(c.full_count(), 3);
    assert_eq!(c.participants().len(), 2);
    assert!(seen.borrow().last().unwrap().now.is_none());
    assert!(c.participant_by_peer(p).is_none());
}
#[test]
fn group_speaking_before_mutation_sound_boundary_and_active_virtual_timer() {
    let f = Fixture::new();
    let c = f.call(gc::GroupCallOrigin::Group);
    c.apply_participants_slice(
        &[participant(1, 0, 10, None)],
        gc::ApplySliceSource::SliceLoaded,
    );
    let peer = c.participants()[0].peer;
    let life = Lifetime::new();
    let states = Rc::new(RefCell::new(vec![]));
    let v = states.clone();
    let w = Rc::downgrade(&c);
    c.participant_speaking().start(
        move |p| {
            v.borrow_mut().push(
                w.upgrade()
                    .unwrap()
                    .participant_by_peer(p)
                    .unwrap()
                    .speaking,
            )
        },
        |_| {},
        || {},
        &life,
    );
    let updates = collect(c.participant_updated(), &life);
    let when = gc::LastSpokeTimes {
        anything: 10000,
        voice: 10000,
    };
    c.apply_last_spoke(10, when, 11500);
    c.apply_last_spoke(10, when, 11500);
    assert_eq!(*states.borrow(), vec![false, true]);
    assert_eq!(updates.borrow().len(), 1);
    c.apply_last_spoke(10, when, 11501);
    assert!(!c.participant_by_peer(peer).unwrap().speaking);
    c.apply_active_update(peer.id(), when, Some(peer));
    assert!(c.participant_by_peer(peer).unwrap().speaking);
    f.clock.advance(5999);
    f.host.process();
    assert!(c.participant_by_peer(peer).unwrap().speaking);
    f.clock.advance(1);
    f.host.process();
    assert!(!c.participant_by_peer(peer).unwrap().speaking);
    f.clock.advance(1000);
    c.apply_active_update(
        peer.id(),
        gc::LastSpokeTimes {
            anything: 17000,
            voice: 17000,
        },
        Some(peer),
    );
    c.set_in_call();
    f.clock.advance(1000);
    f.host.process();
    assert!(!c.participant_by_peer(peer).unwrap().speaking);
    f.owner.joined.set(true);
    c.apply_active_update(
        peer.id(),
        gc::LastSpokeTimes {
            anything: 18000,
            voice: 18000,
        },
        Some(peer),
    );
    assert!(!c.participant_by_peer(peer).unwrap().speaking);
}
#[test]
fn group_queue_initial_conference_exception_gap_reload_and_stale_timeout() {
    let f = Fixture::new();
    let c = f.call(gc::GroupCallOrigin::Conference);
    c.enqueue_update(&participants_update(
        2,
        vec![participant(1, 1 << 5, 10, None)],
    ));
    assert!(c.participants().is_empty());
    c.enqueue_update(&call_update(2));
    assert_eq!(c.participants().len(), 1);
    c.enqueue_update(&participants_update(
        2,
        vec![participant(2, 1 << 5, 20, None)],
    ));
    assert_eq!(c.participants().len(), 1);
    c.enqueue_update(&participants_update(
        4,
        vec![participant(3, 1 << 5, 30, None)],
    ));
    f.clock.advance(2999);
    f.host.process();
    assert!(!f.owner.group.borrow().iter().any(|e| matches!(
        e,
        gc::Effect::Send {
            request: gc::Request::Reload(_),
            ..
        }
    )));
    f.clock.advance(1);
    f.host.process();
    assert_eq!(c.participants().len(), 2);
    let id = f
        .owner
        .group
        .borrow()
        .iter()
        .find_map(|e| {
            if let gc::Effect::Send {
                id,
                request: gc::Request::Reload(_),
            } = e
            {
                Some(*id)
            } else {
                None
            }
        })
        .unwrap();
    c.handle_reply(id, gc::Reply::Reload(Err(())));
    let n = f.owner.group.borrow().len();
    f.clock.advance(16000);
    c.reload_if_stale();
    assert_eq!(f.owner.group.borrow().len(), n);
    f.clock.advance(1);
    c.reload_if_stale();
    assert!(f.owner.group.borrow().len() > n);
}
#[test]
fn group_page_reload_saved_full_failure_and_access_stale_double_fire() {
    let f = Fixture::new();
    let c = f.call(gc::GroupCallOrigin::Conference);
    let life = Lifetime::new();
    let stale = collect(c.stale_participant_ids(), &life);
    let _ = collect(c.participants_with_access_value(), &life);
    let reloads = collect(c.participants_reloaded(), &life);
    c.process_full_call(&full(
        1,
        3,
        vec![
            participant(1, 0, 10, None),
            participant(2, 0, 20, None),
            participant(3, 0, 30, None),
        ],
    ));
    assert_eq!(c.full_count(), 3);
    assert!(c.participants_loaded());
    assert_eq!(reloads.borrow().len(), 1);
    c.set_participants_with_access([UserId::new(99), UserId::new(4)].into_iter().collect());
    assert_eq!(stale.borrow().len(), 2);
    assert_eq!(c.participants_with_access_current().len(), 2);
    c.reload();
    let id = f
        .owner
        .group
        .borrow()
        .iter()
        .rev()
        .find_map(|e| {
            if let gc::Effect::Send {
                id,
                request: gc::Request::Reload(_),
            } = e
            {
                Some(*id)
            } else {
                None
            }
        })
        .unwrap();
    let response = full(2, 5, vec![participant(1, 0, 10, None)]);
    assert!(c.request_participants_after_reload(&response));
    c.handle_reply(id, gc::Reply::Reload(Ok(response)));
    let id = f
        .owner
        .group
        .borrow()
        .iter()
        .rev()
        .find_map(|e| {
            if let gc::Effect::Send {
                id,
                request: gc::Request::Participants(_),
            } = e
            {
                Some(*id)
            } else {
                None
            }
        })
        .unwrap();
    c.handle_reply(
        id,
        gc::Reply::Participants(Ok(page(5, vec![participant(2, 0, 20, None)]))),
    );
    assert_eq!(c.participants().len(), 2);
    assert_eq!(c.full_count(), 5);
    assert_eq!(reloads.borrow().len(), 2);
    c.request_participants();
    let id = f
        .owner
        .group
        .borrow()
        .iter()
        .rev()
        .find_map(|e| {
            if let gc::Effect::Send {
                id,
                request: gc::Request::Participants(_),
            } = e
            {
                Some(*id)
            } else {
                None
            }
        })
        .unwrap();
    c.handle_reply(id, gc::Reply::Participants(Err(())));
    assert!(c.participants_loaded());
    assert_eq!(c.full_count(), 2);
}
#[test]
fn group_unknown_batches_highest_fifty_resolved_and_failure() {
    let f = Fixture::new();
    let c = f.call(gc::GroupCallOrigin::Group);
    let life = Lifetime::new();
    let resolved = collect(c.participants_resolved(), &life);
    c.resolve_participants(&(1..=52).collect());
    let (id, sources) = f
        .owner
        .group
        .borrow()
        .iter()
        .find_map(|e| {
            if let gc::Effect::Send {
                id,
                request: gc::Request::Participants(r),
            } = e
            {
                Some((*id, r.sources.clone()))
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(sources.len(), 50);
    assert_eq!(sources[0], 3);
    assert_eq!(sources[49], 52);
    c.handle_reply(id, gc::Reply::Participants(Ok(page(0, vec![]))));
    assert_eq!(resolved.borrow()[0].len(), 50);
    let id = f
        .owner
        .group
        .borrow()
        .iter()
        .rev()
        .find_map(|e| {
            if let gc::Effect::Send {
                id,
                request: gc::Request::Participants(_),
            } = e
            {
                Some(*id)
            } else {
                None
            }
        })
        .unwrap();
    c.handle_reply(id, gc::Reply::Participants(Err(())));
    let n = f.owner.group.borrow().len();
    c.resolve_participants(&BTreeSet::new());
    assert_eq!(n, f.owner.group.borrow().len());
}
fn settings(
    mute: Option<i32>,
    silent: Option<bool>,
    sound: Option<tl::NotificationSound>,
) -> tl::PeerNotifySettings {
    let mut flags = 0;
    if mute.is_some() {
        flags |= 1 << 2;
    }
    if silent.is_some() {
        flags |= 1 << 1;
    }
    if sound.is_some() {
        flags |= 1 << 5;
    }
    tl::PeerNotifySettings::PeerNotifySettings(
        tl::PeerNotifySettingsData {
            flags: tl::tl::Flags::from_raw(flags),
            show_previews: None,
            silent,
            mute_until: mute,
            ios_sound: None,
            android_sound: None,
            other_sound: sound,
            stories_muted: None,
            stories_hide_sender: None,
            stories_ios_sound: None,
            stories_android_sound: None,
            stories_other_sound: None,
        }
        .into(),
    )
}
#[test]
fn notify_defaults_inheritance_known_bits_self_sound_and_virtual_unmute() {
    let f = Fixture::new();
    let n = f.notify();
    let p = f.peer(peer_from_user(1));
    let me = f.peer(peer_from_user(99));
    assert_eq!(
        notify::default_notify_type(&f.peers, p),
        DefaultNotify::User
    );
    assert!(n.is_muted(notify::Target::Peer(p)));
    assert!(n.settings_unknown(notify::Target::Peer(p)));
    assert_eq!(n.is_muted_with_delay(notify::Target::Peer(p)), (true, 0));
    n.apply(notify::Target::Peer(p), &settings(None, None, None));
    n.apply(
        notify::Target::Peer(me),
        &settings(None, None, Some(tl::mtp_notification_sound_none())),
    );
    assert!(!n.sound_unknown(notify::Target::Peer(me)));
    assert_eq!(n.sound(notify::Target::Peer(me)), NotifySound::default());
    let t = notify::Target::Default(DefaultNotify::User);
    let life = Lifetime::new();
    let events = collect(n.default_updates(DefaultNotify::User), &life);
    let _ = collect(n.default_updates_for(p), &life);
    n.apply(
        t,
        &settings(
            Some(0),
            Some(false),
            Some(tl::mtp_notification_sound_ringtone(23)),
        ),
    );
    assert_eq!(events.borrow().len(), 1);
    assert!(!n.is_muted(notify::Target::Peer(p)));
    assert!(!n.mute_unknown(notify::Target::Peer(p)));
    assert!(!n.silent_posts_unknown(p));
    assert!(!n.silent_posts(p));
    assert_eq!(n.sound(notify::Target::Peer(me)).id, 23);
    f.owner.histories.borrow_mut().insert(p, false);
    n.update(
        notify::Target::Peer(p),
        MuteValue {
            period: 10,
            ..Default::default()
        },
        Some(true),
        None,
        None,
    );
    assert!(n.is_muted(notify::Target::Peer(p)));
    assert!(n.silent_posts(p));
    assert!(f.owner.histories.borrow()[&p]);
    f.clock.advance(9999);
    f.host.process();
    assert!(n.is_muted(notify::Target::Peer(p)));
    f.clock.advance(1);
    f.host.process();
    assert!(!n.is_muted(notify::Target::Peer(p)));
    // Unix seconds truncate independently of the millisecond timer deadline.
    assert!(f.owner.histories.borrow()[&p]);
    f.clock.advance(999);
    f.host.process();
    assert!(!f.owner.histories.borrow()[&p]);
    assert_eq!(notify::muted_from_until(i32::MAX, 0), (true, 86400000));
    assert_eq!(notify::muted_from_until(1, 2), (false, 86400000));
    n.update(
        t,
        MuteValue {
            unmute: true,
            ..Default::default()
        },
        Some(true),
        None,
        None,
    );
    assert!(matches!(
        f.owner.notify.borrow().last(),
        Some(notify::Effect::UpdateDelayed(notify::Target::Default(
            DefaultNotify::User
        )))
    ));
}
#[test]
fn notify_exceptions_realtime_before_bundled_flush_reset_clear_and_request_latch() {
    let f = Fixture::new();
    let n = f.notify();
    let a = f.peer(peer_from_user(2));
    let b = f.peer(peer_from_user(3));
    let life = Lifetime::new();
    let realtime = collect(n.exceptions_updates_realtime(), &life);
    let postponed = collect(n.exceptions_updates(), &life);
    n.update(
        notify::Target::Peer(a),
        MuteValue {
            forever: true,
            ..Default::default()
        },
        None,
        None,
        None,
    );
    n.update(
        notify::Target::Peer(b),
        MuteValue {
            unmute: true,
            ..Default::default()
        },
        None,
        None,
        None,
    );
    assert_eq!(realtime.borrow().len(), 2);
    assert!(postponed.borrow().is_empty());
    assert_eq!(
        n.exceptions(DefaultNotify::User),
        [a, b].into_iter().collect()
    );
    f.queue.process_all();
    assert_eq!(*postponed.borrow(), vec![DefaultNotify::User]);
    n.reset_to_default(notify::Target::Peer(a));
    assert_eq!(n.exceptions(DefaultNotify::User), BTreeSet::from([b]));
    n.clear_exceptions(DefaultNotify::User);
    assert!(n.exceptions(DefaultNotify::User).is_empty());
    let count = f.owner.notify.borrow().len();
    n.clear_exceptions(DefaultNotify::User);
    assert_eq!(f.owner.notify.borrow().len(), count);
    n.load_exceptions();
    n.load_exceptions();
    assert_eq!(
        f.owner
            .notify
            .borrow()
            .iter()
            .filter(|e| matches!(
                e,
                notify::Effect::Send {
                    request: notify::Request::Exceptions(_),
                    ..
                }
            ))
            .count(),
        3
    );
}
#[test]
fn notify_request_dedup_user_one_collision_and_input_notify_tl_variants() {
    let f = Fixture::new();
    let n = f.notify();
    let p = f.peer(peer_from_user(1));
    n.request(p);
    n.request(p);
    assert_eq!(
        f.owner
            .notify
            .borrow()
            .iter()
            .filter(|e| matches!(e, notify::Effect::Send { .. }))
            .count(),
        1
    );
    let input = tl::mtp_input_notify_peer(tl::mtp_input_peer_user(1, 7));
    n.handle_reply(notify::Reply::Settings {
        peer: input,
        settings: settings(None, None, None),
    });
    n.request(p);
    assert_eq!(
        f.owner
            .notify
            .borrow()
            .iter()
            .filter(|e| matches!(e, notify::Effect::Send { .. }))
            .count(),
        2
    );
    for (kind, wire) in [
        (DefaultNotify::User, tl::mtp_notify_users()),
        (DefaultNotify::Group, tl::mtp_notify_chats()),
        (DefaultNotify::Broadcast, tl::mtp_notify_broadcasts()),
    ] {
        n.apply_notify_peer(&wire, &settings(Some(0), None, None));
        assert!(!n.default_settings(kind).settings_unknown());
        assert!(!n.is_muted(notify::Target::Default(kind)));
        let _ = notify::default_notify_to_mtp(kind);
    }
    n.apply_notify_peer(
        &tl::mtp_notify_peer(tl::mtp_peer_user(1)),
        &settings(Some(i32::MAX), None, None),
    );
    assert!(n.is_muted(notify::Target::Peer(p)));
    n.apply_input(
        &tl::mtp_input_notify_peer(tl::mtp_input_peer_self()),
        &settings(Some(0), None, None),
    );
    n.apply_peer_id(peer_from_user(888), &settings(Some(0), None, None));
}
#[test]
fn notify_topic_keys_parent_override_and_destroyed_lifetime() {
    let f = Fixture::new();
    let n = f.notify();
    let p = f.peer(peer_from_channel(5));
    let topic = TopicKey {
        history: p,
        root_id: 77.into(),
    };
    f.owner
        .topics
        .borrow_mut()
        .insert(topic, PeerNotifySettings::new());
    assert!(n.mute_unknown(notify::Target::Topic(topic)));
    assert!(n.sound_unknown(notify::Target::Topic(topic)));
    n.request_thread(notify::Target::Topic(topic));
    assert!(f.owner.notify.borrow().iter().any(|e| matches!(
        e,
        notify::Effect::Send {
            request: notify::Request::Settings(tl::AccountGetNotifySettings {
                peer: tl::InputNotifyPeer::InputNotifyForumTopic(_)
            }),
            ..
        }
    )));
    n.apply(
        notify::Target::Peer(p),
        &settings(Some(0), None, Some(tl::mtp_notification_sound_none())),
    );
    n.apply_topic_id(p.id(), 77.into(), &settings(None, None, None));
    assert!(!n.is_muted(notify::Target::Topic(topic)));
    assert!(n.sound(notify::Target::Topic(topic)).none);
    assert!(!n.mute_unknown(notify::Target::Topic(topic)));
    assert!(!n.sound_unknown(notify::Target::Topic(topic)));
    n.update(
        notify::Target::Topic(topic),
        MuteValue {
            period: 1,
            ..Default::default()
        },
        None,
        None,
        None,
    );
    assert!(n.is_muted(notify::Target::Topic(topic)));
    n.forum_parent_mute_updated(p);
    f.owner.destroyed.fire(topic);
    f.owner.topics.borrow_mut().remove(&topic);
    f.clock.advance(2000);
    f.host.process();
}
fn video(docs: &Documents, id: u64, size: i64, prefix: i32) -> DocumentRecordKey {
    let k = docs.ensure_document(id);
    docs.edit(k, |d| {
        d.size = size;
        d.set_remote_location(2, 3, &[], &NoFiles);
        d.set_attributes(
            &[tl::DocumentAttribute::DocumentAttributeVideo(
                tl::DocumentAttributeVideoData {
                    flags: tl::DocumentAttributeVideoData::F_SUPPORTS_STREAMING,
                    duration: 1.,
                    w: 100,
                    h: 100,
                    preload_prefix_size: Some(prefix),
                    video_start_ts: None,
                    video_codec: None,
                }
                .into(),
            )],
            &NoMime,
        );
    });
    k
}
#[test]
fn notify_ringtone_pending_list_cache_leases_and_null_document() {
    let f = Fixture::new();
    let n = f.notify();
    let p = f.peer(peer_from_user(1));
    n.apply(
        notify::Target::Peer(p),
        &settings(None, None, Some(tl::mtp_notification_sound_ringtone(10))),
    );
    assert!(n.lookup_ringtone(10).is_none());
    assert_eq!(
        f.owner
            .notify
            .borrow()
            .iter()
            .filter(|e| matches!(e, notify::Effect::RequestRingtoneList))
            .count(),
        1
    );
    n.apply(
        notify::Target::Default(DefaultNotify::User),
        &settings(None, None, Some(tl::mtp_notification_sound_ringtone(10))),
    );
    assert_eq!(
        f.owner
            .notify
            .borrow()
            .iter()
            .filter(|e| matches!(e, notify::Effect::RequestRingtoneList))
            .count(),
        1
    );
    let doc = video(&f.docs, 10, 100, 10);
    f.owner.ringtones.fire(());
    let lease = n.lookup_ringtone(10).unwrap();
    assert!(f.docs.read(doc, |d| d.save_to_cache()).is_some());
    n.cache_sound_id(10);
    n.cache_sound(doc);
    n.cache_sound_id(0);
    assert!(n.lookup_ringtone(0).is_none());
    drop(n);
    lease.borrow_mut().set_bytes(b"retained");
    assert_eq!(lease.borrow().bytes(), b"retained");
}
#[test]
fn video_preload_cached_disallowed_repeat_done_and_drop_cancel() {
    let f = Fixture::new();
    let effects = Rc::new(RefCell::new(vec![]));
    let done = Rc::new(Cell::new(0));
    let e = effects.clone();
    let d = done.clone();
    let empty = f.docs.ensure_document(1);
    assert!(!f.docs.read(empty, preload::VideoPreload::can).unwrap());
    let p = preload::VideoPreload::new(
        f.docs.clone(),
        empty,
        FileOrigin::default(),
        Rc::new(move |x| e.borrow_mut().push(x)),
        Rc::new(move || d.set(d.get() + 1)),
    );
    assert_eq!(done.get(), 1);
    assert!(!p.ready_to_request());
    drop(p);
    assert!(matches!(
        effects.borrow().last(),
        Some(preload::VideoPreloadEffect::CancelAllRequests)
    ));
    let k = video(&f.docs, 2, 1000000, 0);
    let e = effects.clone();
    let d = done.clone();
    let p = preload::VideoPreload::new(
        f.docs.clone(),
        k,
        FileOrigin::default(),
        Rc::new(move |x| e.borrow_mut().push(x)),
        Rc::new(move || d.set(d.get() + 1)),
    );
    assert_eq!(p.video(), k);
    assert!(matches!(
        effects.borrow().last(),
        Some(preload::VideoPreloadEffect::CheckCache(_))
    ));
    p.cache_result(b"any cached data");
    p.cache_result(b"again");
    assert_eq!(done.get(), 3);
    assert_eq!(
        f.docs.read(k, preload::VideoPreload::choose_preload_prefix),
        Some(1000000)
    );
}
#[test]
fn video_preload_source_early_finish_partial_map_marker_and_fail_hooks() {
    let f = Fixture::new();
    let k = video(&f.docs, 2, 3 * i64::from(preload::DOWNLOAD_PART_SIZE), 0);
    let effects = Rc::new(RefCell::new(vec![]));
    let done = Rc::new(Cell::new(0));
    let e = effects.clone();
    let d = done.clone();
    let p = preload::VideoPreload::new(
        f.docs.clone(),
        k,
        FileOrigin::default(),
        Rc::new(move |x| e.borrow_mut().push(x)),
        Rc::new(move || d.set(d.get() + 1)),
    );
    p.cache_result(&[]);
    assert!(p.ready_to_request());
    assert_eq!(p.take_next_request_offset(), 0);
    assert_eq!(
        p.take_next_request_offset(),
        i64::from(preload::DOWNLOAD_PART_SIZE)
    );
    p.feed_part(0, b"a");
    assert_eq!(done.get(), 0);
    p.feed_part(i64::from(preload::DOWNLOAD_PART_SIZE), b"b");
    assert_eq!(done.get(), 1);
    assert!(p.ready_to_request());
    let bytes = effects
        .borrow()
        .iter()
        .find_map(|e| {
            if let preload::VideoPreloadEffect::PutIfEmpty { bytes, .. } = e {
                Some(bytes.clone())
            } else {
                None
            }
        })
        .unwrap();
    let parts = BTreeMap::from([
        (0, b"a".to_vec()),
        (preload::DOWNLOAD_PART_SIZE as u32, b"b".to_vec()),
        (2 * preload::DOWNLOAD_PART_SIZE as u32, vec![]),
    ]);
    assert_eq!(bytes, preload::serialize_complex_parts_map(&parts));
    let k = video(&f.docs, 3, 16, 16);
    let e = effects.clone();
    let d = done.clone();
    let p = preload::VideoPreload::new(
        f.docs.clone(),
        k,
        FileOrigin::default(),
        Rc::new(move |x| e.borrow_mut().push(x)),
        Rc::new(move || d.set(d.get() + 1)),
    );
    p.load();
    assert_eq!(p.take_next_request_offset(), 0);
    p.feed_part(0, b"1234");
    let bytes = effects
        .borrow()
        .iter()
        .rev()
        .find_map(|e| {
            if let preload::VideoPreloadEffect::PutIfEmpty { bytes, .. } = e {
                Some(bytes.clone())
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(bytes.len(), 17);
    assert_eq!(bytes[16], 0);
    p.cancel_on_fail();
    assert!(!p.ready_to_request());
    assert!(!p.set_web_file_size_hook(500));
}
struct PhotoOwner {
    loaded: Cell<bool>,
    updates: EventStream<()>,
    load: Cell<usize>,
    cancel: Cell<usize>,
}
impl preload::PhotoPreloadOwner for PhotoOwner {
    fn loaded(&self, _: PhotoRecordKey, _: &Rc<RefCell<PhotoMedia>>) -> bool {
        self.loaded.get()
    }
    fn load(&self, _: PhotoRecordKey, _: FileOrigin) {
        self.load.set(self.load.get() + 1);
    }
    fn cancel(&self, _: PhotoRecordKey) {
        self.cancel.set(self.cancel.get() + 1);
    }
    fn downloader_task_finished(&self) -> Producer<()> {
        self.updates.events()
    }
}
#[test]
fn photo_preload_actual_byte_lease_loaded_contract_done_repeats_cancel() {
    let f = Fixture::new();
    let photos = Photos::new(&f.scope);
    let key = photos.ensure_photo(1);
    let owner = Rc::new(PhotoOwner {
        loaded: Cell::new(false),
        updates: EventStream::new(),
        load: Cell::new(0),
        cancel: Cell::new(0),
    });
    let done = Rc::new(Cell::new(0));
    let d = done.clone();
    let p = preload::PhotoPreload::new(
        &photos,
        key,
        FileOrigin::default(),
        owner.clone(),
        Rc::new(move || d.set(d.get() + 1)),
    );
    assert_eq!(owner.load.get(), 1);
    let lease = p.lease();
    owner.updates.fire(());
    assert_eq!(done.get(), 0);
    owner.loaded.set(true);
    owner.updates.fire(());
    owner.updates.fire(());
    assert_eq!(done.get(), 2);
    drop(p);
    owner.updates.fire(());
    assert_eq!(done.get(), 2);
    assert_eq!(owner.cancel.get(), 1);
    let d = done.clone();
    let p = preload::PhotoPreload::new(
        &photos,
        key,
        FileOrigin::default(),
        owner.clone(),
        Rc::new(move || d.set(d.get() + 1)),
    );
    assert!(Rc::ptr_eq(&lease, &p.lease()));
    assert_eq!(done.get(), 3);
    let called = Rc::new(Cell::new(0));
    let c = called.clone();
    let m = preload::MediaPreload::new(Rc::new(move || c.set(c.get() + 1)));
    m.call_done();
    m.call_done();
    assert_eq!(called.get(), 2);
}
#[test]
fn chart_model_measure_requests_extrema_accumulate_day_lookup_and_colors_data() {
    use tg_core::statistics::statistics_data_deserialize::statistical_chart_from_json;
    let mut c=statistical_chart_from_json(br##"{"columns":[["x",1000,2000],["a",1,5]],"colors":{"a":"theme##123"},"names":{"a":"a-b"}}"##);
    assert!(!c.empty());
    assert_eq!(c.lines[0].color_key, "theme#");
    assert_eq!(c.lines[0].color_spec.as_deref(), Some("#123"));
    assert_eq!(c.lines[0].name, "a—b");
    let r = c.measure_data().unwrap();
    assert_eq!(r.count, 11);
    c.days_lookup = vec!["start".into(), "end".into()];
    assert_eq!(c.get_day_string(0), "start");
    assert_eq!(c.get_day_string(1), "end");
    c.lines[0].max_value = 2;
    c.lines[0].min_value = 3;
    c.measure_data();
    assert_eq!(c.max_value, 5);
    assert_eq!(c.min_value, 1);
    assert_eq!(c.lines[0].segment_tree.r_max_q(0, 1), 5);
    assert!(!c.lines[0].segment_tree.empty());
    assert!(StatisticalGraph::default().chart.empty());
}
fn video_params(endpoint: &[u8], ssrc: i32, paused: bool) -> tl::GroupCallParticipantVideo {
    tl::GroupCallParticipantVideo::GroupCallParticipantVideo(
        tl::GroupCallParticipantVideoData {
            flags: tl::tl::Flags::from_raw(u64::from(paused)),
            endpoint: endpoint.to_vec(),
            source_groups: vec![tl::mtp_group_call_participant_video_source_group(
                b"SIM".to_vec(),
                vec![1, 2],
            )],
            audio_source: Some(ssrc),
        }
        .into(),
    )
}
#[test]
fn group_video_data_pointer_reuse_local_update_endpoints_and_ssrc_collisions() {
    assert_eq!(gc::rtmp_endpoint_id(), b"unified");
    let camera = video_params(b"cam", 11, true);
    let screen = video_params(b"screen", 22, false);
    let a = gc::parse_video_params(Some(&camera), Some(&screen), None).unwrap();
    let b = gc::parse_video_params(Some(&camera), Some(&screen), Some(&a)).unwrap();
    assert!(Rc::ptr_eq(&a, &b));
    assert!(gc::parse_video_params(None, None, Some(&a)).is_none());
    let f = Fixture::new();
    let c = f.call(gc::GroupCallOrigin::Group);
    let tl::GroupCallParticipant::GroupCallParticipant(d) = participant(1, 0, 10, None);
    let mut d = (*d).clone();
    d.video = Some(camera);
    d.presentation = Some(screen);
    c.apply_participants_slice(
        &[tl::GroupCallParticipant::GroupCallParticipant(
            d.clone().into(),
        )],
        gc::ApplySliceSource::SliceLoaded,
    );
    let p = c.participants()[0].clone();
    assert_eq!(p.camera_endpoint(), b"cam");
    assert_eq!(p.screen_endpoint(), b"screen");
    assert!(p.camera_paused());
    assert!(!p.screen_paused());
    assert_eq!(c.participant_by_endpoint(b"screen").unwrap().peer, p.peer);
    assert!(c.participant_by_endpoint(b"").is_none());
    assert_eq!(c.participant_peer_by_audio_ssrc(22), Some(p.peer));
    d.presentation = Some(video_params(b"new", 23, true));
    let u = tl::UpdateGroupCallParticipantsData {
        call: c.input(),
        version: 1,
        participants: vec![tl::GroupCallParticipant::GroupCallParticipant(
            d.clone().into(),
        )],
    };
    c.apply_local_update(&u);
    assert!(Rc::ptr_eq(
        p.video_params.as_ref().unwrap(),
        c.participants()[0].video_params.as_ref().unwrap()
    ));
    c.apply_participants_slice(
        &[tl::GroupCallParticipant::GroupCallParticipant(d.into())],
        gc::ApplySliceSource::UpdateReceived,
    );
    assert_eq!(c.participant_peer_by_audio_ssrc(22), None);
    assert_eq!(c.participant_peer_by_audio_ssrc(23), Some(p.peer));
    assert!(c.participants()[0].screen_paused());
    c.apply_participants_slice(
        &[participant(2, 0, 10, None)],
        gc::ApplySliceSource::SliceLoaded,
    );
    assert_eq!(c.participant_peer_by_audio_ssrc(10), Some(p.peer));
    c.apply_participants_slice(
        &[participant(2, 1 << 1, 10, None)],
        gc::ApplySliceSource::SliceLoaded,
    );
    assert_eq!(c.participant_peer_by_audio_ssrc(10), None);
}
#[test]
fn group_migration_and_peer_call_not_empty_flag_use_actual_records() {
    use tg_core::data::data_channel::ChannelDataFlag;
    let f = Fixture::new();
    let c = f.call(gc::GroupCallOrigin::Group);
    let from = ChatKey::try_from(c.peer()).unwrap();
    let to = f.peers.ensure_channel(ChannelId::new(8));
    f.peers
        .add_channel_flags(to, ChannelDataFlag::Megagroup.into());
    f.peers
        .remove_channel_flags(to, ChannelDataFlag::Forbidden.into());
    f.peers.apply_migration(from, to);
    f.peers.migrate_group_call(to, from.peer(), c.key());
    c.set_peer(to.peer());
    assert_eq!(c.peer(), to.peer());
    c.apply_call_fields(&call_fields(1, 0, 1));
    assert!((f.peers.read_channel(to, |c| c.flags()) & ChannelDataFlag::CallNotEmpty).any());
    c.apply_call_fields(&call_fields(2, 0, 0));
    assert!(!(f.peers.read_channel(to, |c| c.flags()) & ChannelDataFlag::CallNotEmpty).any());
}
#[test]
fn notify_community_fallback_and_default_group_broadcast_types() {
    use tg_core::data::data_channel::ChannelDataFlag;
    let f = Fixture::new();
    let n = f.notify();
    let child = f.peer(peer_from_channel(3));
    let parent = f.peer(peer_from_channel(4));
    let pk = ChannelKey::try_from(parent).unwrap();
    f.peers
        .add_channel_flags(pk, ChannelDataFlag::Community.into());
    f.peers
        .set_channel_linked_community_id(ChannelKey::try_from(child).unwrap(), ChannelId::new(4));
    assert_eq!(
        notify::default_notify_type(&f.peers, child),
        DefaultNotify::Broadcast
    );
    let group = f.peer(peer_from_chat(1));
    assert_eq!(
        notify::default_notify_type(&f.peers, group),
        DefaultNotify::Group
    );
    n.apply(notify::Target::Peer(child), &settings(None, None, None));
    assert!(n.mute_unknown(notify::Target::Peer(child)));
    n.apply(
        notify::Target::Peer(parent),
        &settings(Some(0), Some(true), Some(tl::mtp_notification_sound_none())),
    );
    assert!(!n.is_muted(notify::Target::Peer(child)));
    assert!(n.silent_posts(child));
    assert!(n.sound(notify::Target::Peer(child)).none);
    assert!(!n.settings_unknown(notify::Target::Peer(child)));
    n.community_parent_mute_updated(pk);
    n.request(parent);
    assert!(f.owner.notify.borrow().iter().any(|e| matches!(
        e,
        notify::Effect::Send {
            request: notify::Request::Settings(tl::AccountGetNotifySettings {
                peer: tl::InputNotifyPeer::InputNotifyBroadcasts(_)
            }),
            ..
        }
    )));
    n.apply_notify_peer(
        &tl::mtp_notify_community(4),
        &settings(Some(i32::MAX), None, None),
    );
    assert!(n.is_muted(notify::Target::Peer(child)));
    n.apply_input(
        &tl::mtp_input_notify_community(tl::mtp_input_channel(4, 0)),
        &settings(Some(0), None, None),
    );
    assert!(!n.is_muted(notify::Target::Peer(child)));
}
#[test]
fn notify_reentrant_exception_flush_schedules_next_turn_without_borrow() {
    let f = Fixture::new();
    let n = f.notify();
    let a = f.peer(peer_from_user(2));
    let b = f.peer(peer_from_user(3));
    let life = Lifetime::new();
    let out = collect(n.exceptions_updates(), &life);
    let w = Rc::downgrade(&n);
    let once = Rc::new(Cell::new(false));
    let flag = once.clone();
    n.exceptions_updates().start(
        move |_| {
            if !flag.replace(true) {
                w.upgrade().unwrap().update(
                    notify::Target::Peer(b),
                    MuteValue {
                        forever: true,
                        ..Default::default()
                    },
                    None,
                    None,
                    None,
                );
            }
        },
        |_| {},
        || {},
        &life,
    );
    n.update(
        notify::Target::Peer(a),
        MuteValue {
            unmute: true,
            ..Default::default()
        },
        None,
        None,
        None,
    );
    f.queue.process();
    assert_eq!(out.borrow().len(), 1);
    f.queue.process_all();
    assert_eq!(out.borrow().len(), 2);
}
#[test]
fn actual_sender_bridges_get_participants_reload_save_notify_and_cancel() {
    use std::sync::Mutex;
    use tg_mtproto::{
        mtp_instance::{get_next_request_id, Fields, Instance, Mode},
        mtproto_dc_options::Environment,
        mtproto_response::Response,
        sender::Sender,
    };
    let instance = Instance::new(Mode::Normal, Fields::new(Environment::Test));
    let sender = Sender::new(instance.clone());
    let replies = Arc::new(Mutex::new(vec![]));
    let id = get_next_request_id();
    let q = replies.clone();
    gc::send_request(
        &sender,
        id,
        gc::Request::Participants(tl::PhoneGetGroupParticipants {
            call: tl::mtp_input_group_call(42, 8),
            ids: vec![],
            sources: vec![1],
            offset: vec![],
            limit: 50,
        }),
        Arc::new(move |id, r| q.lock().unwrap().push((id, r))),
    );
    instance.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&page(0, vec![])),
    });
    assert!(matches!(
        replies.lock().unwrap()[0].1,
        gc::Reply::Participants(Ok(_))
    ));
    let id = get_next_request_id();
    let q = replies.clone();
    gc::send_request(
        &sender,
        id,
        gc::Request::Reload(tl::PhoneGetGroupCall {
            call: tl::mtp_input_group_call(42, 8),
            limit: 3,
        }),
        Arc::new(move |id, r| q.lock().unwrap().push((id, r))),
    );
    instance.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&full(1, 0, vec![])),
    });
    assert!(matches!(
        replies.lock().unwrap()[1].1,
        gc::Reply::Reload(Ok(_))
    ));
    let id = get_next_request_id();
    gc::send_request(
        &sender,
        id,
        gc::Request::SaveSendAs(tl::PhoneSaveDefaultSendAs {
            call: tl::mtp_input_group_call(42, 8),
            send_as: tl::mtp_input_peer_self(),
        }),
        Arc::new(|_, _| panic!("fire-and-forget")),
    );
    assert!(gc::send_effect(
        &sender,
        &gc::Effect::Cancel(id),
        Arc::new(|_, _| {})
    ));
    assert!(!sender.pending(id));
    let out = Arc::new(Mutex::new(vec![]));
    let q = out.clone();
    let id = get_next_request_id();
    notify::send_request(
        &sender,
        id,
        notify::Request::Settings(tl::AccountGetNotifySettings {
            peer: tl::mtp_input_notify_users(),
        }),
        Arc::new(move |r| q.lock().unwrap().push(r)),
    );
    instance.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&tl::mtp_rpc_error(400, b"NOT_FOUND".to_vec())),
    });
    {
        let data = out.lock().unwrap();
        let notify::Reply::Settings { settings, .. } = &data[0] else {
            panic!()
        };
        let tl::PeerNotifySettings::PeerNotifySettings(s) = settings;
        assert!(s.flags.is_empty());
    }
    let q = out.clone();
    let id = get_next_request_id();
    notify::send_request(
        &sender,
        id,
        notify::Request::Exceptions(tl::AccountGetNotifyExceptions {
            flags: Default::default(),
            peer: Some(tl::mtp_input_notify_chats()),
        }),
        Arc::new(move |r| q.lock().unwrap().push(r)),
    );
    instance.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&tl::mtp_updates_too_long()),
    });
    assert!(matches!(
        out.lock().unwrap().last(),
        Some(notify::Reply::Exceptions(_))
    ));
}
#[test]
#[should_panic(expected = "source migrateFrom/migrateTo require membership")]
fn group_migration_rejects_destination_without_membership() {
    use tg_core::data::data_channel::ChannelDataFlag;
    let f = Fixture::new();
    let c = f.call(gc::GroupCallOrigin::Group);
    let from = ChatKey::try_from(c.peer()).unwrap();
    let to = f.peers.ensure_channel(ChannelId::new(8));
    f.peers
        .add_channel_flags(to, ChannelDataFlag::Megagroup.into());
    f.peers.apply_migration(from, to);
    c.set_peer(to.peer());
}
#[test]
fn photo_preload_should_uses_actual_photo_peer_fields_and_download_policy() {
    use tg_core::data::data_auto_download::{Full, Override};
    let f = Fixture::new();
    let p = f.peer(peer_from_chat(7));
    let chat = ChatKey::try_from(p).unwrap();
    let photos = Photos::new(&f.scope);
    let photo = photos.ensure_photo(1);
    let mut auto = Full::default();
    let should = |auto: &Full| {
        f.peers.read_chat(chat, |c| {
            f.peers.read(p, |peer| {
                photos
                    .read(photo, |photo| {
                        preload::PhotoPreload::should(photo, peer, &c.peer_fields(), auto)
                    })
                    .unwrap()
            })
        })
    };
    assert!(should(&auto));
    auto.set_peer_override(p.id(), Override::ForceDeny);
    assert!(!should(&auto));
}
#[test]
fn participant_about_changes_can_reenter_speaking_before_source_snapshot() {
    use tg_core::data::data_changes::PeerFlag;
    let f = Fixture::new();
    let c = f.call(gc::GroupCallOrigin::Group);
    c.apply_participants_slice(
        &[participant(1, 0, 10, None)],
        gc::ApplySliceSource::SliceLoaded,
    );
    let life = Lifetime::new();
    let w = Rc::downgrade(&c);
    f.changes.realtime_peer_updates(PeerFlag::About).start(
        move |_| {
            w.upgrade().unwrap().apply_last_spoke(
                10,
                gc::LastSpokeTimes {
                    anything: 10000,
                    voice: 10000,
                },
                10000,
            );
        },
        |_| {},
        || {},
        &life,
    );
    let tl::GroupCallParticipant::GroupCallParticipant(d) = participant(1, 0, 10, None);
    let mut d = (*d).clone();
    d.about = Some(b"new about".to_vec());
    c.apply_participants_slice(
        &[tl::GroupCallParticipant::GroupCallParticipant(d.into())],
        gc::ApplySliceSource::UpdateReceived,
    );
    assert!(c.participants()[0].speaking);
    c.apply_participants_slice(
        &[participant(1, 1, 10, None)],
        gc::ApplySliceSource::UpdateReceived,
    );
    let p = &c.participants()[0];
    assert!(!p.can_self_unmute);
    assert!(!p.speaking);
    assert!(!p.sounding);
}
