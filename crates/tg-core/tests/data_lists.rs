use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};
use tg_core::data::{
    business::{
        data_business_common::*,
        data_business_info::{
            find_closest_timezone_id, BusinessInfo, BusinessOwner, Reply, Request,
        },
    },
    data_messages::*,
    data_msg_id::*,
    data_peer_id::*,
    data_pts_waiter::{Effect as PtsEffect, PtsUpdate, PtsWaiter},
    data_shared_media::*,
    data_sparse_ids::*,
    data_store_keys::*,
    data_user_photos::*,
    stickers::data_stickers_set::*,
};
use tg_core::storage::{
    storage_shared_media::*,
    storage_sparse_ids_list::{MsgRange, SparseIdsListResult, SparseIdsSliceUpdate},
    storage_user_photos::*,
};
use tg_rpl::{Lifetime, Producer};
use tg_scheme as tl;
fn pos(n: i32) -> MessagePosition {
    MessagePosition {
        full_id: FullMsgId::new(peer_from_user(9), MsgId::new(n.into())),
        date: n,
    }
}
fn collect<T: Clone + 'static>(p: Producer<T>, life: &Lifetime) -> Rc<RefCell<Vec<T>>> {
    let v = Rc::new(RefCell::new(vec![]));
    let out = v.clone();
    p.start(move |x| out.borrow_mut().push(x), |_| {}, || {}, life);
    v
}
fn sparse(
    ids: &[i64],
    count: Option<i32>,
    before: Option<i32>,
    after: Option<i32>,
) -> SparseIdsSlice {
    SparseIdsSlice::new(
        ids.iter().copied().map(MsgId::new).collect(),
        count,
        before,
        after,
    )
}
fn key(universal: i64) -> Key {
    Key::new(
        peer_from_user(9),
        0.into(),
        PeerId::new(0),
        PeerId::new(0),
        universal.into(),
    )
}
#[test]
fn positions_order_by_date_then_peer_then_id_and_constants() {
    assert!(!MessagePosition::default().is_present());
    assert!(pos(1).is_present());
    let mut p = pos(3);
    p.date = 0;
    assert!(p < pos(2));
    p.date = 2;
    assert!(p > pos(2));
    assert!(MIN_MESSAGE_POSITION < pos(1));
    assert!(MAX_MESSAGE_POSITION > pos(i32::MAX - 1));
    assert!(!UNREAD_MESSAGE_POSITION.is_present());
    assert_ne!(FULL_MESSAGES_RANGE, MessagesRange::default());
}
#[test]
fn messages_query_lazy_viewer_initial_eager_lifetime_and_reentrant_update() {
    let l = Rc::new(MessagesList::default());
    let q = MessagesQuery {
        around_id: pos(20),
        limit_before: 2,
        limit_after: 2,
    };
    l.add_slice(vec![pos(10)], FULL_MESSAGES_RANGE, Some(1));
    let viewer = l.viewer(q);
    let query = l.query(q);
    l.add_one(pos(20));
    let life = Lifetime::new();
    let a = collect(viewer, &life);
    let b = collect(query, &life);
    assert_eq!(a.borrow()[0].message_ids.len(), 1);
    assert_eq!(b.borrow()[0].message_ids.len(), 2);
    let called = Rc::new(Cell::new(false));
    let call = called.clone();
    let inner = l.clone();
    l.slice_updated().start(
        move |_| {
            assert!(!inner.empty());
            if !call.replace(true) {
                inner.add_one(pos(30));
            }
        },
        |_| {},
        || {},
        &life,
    );
    l.add_new(pos(40));
    assert!(called.get());
    assert!(a.borrow().len() >= 3);
    life.destroy();
    let n = a.borrow().len();
    l.add_one(pos(50));
    assert_eq!(a.borrow().len(), n);
}
#[test]
fn messages_disjoint_merge_remove_boundary_and_invalidation() {
    let l = MessagesList::default();
    l.add_slice(
        vec![pos(10)],
        MessagesRange {
            from: pos(1),
            till: pos(10),
        },
        Some(5),
    );
    l.add_slice(
        vec![pos(30)],
        MessagesRange {
            from: pos(30),
            till: pos(40),
        },
        None,
    );
    assert_eq!(
        l.snapshot(MessagesQuery {
            around_id: pos(20),
            ..Default::default()
        }),
        MessagesResult::default()
    );
    l.add_slice(
        vec![pos(20)],
        MessagesRange {
            from: pos(10),
            till: pos(30),
        },
        None,
    );
    let q = MessagesQuery {
        around_id: pos(20),
        limit_before: 8,
        limit_after: 8,
    };
    assert_eq!(l.snapshot(q).message_ids.len(), 3);
    l.remove_one(pos(999));
    assert_eq!(l.snapshot(q).count, Some(4));
    l.remove_less_than(pos(40));
    assert!(l.empty());
    l.invalidate();
    assert_eq!(l.snapshot(q), MessagesResult::default());
    l.add_slice(vec![], FULL_MESSAGES_RANGE, None);
    assert_eq!(l.snapshot(q).count, Some(0));
}
#[test]
fn messages_builder_merge_remove_no_refill_invalidated_false_and_snapshots() {
    let b = Rc::new(MessagesSliceBuilder::new(pos(20), 1, 1));
    let life = Lifetime::new();
    let req = collect(b.insufficient_around(), &life);
    b.check_insufficient();
    assert_eq!(req.borrow().len(), 2);
    b.apply_initial(&MessagesResult {
        message_ids: [pos(10), pos(20), pos(30), pos(40)].into_iter().collect(),
        count: Some(4),
        skipped_before: Some(0),
        skipped_after: Some(0),
    });
    let s = b.snapshot();
    assert_eq!(s.ids.len(), 3);
    assert_eq!(s.nearest_to_around, pos(20).full_id);
    assert_eq!(s.skipped_after, Some(1));
    let n = req.borrow().len();
    assert!(b.remove_one(pos(10)));
    assert_eq!(req.borrow().len(), n);
    assert!(!b.apply_update(&MessagesSliceUpdate {
        messages: None,
        range: FULL_MESSAGES_RANGE,
        count: None
    }));
    assert!(b.apply_update(&MessagesSliceUpdate {
        messages: None,
        range: FULL_MESSAGES_RANGE,
        count: Some(0)
    }));
    assert!(b.bottom_invalidated());
    assert!(!b.invalidated());
    assert!(b.snapshot().ids.is_empty());
    assert!(b.remove_all());
    assert_eq!(b.snapshot().full_count, Some(0));
}
#[test]
fn sparse_builder_callbacks_see_trim_phase_and_can_snapshot_without_borrow() {
    let b = Rc::new(SparseIdsSliceBuilder::new(10.into(), 2, 0));
    let life = Lifetime::new();
    let seen = Rc::new(RefCell::new(vec![]));
    let inner = b.clone();
    let v = seen.clone();
    b.insufficient_around().start(
        move |a| v.borrow_mut().push((a.direction, inner.snapshot().size())),
        |_| {},
        || {},
        &life,
    );
    b.apply_initial(&SparseIdsListResult {
        message_ids: [10, 20, 30].into_iter().collect(),
        count: Some(5),
        skipped_before: None,
        skipped_after: None,
    });
    assert_eq!(seen.borrow()[0], (LoadDirection::Before, 3));
    assert_eq!(b.snapshot().size(), 1);
    assert!(!b.apply_update(&SparseIdsSliceUpdate {
        messages: Some([50].into_iter().collect()),
        range: MsgRange { from: 50, till: 60 },
        count: None
    }));
    assert!(b.apply_update(&SparseIdsSliceUpdate {
        messages: Some([5, 10].into_iter().collect()),
        range: MsgRange { from: 0, till: 10 },
        count: Some(5)
    }));
    assert_eq!(b.snapshot().skipped_before(), Some(0));
    b.remove_one(5.into());
    b.invalidate_bottom();
    b.remove_all();
    assert_eq!(b.snapshot().full_count(), Some(0));
}
#[test]
fn sparse_merge_keys_unsorted_nearest_and_lazy_viewer() {
    let k = Key::new(
        peer_from_user(9),
        1.into(),
        PeerId::new(0),
        peer_from_chat(8),
        (-20).into(),
    );
    assert!(k.migrated_peer_id.is_zero());
    let k = Key::new(
        peer_from_user(9),
        0.into(),
        PeerId::new(0),
        peer_from_chat(8),
        (-20).into(),
    );
    assert_eq!(k.part_key(), 1.into());
    assert_eq!(k.migrated_key(), SERVER_MAX_MSG_ID - MsgId::new(20));
    assert_eq!(k.compute_id().peer, peer_from_chat(8));
    let u = SparseIdsMergedSlice::from_unsorted(
        key(10),
        SparseUnsortedIdsSlice::new(
            vec![30.into(), 10.into(), 20.into()],
            Some(3),
            Some(0),
            Some(0),
        ),
    );
    assert_eq!(
        u.index_of(FullMsgId::new(peer_from_user(77), 10.into())),
        Some(1)
    );
    assert_eq!(u.nearest(999.into()).unwrap().msg, 10.into());
    assert_eq!(u.at(0).msg, 30.into());
    assert_eq!(u.distance(key(30), key(20)), Some(2));
    let calls = Rc::new(Cell::new(0));
    let n = calls.clone();
    let p = SparseIdsMergedSlice::create_viewer(
        key(10),
        1,
        1,
        Rc::new(move |_, _, _, id, _, _| {
            n.set(n.get() + 1);
            assert_eq!(id, 10.into());
            tg_rpl::single(sparse(&[10], Some(1), Some(0), Some(0)))
        }),
    );
    assert_eq!(calls.get(), 0);
    let life = Lifetime::new();
    let values = collect(p, &life);
    assert_eq!(values.borrow()[0].size(), 1);
    assert_eq!(calls.get(), 1);
}
#[test]
fn user_photos_builder_replaces_unknown_count_skips_and_reverse_distance() {
    let k = UserPhotosKey {
        user_id: 9,
        photo_id: 20,
        back: false,
    };
    let b = UserPhotosSliceBuilder::new(k, 1, 1);
    let life = Lifetime::new();
    let req = collect(b.insufficient_photos_around(), &life);
    b.check_insufficient_photos();
    assert_eq!(*req.borrow(), vec![0]);
    b.apply_result(&UserPhotosResult {
        count: Some(5),
        skipped_before: Some(999),
        skipped_after: 1,
        photo_ids: vec![10, 20, 30].into(),
    });
    let mut s = b.snapshot();
    assert_eq!(s.data.skipped_before(), Some(1));
    assert_eq!(s.data.skipped_after(), Some(1));
    assert_eq!(s.key(), k);
    assert!(!b.apply_update(&UserPhotosSliceUpdate {
        user_id: 8,
        count: None,
        photo_ids: vec![1].into()
    }));
    let a = UserPhotosKey { photo_id: 10, ..k };
    let z = UserPhotosKey { photo_id: 30, ..k };
    assert_eq!(s.distance(a, z), Some(2));
    assert_eq!(s.distance(UserPhotosKey { user_id: 8, ..a }, z), None);
    s.reverse();
    assert_eq!(s.data.at(0), 30);
    b.apply_result(&UserPhotosResult::default());
    assert_eq!(b.snapshot().data.full_count(), None);
    b.apply_update(&UserPhotosSliceUpdate {
        user_id: 9,
        count: Some(2),
        photo_ids: vec![20, 99].into(),
    });
    assert_eq!(b.snapshot().data.at(0), 20);
}
#[test]
fn user_photos_real_storage_viewer_reverse_fallback_and_cancellation() {
    let storage = Rc::new(RefCell::new(UserPhotos::default()));
    storage.borrow_mut().add_slice(9, vec![30, 20, 10], 3);
    storage.borrow_mut().set_back(9, 5);
    assert_eq!(
        sync_user_fallback_photo_viewer(&storage.borrow(), 9),
        Some(5)
    );
    let life = Lifetime::new();
    let req = Rc::new(Cell::new(0));
    let n = req.clone();
    let k = UserPhotosKey {
        user_id: 9,
        photo_id: 20,
        back: false,
    };
    let v = collect(
        user_photos_viewer(
            storage.clone(),
            k,
            1,
            1,
            Rc::new(move |_| n.set(n.get() + 1)),
        ),
        &life,
    );
    let r = collect(
        user_photos_reversed_viewer(storage.clone(), k, 1, 1, Rc::new(|_| {})),
        &life,
    );
    assert_eq!(v.borrow()[0].data.size(), 3);
    assert_eq!(r.borrow()[0].data.at(0), 30);
    storage.borrow_mut().add_new(9, 40);
    assert!(v.borrow().len() > 1);
    life.destroy();
    let n = v.borrow().len();
    storage.borrow_mut().add_new(9, 50);
    assert_eq!(v.borrow().len(), n);
}
#[test]
fn shared_media_types_and_real_storage_filters_mutations() {
    let kinds = [
        SharedMediaType::Photo,
        SharedMediaType::Video,
        SharedMediaType::PhotoVideo,
        SharedMediaType::MusicFile,
        SharedMediaType::File,
        SharedMediaType::VoiceFile,
        SharedMediaType::Link,
        SharedMediaType::ChatPhoto,
        SharedMediaType::RoundVoiceFile,
        SharedMediaType::GIF,
        SharedMediaType::RoundFile,
        SharedMediaType::Pinned,
        SharedMediaType::Poll,
    ];
    for t in kinds {
        assert_eq!(
            shared_media_allow_search(t),
            matches!(
                t,
                SharedMediaType::MusicFile
                    | SharedMediaType::File
                    | SharedMediaType::Link
                    | SharedMediaType::Poll
            )
        );
        assert_eq!(
            shared_media_overview_type(t).is_some(),
            matches!(
                t,
                SharedMediaType::Photo
                    | SharedMediaType::Video
                    | SharedMediaType::MusicFile
                    | SharedMediaType::File
                    | SharedMediaType::RoundVoiceFile
                    | SharedMediaType::Link
                    | SharedMediaType::Poll
            )
        );
    }
    let st = Rc::new(RefCell::new(SharedMedia::default()));
    let q = SharedMediaKey {
        peer_id: 9,
        topic_root_id: 0,
        monoforum_peer_id: 0,
        type_: SharedMediaType::Photo,
        message_id: 20,
    };
    let life = Lifetime::new();
    let req = Rc::new(RefCell::new(vec![]));
    let send = req.clone();
    let v = collect(
        shared_media_viewer(
            st.clone(),
            q,
            1,
            1,
            Rc::new(move |k, a, d| send.borrow_mut().push((k, a, d))),
        ),
        &life,
    );
    assert!(!req.borrow().is_empty());
    let slice = SharedMediaAddSlice {
        peer_id: 9,
        topic_root_id: 0,
        monoforum_peer_id: 0,
        message_ids: vec![10, 20, 30],
        no_skip_range: MsgRange {
            from: 0,
            till: SERVER_MAX_MSG_ID.bare,
        },
        type_: SharedMediaType::Photo,
        count: Some(3),
    };
    st.borrow_mut().add_slice(SharedMediaAddSlice {
        peer_id: 8,
        ..slice.clone()
    });
    assert!(v.borrow().is_empty());
    st.borrow_mut().add_slice(slice);
    assert_eq!(v.borrow().last().unwrap().size(), 3);
    let mask = SharedMediaTypesMask::from(SharedMediaType::Photo);
    st.borrow_mut().remove_one(SharedMediaRemoveOne {
        peer_id: 9,
        topic_root_id: 999,
        monoforum_peer_id: 0,
        message_id: 20,
        types: mask,
    });
    assert_eq!(v.borrow().last().unwrap().size(), 2);
    st.borrow_mut()
        .invalidate(SharedMediaInvalidateBottom { peer_id: 9 });
    assert_eq!(v.borrow().last().unwrap().full_count(), None);
    st.borrow_mut().remove_all(SharedMediaRemoveAll {
        peer_id: 9,
        topic_root_id: 0,
        monoforum_peer_id: 0,
        types: mask,
    });
    assert_eq!(v.borrow().last().unwrap().full_count(), Some(0));
}
#[test]
fn with_last_photo_known_unknown_duplicate_extra_reverse_and_keys() {
    let scope = SessionScope::new();
    let photos = Rc::new(tg_core::data::data_media_store::Photos::new(&scope));
    let photo = photos.ensure_photo(77);
    let k = WithLastKey {
        merged_key: key(20),
        type_: SharedMediaType::ChatPhoto,
        universal_id: UniversalMsgId::Message(20.into()),
    };
    assert_eq!(k.viewer_key(), key(20));
    assert_eq!(
        k.ending_key().universal_id,
        SERVER_MAX_MSG_ID - MsgId::new(1)
    );
    let p = WithLastKey {
        universal_id: UniversalMsgId::Photo(photo),
        ..k
    };
    assert_eq!(p.compute_id(), MediaValue::Photo(photo));
    assert_eq!(
        p.viewer_key().universal_id,
        SERVER_MAX_MSG_ID - MsgId::new(1)
    );
    let slice = SparseIdsMergedSlice::from_parts(
        key(20),
        sparse(&[10, 20], Some(2), Some(0), Some(0)),
        None,
    );
    assert_eq!(
        last_full_msg_id(&slice),
        Some(FullMsgId::new(key(20).peer_id, 20.into()))
    );
    let mut s =
        SharedMediaWithLastSlice::new(k, slice.clone(), None, Some(77), photos.clone(), |_| 0);
    assert_eq!(s.full_count(), Some(3));
    assert_eq!(s.size(), 3);
    assert_eq!(s.at(2), MediaValue::Photo(photo));
    assert_eq!(s.index_of(MediaValue::Photo(photo)), Some(2));
    assert_eq!(s.distance(k, p), Some(1));
    s.reverse();
    assert_eq!(s.at(0), MediaValue::Photo(photo));
    assert_eq!(s.skipped_before(), Some(0));
    assert_eq!(s.skipped_after(), Some(0));
    assert_eq!(s.index_of(MediaValue::Photo(photo)), Some(0));
    let same =
        SharedMediaWithLastSlice::new(k, slice.clone(), None, Some(77), photos.clone(), |_| 77);
    assert_eq!(same.size(), 2);
    assert_eq!(same.index_of(MediaValue::Photo(photo)), Some(1));
    let unknown = SharedMediaWithLastSlice::new(k, slice, None, None, photos, |_| 0);
    assert_eq!(unknown.full_count(), None);
}
fn update() -> PtsUpdate {
    PtsUpdate::Updates(tl::mtp_updates_too_long())
}
#[test]
fn pts_gaps_order_duplicate_requesting_and_reentrant_apply() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let ev = events.clone();
    let slot = Rc::new(RefCell::new(None::<Rc<PtsWaiter>>));
    let s = slot.clone();
    let p = Rc::new(PtsWaiter::new(Rc::new(move |e| {
        if matches!(&e, PtsEffect::ApplyUpdatesNoPtsCheck(_)) {
            assert!(s.borrow().as_ref().unwrap().updated(None, 1, 1, None));
        }
        ev.borrow_mut().push(e);
    })));
    *slot.borrow_mut() = Some(p.clone());
    assert!(!p.inited());
    p.init(10);
    assert_eq!(p.current(), 10);
    assert!(!p.update_and_apply(None, 12, 1, Some(update())));
    assert!(p.waiting_for_skipped());
    assert!(matches!(
        events.borrow()[0],
        PtsEffect::StartTimerFor { ms: 1000, .. }
    ));
    assert!(p.update_and_apply(None, 11, 1, Some(update())));
    assert_eq!(p.current(), 12);
    assert!(!p.waiting_for_skipped());
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|e| matches!(e, PtsEffect::ApplyUpdatesNoPtsCheck(_)))
            .count(),
        2
    );
    assert!(!p.updated(None, 12, 1, None));
    p.set_requesting(true);
    assert!(p.requesting());
    assert!(p.updated(None, 1, 1, None));
    p.set_requesting(false);
    assert!(p.updated(None, 12, 0, None));
    p.set_waiting_for_short_poll(None, 500);
    p.set_waiting_for_skipped(None, -1);
    assert!(p.waiting_for_short_poll());
    p.set_waiting_for_short_poll(None, -1);
    assert!(matches!(
        events.borrow().last(),
        Some(PtsEffect::StartTimerFor { ms: -1, .. })
    ));
    p.clear_skipped_updates();
    slot.borrow_mut().take();
}
#[test]
fn pts_virtual_timer_delivery_overshoot_and_cancel() {
    use tg_base::timer::{ManualClock, Timer, TimerHost};
    let clock = ManualClock::new(100);
    let host = TimerHost::new(Arc::new(clock.clone()));
    let fires = Rc::new(Cell::new(0));
    let n = fires.clone();
    let timer = Rc::new(Timer::in_host(
        &host,
        Some(Rc::new(move || n.set(n.get() + 1))),
    ));
    let t = timer.clone();
    let p = PtsWaiter::new(Rc::new(move |e| {
        if let PtsEffect::StartTimerFor { ms, .. } = e {
            if ms < 0 {
                t.cancel()
            } else {
                t.call_once(ms)
            }
        }
    }));
    p.init(10);
    assert!(!p.updated(None, 12, 1, Some(update())));
    clock.advance(999);
    host.process();
    assert_eq!(fires.get(), 0);
    clock.advance(1);
    host.process();
    assert_eq!(fires.get(), 1);
    assert!(!p.updated(None, 11, 5, None));
    clock.advance(1);
    host.process();
    assert_eq!(fires.get(), 2);
    p.set_waiting_for_short_poll(None, 10);
    p.set_waiting_for_skipped(None, -1);
    p.set_waiting_for_short_poll(None, -1);
    clock.advance(100);
    host.process();
    assert_eq!(fires.get(), 2);
}
struct Owner {
    scope: SessionScope,
    details: RefCell<BusinessDetails>,
    log: Rc<RefCell<Vec<&'static str>>>,
}
impl BusinessOwner for Owner {
    fn scope(&self) -> SessionScopeId {
        self.scope.id()
    }
    fn business_details(&self) -> BusinessDetails {
        self.details.borrow().clone()
    }
    fn set_business_details(&self, d: BusinessDetails) {
        *self.details.borrow_mut() = d;
        self.log.borrow_mut().push("set");
    }
    fn input_user(&self, _: UserKey) -> tl::InputUser {
        tl::mtp_input_user_self()
    }
    fn input_document(&self, _: DocumentRecordKey) -> tl::InputDocument {
        tl::mtp_input_document_empty()
    }
}
type BusinessFixture = (
    Rc<Owner>,
    Rc<BusinessInfo>,
    Rc<RefCell<Vec<(i32, Request)>>>,
);
fn business() -> BusinessFixture {
    let log = Rc::new(RefCell::new(vec![]));
    let o = Rc::new(Owner {
        scope: SessionScope::new(),
        details: RefCell::default(),
        log: log.clone(),
    });
    let requests = Rc::new(RefCell::new(vec![]));
    let r = requests.clone();
    let b = Rc::new(BusinessInfo::new(
        o.clone(),
        Rc::new(move |id, rq| {
            log.borrow_mut().push("send");
            r.borrow_mut().push((id, rq));
        }),
    ));
    (o, b, requests)
}
#[test]
fn business_optimistic_order_rollback_fresh_other_fields_noop_and_success() {
    let (o, b, r) = business();
    assert_eq!(b.owner(), o.scope.id());
    let hours = WorkingHours {
        timezone_id: "UTC".into(),
        intervals: WorkingIntervals {
            list: vec![WorkingInterval {
                start: 0,
                end: 3600,
            }],
        },
    };
    let errors = Rc::new(RefCell::new(vec![]));
    let err = errors.clone();
    b.save_working_hours(
        hours.clone(),
        Some(Rc::new(move |s| err.borrow_mut().push(s))),
    );
    assert_eq!(*o.log.borrow(), vec!["send", "set"]);
    b.save_working_hours(hours, None);
    assert_eq!(r.borrow().len(), 1);
    b.save_chat_intro(
        ChatIntro {
            title: "hello".into(),
            ..Default::default()
        },
        None,
    );
    let id = r.borrow()[0].0;
    b.handle_reply(Reply::Failed(id, "FAIL".into()));
    assert_eq!(o.details.borrow().intro.title, "hello");
    assert_eq!(o.details.borrow().hours, WorkingHours::default());
    assert_eq!(*errors.borrow(), vec!["FAIL"]);
    let id = r.borrow()[1].0;
    b.handle_reply(Reply::Saved(id));
    b.handle_reply(Reply::Failed(id, "IGNORED".into()));
    assert_eq!(o.details.borrow().intro.title, "hello");
    b.save_location(
        BusinessLocation {
            address: "office".into(),
            point: Some(tg_core::data::data_location::LocationPoint {
                lat: 1.,
                lon: 2.,
                access_hash: 3,
            }),
        },
        None,
    );
    let (id, Request::Location(rq)) = r.borrow().last().unwrap().clone() else {
        panic!()
    };
    assert_eq!(rq.address, Some(b"office".to_vec()));
    let Some(tl::InputGeoPoint::InputGeoPoint(p)) = rq.geo_point else {
        panic!()
    };
    assert_eq!((p.lat, p.long), (1., 2.));
    b.handle_reply(Reply::Failed(id, "F".into()));
    assert_eq!(o.details.borrow().location, BusinessLocation::default());
}
#[test]
fn business_away_greeting_loaded_local_only_and_rollback_reentrant_events() {
    let (_, b, r) = business();
    assert!(!b.away_settings_loaded());
    assert!(!b.greeting_settings_loaded());
    let life = Lifetime::new();
    let a = collect(b.away_settings_changed(), &life);
    let g = collect(b.greeting_settings_changed(), &life);
    let inner = b.clone();
    b.away_settings_changed().start(
        move |_| {
            let _ = inner.away_settings();
        },
        |_| {},
        || {},
        &life,
    );
    let away = AwaySettings {
        schedule: AwaySchedule {
            kind: AwayScheduleType::Always,
            ..Default::default()
        },
        ..Default::default()
    };
    b.save_away_settings(away.clone(), None);
    assert!(b.away_settings_loaded());
    assert!(r.borrow().is_empty());
    b.apply_away_settings(away.clone());
    assert_eq!(a.borrow().len(), 1);
    b.save_away_settings(
        AwaySettings {
            shortcut_id: 7,
            ..away.clone()
        },
        None,
    );
    let id = r.borrow().last().unwrap().0;
    b.handle_reply(Reply::Failed(id, "F".into()));
    assert_eq!(b.away_settings(), away);
    assert_eq!(a.borrow().len(), 3);
    b.save_greeting_settings(
        GreetingSettings {
            no_activity_days: 7,
            shortcut_id: 8,
            ..Default::default()
        },
        None,
    );
    let id = r.borrow().last().unwrap().0;
    b.handle_reply(Reply::Failed(id, "F".into()));
    assert!(!b.greeting_settings_loaded());
    assert_eq!(g.borrow().len(), 2);
    b.apply_greeting_settings(GreetingSettings::default());
    b.apply_greeting_settings(GreetingSettings::default());
    assert_eq!(g.borrow().len(), 3);
    assert_eq!(b.greeting_settings(), GreetingSettings::default());
}
#[test]
fn business_timezone_preload_latches_even_on_empty_failed_not_modified() {
    let (_, b, r) = business();
    let life = Lifetime::new();
    let t = collect(b.timezones_value(), &life);
    assert_eq!(r.borrow().len(), 1);
    assert_eq!(t.borrow().len(), 1);
    b.preload();
    b.preload_timezones();
    assert_eq!(r.borrow().len(), 1);
    let id = r.borrow()[0].0;
    b.handle_reply(Reply::Failed(id, "F".into()));
    b.handle_reply(Reply::Timezones(tl::mtp_help_timezones_list_not_modified()));
    b.preload();
    assert_eq!(r.borrow().len(), 1);
    assert!(!b.timezones_loaded());
    b.handle_reply(Reply::Timezones(tl::mtp_help_timezones_list(
        vec![tl::mtp_timezone(b"UTC".to_vec(), b"World".to_vec(), 0)],
        4,
    )));
    assert!(b.timezones_loaded());
    assert_eq!(t.borrow().last().unwrap().list[0].id, "UTC");
    let list = [
        Timezone {
            id: "first".into(),
            utc_offset: 0,
            ..Default::default()
        },
        Timezone {
            id: "other".into(),
            utc_offset: 3600,
            ..Default::default()
        },
    ];
    assert_eq!(find_closest_timezone_id(&list, 1800, 0), "first");
    assert_eq!(find_closest_timezone_id(&list, 25 * 3600, 0), "other");
    assert_eq!(find_closest_timezone_id(&list, 3600, 3600), "first");
}
#[test]
fn stickers_flags_identity_type_thumbnail_and_document_lookup() {
    use tg_core::data::{
        data_document::{StickerType, StickersType},
        data_image_location::*,
        data_media_location::ImageWithLocation,
        data_media_store::Documents,
        data_types::StickerSetIdentifier,
    };
    let scope = SessionScope::new();
    struct UnusedFiles;
    impl tg_core::core::file_location::FileLocations for UnusedFiles {
        fn inspect(
            &self,
            _: &tg_core::core::file_location::FileLocation,
        ) -> tg_core::core::file_location::FileInfo {
            panic!("no file access in stickers model test")
        }
        fn read(&self, _: (u64, u64)) -> tg_core::core::file_location::FileLocation {
            panic!("no file access in stickers model test")
        }
    }
    let docs = Documents::new(&scope, Rc::new(UnusedFiles));
    let d = docs.ensure_document(10);
    let c = docs.ensure_document(20);
    let mut s = StickersSet::new(
        &scope,
        1,
        2,
        3,
        "Title".into(),
        "short".into(),
        1,
        StickersSetFlag::Emoji
            | StickersSetFlag::Masks
            | StickersSetFlag::TextColor
            | StickersSetFlag::ChannelStatus,
        4,
    );
    assert_eq!(s.owner(), scope.id());
    assert_eq!(s.session(), scope.id());
    assert_eq!(s.identifier().id, 1);
    assert!(matches!(
        s.mtp_input(),
        tl::InputStickerSet::InputStickerSetID(_)
    ));
    assert_eq!(s.kind(), StickersType::Emoji);
    assert!(s.text_color());
    assert!(s.channel_status());
    s.flags = StickersSetFlag::Masks.into();
    assert_eq!(s.kind(), StickersType::Masks);
    s.flags = Default::default();
    assert_eq!(s.kind(), StickersType::Stickers);
    assert!(!s.has_thumbnail());
    assert_eq!(s.thumbnail_byte_size(), 0);
    s.set_thumbnail(&ImageWithLocation::default(), StickerType::Tgs);
    assert_eq!(s.thumbnail_type(), StickerType::Tgs);
    assert!(!s.thumbnail_location().valid());
    let loc = StorageFileLocation {
        kind: StorageFileType::StickerSetThumb,
        dc_id: 1,
        id: 2,
        ..Default::default()
    };
    let image = ImageWithLocation {
        location: ImageLocation {
            file: DownloadLocation::Storage(loc),
            width: 10,
            height: 20,
        },
        bytes: vec![1, 2],
        bytes_count: 2,
        ..Default::default()
    };
    assert!(s.set_thumbnail(&image, StickerType::Webm).changed);
    assert!(s.has_thumbnail());
    assert_eq!(s.thumbnail_byte_size(), 2);
    assert_ne!(s.thumbnail_big_file_base_cache_key(), Default::default());
    s.covers.push(c);
    assert_eq!(s.lookup_thumbnail_document(&docs), Some(c));
    s.stickers.push(d);
    assert_eq!(s.lookup_thumbnail_document(&docs), Some(d));
    s.stickers.push(c);
    s.thumbnail_document_id = 20;
    assert_eq!(s.lookup_thumbnail_document(&docs), Some(c));
    s.id = 0;
    s.access_hash = 0;
    assert!(matches!(
        s.mtp_input(),
        tl::InputStickerSet::InputStickerSetShortName(_)
    ));
    assert!(s.identifier().empty());
    assert!(matches!(
        input_sticker_set(s.identifier()),
        tl::InputStickerSet::InputStickerSetEmpty(_)
    ));
    assert!(matches!(
        input_sticker_set(StickerSetIdentifier {
            short_name: "x".into(),
            ..Default::default()
        }),
        tl::InputStickerSet::InputStickerSetShortName(_)
    ));
    assert_eq!(
        from_input_set(&tl::mtp_input_sticker_set_id(9, 8)).access_hash,
        8
    );
    let data = tl::StickerSetData {
        flags: tl::tl::Flags::from_raw(
            (1 << 1) | (1 << 2) | (1 << 3) | (1 << 7) | (1 << 9) | (1 << 10) | (1 << 11),
        ),
        installed_date: Some(0),
        id: 1,
        access_hash: 2,
        title: vec![],
        short_name: vec![],
        thumbs: None,
        thumb_dc_id: None,
        thumb_version: None,
        thumb_document_id: None,
        count: 3,
        hash: 4,
    };
    let flags = parse_stickers_set_flags(&data);
    assert!((flags & StickersSetFlag::Installed).any());
    assert!((flags & StickersSetFlag::Emoji).any());
    assert!((flags & StickersSetFlag::Archived).any());
}
#[test]
fn with_last_viewer_routes_by_photo_topic_and_reversed_is_lazy() {
    struct Source {
        photos: Rc<tg_core::data::data_media_store::Photos>,
        calls: Rc<RefCell<Vec<&'static str>>>,
    }
    impl Source {
        fn next(
            &self,
            k: SharedMediaMergedKey,
            kind: &'static str,
        ) -> Producer<SparseIdsMergedSlice> {
            self.calls.borrow_mut().push(kind);
            tg_rpl::single(SparseIdsMergedSlice::from_parts(
                k.merged_key,
                sparse(&[10, 20], Some(2), Some(0), Some(0)),
                None,
            ))
        }
    }
    impl WithLastSource for Source {
        fn merged_viewer(
            &self,
            k: SharedMediaMergedKey,
            _: i32,
            _: i32,
        ) -> Producer<SparseIdsMergedSlice> {
            self.next(k, "merged")
        }
        fn scheduled_viewer(
            &self,
            k: SharedMediaMergedKey,
            _: i32,
            _: i32,
        ) -> Producer<SparseIdsMergedSlice> {
            self.next(k, "scheduled")
        }
        fn saved_music_viewer(
            &self,
            k: SharedMediaMergedKey,
            _: i32,
            _: i32,
        ) -> Producer<SparseIdsMergedSlice> {
            self.next(k, "music")
        }
        fn last_peer_photo_id(&self, _: PeerId) -> Option<u64> {
            Some(77)
        }
        fn message_photo_id(&self, _: FullMsgId) -> u64 {
            0
        }
        fn photos(&self) -> Rc<tg_core::data::data_media_store::Photos> {
            self.photos.clone()
        }
    }
    let scope = SessionScope::new();
    let photos = Rc::new(tg_core::data::data_media_store::Photos::new(&scope));
    let calls = Rc::new(RefCell::new(vec![]));
    let src = Rc::new(Source {
        photos: photos.clone(),
        calls: calls.clone(),
    });
    let k = WithLastKey {
        merged_key: key(20),
        type_: SharedMediaType::ChatPhoto,
        universal_id: UniversalMsgId::Message(20.into()),
    };
    let p = shared_media_with_last_reversed_viewer(src.clone(), k, 1, 1);
    assert!(calls.borrow().is_empty());
    let life = Lifetime::new();
    let a = collect(p, &life);
    assert_eq!(*calls.borrow(), vec!["merged", "merged"]);
    assert!(matches!(a.borrow()[0].at(0), MediaValue::Photo(_)));
    for (topic, expected) in [
        (SparseIdsMergedSlice::SCHEDULED_TOPIC_ID, "scheduled"),
        (SparseIdsMergedSlice::SAVED_MUSIC_TOPIC_ID, "music"),
    ] {
        calls.borrow_mut().clear();
        let k = WithLastKey {
            merged_key: Key {
                topic_root_id: topic,
                ..key(20)
            },
            ..k
        };
        let a = collect(shared_media_with_last_viewer(src.clone(), k, 1, 1), &life);
        assert_eq!(*calls.borrow(), vec![expected]);
        assert_eq!(a.borrow()[0].size(), 3);
    }
    calls.borrow_mut().clear();
    let k = WithLastKey {
        universal_id: UniversalMsgId::Photo(photos.ensure_photo(77)),
        ..k
    };
    let a = collect(shared_media_with_last_viewer(src, k, 1, 1), &life);
    assert_eq!(*calls.borrow(), vec!["merged"]);
    assert_eq!(a.borrow()[0].size(), 3);
    // Even non-chat-photo source constructors retain the peer photo ID, so
    // indexOf(Photo) can alias the last message when the extra-photo skip is0.
    let plain = WithLastKey {
        type_: SharedMediaType::Photo,
        universal_id: UniversalMsgId::Message(20.into()),
        ..k
    };
    let src = Rc::new(Source {
        photos: photos.clone(),
        calls: Rc::new(RefCell::new(vec![])),
    });
    let a = collect(shared_media_with_last_viewer(src, plain, 1, 1), &life);
    assert_eq!(
        a.borrow()[0].index_of(MediaValue::Photo(photos.ensure_photo(77))),
        Some(1)
    );
}
#[test]
fn shared_media_merged_viewer_and_last_peer_photo_use_actual_stores() {
    use tg_core::data::{data_changes::Changes, data_session::Peers};
    let scope = SessionScope::new();
    let peers = Peers::new(
        &scope,
        Changes::with_queue(&scope, tg_runtime::MainQueue::new()),
    );
    assert_eq!(last_peer_photo_id(&peers, peer_from_user(9)), None);
    let storage = Rc::new(RefCell::new(SharedMedia::default()));
    storage.borrow_mut().add_slice(SharedMediaAddSlice {
        peer_id: key(20).peer_id.value,
        topic_root_id: 0,
        monoforum_peer_id: 0,
        message_ids: vec![10, 20],
        no_skip_range: MsgRange {
            from: 0,
            till: SERVER_MAX_MSG_ID.bare,
        },
        type_: SharedMediaType::Photo,
        count: Some(2),
    });
    let life = Lifetime::new();
    let a = collect(
        shared_media_merged_viewer(
            storage,
            SharedMediaMergedKey {
                merged_key: key(20),
                type_: SharedMediaType::Photo,
            },
            1,
            1,
            Rc::new(|_, _, _| {}),
        ),
        &life,
    );
    assert_eq!(a.borrow()[0].size(), 2);
}
#[test]
fn business_real_sender_all_saves_success_failure_and_timezones() {
    use std::sync::Mutex;
    use tg_core::data::business::data_business_info::send_request;
    use tg_mtproto::{
        mtp_instance::{get_next_request_id, Fields, Instance, Mode},
        mtproto_dc_options::Environment,
        mtproto_response::Response,
        sender::Sender,
    };
    let instance = Instance::new(Mode::Normal, Fields::new(Environment::Test));
    let sender = Sender::new(instance.clone());
    let replies = Arc::new(Mutex::new(vec![]));
    let requests = [
        Request::WorkingHours(tl::AccountUpdateBusinessWorkHours {
            flags: Default::default(),
            business_work_hours: None,
        }),
        Request::Intro(tl::AccountUpdateBusinessIntro {
            flags: Default::default(),
            intro: None,
        }),
        Request::Location(tl::AccountUpdateBusinessLocation {
            flags: Default::default(),
            geo_point: None,
            address: None,
        }),
        Request::Away(tl::AccountUpdateBusinessAwayMessage {
            flags: Default::default(),
            message: None,
        }),
        Request::Greeting(tl::AccountUpdateBusinessGreetingMessage {
            flags: Default::default(),
            message: None,
        }),
    ];
    for rq in requests {
        let id = get_next_request_id();
        let q = replies.clone();
        send_request(
            &sender,
            id,
            rq,
            Arc::new(move |r| q.lock().unwrap().push(r)),
        );
        assert!(sender.pending(id));
        instance.process_callback(&Response {
            request_id: id,
            outer_msg_id: 0,
            reply: tg_tl::serialize(&true),
        });
        assert!(matches!(replies.lock().unwrap().last(),Some(Reply::Saved(i)) if *i==id));
    }
    let id = get_next_request_id();
    let q = replies.clone();
    send_request(
        &sender,
        id,
        Request::Away(tl::AccountUpdateBusinessAwayMessage {
            flags: Default::default(),
            message: None,
        }),
        Arc::new(move |r| q.lock().unwrap().push(r)),
    );
    instance.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&tl::mtp_rpc_error(400, b"TEST_FAILED".to_vec())),
    });
    assert!(
        matches!(replies.lock().unwrap().last(),Some(Reply::Failed(i,e)) if *i==id && e=="TEST_FAILED")
    );
    let id = get_next_request_id();
    let q = replies.clone();
    send_request(
        &sender,
        id,
        Request::Timezones(tl::HelpGetTimezonesList { hash: 0 }),
        Arc::new(move |r| q.lock().unwrap().push(r)),
    );
    instance.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&tl::mtp_help_timezones_list_not_modified()),
    });
    assert!(matches!(
        replies.lock().unwrap().last(),
        Some(Reply::Timezones(_))
    ));
}
#[test]
fn business_tl_recipients_and_all_schedule_variants() {
    use tg_core::data::business::data_business_info::*;
    let a = AwaySettings {
        shortcut_id: 7,
        offline_only: true,
        schedule: AwaySchedule {
            kind: AwayScheduleType::Custom,
            custom_interval: WorkingInterval {
                start: 123,
                end: 456,
            },
        },
        ..Default::default()
    };
    let tl::InputBusinessAwayMessage::InputBusinessAwayMessage(d) =
        away_to_mtp(&a, |_| tl::mtp_input_user_self());
    assert!(d.is_offline_only());
    assert_eq!(d.shortcut_id, 7);
    let tl::BusinessAwayMessageSchedule::BusinessAwayMessageScheduleCustom(s) = &d.schedule else {
        panic!()
    };
    assert_eq!((s.start_date, s.end_date), (123, 456));
    let g = GreetingSettings {
        shortcut_id: 3,
        no_activity_days: 7,
        ..Default::default()
    };
    let tl::InputBusinessGreetingMessage::InputBusinessGreetingMessage(d) =
        greeting_to_mtp(&g, |_| tl::mtp_input_user_self());
    assert_eq!((d.shortcut_id, d.no_activity_days), (3, 7));
}

