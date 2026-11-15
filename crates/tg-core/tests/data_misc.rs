use std::{cell::Cell, rc::Rc};
use tg_base::qt_stream::{Reader, Writer};
use tg_core::{
    data::{
        business::data_business_common as business,
        data_abstract_sparse_ids::*,
        data_abstract_structure::*,
        data_audio_msg_id::{AudioMsgId, Type as AudioType},
        data_authorization::*,
        data_auto_download as auto,
        data_boosts::*,
        data_bot_app::*,
        data_changes::Changes,
        data_document::{DocumentData, DocumentType},
        data_file_origin::*,
        data_peer_id::*,
        data_photo::*,
        data_session::Peers,
        data_store_keys::*,
    },
    storage::details::storage_settings_scheme as settings,
};
use tg_scheme as tl;
#[test]
fn auto_defaults_unsigned_limits_overrides_and_disable() {
    let mut f = auto::Full::default();
    for s in auto::SOURCES {
        assert_eq!(f.bytes_limit(s, auto::Type::Photo), 12 * 1024 * 1024);
        assert!(auto::has_enabled_types(&f, s));
        for t in auto::STREAMED_TYPES {
            assert!(!f.should_download(s, t, -1));
        }
    }
    assert_eq!(f.bytes_limit(auto::Source::Channel, auto::Type::File), 0);
    let mut single = auto::Single::default();
    assert!(!single.has_value());
    assert_eq!(single.serialize(), -1);
    assert!(!single.set_from_serialized(-2));
    single.set_bytes_limit(auto::MAX_BYTES_LIMIT);
    assert!(single.has_value());
    assert_eq!(single.bytes_limit(), auto::MAX_BYTES_LIMIT);
    assert!(single.should_download(-1));
    assert!(!single.should_download(auto::MAX_BYTES_LIMIT + 1));
    let mut set = auto::Set::default();
    assert!(!set.has_value(auto::Type::File));
    assert!(set.set_from_serialized(auto::Type::File, 123));
    assert_eq!(set.bytes_limit(auto::Type::File), 123);
    assert!(set.should_download(auto::Type::File, 123));
    assert_eq!(set.serialize(auto::Type::File), 123);
    set.set_bytes_limit(auto::Type::File, 0);
    assert!(!set.should_download(auto::Type::File, 0));
    for s in auto::SOURCES {
        auto::set_disabled_for_source(&mut f, s);
        assert!(!auto::has_enabled_types(&f, s));
        auto::set_defaults_for_source(&mut f, s);
        assert!(auto::has_enabled_types(&f, s));
    }
    let a = peer_from_user(1);
    let b = peer_from_user(2);
    f.set_peer_override(b, auto::Override::ForceDeny);
    f.set_peer_override(a, auto::Override::ForceAllow);
    let mut ids = Vec::new();
    f.enumerate_peer_overrides(|id, v| ids.push((id, v)));
    assert_eq!(
        ids,
        [
            (a, auto::Override::ForceAllow),
            (b, auto::Override::ForceDeny)
        ]
    );
    let off = auto::with_disabled_auto_play(&f);
    assert_eq!(off.peer_override(a), auto::Override::Default);
    assert_eq!(off.peer_override(b), auto::Override::ForceDeny);
    for s in auto::SOURCES {
        for t in auto::AUTO_PLAY_TYPES {
            assert_eq!(off.bytes_limit(s, t), 0);
        }
    }
    assert_eq!(f.peer_override(a), auto::Override::ForceAllow);
}
#[test]
fn auto_legacy_effect_applies_to_real_full() {
    let mut ctx = settings::ReadSettingsContext::default();
    let mut w = Writer::default();
    w.i32(1);
    w.i32(2);
    w.i32(3);
    assert!(settings::read_setting(
        settings::DBI_AUTO_DOWNLOAD_OLD,
        &mut Reader::new(&w.0),
        1,
        &mut ctx
    ));
    let mut f = auto::Full::default();
    for e in ctx.session_settings() {
        assert!(f.apply_settings_effect(e));
    }
    assert_eq!(f.bytes_limit(auto::Source::User, auto::Type::Photo), 0);
    assert_eq!(
        f.bytes_limit(auto::Source::Group, auto::Type::VoiceMessage),
        0
    );
    assert_eq!(
        f.bytes_limit(auto::Source::User, auto::Type::AutoPlayGIF),
        0
    );
    assert!(!f.apply_settings_effect(&settings::SessionEffect::DialogsFiltersEnabled(true)));
}
#[test]
fn auto_decisions_stickers_streaming_gifv_override_and_photo() {
    let scope = SessionScope::new();
    let peers = Peers::new(&scope, Changes::new(&scope));
    let chat = ChatKey::try_from(peers.ensure_peer(peer_from_chat(1))).unwrap();
    peers.read_chat(chat, |chat| {
        let peer = &chat.peer;
        let fields = chat.peer_fields();
        assert_eq!(auto::source_from_peer(peer, &fields), auto::Source::Group);
        let mut f = auto::Full::full_disabled();
        let mut d = DocumentData::new(&scope, 7);
        d.size = 1;
        assert!(!auto::should_document(&f, peer, &fields, &d));
        assert!(!auto::should_document_any_source(&f, &d));
        f.set_peer_override(peer.id(), auto::Override::ForceAllow);
        assert!(auto::should_document(&f, peer, &fields, &d));
        for kind in [
            DocumentType::Voice,
            DocumentType::RoundVideo,
            DocumentType::Video,
            DocumentType::Song,
        ] {
            d.kind = kind;
            assert!(!auto::should_document(&f, peer, &fields, &d));
            assert!(!auto::should_document_any_source(&f, &d));
        }
        d.kind = DocumentType::Animated;
        d.set_mime_string("video/mp4");
        assert!(auto::should_document(&f, peer, &fields, &d));
        assert!(auto::should_auto_play_document(&f, peer, &fields, &d));
        f.set_peer_override(peer.id(), auto::Override::ForceDeny);
        assert!(!auto::should_document(&f, peer, &fields, &d));
        d = DocumentData::new(&scope, 8);
        d.size = 1;
        d.set_attributes(
            &[
                tl::mtp_document_attribute_image_size(128, 128),
                tl::mtp_document_attribute_sticker(
                    Default::default(),
                    Vec::new(),
                    tl::mtp_input_sticker_set_empty(),
                    None,
                ),
            ],
            &Registry,
        );
        assert!(auto::should_document(&f, peer, &fields, &d));
        assert!(auto::should_auto_play_document(&f, peer, &fields, &d));
        let photo = PhotoData::new(&scope, 9);
        assert!(!auto::should_photo(&f, peer, &fields, &photo));
        assert!(!auto::should_auto_play_photo(&f, peer, &fields, &photo));
        f.set_peer_override(peer.id(), auto::Override::ForceAllow);
        assert!(auto::should_photo(&f, peer, &fields, &photo));
    });
}
struct Registry;
impl tg_core::core::mime_type::MimeDatabase for Registry {
    fn for_name(&self, name: &str) -> tg_core::core::mime_type::MimeType {
        panic!("unexpected registry query {name}")
    }
}
#[test]
fn origins_preserve_variant_order_identity_fields_and_utf16() {
    let a = FileOrigin::StickerSet(FileOriginStickerSet {
        set_id: 1,
        access_hash: 2,
    });
    let b = FileOrigin::StickerSet(FileOriginStickerSet {
        set_id: 1,
        access_hash: 999,
    });
    assert_eq!(a, b);
    assert!(a.is_present());
    assert!(!FileOrigin::None.is_present());
    let p = FileOrigin::Wallpaper(FileOriginWallpaper {
        paper_id: 1,
        access_hash: 5,
        owner_id: UserId::new(7),
        slug: "x".into(),
    });
    let q = FileOrigin::Wallpaper(FileOriginWallpaper {
        paper_id: 1,
        access_hash: 9,
        owner_id: UserId::new(8),
        slug: "y".into(),
    });
    assert_eq!(p, q);
    assert!(a < p);
    assert!(
        FileOriginWebPage { url: "𐀀".into() }
            < FileOriginWebPage {
                url: "\u{e000}".into()
            }
    );
    assert!(
        FileOriginCloudDraft {
            peer_id: peer_from_user(1),
            topic_root_id: 2.into(),
            monoforum_peer_id: PeerId::default()
        } < FileOriginCloudDraft {
            peer_id: peer_from_user(1),
            topic_root_id: 3.into(),
            monoforum_peer_id: PeerId::default()
        }
    );
}
#[test]
fn audio_identity_classification_and_cached_fields() {
    let scope = SessionScope::new();
    let mut d = DocumentData::new(&scope, 1);
    let empty = AudioMsgId::default();
    assert!(!empty.is_present());
    assert_eq!(empty.kind(), AudioType::Unknown);
    let context = Default::default();
    let a = AudioMsgId::new(&d, context, 0);
    assert!(a.is_present());
    assert_eq!(a.kind(), AudioType::Unknown);
    assert_eq!(a.audio(), Some(d.key()));
    assert_eq!(a.context_id(), context);
    assert_eq!(a.external_play_id(), 0);
    assert!(!a.changeable_playback_speed());
    for (kind, expected, speed) in [
        (DocumentType::Voice, AudioType::Voice, true),
        (DocumentType::RoundVideo, AudioType::Voice, true),
        (DocumentType::Video, AudioType::Video, false),
        (DocumentType::Song, AudioType::Song, false),
    ] {
        d.kind = kind;
        let b = AudioMsgId::new(&d, context, 0);
        assert_eq!(b, a);
        assert_eq!(b.kind(), expected);
        assert_eq!(b.changeable_playback_speed(), speed);
    }
    // Source compares millisecond DocumentData::duration with TimeId(60),
    // despite its comment saying "1 minute". Preserve 60ms, not 60 seconds.
    for (millis, expected) in [(59, false), (60, true)] {
        d = DocumentData::new(&scope, 900 + millis as u64);
        d.set_attributes(
            &[tl::mtp_document_attribute_video(
                Default::default(),
                f64::from(millis) / 1000.,
                1,
                1,
                None,
                None,
                None,
            )],
            &Registry,
        );
        assert_eq!(d.duration(), i64::from(millis));
        assert_eq!(
            AudioMsgId::new(&d, context, 0).changeable_playback_speed(),
            expected
        );
    }
    let video = AudioMsgId::for_video();
    let video2 = AudioMsgId::for_video();
    assert!(video.is_present());
    assert_eq!(video.kind(), AudioType::Video);
    assert!(video < video2);
    assert_ne!(video, video2);
    assert_ne!(AudioMsgId::create_external_play_id(), 0);
    assert!(!a.changeable_playback_speed());
}
#[test]
fn sparse_vectors_and_ordered_nearest_have_source_semantics() {
    let mut v = AbstractSparseIds::new(vec![10, 30, 20], Some(100), Some(7), None);
    assert_eq!(v.full_count(), Some(100));
    assert_eq!(v.size(), 3);
    assert_eq!(v.index_of(30), Some(1));
    assert_eq!(v.index_of(40), None);
    assert_eq!(v.distance(20, 10), Some(-2));
    assert_eq!(v.distance(20, 40), None);
    v.reverse();
    assert_eq!(v.at(0), 20);
    assert_eq!(v.skipped_before(), None);
    assert_eq!(v.skipped_after(), Some(7));
    let set: tg_base::flat_set::FlatSet<i32> = [10, 20, 30].into_iter().collect();
    let s = AbstractSparseIds::new(set, None, None, None);
    assert_eq!(s.nearest(21), Some(30));
    assert_eq!(s.nearest(40), Some(30));
    let e = AbstractSparseIds::new(
        tg_base::flat_set::FlatSet::<i32>::default(),
        None,
        None,
        None,
    );
    assert_eq!(e.nearest(0), None);
}
#[test]
fn global_registry_clears_handles_once_and_can_recreate() {
    struct Count(Rc<Cell<i32>>);
    impl Drop for Count {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let count = Rc::new(Cell::new(0));
    let r = GlobalStructures::new();
    let p = GlobalStructurePointer::new(&r);
    assert!(!p.is_present());
    p.create_if_null(|| Count(count.clone()));
    p.create_if_null(|| panic!("must not construct twice"));
    p.with(|_| {});
    p.with_mut(|_| {});
    r.clear_global_structures();
    assert!(!p.is_present());
    assert_eq!(count.get(), 1);
    r.clear_global_structures();
    p.create_if_null(|| Count(count.clone()));
    drop(r);
    assert!(!p.is_present());
    assert_eq!(count.get(), 2);
    struct Observe(Rc<dyn Fn()>);
    impl Drop for Observe {
        fn drop(&mut self) {
            (self.0)();
        }
    }
    let registry = GlobalStructures::new();
    let pointer = Rc::new(GlobalStructurePointer::new(&registry));
    let weak = Rc::downgrade(&pointer);
    pointer.create_if_null(|| {
        Observe(Rc::new(move || {
            assert!(weak.upgrade().unwrap().is_present())
        }))
    });
    registry.clear_global_structures();
    assert!(!pointer.is_present());
}
#[test]
fn authorization_boosts_and_bot_app_preserve_data_contracts() {
    let s = SessionScope::new();
    let app = BotAppData::new(&s, 91);
    assert_eq!(app.id, 91);
    assert_eq!(app.owner(), s.id());
    assert!(app.photo.is_none() && app.document.is_none());
    assert_eq!(app.access_hash, 0);
    let auth = UnreviewedAuth::default();
    assert!(!auth.unconfirmed && auth.device.is_empty());
    let status = BoostStatus::default();
    assert!(status.prepaid_giveaway.is_empty());
    assert_eq!(status.overview.premium_member_percentage, 0.);
    let boost = Boost {
        credits: u64::MAX,
        is_gift: true,
        ..Default::default()
    };
    let slice = BoostsListSlice {
        list: vec![boost.clone()],
        token: OffsetToken {
            next: "next".into(),
            gifts: true,
        },
        ..Default::default()
    };
    assert_eq!(slice.list[0].credits, u64::MAX);
    assert!(slice.token.gifts);
    assert_eq!(boost.expires_at, None);
    assert_eq!(BoostPrepaidGiveaway::default().date, None);
}
#[test]
fn business_recipients_flags_user_contract_and_permissions() {
    use business::*;
    let s = SessionScope::new();
    let u = |id| UserKey::try_from(s.peer_key(peer_from_user(id))).unwrap();
    let input = |k: UserKey| tl::mtp_input_user(k.peer().id().value, 99);
    let mut r = BusinessRecipients::default();
    assert!(BusinessRecipients::make_valid(r.clone()).all_but_excluded);
    r.included.types = BusinessChatType::NewChats | BusinessChatType::ExistingChats;
    r.included.list = vec![u(1), u(2)];
    r.excluded.list = vec![u(3)];
    let out = for_messages_to_mtp(&r, input);
    let tl::InputBusinessRecipients::InputBusinessRecipients(d) = out;
    assert_eq!(d.flags.value(), 19);
    assert_eq!(d.users.as_ref().unwrap().len(), 2);
    let tl::InputBusinessBotRecipients::InputBusinessBotRecipients(d) = for_bots_to_mtp(&r, input);
    assert_eq!(d.flags.value(), 83);
    assert_eq!(d.exclude_users.as_ref().unwrap().len(), 1);
    let wire = tl::mtp_business_recipients(tl::tl::Flags::from_raw(0x3f), Some(vec![1, 2]));
    let parsed = recipients_from_mtp(&wire, |id| u(id.bare));
    assert!(parsed.all_but_excluded);
    assert_eq!(parsed.excluded.types.value(), 15);
    assert_eq!(parsed.excluded.list, vec![u(1), u(2)]);
    let wire = tl::mtp_business_bot_recipients(Default::default(), Some(vec![1]), Some(vec![3]));
    let parsed = bot_recipients_from_mtp(&wire, |id| u(id.bare));
    assert_eq!(parsed.excluded.list, vec![u(3)]);
    for bits in 0..=0x3fff {
        let rights = tl::mtp_business_bot_rights(tl::tl::Flags::from_raw(bits));
        let p = permissions_from_mtp(&rights);
        assert_eq!(p.value(), 1 | ((bits as u32) << 1));
        let tl::BusinessBotRights::BusinessBotRights(d) = permissions_to_mtp(p);
        assert_eq!(d.flags.value(), bits);
    }
}
#[test]
fn business_intervals_keep_source_suffix_loss_and_presence_rules() {
    use business::*;
    let a = WorkingInterval {
        start: 0,
        end: 3 * WorkingInterval::DAY,
    };
    let v = WorkingIntervals { list: vec![a] };
    let r = remove_day_intervals(&v, 1);
    assert_eq!(
        r.list,
        vec![WorkingInterval {
            start: 0,
            end: WorkingInterval::DAY
        }]
    );
    assert_eq!(
        a.intersected(WorkingInterval { start: 1, end: 2 }),
        WorkingInterval { start: 1, end: 2 }
    );
    assert_eq!(WorkingInterval::default().united(a), a);
    assert_eq!(a.shifted(1).start, 1);
    assert!(!WorkingHours {
        timezone_id: "UTC".into(),
        intervals: Default::default()
    }
    .is_present());
    assert!(!BusinessLocation {
        point: Some(Default::default()),
        address: String::new()
    }
    .is_present());
    assert!(!BusinessDetails::default().is_present());
    assert!(!ChatIntro::default().custom_phrases());
    let intro = ChatIntro {
        title: "Hi".into(),
        ..Default::default()
    };
    assert!(intro.is_present());
    assert!(!AwaySettings::default().is_present());
    assert!(!GreetingSettings::default().is_present());
    let _ = Timezone::default();
    let _ = Timezones::default();
    assert_eq!(
        WorkingHours::default().normalized(),
        WorkingHours::default()
    );
}
#[test]
fn business_tl_details_use_real_media_store_and_schedule_variants() {
    use business::*;
    use tg_core::data::data_media_store::Documents;
    let s = SessionScope::new();
    let docs = Documents::new(&s, Rc::new(UnusedFiles));
    let hours = tl::mtp_business_work_hours(
        Default::default(),
        b"UTC".to_vec(),
        vec![tl::mtp_business_weekly_open(60, 120)],
    );
    let location = tl::mtp_business_location(
        Default::default(),
        Some(tl::mtp_geo_point(Default::default(), 20., 30., 5, None)),
        b"address".to_vec(),
    );
    let intro = tl::mtp_business_intro(
        Default::default(),
        b"title".to_vec(),
        b"description".to_vec(),
        Some(tl::mtp_document_empty(9)),
    );
    let d = details_from_mtp(
        &docs,
        &Registry,
        Some(&hours),
        Some(&location),
        Some(&intro),
    );
    assert!(d.is_present());
    assert_eq!(d.hours.timezone_id, "UTC");
    assert_eq!(
        d.hours.intervals.list,
        vec![WorkingInterval {
            start: 3600,
            end: 7200
        }]
    );
    assert_eq!(d.location.point.unwrap().lat, 30.);
    assert_eq!(d.intro.title, "title");
    assert_eq!(d.intro.sticker, None);
    assert!(docs.find(s.document_key(9)).is_some());
    let users = |id: UserId| UserKey::try_from(s.peer_key(peer_from_user(id))).unwrap();
    let recipients = tl::mtp_business_recipients(Default::default(), Some(vec![1]));
    for (schedule, expected) in [
        (
            tl::mtp_business_away_message_schedule_always(),
            AwayScheduleType::Always,
        ),
        (
            tl::mtp_business_away_message_schedule_outside_work_hours(),
            AwayScheduleType::OutsideWorkingHours,
        ),
        (
            tl::mtp_business_away_message_schedule_custom(10, 20),
            AwayScheduleType::Custom,
        ),
    ] {
        let wire = tl::mtp_business_away_message(
            tl::tl::Flags::from_raw(1),
            7,
            schedule,
            recipients.clone(),
        );
        let result = away_from_mtp(Some(&wire), users);
        assert!(result.is_present());
        assert_eq!(result.schedule.kind, expected);
        assert!(result.offline_only);
        assert_eq!(result.shortcut_id, 7);
        if expected == AwayScheduleType::Custom {
            assert_eq!(
                result.schedule.custom_interval,
                WorkingInterval { start: 10, end: 20 }
            );
        }
    }
    assert!(!away_from_mtp(None, users).is_present());
    let wire = tl::mtp_business_greeting_message(3, recipients, 7);
    let greeting = greeting_from_mtp(Some(&wire), users);
    assert_eq!(greeting.no_activity_days, 7);
    assert!(greeting.is_present());
    assert!(!greeting_from_mtp(None, users).is_present());
    assert!(!details_from_mtp(&docs, &Registry, None, None, None).is_present());
}
#[test]
fn auto_photo_video_boundary_and_failed_deserialize_is_transactional() {
    use tg_core::data::data_media_store::Photos;
    let scope = SessionScope::new();
    let peers = Peers::new(&scope, Changes::new(&scope));
    let key = ChatKey::try_from(peers.ensure_peer(peer_from_chat(1))).unwrap();
    let photos = Photos::new(&scope);
    let p = photos.process_photo(&tl::mtp_photo(
        Default::default(),
        1,
        2,
        vec![1],
        10,
        vec![tl::mtp_photo_size(b"x".to_vec(), 10, 10, 123)],
        Some(vec![tl::mtp_video_size(
            Default::default(),
            b"u".to_vec(),
            1,
            1,
            124,
            None,
        )]),
        4,
    ));
    peers.read_chat(key, |chat| {
        photos.read(p, |photo| {
            let fields = chat.peer_fields();
            let mut f = auto::Full::full_disabled();
            assert!(!auto::should_auto_play_photo(
                &f, &chat.peer, &fields, photo
            ));
            f.set_peer_override(chat.peer.id(), auto::Override::ForceAllow);
            assert!(auto::should_auto_play_photo(&f, &chat.peer, &fields, photo));
            f.set_peer_override(chat.peer.id(), auto::Override::Default);
            f.set_bytes_limit(auto::Source::Group, auto::Type::AutoPlayVideo, 124);
            assert!(auto::should_auto_play_photo(&f, &chat.peer, &fields, photo));
            f.set_bytes_limit(auto::Source::Group, auto::Type::Photo, 122);
            assert!(!auto::should_photo(&f, &chat.peer, &fields, photo));
            f.set_bytes_limit(auto::Source::Group, auto::Type::Photo, 123);
            assert!(auto::should_photo(&f, &chat.peer, &fields, photo));
            let old = f.clone();
            assert!(!f.set_from_serialized(&[4]));
            assert_eq!(f, old);
            let mut bytes = f.serialize();
            bytes[1..5].copy_from_slice(&(-2i32).to_be_bytes());
            assert!(!f.set_from_serialized(&bytes));
            assert_eq!(f, old);
        })
    });
}

struct UnusedFiles;
impl tg_core::core::file_location::FileLocations for UnusedFiles {
    fn inspect(
        &self,
        _: &tg_core::core::file_location::FileLocation,
    ) -> tg_core::core::file_location::FileInfo {
        panic!("unexpected platform file inspection")
    }
    fn read(&self, _: (u64, u64)) -> tg_core::core::file_location::FileLocation {
        panic!("unexpected local file lookup")
    }
}
#[test]
fn sparse_wrapped_deque_preserves_photos_order() {
    let mut ids = std::collections::VecDeque::with_capacity(3);
    ids.extend([1, 2, 3]);
    ids.pop_front();
    ids.push_back(4);
    let mut s = AbstractSparseIds::new(ids, Some(4), Some(1), Some(0));
    assert_eq!(s.index_of(2), Some(0));
    assert_eq!(s.at(2), 4);
    s.reverse();
    assert_eq!(s.at(0), 4);
    assert_eq!(s.distance(4, 2), Some(2));
    assert_eq!(s.skipped_before(), Some(0));
    assert_eq!(s.skipped_after(), Some(1));
}
#[test]
fn origins_all_constructors_keep_cpp_variant_discriminant_order() {
    let values = vec![
        FileOrigin::default(),
        FileOrigin::from(tg_core::data::data_msg_id::FullMsgId::default()),
        FileOrigin::from(FileOriginUserPhoto::default()),
        FileOrigin::from(FileOriginFullUser::default()),
        FileOrigin::from(FileOriginPeerPhoto::default()),
        FileOrigin::from(FileOriginStickerSet::default()),
        FileOrigin::from(FileOriginSavedGifs::default()),
        FileOrigin::from(FileOriginWallpaper::default()),
        FileOrigin::from(FileOriginTheme::default()),
        FileOrigin::from(FileOriginRingtones::default()),
        FileOrigin::from(FileOriginPremiumPreviews::default()),
        FileOrigin::from(FileOriginWebPage::default()),
        FileOrigin::from(FileOriginCloudDraft::default()),
        FileOrigin::from(tg_core::data::data_msg_id::FullStoryId::default()),
    ];
    for pair in values.windows(2) {
        assert!(pair[0] < pair[1]);
        assert!(pair[1].is_present());
    }
}
