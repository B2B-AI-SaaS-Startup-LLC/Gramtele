use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, VecDeque},
    rc::Rc,
    sync::Arc,
};
use tg_base::timer::{ManualClock, TimerHost};
use tg_core::{
    core::{file_location::*, mime_type::*},
    data::{
        components::{
            location_pickers::LocationPickers, recent_inline_bots as inline,
            recent_shared_media_gifts as gifts,
        },
        data_changes::Changes,
        data_cloud_themes as cloud,
        data_game::*,
        data_media_rotation::*,
        data_media_store::{Documents, Photos},
        data_msg_id::*,
        data_peer_colors::*,
        data_peer_id::*,
        data_report::*,
        data_session::Peers,
        data_star_gift::*,
        data_statistics::*,
        data_statistics_lists::*,
        data_store_keys::*,
        data_subscriptions::*,
        data_wall_paper::*,
    },
};
use tg_rpl::{Lifetime, Producer};
use tg_scheme as tl;
fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
fn hex(b: &[u8]) -> String {
    b.iter().map(|b| format!("{b:02x}")).collect()
}
fn tlhex(v: &impl tg_tl::Serialize) -> String {
    hex(&tg_tl::serialize(v)
        .iter()
        .flat_map(|i| i.to_le_bytes())
        .collect::<Vec<_>>())
}
fn collect<T: Clone + 'static>(p: Producer<T>, life: &Lifetime) -> Rc<RefCell<Vec<T>>> {
    let v = Rc::new(RefCell::new(vec![]));
    let r = v.clone();
    p.start(move |v| r.borrow_mut().push(v), |_| {}, || {}, life);
    v
}
#[test]
fn native_original_wallpaper_read_write_keys_tl_legacy_corruption() {
    for line in include_str!("fixtures/data-themes/native.jsonl").lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let op = v["op"].as_str().unwrap();
        if matches!(op, "rarity" | "attribute" | "saved_id") {
            continue;
        }
        if op == "params" {
            let value = v["input"].as_str().unwrap();
            let p = default_wall_paper().with_url_params(&BTreeMap::from([
                ("intensity".into(), value.into()),
                ("rotation".into(), value.into()),
                ("mode".into(), "foo+blur".into()),
                ("bg_color".into(), "112233-445566".into()),
            ]));
            assert_eq!(hex(&p.serialize()), v["canonical"], "{line}");
            assert_eq!(p.key(), v["key"], "{line}");
            continue;
        }
        if op == "slug_input" {
            assert_eq!(
                WallPaper::from_colors_slug(v["slug"].as_str().unwrap()).is_some(),
                v["ok"].as_bool().unwrap()
            );
            continue;
        }
        let bytes = unhex(v["bytes"].as_str().unwrap());
        let p = if op == "emoji" {
            Some(WallPaper::from_emoji_id("🙂"))
        } else {
            WallPaper::from_serialized(&bytes)
        };
        assert_eq!(p.is_some(), v["ok"].as_bool().unwrap(), "{line}");
        if let Some(p) = p {
            assert_eq!(hex(&p.serialize()), v["canonical"], "{line}");
            assert_eq!(p.key(), v["key"], "{line}");
            assert_eq!(p.id().to_string(), v["id"]);
            assert_eq!(p.owner_id().bare.to_string(), v["owner"]);
            assert_eq!(
                p.gradient_rotation(),
                v["rotation"].as_i64().unwrap() as i32
            );
            assert_eq!(
                p.pattern_intensity(),
                v["intensity"].as_i64().unwrap() as i32
            );
            assert_eq!(p.is_blurred(), v["blurred"].as_bool().unwrap());
            assert_eq!(p.is_null(), v["null"].as_bool().unwrap());
            assert_eq!(tlhex(&p.mtp_settings()), v["settings"], "{line}");
            let colors = p
                .background_colors()
                .iter()
                .map(|c| format!("#{:06x}", c.serialized()))
                .collect::<Vec<_>>();
            assert_eq!(serde_json::json!(colors), v["colors"]);
        }
    }
}
#[test]
fn native_gift_attribute_tl_and_rarity_saved_id_values() {
    let scope = SessionScope::new();
    let peer = scope.peer_key(peer_from_chat(7u64));
    for line in include_str!("fixtures/data-themes/native.jsonl").lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        match v["op"].as_str().unwrap() {
            "rarity" => {
                let a = UniqueGiftAttribute {
                    name: "attr".into(),
                    rarity_value: v["value"].as_i64().unwrap() as i32,
                };
                assert_eq!(a.rarity_type().0, v["type"].as_i64().unwrap() as i32);
                assert_eq!(a.rarity_permille(), v["permille"].as_i64().unwrap() as i32);
                assert_eq!(
                    unique_gift_attribute_has_special_rarity(&a),
                    v["special"].as_bool().unwrap()
                );
                if let Some(bg) = v["bg"].as_str() {
                    let c = unique_gift_rarity_badge_colors(a.rarity_type());
                    assert_eq!(c.bg.rgba.to_string(), bg);
                    assert_eq!(c.fg.rgba.to_string(), v["fg"])
                }
            }
            "attribute" => {
                let id = GiftAttributeId {
                    value: v["value"].as_str().unwrap().parse().unwrap(),
                    type_: match v["kind"].as_i64().unwrap() {
                        0 => GiftAttributeIdType::Model,
                        1 => GiftAttributeIdType::Pattern,
                        _ => GiftAttributeIdType::Backdrop,
                    },
                };
                let wire = attribute_to_tl(id);
                assert_eq!(tlhex(&wire), v["wire"]);
                assert_eq!(attribute_from_tl(&wire).value.to_string(), v["back"])
            }
            "saved_id" => {
                let n = v["n"].as_str().unwrap().parse::<i64>().unwrap();
                let id = if v["chat"].as_bool().unwrap() {
                    SavedStarGiftId::chat(peer, n as u64)
                } else {
                    SavedStarGiftId::user(MsgId::new(n))
                };
                assert_eq!(id.is_user(), v["user"].as_bool().unwrap());
                assert_eq!(id.is_present(), v["present"].as_bool().unwrap());
                assert_eq!(id.user_message_id().bare.to_string(), v["message"]);
                assert_eq!(id.chat_saved_id().to_string(), v["saved"])
            }
            _ => {}
        }
    }
}
#[test]
fn export_rust_wallpapers_for_original_reader() {
    let p = default_wall_paper();
    let papers = [
        p.clone(),
        theme_wall_paper(),
        custom_wall_paper(),
        WallPaper::from_emoji_id("🙂"),
        WallPaper::from_legacy_id(-999),
        WallPaper::from_colors_slug("aAbBcC-DdEeFf")
            .unwrap()
            .with_blurred(true)
            .with_gradient_rotation(315)
            .with_pattern_intensity(-100),
        p.with_gradient_rotation(90),
    ];
    let reference: Vec<serde_json::Value> =
        include_str!("fixtures/data-themes/rust-in-original.jsonl")
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
    assert_eq!(reference.len(), papers.len());
    for (p, original) in papers.iter().zip(reference) {
        assert!(WallPaper::from_serialized(&p.serialize()).is_some());
        assert_eq!(hex(&p.serialize()), original["bytes"]);
        assert_eq!(original["bytes"], original["canonical"]);
        assert_eq!(original["ok"], true);
    }
    if let Ok(path) = std::env::var("AYUGRAM_THEMES_REFERENCE_OUTPUT") {
        let lines = papers
            .iter()
            .map(|p| hex(&p.serialize()))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(path, lines + "\n").unwrap();
    }
}
#[test]
fn wallpaper_factories_flags_equality_and_nonserialized_emoji() {
    let a = WallPaper::from_legacy_serialized(1, 2, 31, None);
    let b = WallPaper::from_legacy_serialized(99, 999, 31, Some(vec![]));
    assert!(a.equals(&b));
    assert!(a.is_pattern() && a.is_creator() && a.is_dark() && a.is_default());
    assert_eq!(a.pattern_opacity(), 0.5);
    assert_eq!(a.share_url(|s| format!("tg://{s}")), "");
    assert!(is_cloud_wall_paper(&a));
    for p in [
        theme_wall_paper(),
        custom_wall_paper(),
        legacy1_default_wall_paper(),
        details::uninitialized_wall_paper(),
        details::testing_theme_wall_paper(),
        details::testing_default_wall_paper(),
        details::testing_editor_wall_paper(),
    ] {
        assert!(!is_cloud_wall_paper(&p));
    }
    assert!(is_default_wall_paper(&default_wall_paper()));
    assert!(is_legacy2_default_wall_paper(&WallPaper::from_legacy_id(
        105
    )));
    assert!(is_legacy3_default_wall_paper(&WallPaper::new(
        LEGACY3_BACKGROUND
    )));
    assert!(is_legacy4_default_wall_paper(&WallPaper::new(
        LEGACY4_BACKGROUND
    )));
    let p = WallPaper::from_emoji_id("x");
    assert!(p.is_null());
    assert_eq!(
        WallPaper::from_serialized(&p.serialize())
            .unwrap()
            .emoji_id(),
        ""
    );
    assert!(WallPaper::from_serialized(&a.with_pattern_intensity(101).serialize()).is_none());
    let scope = SessionScope::new();
    let documents = Documents::new(&scope, Rc::new(FileTable::default()));
    let key = documents.ensure_document(7);
    let settings = WallPaper::from_colors_slug("112233-445566")
        .unwrap()
        .with_pattern_intensity(-25)
        .with_gradient_rotation(90)
        .with_blurred(true)
        .mtp_settings();
    let mut wire = tl::WallPaperData {
        flags: tl::WallPaperData::F_SETTINGS | tl::WallPaperData::F_PATTERN,
        id: 15,
        access_hash: 16,
        slug: b"cloud".to_vec(),
        document: tl::mtp_document_empty(7),
        settings: Some(settings.clone()),
    };
    assert!(WallPaper::create(UserId::new(1), &wire.clone().into(), |_| None).is_none());
    let p = WallPaper::create(UserId::new(1), &wire.clone().into(), |_| Some(key)).unwrap();
    assert_eq!(p.document(), Some(key));
    assert_eq!(p.gradient_rotation(), 90);
    assert_eq!(p.pattern_intensity(), -25);
    assert!(p.is_blurred());
    assert!(matches!(
        p.mtp_input(UserId::new(1)),
        tl::InputWallPaper::InputWallPaper(_)
    ));
    assert!(matches!(
        p.mtp_input(UserId::new(2)),
        tl::InputWallPaper::InputWallPaperSlug(_)
    ));
    let tg_core::data::data_file_origin::FileOrigin::Wallpaper(origin) = p.file_origin() else {
        panic!("wallpaper origin")
    };
    assert_eq!(
        (
            origin.paper_id,
            origin.access_hash,
            origin.owner_id,
            origin.slug.as_str()
        ),
        (15, 16, UserId::new(1), "cloud")
    );
    assert_eq!(
        p.share_url(|s| format!("tg://{s}")),
        "tg://bg/cloud?bg_color=112233-445566&intensity=-25&rotation=90&mode=blur"
    );
    wire.flags = tl::WallPaperData::F_SETTINGS;
    let p = WallPaper::create(UserId::new(1), &wire.into(), |_| Some(key)).unwrap();
    assert!(p.background_colors().is_empty());
    assert_eq!(p.key(), "bg/cloud?mode=blur&&intensity=-25");
    let no_file = tl::WallPaperNoFileData {
        id: 9,
        flags: tl::WallPaperNoFileData::F_SETTINGS,
        settings: Some(settings),
    };
    let p = WallPaper::create_no_file(&no_file);
    assert_eq!(p.pattern_intensity(), 50); // no-file ignores supplied intensity.
    assert_eq!(p.background_colors().len(), 2);
    assert_eq!(p.gradient_rotation(), 90);
}
#[test]
fn wallpaper_url_rules_rotation_and_qstring_nullness() {
    let a = WallPaper::from_colors_slug("112233-445566")
        .unwrap()
        .with_gradient_rotation(90);
    assert_eq!(
        a.with_params_from(&default_wall_paper())
            .gradient_rotation(),
        0
    ); // >=3 colors force dynamic rotation.
    let p = a.with_background_colors(vec![Color::rgb(0)]);
    assert_eq!(p.slug(), "000000");
    let q = WallPaper::from_legacy_serialized(9, 0, 0, Some("slug".encode_utf16().collect()))
        .with_url_params(&BTreeMap::from([
            ("gradient".into(), "000000~111111~222222".into()),
            ("rotation".into(), "315".into()),
            ("intensity".into(), "-100".into()),
        ]));
    assert_eq!(q.gradient_rotation(), 0);
    assert_eq!(q.pattern_intensity(), -100);
    let null = WallPaper::new(7).serialize();
    let empty = WallPaper::from_legacy_serialized(7, 0, 0, Some(vec![])).serialize();
    assert_ne!(null, empty);
    assert!(WallPaper::from_serialized(&null)
        .unwrap()
        .equals(&WallPaper::from_serialized(&empty).unwrap()));
    let surrogate = WallPaper::from_legacy_serialized(7, 0, 0, Some(vec![0xd800])).serialize();
    assert_eq!(
        WallPaper::from_serialized(&surrogate).unwrap().serialize(),
        surrogate
    );
}
#[derive(Default)]
struct FileTable {
    locations: RefCell<BTreeMap<(u64, u64), FileLocation>>,
}
impl FileLocations for FileTable {
    fn read(&self, key: (u64, u64)) -> FileLocation {
        self.locations
            .borrow()
            .get(&key)
            .cloned()
            .unwrap_or_default()
    }
    fn inspect(&self, l: &FileLocation) -> FileInfo {
        let m = std::fs::metadata(&l.fname).ok();
        FileInfo {
            path: l.fname.clone(),
            exists: m.is_some(),
            readable: std::fs::File::open(&l.fname).is_ok(),
            modified_ms: m
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64),
            size: m.map_or(0, |m| m.len() as i64),
        }
    }
}
struct MimeRegistry;
impl MimeDatabase for MimeRegistry {
    fn for_name(&self, n: &str) -> MimeType {
        assert!(matches!(
            n,
            "image/jpeg" | "image/png" | "application/octet-stream"
        ));
        MimeType {
            name: n.into(),
            glob_patterns: vec![if n == "image/jpeg" {
                "*.jpg"
            } else if n == "image/png" {
                "*.png"
            } else {
                "*"
            }
            .into()],
            filter_string: String::new(),
        }
    }
}
struct ThemeFacts {
    scope: SessionScopeId,
    documents: Documents,
    peers: Peers,
    current: RefCell<cloud::CloudTheme>,
    night: Cell<bool>,
    window: Cell<bool>,
    owns_paper: Cell<bool>,
    effects: RefCell<Vec<cloud::Effect>>,
    users: RefCell<Vec<tl::User>>,
    chats: RefCell<Vec<tl::Chat>>,
    gifts: RefCell<BTreeMap<u64, Rc<UniqueGift>>>,
}
impl cloud::ThemeContext for ThemeFacts {
    fn user_id(&self) -> UserId {
        UserId::new(1)
    }
    fn process_document(&self, v: &tl::Document) -> u64 {
        let k = self.documents.process_document(v, &MimeRegistry);
        self.documents.read(k, |d| d.id()).expect("stored document")
    }
    fn wall_paper(&self, v: &tl::WallPaper) -> Option<WallPaper> {
        create_with_documents(&self.documents, &MimeRegistry, UserId::new(1), v)
    }
    fn unique_gift(&self, v: &tl::StarGift) -> Option<Rc<UniqueGift>> {
        let tl::StarGift::StarGiftUnique(d) = v else {
            return None;
        };
        self.gifts.borrow().get(&d.id).cloned()
    }
    fn emoji_key(&self, s: &str) -> Option<Vec<u16>> {
        match s {
            "🙂" => Some(vec![1]),
            "☀" | "☀️" => Some(vec![2]),
            _ => None,
        }
    }
}
impl cloud::ThemeOwner for ThemeFacts {
    fn current_cloud(&self) -> cloud::CloudTheme {
        self.current.borrow().clone()
    }
    fn night_mode(&self) -> bool {
        self.night.get()
    }
    fn has_chat_window(&self) -> bool {
        self.window.get()
    }
    fn chat_theme_owns_paper(&self, _: &cloud::CloudTheme) -> bool {
        self.owns_paper.get()
    }
    fn effect(&self, e: cloud::Effect) {
        self.effects.borrow_mut().push(e.clone());
        if let cloud::Effect::SetCurrentCloud(c) = e {
            *self.current.borrow_mut() = c
        }
    }
    fn process_users(&self, v: &[tl::User]) {
        self.users.borrow_mut().extend_from_slice(v)
    }
    fn process_chats(&self, v: &[tl::Chat]) {
        self.chats.borrow_mut().extend_from_slice(v)
    }
}
fn setup() -> (
    SessionScope,
    Rc<ThemeFacts>,
    ManualClock,
    Rc<TimerHost>,
    Rc<cloud::CloudThemes>,
) {
    let scope = SessionScope::new();
    let c = Changes::with_queue(&scope, tg_runtime::MainQueue::new());
    let facts = Rc::new(ThemeFacts {
        scope: scope.id(),
        documents: Documents::new(&scope, Rc::new(FileTable::default())),
        peers: Peers::new_with_self(&scope, c, UserId::new(1)),
        current: RefCell::default(),
        night: Cell::new(false),
        window: Cell::new(true),
        owns_paper: Cell::new(false),
        effects: RefCell::new(vec![]),
        users: RefCell::new(vec![]),
        chats: RefCell::new(vec![]),
        gifts: RefCell::default(),
    });
    let clock = ManualClock::new(10000);
    let host = TimerHost::new(Arc::new(clock.clone()));
    let service = cloud::CloudThemes::new(&scope, facts.clone(), &host);
    (scope, facts, clock, host, service)
}
fn theme(id: u64, emoji: &str, settings: Vec<tl::ThemeSettings>) -> tl::Theme {
    tl::ThemeData {
        flags: tl::ThemeData::F_EMOTICON | tl::ThemeData::F_SETTINGS | tl::ThemeData::F_CREATOR,
        id,
        access_hash: 99,
        slug: b"slug".to_vec(),
        title: b"Title".to_vec(),
        emoticon: Some(emoji.as_bytes().to_vec()),
        settings: Some(settings),
        ..Default::default()
    }
    .into()
}
fn setting(dark: bool, accent: i32) -> tl::ThemeSettings {
    tl::ThemeSettingsData {
        base_theme: if dark {
            tl::mtp_base_theme_night()
        } else {
            tl::mtp_base_theme_day()
        },
        accent_color: accent,
        ..Default::default()
    }
    .into()
}
fn sends(o: &ThemeFacts) -> usize {
    o.effects
        .borrow()
        .iter()
        .filter(|e| matches!(e, cloud::Effect::Send { .. }))
        .count()
}
#[test]
fn cloud_parse_settings_first_type_wins_color_presence_and_paper() {
    let (_scope, o, _, _, _) = setup();
    let w = tl::WallPaperNoFileData {
        flags: tl::WallPaperNoFileData::F_SETTINGS,
        id: 7,
        settings: Some(default_wall_paper().mtp_settings()),
    }
    .into();
    let settings = tl::ThemeSettingsData {
        flags: tl::ThemeSettingsData::F_WALLPAPER | tl::ThemeSettingsData::F_OUTBOX_ACCENT_COLOR,
        wallpaper: Some(w),
        outbox_accent_color: Some(-1),
        ..tl::ThemeSettingsData::default()
    }
    .into();
    let parsed = cloud::CloudTheme::parse(
        o.as_ref(),
        &theme(
            5,
            "🙂",
            vec![settings, setting(false, 0x112233), setting(true, 0x445566)],
        ),
        true,
    );
    assert_eq!(parsed.created_by, UserId::new(1));
    assert_eq!(parsed.settings.len(), 2);
    let light = &parsed.settings[&cloud::CloudThemeType::Light];
    assert_eq!(light.accent_color, Color::rgb(0));
    assert_eq!(light.outgoing_accent_color, Some(Color::rgb(0xffffff)));
    assert!(light.paper.is_some());
    assert!(
        cloud::CloudTheme::parse(o.as_ref(), &theme(5, "🙂", vec![]), false)
            .settings
            .is_empty()
    );
}
#[test]
fn cloud_refresh_hash_notmodified_failure_and_reentrant_updates() {
    let (_scope, o, _, _, s) = setup();
    s.refresh();
    s.refresh();
    assert_eq!(sends(&o), 1);
    let l = Lifetime::new();
    let weak = Rc::downgrade(&s);
    s.updated().start(
        move |_| {
            weak.upgrade().unwrap().list();
        },
        |_| {},
        || {},
        &l,
    );
    let result = tl::mtp_account_themes(88, vec![theme(1, "🙂", vec![setting(false, 1)])]);
    s.themes_reply(&result);
    assert_eq!(s.list().len(), 1);
    s.refresh();
    assert!(o.effects.borrow().iter().any(
        |e| matches!(e,cloud::Effect::Send{request:cloud::Request::Themes(r),..}if r.hash==88)
    ));
    s.themes_reply(&tl::mtp_account_themes_not_modified());
    s.refresh_failed();
    s.saved_from_editor(cloud::CloudTheme {
        id: 2,
        ..Default::default()
    });
    assert_eq!(s.list()[0].id, 2);
    s.remove(2);
    assert_eq!(s.list()[0].id, 1);
}
#[test]
fn cloud_token_stream_one_shot_testing_and_unknown_emoji() {
    let (_scope, o, _, _, s) = setup();
    cloud::CloudThemes::set_testing_colors(false);
    let l = Lifetime::new();
    let v = collect(s.theme_for_token_value("🙂".into()), &l);
    assert!(v.borrow()[0].is_none());
    s.chat_themes_reply(&tl::mtp_account_themes(9, vec![theme(1, "🙂", vec![])]));
    assert_eq!(v.borrow().last().unwrap().as_ref().unwrap().id, 1);
    s.chat_themes_reply(&tl::mtp_account_themes(10, vec![theme(2, "🙂", vec![])]));
    assert_eq!(v.borrow().len(), 2);
    let unknown = collect(s.theme_for_token_value("none".into()), &l);
    assert!(unknown.borrow()[0].is_none());
    let n = sends(&o);
    s.refresh_chat_themes_for("gift:7");
    assert_eq!(sends(&o), n);
    cloud::CloudThemes::set_testing_colors(true);
    let vv = collect(s.theme_for_token_value("🙂".into()), &l);
    s.chat_themes_reply(&tl::mtp_account_themes(11, vec![theme(3, "🙂", vec![])]));
    assert_eq!(vv.borrow().last().unwrap().as_ref().unwrap().id, 3);
    cloud::CloudThemes::set_testing_colors(false);
}
#[test]
fn cloud_install_reload_virtual_clock_and_update_effects() {
    let (_scope, o, clock, host, s) = setup();
    let mut c = cloud::CloudTheme {
        id: 7,
        access_hash: 3,
        document_id: 9,
        ..Default::default()
    };
    *o.current.borrow_mut() = c.clone();
    s.setup_reload();
    clock.advance(9999);
    host.process();
    assert_eq!(sends(&o), 0);
    clock.advance(1);
    host.process();
    assert_eq!(sends(&o), 1);
    s.install();
    s.install();
    assert_eq!(sends(&o), 2);
    o.night.set(true);
    s.install();
    assert_eq!(sends(&o), 3);
    s.reload_failed();
    clock.advance(3600000);
    host.process();
    assert_eq!(sends(&o), 4);
    c.document_id = 0;
    c.emoticon = "🙂".into();
    *o.current.borrow_mut() = c;
    s.applying_theme();
    s.apply_update(&theme(7, "🙂", vec![setting(false, 1)]));
    assert!(o
        .effects
        .borrow()
        .iter()
        .any(|e| matches!(e, cloud::Effect::SetCurrentCloud(_))));
    assert!(o
        .effects
        .borrow()
        .iter()
        .any(|e| matches!(e, cloud::Effect::CheckChatThemeWallPaper)));
}
#[test]
fn cloud_testing_link_source_missing_equals_and_ignored_dark_prefix() {
    let (_scope, o, _, _, s) = setup();
    s.chat_themes_reply(&tl::mtp_account_themes(
        1,
        vec![theme(
            2,
            "🙂",
            vec![setting(true, 0x112233), setting(false, 0x445566)],
        )],
    ));
    let mut c = s.chat_themes()[0].clone();
    c.settings
        .get_mut(&cloud::CloudThemeType::Dark)
        .unwrap()
        .outgoing_accent_color = Some(Color::rgb(0xabcdef));
    let link = cloud::prepare_testing_link(&c);
    assert!(link.contains("dark_out_accentabcdef") && !link.contains("dark_out_accent="));
    cloud::CloudThemes::set_testing_colors(true);
    let p = BTreeMap::from([
        ("dark_accent".into(), "ffffff".into()),
        ("accent".into(), "010203tail".into()),
        ("out_bg".into(), "112233,445566".into()),
    ]);
    let updated = s.update_theme_from_link("🙂", &p).unwrap();
    assert_eq!(
        updated.settings[&cloud::CloudThemeType::Dark].accent_color,
        Color::rgb(0x010203)
    );
    assert_eq!(
        updated.settings[&cloud::CloudThemeType::Light].accent_color,
        Color::rgb(0x010203)
    );
    assert!(s.update_theme_from_link("?", &p).is_none());
    assert_eq!(o.scope, s.session());
    cloud::CloudThemes::set_testing_colors(false);
}
fn gift(o: &ThemeFacts, id: u64) -> Rc<UniqueGift> {
    let d = o.documents.ensure_document(id);
    Rc::new(UniqueGift::new(
        id,
        UniqueGiftModel {
            attribute: Default::default(),
            document: d,
        },
        UniqueGiftPattern {
            attribute: Default::default(),
            document: d,
        },
        UniqueGiftBackdrop::default(),
    ))
}
#[test]
fn cloud_gift_theme_pagination_notmodified_and_captured_gift_parser() {
    let (_scope, o, _, _, s) = setup();
    let g = gift(&o, 7);
    o.gifts.borrow_mut().insert(7, g);
    let d = tl::ChatThemeUniqueGiftData {
        gift: tl::StarGiftUniqueData {
            id: 7,
            ..Default::default()
        }
        .into(),
        theme_settings: vec![setting(false, 1)],
    };
    let token = s.process_gift_theme_get_token(&d);
    assert_eq!(token, "gift:7");
    assert_eq!(s.theme_for_token("gift:7").unwrap().id, 7);
    s.my_gift_themes_load_more(false);
    let result = tl::AccountChatThemesData {
        flags: tl::AccountChatThemesData::F_NEXT_OFFSET,
        hash: 22,
        themes: vec![d.into()],
        next_offset: Some(b"next".to_vec()),
        ..Default::default()
    }
    .into();
    s.gift_themes_reply(false, &result);
    assert_eq!(s.my_gift_themes_tokens(), ["gift:7"]);
    assert!(s.my_gift_themes_ready());
    s.my_gift_themes_load_more(false);
    s.gift_themes_reply(false, &tl::mtp_account_chat_themes_not_modified());
    s.my_gift_themes_load_more(true);
    s.gift_themes_failed();
    assert!(s.my_gift_themes_ready());
}
#[test]
fn report_statistics_subscription_game_and_color_values() {
    let a = ReportInput {
        option_id: vec![1],
        comment: "x".into(),
        ..Default::default()
    };
    let mut b = a.clone();
    b.option_text = "ignored".into();
    b.ids.push(MsgId::new(77));
    assert_eq!(a, b);
    b.comment = "changed".into();
    assert_ne!(a, b);
    let mut c = ChannelStatistics::default();
    assert!(c.empty());
    c.start_date = -1;
    c.end_date = 2;
    assert!(c.is_present());
    let mut supergroup = tg_core::data::data_statistics::SupergroupStatistics::default();
    assert!(supergroup.empty());
    supergroup.start_date = 1;
    assert!(!supergroup.is_present());
    supergroup.end_date = -1;
    assert!(supergroup.is_present());
    let mut m = MessageStatistics::default();
    assert!(!m.is_present());
    m.views = -1;
    assert!(m.is_present());
    assert!(!PeerSubscription {
        credits: 1,
        period: 0
    }
    .is_present());
    assert!(PeerSubscription {
        credits: 1,
        period: 1
    }
    .is_present());
    let s = SubscriptionEntry {
        id: "id".into(),
        ..Default::default()
    };
    assert!(s.is_present());
    let post = RecentPostId {
        message_id: FullMsgId::new(peer_from_user(1u64), MsgId::new(1)),
        ..Default::default()
    };
    assert!(post.valid());
    let scope = SessionScope::new();
    let g = GameData::new(scope.id(), 8);
    assert_eq!((g.owner, g.id), (scope.id(), 8));
    assert!(g.photo.is_none() && g.document.is_none());
    assert_ne!(Color::default(), Color::rgb(0));
    let p = ColorProfileData::default();
    assert!(p.light.palette.is_empty());
}
#[test]
fn media_rotation_stable_keys_convert_and_negative_multiples() {
    let (scope, o, _, _, _) = setup();
    let d = o.documents.ensure_document(1);
    let photos = Photos::new(&scope);
    let p = photos.ensure_photo(1);
    let mut r = MediaRotation::default();
    for (v, expected) in [
        (-721, 359),
        (-360, 0),
        (-1, 359),
        (0, 0),
        (90, 90),
        (360, 0),
        (721, 1),
        (i32::MIN, 232),
    ] {
        r.set_photo(p, v);
        r.set_document(d, v);
        assert_eq!(r.get_photo(p), expected);
        assert_eq!(r.get_document(d), expected);
    }
    r.set_document(d, 90);
    o.documents
        .document_convert(d, &tl::mtp_document_empty(2), &MimeRegistry, false);
    assert_eq!(o.documents.read(d, |d| d.id()), Some(2));
    assert_eq!(r.get_document(d), 90);
}
#[test]
fn gift_values_ids_default_sort_pointer_equality_and_real_document_ids() {
    let (scope, o, _, _, _) = setup();
    let unique = gift(&o, 7);
    let d = o.documents.ensure_document(7);
    let mut a = StarGift::new(d);
    let mut b = a.clone();
    a.unique = Some(unique.clone());
    b.unique = Some(unique.clone());
    assert_eq!(a, b);
    b.unique = Some(gift(&o, 7));
    assert_ne!(a, b);
    a.auction_slug = "auction".into();
    assert!(a.auction());
    assert_eq!(unique.nano_ton_for_resale, -1);
    assert_eq!(ResaleGiftsFilter::default().sort, ResaleGiftsSort::Price);
    let id = id_for_model(&unique.model, |k| {
        o.documents.read(k, |d| d.id()).expect("stored document")
    });
    assert_eq!(id.value, 7);
    let back = StarGiftBackground {
        center: Color::rgb(1),
        edge: Color::rgb(2),
        text: Color::rgb(3),
    }
    .backdrop();
    assert_eq!(back.pattern_color, back.edge_color);
    let peer = scope.peer_key(peer_from_chat(7u64));
    let cid = SavedStarGiftId::chat(peer, 3);
    assert!(cid.is_chat());
    assert_eq!(cid.chat_peer(), Some(peer));
    assert!(matches!(
        input_saved_star_gift_id(cid, |_| tl::mtp_input_peer_chat(7)),
        tl::InputSavedStarGift::InputSavedStarGiftChat(_)
    ));
    let filter = ResaleGiftsFilter {
        attributes: BTreeMap::from([(id.clone(), ())]).into_keys().collect(),
        for_craft: true,
        ..Default::default()
    };
    let r = resale_gifts_request(9, &filter, "next".into());
    assert!(r.flags.contains(
        tl::PaymentsGetResaleStarGifts::F_SORT_BY_PRICE
            | tl::PaymentsGetResaleStarGifts::F_FOR_CRAFT
    ));
    assert_eq!(r.limit, 50);
    assert_eq!(
        my_unique_gifts_request(
            tl::mtp_input_peer_self(),
            MyUniqueType::OnlyOwned,
            String::new()
        )
        .limit,
        50
    );
    assert_eq!(craft_gifts_request(3, String::new()).limit, 50);
    assert_eq!(
        unique_gift_name("Gift", 1000, |n| format!("{n}")),
        "Gift #1000"
    );
    let spinner = GiftUpgradeSpinner::default();
    assert_eq!(spinner.state.current(), GiftUpgradeState::Initial);
}
#[test]
fn recent_inline_read_bump_limit_rotate_remove_and_reentry() {
    let (scope, o, _, _, _) = setup();
    let effects = Rc::new(RefCell::new(vec![]));
    let output = effects.clone();
    let r = Rc::new(inline::RecentInlineBots::new(
        scope.id(),
        Rc::new(move |e| output.borrow_mut().push(e)),
    ));
    let l = Lifetime::new();
    let weak = Rc::downgrade(&r);
    r.updates().start(
        move |_| {
            weak.upgrade().unwrap().list();
        },
        |_| {},
        || {},
        &l,
    );
    let keys = (1..=12)
        .map(|i| o.peers.ensure_user(UserId::new(i)))
        .collect::<Vec<_>>();
    for k in &keys {
        r.bump(*k)
    }
    assert_eq!(r.list().len(), 10);
    assert_eq!(r.list()[0], keys[11]);
    r.bump(keys[3]);
    assert_eq!(r.list()[0], keys[3]);
    r.remove(keys[3]);
    assert_eq!(r.list().len(), 9);
    r.apply_local(vec![keys[0]; 12]);
    r.bump(keys[1]);
    assert_eq!(r.list().len(), 12); // Original pops one rather than truncating oversized saved input.
    assert!(effects
        .borrow()
        .contains(&inline::Effect::WriteRecentHashtagsAndBots));
}
#[test]
fn location_registry_weak_cleanup_early_return_and_duplicates() {
    let mut r = LocationPickers::<u64, Cell<u32>>::default();
    let a = Rc::new(Cell::new(1));
    let b = Rc::new(Cell::new(2));
    r.emplace(7, &a);
    r.emplace(7, &b);
    assert_eq!(r.lookup(&7).unwrap().get(), 1);
    drop(a);
    assert_eq!(r.lookup(&7).unwrap().get(), 2);
    drop(b);
    assert!(r.lookup(&7).is_none());
}
fn saved(o: &ThemeFacts, n: i64, pinned: bool) -> SavedStarGift {
    let mut g = SavedStarGift::new(
        StarGift::new(o.documents.ensure_document(n as u64)),
        SavedStarGiftId::user(MsgId::new(n)),
    );
    g.pinned = pinned;
    g
}
#[test]
fn gifts_cache_cold_callbacks_warm_threshold_and_reentrant_reads() {
    let (scope, o, clock, _, _) = setup();
    let output = Rc::new(RefCell::new(vec![]));
    let e = output.clone();
    let clk = clock.clone();
    let r = gifts::RecentSharedMediaGifts::new(
        scope.id(),
        Rc::new(move || tg_base::timer::Clock::now(&clk)),
        Rc::new(move |x| e.borrow_mut().push(x)),
    );
    let peer = scope.peer_key(peer_from_user(1u64));
    let l = Rc::new(RefCell::new(vec![]));
    let ll = l.clone();
    r.request(
        peer,
        tl::mtp_input_peer_self(),
        Rc::new(move |v| ll.borrow_mut().push(v.len())),
        false,
    );
    let ll = l.clone();
    r.request(
        peer,
        tl::mtp_input_peer_self(),
        Rc::new(move |v| ll.borrow_mut().push(v.len())),
        true,
    );
    assert_eq!(output.borrow().len(), 1);
    r.received(peer, (1..=8).map(|n| saved(&o, n, n % 2 == 0)).collect());
    assert_eq!(*l.borrow(), [3, 4]);
    let ll = l.clone();
    r.request(
        peer,
        tl::mtp_input_peer_self(),
        Rc::new(move |v| ll.borrow_mut().push(v.len())),
        false,
    );
    assert_eq!(output.borrow().len(), 1);
    clock.advance(60000);
    let ll = l.clone();
    r.request(
        peer,
        tl::mtp_input_peer_self(),
        Rc::new(move |v| ll.borrow_mut().push(v.len())),
        false,
    );
    assert_eq!(output.borrow().len(), 2);
    r.clear_last_request_time(peer);
    // A callback starts another request when lastRequestTime is zero. The new
    // request's first completion must wait for its own response, like C++'s capture.
    let zero_time = Rc::new(Cell::new(0));
    let t = zero_time.clone();
    let requests = Rc::new(Cell::new(0));
    let q = requests.clone();
    let fresh = gifts::RecentSharedMediaGifts::new(
        scope.id(),
        Rc::new(move || t.get()),
        Rc::new(move |_| q.set(q.get() + 1)),
    );
    let completed = Rc::new(Cell::new(0));
    let weak = Rc::downgrade(&fresh);
    let c = completed.clone();
    fresh.request(
        peer,
        tl::mtp_input_peer_self(),
        Rc::new(move |_| {
            let c = c.clone();
            weak.upgrade().unwrap().request(
                peer,
                tl::mtp_input_peer_self(),
                Rc::new(move |_| c.set(c.get() + 1)),
                false,
            );
        }),
        false,
    );
    fresh.received(peer, vec![]);
    assert_eq!(requests.get(), 2);
    assert_eq!(completed.get(), 0);
    zero_time.set(1);
    fresh.received(peer, vec![]);
    assert_eq!(completed.get(), 1);
    assert_eq!(
        gifts::RecentSharedMediaGifts::filter_gifts(
            &(1..=8).map(|n| saved(&o, n, true)).collect::<VecDeque<_>>(),
            true
        )
        .len(),
        6
    );
}
#[test]
fn gifts_pin_fetch_update_order_unpin_and_boundary_limit_one() {
    let (scope, o, clock, _, _) = setup();
    let events = Rc::new(RefCell::new(vec![]));
    let e = events.clone();
    let clk = clock.clone();
    let r = gifts::RecentSharedMediaGifts::new(
        scope.id(),
        Rc::new(move || tg_base::timer::Clock::now(&clk)),
        Rc::new(move |v| e.borrow_mut().push(v)),
    );
    let p = scope.peer_key(peer_from_user(1u64));
    let one = saved(&o, 1, true);
    let two = saved(&o, 2, true);
    r.received(p, vec![one.clone(), two.clone()]);
    let new = saved(&o, 3, false);
    assert_eq!(
        gifts::RecentSharedMediaGifts::toggle_ids(
            &[one.clone(), two.clone()],
            new.manage_id,
            true,
            1
        )
        .len(),
        2
    );
    r.toggle_pinned(p, tl::mtp_input_peer_self(), new.manage_id, true, 2);
    let id = match events.borrow().last().unwrap() {
        gifts::Effect::FetchGift { id, .. } => *id,
        _ => panic!("fetch"),
    };
    r.pin_gift_received(id, Some(new));
    let id = match events.borrow().last().unwrap() {
        gifts::Effect::UpdatePinned { id, request, .. } => {
            assert_eq!(request.stargift.len(), 2);
            *id
        }
        _ => panic!("order"),
    };
    r.order_succeeded(id);
    assert!(matches!(
        events.borrow().last(),
        Some(gifts::Effect::GiftUpdate { pinned: true, .. })
    ));
    r.reorder_pinned(p, tl::mtp_input_peer_self(), 0, 1);
    let id = match events.borrow().last().unwrap() {
        gifts::Effect::UpdatePinned { id, .. } => *id,
        _ => panic!("reorder"),
    };
    r.order_succeeded(id);
    assert!(gifts::RecentSharedMediaGifts::reorder_ids(&[one], 0, 0).is_none());
    r.toggle_pinned(p, tl::mtp_input_peer_self(), two.manage_id, false, 2);
    let id = match events.borrow().last().unwrap() {
        gifts::Effect::UpdatePinned { id, .. } => *id,
        _ => panic!("unpin"),
    };
    r.order_failed(id, "ERROR".into());
    assert!(matches!(
        events.borrow().last(),
        Some(gifts::Effect::GiftError(_))
    ));
}
#[test]
fn actual_sender_theme_and_gift_requests_without_network() {
    use std::sync::Mutex;
    use tg_mtproto::{
        mtp_instance::{get_next_request_id, Fields, Instance, Mode},
        mtproto_dc_options::Environment,
        mtproto_response::Response,
        sender::Sender,
    };
    let i = Instance::new(Mode::Normal, Fields::new(Environment::Test));
    let sender = Sender::new(i.clone());
    let out = Arc::new(Mutex::new(vec![]));
    let id = get_next_request_id();
    let o = out.clone();
    cloud::send_request(
        &sender,
        id,
        cloud::Request::Save(tl::AccountSaveTheme::new(tl::mtp_input_theme(1, 2), true)),
        Arc::new(move |v| o.lock().unwrap().push(v)),
    );
    i.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&true),
    });
    assert!(out.lock().unwrap()[0].is_ok());
    let id = get_next_request_id();
    let o = out.clone();
    gifts::send_request(
        &sender,
        gifts::Effect::UpdatePinned {
            id,
            peer: SessionScope::new().peer_key(peer_from_user(1u64)),
            request: tl::PaymentsToggleStarGiftsPinnedToTop::new(tl::mtp_input_peer_self(), vec![]),
        },
        Arc::new(move |_, r| o.lock().unwrap().push(r)),
    );
    i.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&tl::mtp_rpc_error(400, b"ERROR".to_vec())),
    });
    assert_eq!(out.lock().unwrap()[1], Err("ERROR".into()));
}

#[test]
fn resale_attribute_counts_first_duplicate_zero_missing_and_document_identity() {
    let (_scope, owner, _clock, _host, _service) = setup();
    let unique = gift(&owner, 77);
    let model_id = id_for_model(&unique.model, |k| {
        owner.documents.read(k, |d| d.id()).unwrap()
    });
    let counter = |id, count| {
        tl::StarGiftAttributeCounterData {
            attribute: attribute_to_tl(id),
            count,
        }
        .into()
    };
    let counters = [
        counter(model_id.clone(), 7),
        counter(model_id, 99),
        counter(id_for_backdrop(&unique.backdrop), -1),
    ];
    let mut descriptor = ResaleGiftsDescriptor::default();
    descriptor.set_attributes(
        UniqueGiftAttributes {
            models: vec![unique.model.clone()],
            patterns: vec![unique.pattern.clone()],
            backdrops: vec![unique.backdrop.clone()],
        },
        &counters,
        |k| owner.documents.read(k, |d| d.id()).unwrap(),
    );
    assert_eq!(descriptor.models[0].count, 7);
    assert_eq!(descriptor.patterns[0].count, 0);
    assert_eq!(descriptor.backdrops[0].count, -1);
}