#[test]
fn level_limits_keys_defaults_and_boost_environment_are_live() {
    use tg_core::data::data_premium_limits::*;
    struct Context {
        calls: RefCell<Vec<(String, i32)>>,
        test: Cell<bool>,
    }
    impl LimitsContext for Context {
        fn app_config_limit(&self, k: &str, f: i32) -> i32 {
            self.calls.borrow_mut().push((k.into(), f));
            f + 100
        }
        fn is_premium(&self) -> bool {
            false
        }
        fn is_test_mode(&self) -> bool {
            self.test.get()
        }
    }
    let c = Context {
        calls: RefCell::default(),
        test: Cell::new(false),
    };
    let l = LevelLimits::new(&c);
    assert_eq!(l.channel_color_level_min(), 5 + 100);
    assert_eq!(
        c.calls.borrow().last(),
        Some(&("channel_color_level_min".into(), 5))
    );
    assert_eq!(l.channel_bg_icon_level_min(), 4 + 100);
    assert_eq!(
        c.calls.borrow().last(),
        Some(&("channel_bg_icon_level_min".into(), 4))
    );
    assert_eq!(l.channel_profile_bg_icon_level_min(), 7 + 100);
    assert_eq!(
        c.calls.borrow().last(),
        Some(&("channel_profile_bg_icon_level_min".into(), 7))
    );
    assert_eq!(l.channel_emoji_status_level_min(), 8 + 100);
    assert_eq!(
        c.calls.borrow().last(),
        Some(&("channel_emoji_status_level_min".into(), 8))
    );
    assert_eq!(l.channel_wallpaper_level_min(), 9 + 100);
    assert_eq!(
        c.calls.borrow().last(),
        Some(&("channel_wallpaper_level_min".into(), 9))
    );
    assert_eq!(l.channel_custom_wallpaper_level_min(), 10 + 100);
    assert_eq!(
        c.calls.borrow().last(),
        Some(&("channel_custom_wallpaper_level_min".into(), 10))
    );
    assert_eq!(l.channel_restrict_sponsored_level_min(), 20 + 100);
    assert_eq!(
        c.calls.borrow().last(),
        Some(&("channel_restrict_sponsored_level_min".into(), 20))
    );
    assert_eq!(l.channel_auto_translate_level_min(), 3 + 100);
    assert_eq!(
        c.calls.borrow().last(),
        Some(&("channel_autotranslation_level_min".into(), 3))
    );
    assert_eq!(l.group_transcribe_level_min(), 6 + 100);
    assert_eq!(
        c.calls.borrow().last(),
        Some(&("group_transcribe_level_min".into(), 6))
    );
    assert_eq!(l.group_emoji_stickers_level_min(), 4 + 100);
    assert_eq!(
        c.calls.borrow().last(),
        Some(&("group_emoji_stickers_level_min".into(), 4))
    );
    assert_eq!(l.group_profile_bg_icon_level_min(), 5 + 100);
    assert_eq!(
        c.calls.borrow().last(),
        Some(&("group_profile_bg_icon_level_min".into(), 5))
    );
    assert_eq!(l.group_emoji_status_level_min(), 8 + 100);
    assert_eq!(
        c.calls.borrow().last(),
        Some(&("group_emoji_status_level_min".into(), 8))
    );
    assert_eq!(l.group_wallpaper_level_min(), 9 + 100);
    assert_eq!(
        c.calls.borrow().last(),
        Some(&("group_wallpaper_level_min".into(), 9))
    );
    assert_eq!(l.group_custom_wallpaper_level_min(), 10 + 100);
    assert_eq!(
        c.calls.borrow().last(),
        Some(&("group_custom_wallpaper_level_min".into(), 10))
    );
    let p = PremiumLimits::new(&c);
    assert_eq!(p.max_boost_level(), 199);
    c.test.set(true);
    assert_eq!(p.max_boost_level(), 109);
}
#[test]
fn empty_with_last_counts_and_sparse_count_only_contract() {
    let scope = SessionScope::new();
    let photos = Rc::new(tg_core::data::data_media_store::Photos::new(&scope));
    let k = WithLastKey {
        merged_key: key(20),
        type_: SharedMediaType::Photo,
        universal_id: UniversalMsgId::Message(20.into()),
    };
    let s = SharedMediaWithLastSlice::from_key(k, None, photos, |_| 0);
    assert_eq!(s.size(), 0);
    assert_eq!(s.full_count(), None);
    let b = SparseIdsSliceBuilder::new(0.into(), 0, 0);
    let life = Lifetime::new();
    let r = collect(b.insufficient_around(), &life);
    b.check_insufficient();
    assert_eq!(r.borrow().len(), 1);
    b.apply_initial(&SparseIdsListResult {
        count: Some(12),
        ..Default::default()
    });
    assert_eq!(b.snapshot().full_count(), Some(12));
    assert_eq!(r.borrow().len(), 1);
}
#[test]
fn source_corner_contracts_constructor_order_and_multiple_range_overcount() {
    let a = MessagesAroundData {
        around_id: pos(10),
        direction: LoadDirection::Before,
    };
    let b = MessagesAroundData {
        direction: LoadDirection::After,
        ..a
    };
    assert!(a < b);
    let a = SparseIdsAroundData {
        around_id: 10.into(),
        direction: LoadDirection::Around,
    };
    let b = SparseIdsAroundData {
        around_id: 20.into(),
        ..a
    };
    assert!(a < b);
    let k = UserPhotosKey {
        user_id: 9,
        photo_id: 20,
        back: false,
    };
    let u = UserPhotosSlice::new(k);
    assert_eq!(u.data.size(), 0);
    assert_eq!(u.data.full_count(), None);
    let l = MessagesList::default();
    l.add_slice(
        vec![pos(10)],
        MessagesRange {
            from: pos(1),
            till: pos(10),
        },
        Some(5),
    );
    l.add_slice(
        vec![pos(30)],
        MessagesRange {
            from: pos(30),
            till: pos(40),
        },
        None,
    );
    l.add_new(pos(5));
    assert_eq!(
        l.snapshot(MessagesQuery {
            around_id: pos(20),
            limit_before: 10,
            limit_after: 10
        })
        .count,
        Some(7)
    );
    let k = Key::new(
        peer_from_user(9),
        0.into(),
        peer_from_user(3),
        peer_from_chat(8),
        20.into(),
    );
    assert_eq!(k.migrated_peer_id, PeerId::new(0));
    let a = WithLastKey {
        merged_key: key(20),
        type_: SharedMediaType::Photo,
        universal_id: UniversalMsgId::Message(20.into()),
    };
    let b = WithLastKey {
        merged_key: key(30),
        ..a
    };
    assert_eq!(a, b);
    assert_eq!(a.cmp(&b), std::cmp::Ordering::Equal);
    assert!(a <= b);
    let b = WithLastKey {
        universal_id: UniversalMsgId::Message(30.into()),
        ..a
    };
    assert!(a < b);
}
#[test]
fn pts_single_update_and_timer_flag_order_preserve_initial_state() {
    let slot = Rc::new(RefCell::new(None::<Rc<PtsWaiter>>));
    let inner = slot.clone();
    let log = Rc::new(RefCell::new(vec![]));
    let out = log.clone();
    let p = Rc::new(PtsWaiter::new(Rc::new(move |e| {
        let p = inner.borrow().as_ref().unwrap().clone();
        match e {
            PtsEffect::StartTimerFor { ms, .. } => {
                out.borrow_mut().push((ms, p.waiting_for_skipped()))
            }
            PtsEffect::ApplyUpdateNoPtsCheck(_) => {
                out.borrow_mut().push((42, p.waiting_for_skipped()))
            }
            _ => {}
        }
    })));
    *slot.borrow_mut() = Some(p.clone());
    assert!(p.update_and_apply(
        None,
        10,
        1,
        Some(PtsUpdate::Update(tl::mtp_update_pts_changed()))
    ));
    assert_eq!(p.current(), 10);
    assert_eq!(*log.borrow(), vec![(42, false)]);
    assert!(!p.update_and_apply(
        None,
        12,
        1,
        Some(PtsUpdate::Update(tl::mtp_update_pts_changed()))
    ));
    assert_eq!(*log.borrow().last().unwrap(), (1000, false));
    p.init(5);
    assert!(p.waiting_for_skipped());
    p.apply_skipped_updates(None);
    assert!(!p.waiting_for_skipped());
    slot.borrow_mut().take();
}
