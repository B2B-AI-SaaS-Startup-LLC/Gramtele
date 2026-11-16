use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::{Arc, Mutex},
};
use tg_base::timer::{ManualClock, TimerHost};
use tg_core::{
    core::{
        file_location::{FileInfo, FileLocation, FileLocations},
        mime_type::{FilenamePlatform, NameType},
    },
    data::{
        data_ai_compose_tones as ai,
        data_birthday::CalendarDate,
        data_changes::Changes,
        data_community as community, data_document_resolver as resolve, data_download_manager as d,
        data_file_click_handler as click,
        data_media_store::{Documents, Photos},
        data_msg_id::*,
        data_peer_id::*,
        data_search_calendar as calendar, data_send_action as action,
        data_session::Peers,
        data_store_keys::*,
    },
    storage::storage_shared_media::SharedMediaType,
};
use tg_rpl::{Lifetime, Producer};
use tg_scheme as tl;
fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
fn hex(v: &[u8]) -> String {
    v.iter().map(|b| format!("{b:02x}")).collect()
}
fn collect<T: Clone + 'static>(p: Producer<T>, l: &Lifetime) -> Rc<RefCell<Vec<T>>> {
    let out = Rc::new(RefCell::new(vec![]));
    let v = out.clone();
    p.start(move |x| v.borrow_mut().push(x), |_| {}, || {}, l);
    out
}
#[derive(Default)]
struct Files(RefCell<BTreeMap<(u64, u64), FileLocation>>);
impl FileLocations for Files {
    fn read(&self, k: (u64, u64)) -> FileLocation {
        self.0.borrow().get(&k).cloned().unwrap_or_default()
    }
    fn inspect(&self, l: &FileLocation) -> FileInfo {
        let m = std::fs::metadata(&l.fname);
        FileInfo {
            path: l.fname.clone(),
            exists: m.is_ok(),
            readable: std::fs::File::open(&l.fname).is_ok(),
            size: m.map(|m| m.len() as i64).unwrap_or_default(),
            modified_ms: None,
        }
    }
}
struct Fixture {
    scope: SessionScope,
    documents: Documents,
    photos: Photos,
    messages: RefCell<MessageKeys>,
    clock: ManualClock,
    host: Rc<TimerHost>,
}
impl Fixture {
    fn new() -> Self {
        let scope = SessionScope::new();
        let documents = Documents::new(&scope, Rc::new(Files::default()));
        let photos = Photos::new(&scope);
        let messages = RefCell::new(MessageKeys::new(&scope));
        let clock = ManualClock::new(10000);
        let host = TimerHost::new(Arc::new(clock.clone()));
        Self {
            scope,
            documents,
            photos,
            messages,
            clock,
            host,
        }
    }
    fn object(&self, id: i64) -> d::DownloadObject {
        d::DownloadObject {
            item: self
                .messages
                .borrow_mut()
                .insert(FullMsgId::new(peer_from_user(7u64), MsgId::new(id))),
            document: Some(self.documents.ensure_document(id as u64)),
            photo: None,
        }
    }
    fn loaded(&self, o: d::DownloadObject, size: i64) -> d::DownloadedId {
        d::DownloadedId {
            download: d::DownloadId {
                object_id: self
                    .documents
                    .read(o.document.unwrap(), |d| d.id())
                    .unwrap(),
                type_: d::DownloadType::Document,
            },
            started: 17,
            path: Some("/tmp/file".encode_utf16().collect()),
            size,
            item_id: o.item.id(),
            peer_access_hash: 18,
            object: Some(o),
        }
    }
    fn manager(&self) -> (Rc<d::DownloadManager>, Rc<RefCell<Vec<d::Effect>>>) {
        let events = Rc::new(RefCell::new(vec![]));
        let e = events.clone();
        let m = d::DownloadManager::new(
            Rc::new(|| 1700000000),
            Rc::new(move |v| e.borrow_mut().push(v)),
            &self.host,
        );
        m.track_session(self.scope.id(), &[]);
        (m, events)
    }
}
#[test]
fn native_download_codec_bytes_dates_legacy_corruption() {
    for line in include_str!("fixtures/data-downloads/native.jsonl").lines() {
        let r: serde_json::Value = serde_json::from_str(line).unwrap();
        if r["op"] == "date" {
            let n = r["value"].as_str().unwrap().parse().unwrap();
            assert_eq!(
                d::date_from_download_date(n),
                r["date"].as_i64().unwrap() as i32
            );
            continue;
        }
        let bytes = unhex(r["bytes"].as_str().unwrap());
        let list = d::deserialize(&bytes);
        assert_eq!(hex(&d::serialize(&list)), r["canonical"], "{r}");
        let rows = r["rows"].as_array().unwrap();
        assert_eq!(list.len(), rows.len());
        for (v, o) in list.iter().zip(rows) {
            assert_eq!(v.download.object_id.to_string(), o["id"]);
            assert_eq!(v.size.to_string(), o["size"]);
            assert_eq!(v.started.to_string(), o["started"]);
            assert_eq!(v.item_id.peer.value.to_string(), o["peer"]);
            assert_eq!(v.item_id.msg.bare.to_string(), o["msg"]);
            assert_eq!(v.peer_access_hash.to_string(), o["hash"]);
        }
    }
}
#[test]
fn codec_full_rejection_trailing_raw_qstring_and_inverse_export() {
    let f = Fixture::new();
    let mut id = f.loaded(f.object(1), d::MAX_FILE_SIZE);
    id.path = Some(vec![0xd800]);
    let b = d::serialize(&[id.clone()]);
    assert_eq!(d::serialize(&d::deserialize(&b)), b);
    let mut records = vec![b];
    for size in [1, d::MAX_FILE_SIZE, 4294967297] {
        id.size = size;
        id.path = Some("путь🙂".encode_utf16().collect());
        let b = d::serialize(&[id.clone()]);
        records.push(b);
    }
    let original = include_str!("fixtures/data-downloads/rust-in-original.jsonl")
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(original.len(), records.len());
    for (bytes, row) in records.iter().zip(original) {
        assert_eq!(hex(bytes), row["bytes"]);
        assert_eq!(row["canonical"], row["bytes"]);
    }
    let mut good = records[1].clone();
    good.extend(b"tail");
    assert_eq!(d::deserialize(&good).len(), 1);
    for i in 0..records[1].len() {
        assert!(d::deserialize(&records[1][..i]).is_empty());
    }
    id.path = None;
    let bytes = d::serialize(&[f.loaded(f.object(2), 2), id]);
    assert!(d::deserialize(&bytes).is_empty());
    if let Ok(p) = std::env::var("AYUGRAM_DOWNLOADS_REFERENCE_OUTPUT") {
        std::fs::write(
            p,
            records
                .iter()
                .map(|r| hex(r))
                .collect::<Vec<_>>()
                .join("\n")
                + "\n",
        )
        .unwrap();
    }
}
#[test]
fn progress_normal_completion_clear_virtual_and_reentry() {
    let f = Fixture::new();
    let (m, _) = f.manager();
    let o = f.object(1);
    let life = Lifetime::new();
    let seen = collect(m.loading_progress_value(), &life);
    let weak = Rc::downgrade(&m);
    m.loading_list_changes().start(
        move |_| {
            weak.upgrade().unwrap().loading_list();
        },
        |_| {},
        || {},
        &life,
    );
    assert!(m.add_loading(o, "load".into(), 100, true));
    assert!(!m.add_loading(o, "load".into(), 100, true));
    m.check_progress(o.item, true, false, true, 23, 120, None);
    assert_eq!(
        m.loading_progress(),
        d::DownloadProgress {
            ready: 23,
            total: 120
        }
    );
    m.item_visibilities_updated(f.scope.id(), |_| false);
    assert!(!m.loading_list()[0].hidden_by_view);
    m.add_loaded(f.loaded(o, 120), 120);
    assert!(!m.loading_in_progress(None));
    f.clock.advance(4999);
    f.host.process();
    assert_eq!(m.loading_list().len(), 1);
    f.clock.advance(1);
    f.host.process();
    assert!(m.loading_list().is_empty());
    assert_eq!(
        seen.borrow().last().unwrap(),
        &d::DownloadProgress::default()
    );
}
#[test]
fn external_adjust_cancel_finish_and_remove_without_callback() {
    let f = Fixture::new();
    let (m, e) = f.manager();
    let o = f.object(1);
    let calls = Rc::new(Cell::new(0));
    let c = calls.clone();
    m.add_loading_external(o, "path".into(), 100, Rc::new(move || c.set(c.get() + 1)));
    m.update_loading_external(o.item, 20, 200);
    assert_eq!(m.loading_external_state(o.item).unwrap().total, 200);
    m.finish_loading_external(o.item, f.loaded(o, 200));
    assert!(m.loading_external_state(o.item).unwrap().done);
    m.update_loading_external(o.item, 1, 1);
    assert_eq!(m.loading_progress().ready, 200);
    assert!(e
        .borrow()
        .iter()
        .any(|e| matches!(e, d::Effect::Repaint(_))));
    m.clear_if_finished();
    assert_eq!(calls.get(), 0);
    m.add_loading_external(
        o,
        "path".into(),
        3,
        Rc::new({
            let c = calls.clone();
            move || c.set(c.get() + 1)
        }),
    );
    m.cancel_loading_external(o.item);
    assert_eq!(calls.get(), 1);
    m.add_loading_external(
        o,
        "path".into(),
        3,
        Rc::new({
            let c = calls.clone();
            move || c.set(c.get() + 1)
        }),
    );
    m.remove_loading_external(o.item);
    assert_eq!(calls.get(), 1);
}
#[test]
fn all_downloads_for_one_message_delete_and_detach_last_reference() {
    let f = Fixture::new();
    let (m, e) = f.manager();
    let o = f.object(1);
    let mut id = f.loaded(o, 5);
    m.add_loaded(id.clone(), 5);
    id.download.object_id = 9;
    m.add_loaded(id, 5);
    let l = Lifetime::new();
    let removed = collect(m.loaded_removed(), &l);
    let replacement = f.object(2).item;
    m.detached(f.scope.id(), 0, replacement);
    assert!(removed.borrow().is_empty());
    m.detached(f.scope.id(), 1, replacement);
    assert_eq!(*removed.borrow(), [o.item]);
    m.delete_files(&[replacement], |_| false);
    assert_eq!(
        e.borrow()
            .iter()
            .filter(|e| matches!(e, d::Effect::DeleteFile { .. }))
            .count(),
        1
    );
    assert!(m.empty());
}
#[test]
fn download_resolve_reverse_batches_invalid_files_grouping_and_sticky_done() {
    let f = Fixture::new();
    let (m, e) = f.manager();
    let ids = (1..=101)
        .map(|n| {
            let mut d = f.loaded(f.object(n), n);
            d.object = None;
            d.item_id.peer = if n % 2 == 0 {
                peer_from_channel(77u64)
            } else {
                peer_from_user(7u64)
            };
            d
        })
        .collect::<Vec<_>>();
    m.track_session(f.scope.id(), &d::serialize(&ids));
    let requests = m.resolve(
        f.scope.id(),
        peer_from_user(1u64),
        |id| if id.size == 2 { None } else { Some(id.size) },
        |_| false,
    );
    assert_eq!(requests.len(), 2);
    assert!(!m.resolve_request_finished(f.scope.id()));
    assert!(m.resolve_request_finished(f.scope.id()));
    m.resolve_requests_finished(f.scope.id(), |_| None);
    assert_eq!(
        e.borrow()
            .iter()
            .filter(|e| matches!(e, d::Effect::GenerateEntry { .. }))
            .count(),
        99
    );
    let requests = m.resolve(
        f.scope.id(),
        peer_from_user(1u64),
        |id| Some(id.size),
        |_| false,
    );
    assert_eq!(requests.len(), 1);
    m.resolve_request_finished(f.scope.id());
    m.resolve_requests_finished(f.scope.id(), |_| None);
    assert_eq!(d::deserialize(&m.serialized(f.scope.id())).len(), 100);
    let l = Lifetime::new();
    assert_eq!(*collect(m.loaded_resolve_done(), &l).borrow(), [()]);
}
#[test]
fn download_cancel_admin_stop_untrack_serializator_and_delete_all() {
    let f = Fixture::new();
    let (m, e) = f.manager();
    let serialize = m.serializator(&f.scope);
    let o = f.object(1);
    m.add_loading(o, "p".into(), 4, false);
    m.cancel(o.item, true);
    assert!(!e.borrow().iter().any(|e| matches!(e, d::Effect::Cancel(_))));
    m.add_loading(o, "p".into(), 4, false);
    m.loading_stop(Some(f.scope.id()), |_| false);
    assert!(e.borrow().iter().any(|e| matches!(e, d::Effect::Cancel(_))));
    m.add_loaded(f.loaded(o, 4), 4);
    m.delete_all(|_| false);
    assert!(m.empty());
    m.untrack(f.scope.id());
    assert_eq!(serialize(), Some(vec![]));
    drop(f);
    assert!(serialize().is_none());
}
#[test]
fn rich_export_whitelist_head_limit_and_real_filesystem_delete() {
    let p = std::env::temp_dir()
        .join(format!("gramtele-download-export-{}", std::process::id()))
        .join("article");
    std::fs::create_dir_all(p.join("media")).unwrap();
    let file = p.join("article.HTML");
    std::fs::write(&file, b"Telegram Desktop rich message export").unwrap();
    assert_eq!(d::rich_export_folder_to_remove(&file), Some(p.clone()));
    std::fs::write(p.join(".extra"), b"keep").unwrap();
    assert!(d::rich_export_folder_to_remove(&file).is_none());
    std::fs::remove_file(p.join(".extra")).unwrap();
    d::delete_download_file(&file).unwrap();
    assert!(!p.exists());
    std::fs::remove_dir(p.parent().unwrap()).unwrap();
}
#[test]
fn executable_ip_warning_and_verified_extension_rules() {
    let allowed = BTreeSet::from(["exe".to_string()]);
    assert!(resolve::launcher_would_warn(
        NameType::Executable,
        false,
        "bin",
        false,
        true,
        &allowed
    ));
    assert!(!resolve::launcher_would_warn(
        NameType::Executable,
        true,
        "exe",
        true,
        true,
        &allowed
    ));
    assert!(!resolve::launcher_would_warn(
        NameType::Executable,
        false,
        "exe",
        false,
        true,
        &allowed
    ));
    assert!(resolve::launcher_would_warn(
        NameType::Image,
        true,
        "svg",
        false,
        true,
        &allowed
    ));
    assert_eq!(
        resolve::launch_decision("extensionless", true),
        resolve::LaunchDecision::OpenWith
    );
    assert_eq!(
        resolve::launch_decision("a.exe", true),
        resolve::LaunchDecision::Confirm
    );
    assert!(resolve::is_ip_revealing_path(
        "a.WPL",
        "",
        FilenamePlatform::Windows
    ));
    assert!(!resolve::is_ip_revealing_path(
        "a.WPL",
        "",
        FilenamePlatform::Linux
    ));
    assert!(resolve::is_ip_revealing_path(
        "a",
        "image/svg+xml",
        FilenamePlatform::Linux
    ));
    assert!(resolve::is_ip_revealing_path(
        "a.DOCX",
        "",
        FilenamePlatform::Mac
    ));
    assert_eq!(
        resolve::confirmed_warning_change(true, true),
        Some(resolve::WarningChange::DisableIpWarning)
    );
}
#[test]
fn image_open_facts_precedence_plugin_and_media_route() {
    assert!(
        resolve::check_image_open_in_app(64 * 1024 * 1024, None, "image/png", true, true)
            .size_over_limit
    );
    assert!(
        resolve::check_image_open_in_app(64 * 1024 * 1024 - 1, None, "image/png", true, true)
            .open_in_app
    );

    assert!(resolve::check_image_open(10, 10, None, "image/png", true, true).size_over_limit);
    let v = resolve::check_image_open(
        1,
        10,
        Some(("application/octet-stream", true)),
        "image/png",
        true,
        true,
    );
    assert_eq!(v.source, resolve::ImageOpenSource::Location);
    assert!(!v.open_in_app);
    assert!(resolve::check_image_open(1, 10, None, "image/png", true, true).open_in_app);
    assert!(resolve::should_inspect_plugin(
        9, 10, true, "A.PLUGIN", true
    ));
    assert!(!resolve::should_inspect_plugin(
        10, 10, true, "A.PLUGIN", true
    ));
    assert_eq!(
        resolve::resolve_document(false, true, true, true),
        resolve::ResolveDecision::ShowTheme
    );
    assert_eq!(
        resolve::resolve_document(false, false, true, true),
        resolve::ResolveDecision::PlayPause
    );
    assert_eq!(
        resolve::resolve_after_save(false, false, false, true),
        resolve::ResolveDecision::Save
    );
}
#[test]
fn file_click_keys_save_cancel_photo_and_callback_order() {
    let f = Fixture::new();
    let o = f.object(1);
    let mut h = click::DocumentClickHandler::new(o.document.unwrap(), o.item.id());
    h.file.set_message_id(f.object(2).item.id());
    let hit = Cell::new(0);
    click::open(h.file.context(), |id| hit.set(id.msg.bare));
    assert_eq!(hit.get(), 2);
    assert_eq!(
        click::save(false, false, true, click::SaveMode::ToCacheOrFile),
        click::SaveDecision::SaveToCache
    );
    assert_eq!(
        click::save(false, false, true, click::SaveMode::ToFile),
        click::SaveDecision::QueueFileSave
    );
    assert_eq!(
        click::queued_save(click::SaveMode::ToNewFile, true),
        click::SaveDecision::ChooseFilename
    );
    assert!(click::should_track(true, "path", true));
    assert_eq!(
        click::cancel(false, true, true),
        click::CancelDecision::UploadCallback
    );
    assert_eq!(
        click::open_with(false, ""),
        click::OpenWithDecision::SaveToFile
    );
    let photo = f.photos.ensure_photo(1);
    let p = click::PhotoClickHandler::new(
        photo,
        o.item.id(),
        Some(f.scope.peer_key(peer_from_user(7u64))),
    );
    assert_eq!(p.photo, photo);
    assert!(click::photo_save(false));
    assert_eq!(click::filename_chosen(""), click::SaveDecision::Ignore);
}
#[test]
fn send_action_start_dedup_callback_clear_reentry_and_events() {
    let f = Fixture::new();
    let now = f.clock.clone();
    let s = Rc::new(action::SendActionManager::new(Rc::new(move || {
        tg_base::timer::Clock::now(&now)
    })));
    let key = action::ActionKey {
        history: f.scope.peer_key(peer_from_user(7u64)),
        root_id: MsgId::new(3),
    };
    let made = Cell::new(0);
    assert!(!s.register_for(key, true, || panic!("self painter skipped"), || {}));
    assert!(s.register_for(key, false, || true, || made.set(made.get() + 1)));
    assert!(!s.register_for(key, false, || true, || made.set(made.get() + 1)));
    assert_eq!(made.get(), 2);
    assert_eq!(s.started(key), Some(10000));
    assert!(s.callback(10100, |k, t| {
        assert_eq!(k, key);
        assert_eq!(t, 10100);
        s.started(k);
        true
    }));
    assert!(!s.callback(12000, |_, _| false));
    let l = Lifetime::new();
    let a = collect(s.animation_updated(), &l);
    let p = collect(s.speaking_animation_updated(), &l);
    s.update_animation(action::AnimationUpdate {
        thread: key,
        rect: [1, 2, 3, 4],
        text_updated: true,
    });
    s.update_speaking_animation(key.history);
    assert_eq!(a.borrow().len(), 1);
    assert_eq!(*p.borrow(), [key.history]);
    s.clear();
}
fn tone(id: u64) -> tl::AiComposeTone {
    tl::AiComposeToneData {
        id,
        access_hash: id + 10,
        title: format!("tone{id}").into_bytes(),
        slug: b"slug".to_vec(),
        ..Default::default()
    }
    .into()
}
#[test]
fn tones_refresh_pending_full_priority_notmodified_failure_timer_and_reentry() {
    let f = Fixture::new();
    let effects = Rc::new(RefCell::new(vec![]));
    let e = effects.clone();
    let s = ai::AiComposeTones::new(Rc::new(move |v| e.borrow_mut().push(v)), &f.host);
    s.refresh_with_hash(77);
    s.refresh_with_hash(0);
    s.refresh_with_hash(9);
    let l = Lifetime::new();
    let w = Rc::downgrade(&s);
    s.updated().start(
        move |_| {
            w.upgrade().unwrap().list();
        },
        |_| {},
        || {},
        &l,
    );
    s.refresh_reply(Ok(tl::mtp_aicompose_tones(10, vec![tone(1)], vec![])));
    assert_eq!(s.list()[0].id, 1);
    assert!(
        matches!(effects.borrow().last().unwrap(),ai::Effect::Send{request:ai::Request::Refresh(r),..}if r.hash==0)
    );
    s.refresh_reply(Ok(tl::mtp_aicompose_tones_not_modified()));
    let count = effects.borrow().len();
    f.clock.advance(3600000);
    f.host.process();
    assert_eq!(effects.borrow().len(), count + 1);
    s.refresh_reply(Err(tg_mtproto::mtproto_response::Error::local(
        "FAIL", "failure",
    )));
}
#[test]
fn tones_mutations_promote_save_remove_and_optional_false_tl() {
    let f = Fixture::new();
    let events = Rc::new(RefCell::new(vec![]));
    let e = events.clone();
    let s = ai::AiComposeTones::new(Rc::new(move |v| e.borrow_mut().push(v)), &f.host);
    s.refresh_reply(Ok(tl::mtp_aicompose_tones(
        99,
        vec![
            tone(1),
            tone(2),
            tl::mtp_ai_compose_tone_default(b"default".to_vec(), 9, b"Default".to_vec()),
        ],
        vec![],
    )));
    let t = ai::parse_tone(&tone(2));
    let id = s.update(&t, Some(String::new()), None, None, Some(false));
    assert!(
        matches!(events.borrow().last().unwrap(),ai::Effect::Send{request:ai::Request::Update(r),..}if r.display_author==Some(false)&&r.flags.contains(tl::AicomposeUpdateTone::F_DISPLAY_AUTHOR))
    );
    s.tone_reply(id, &tone(2));
    assert_eq!(s.list()[0].id, 2);
    s.refresh_reply(Ok(tl::mtp_aicompose_tones(
        100,
        vec![tone(1), tone(2)],
        vec![],
    )));
    assert_eq!(s.list().iter().map(|t| t.id).collect::<Vec<_>>(), [2, 1]);
    let id = s.save(&t, true);
    s.bool_reply(id);
    assert_eq!(s.list()[0].id, 1);
    s.refresh_reply(Ok(tl::mtp_aicompose_tones_not_modified()));
    let id = s.create("new".into(), "prompt".into(), 7, true);
    s.tone_reply(id, &tone(3));
    assert_eq!(s.list()[0].id, 3);
    let id = s.remove(&ai::parse_tone(&tone(3)));
    s.bool_reply(id);
    assert!(s.list().iter().all(|t| t.id != 3));
}
#[test]
fn tones_resolve_local_errors_wire_example_and_silent_delete_failure() {
    let f = Fixture::new();
    let events = Rc::new(RefCell::new(vec![]));
    let e = events.clone();
    let s = ai::AiComposeTones::new(Rc::new(move |v| e.borrow_mut().push(v)), &f.host);
    let id = s.resolve("missing".into());
    s.resolve_reply(id, &tl::mtp_aicompose_tones(1, vec![], vec![]));
    assert!(
        matches!(events.borrow().last().unwrap(),ai::Effect::Failed{error,..}if error.kind.contains("TONE_NOT_FOUND"))
    );
    let id = s.resolve("unchanged".into());
    s.resolve_reply(id, &tl::mtp_aicompose_tones_not_modified());
    assert!(
        matches!(events.borrow().last().unwrap(),ai::Effect::Failed{error,..}if error.kind.contains("TONE_NOT_MODIFIED"))
    );
    let t = ai::parse_tone(&tone(1));
    let id = s.remove(&t);
    let n = events.borrow().len();
    s.failed(id, tg_mtproto::mtproto_response::Error::local("ERR", ""));
    assert_eq!(events.borrow().len(), n);
    let id = s.get_tone_example_wire(&t, 2);
    let wire = tl::mtp_ai_compose_tone_example(
        tl::mtp_text_with_entities(b"from".to_vec(), vec![]),
        tl::mtp_text_with_entities(b"to".to_vec(), vec![]),
    );
    s.example_reply_wire(id, wire);
    assert!(matches!(
        events.borrow().last().unwrap(),
        ai::Effect::ExampleWireDone { .. }
    ));
}
#[test]
fn community_links_hidden_values_real_peer_mutations_and_stale_unlink() {
    let f = Fixture::new();
    let peers = Rc::new(Peers::new(
        &f.scope,
        Changes::with_queue(&f.scope, tg_runtime::MainQueue::new()),
    ));
    let channel = peers.ensure_channel(ChannelId::new(1));
    let u = peers.ensure_user(UserId::new(2));
    let mut c = community::CommunityInfo::new(peers.clone(), channel);
    let entries = vec![tl::CommunityPeerData {
        flags: tl::CommunityPeerData::F_VISIBLE | tl::CommunityPeerData::F_CAN_VIEW_HISTORY,
        peer: tl::mtp_peer_user(2),
        visible: Some(false),
    }
    .into()];
    let effects = c.apply_linked_peers(&entries);
    for e in effects {
        community::CommunityInfo::apply_link_effect(&peers, e);
    }
    assert!(c.is_hidden(u.peer()));
    assert!(community::is_community_chat_viewable(&c.linked_peers()[0]));
    assert_eq!(
        community::peer_linked_community_id(&peers, u.peer()),
        ChannelId::new(1)
    );
    c.register_one(u.peer(), true, 7, |_| 7);
    let effects = c.apply_linked_peers(&[]);
    for e in effects {
        community::CommunityInfo::apply_link_effect(&peers, e);
    }
    assert_eq!(
        community::peer_linked_community_id(&peers, u.peer()),
        ChannelId::new(0)
    );
    assert!(c.apply_linked_peers(&[]).is_empty());
    let l = Lifetime::new();
    let p = collect(c.linked_peers_value(), &l);
    c.notify_linked_changed();
    assert_eq!(*p.borrow(), [(), ()]);
}
#[test]
fn community_membership_top_twenty_dates_collapse_and_refresh() {
    use tg_core::data::data_channel::ChannelDataFlag;
    let f = Fixture::new();
    let peers = Rc::new(Peers::new(
        &f.scope,
        Changes::with_queue(&f.scope, tg_runtime::MainQueue::new()),
    ));
    let ch = peers.ensure_channel(ChannelId::new(1));
    let mut c = community::CommunityInfo::new(peers.clone(), ch);
    for n in 1..=22 {
        let k = peers.ensure_user(UserId::new(n)).peer();
        c.register_one(k, true, n as i32, |k| k.id().value as i32);
    }
    assert_eq!(c.last_histories().len(), 20);
    assert_eq!(c.last_histories()[0].id(), peer_from_user(22u64));
    assert_eq!(c.chats_list_date(), 22);
    let p = peers.ensure_user(UserId::new(22)).peer();
    c.refresh_one_membership(p, false, |k| k.id().value as i32, |k| k.id().value as i32);
    assert_eq!(c.chats_list_date(), 21);
    peers.add_channel_flags(ch, ChannelDataFlag::CommunityCollapsed.into());
    let old = Rc::new(RefCell::new(vec![]));
    let o = old.clone();
    let plan = c.collapsed_change_plan(|_| true);
    for &key in &plan.moved {
        o.borrow_mut().push((key, plan.old_collapsed));
        assert!(!c.collapsed_in_chat_lists());
    }
    let effects = c.finish_collapsed_change(plan);
    assert!(old.borrow().iter().all(|(_, v)| !*v));
    assert!(c.collapsed_in_chat_lists());
    assert!(effects.contains(&community::CommunityEffect::EnsureRow));
    assert!(community::community_chat_joined(&peers, p, |_| true));
}
#[cfg(unix)]
struct UtcCalendar;
#[cfg(unix)]
impl UtcCalendar {
    fn stamp(year: i32, month: i32, day: i32, hour: i32, min: i32, sec: i32) -> i32 {
        // SAFETY: zeroed tm contains only integer/pointer fields; timegm receives a valid exclusive tm.
        unsafe {
            let mut t: libc::tm = std::mem::zeroed();
            t.tm_year = year - 1900;
            t.tm_mon = month - 1;
            t.tm_mday = day;
            t.tm_hour = hour;
            t.tm_min = min;
            t.tm_sec = sec;
            libc::timegm(&mut t) as i32
        }
    }
}
#[cfg(unix)]
impl calendar::Calendar for UtcCalendar {
    fn date(&self, time: i32) -> CalendarDate {
        // SAFETY: both pointers refer to valid initialized input/output storage during gmtime_r.
        unsafe {
            let t = libc::time_t::from(time);
            let mut out: libc::tm = std::mem::zeroed();
            assert!(!libc::gmtime_r(&t, &mut out).is_null());
            CalendarDate {
                year: out.tm_year + 1900,
                month: out.tm_mon + 1,
                day: out.tm_mday,
            }
        }
    }
    fn day_start(&self, t: i32) -> i32 {
        let d = self.date(t);
        Self::stamp(d.year, d.month, d.day, 0, 0, 0)
    }
    fn month_start(&self, y: i32, m: i32) -> i32 {
        Self::stamp(y, m, 1, 0, 0, 0)
    }
    fn month_end(&self, y: i32, m: i32) -> i32 {
        self.next_month_start(y, m) - 1
    }
    fn next_month_start(&self, y: i32, m: i32) -> i32 {
        Self::stamp(y, m + 1, 1, 0, 0, 0)
    }
}
#[cfg(unix)]
#[test]
fn calendar_month_cache_authoritative_periods_dedup_deferred_and_reentrant_callbacks() {
    let f = Fixture::new();
    let e = Rc::new(RefCell::new(vec![]));
    let out = e.clone();
    let c = calendar::SearchCalendarController::new(
        f.scope.peer_key(peer_from_user(7u64)),
        tl::mtp_input_peer_self(),
        SharedMediaType::Photo,
        Rc::new(UtcCalendar),
        Rc::new(move |v| out.borrow_mut().push(v)),
    );
    let march = UtcCalendar::stamp(2024, 3, 8, 12, 0, 0);
    let feb = UtcCalendar::stamp(2024, 2, 8, 12, 0, 0);
    let hit = Rc::new(Cell::new(0));
    let n = hit.clone();
    let w = Rc::downgrade(&c);
    c.month_days(
        march,
        Rc::new(move |v| {
            n.set(n.get() + 1);
            assert_eq!(v.len(), 1);
            w.upgrade().unwrap().resolve_msg_id_by_date(march);
        }),
    );
    c.month_days(feb, Rc::new(|_| {}));
    assert_eq!(e.borrow().len(), 1);
    let calendar::Effect::Send { key, .. } = e.borrow()[0].clone();
    let photo = f.photos.ensure_photo(1);
    let day = calendar::MonthDay {
        date: 0,
        msg_id: MsgId::new(0),
        origin: f.object(1).item.id(),
        photo: Some(photo),
        document: None,
    };
    let media = c.collect_day_media([(march, day, false), (march, day, false)]);
    let period = tl::SearchResultsCalendarPeriodData {
        date: march,
        min_msg_id: 1,
        max_msg_id: 99,
        count: 7,
    }
    .into();
    let result = tl::mtp_messages_search_results_calendar(
        Default::default(),
        7,
        0,
        0,
        None,
        vec![period],
        vec![],
        vec![],
        vec![],
    );
    c.reply(key, &result, media);
    assert_eq!(hit.get(), 1);
    assert_eq!(c.resolve_msg_id_by_date(march), Some(MsgId::new(99)));
    assert_eq!(e.borrow().len(), 2);
    c.month_days(march, Rc::new(|v| assert_eq!(v.len(), 1)));
    assert_eq!(e.borrow().len(), 2);
    let calendar::Effect::Send { key, .. } = e.borrow()[1].clone();
    c.failed(key);
    c.month_days(feb, Rc::new(|v| assert!(v.is_empty())));
    assert_eq!(e.borrow().len(), 2);
}
#[cfg(unix)]
#[test]
fn calendar_pagination_same_offset_ends_and_covered_months() {
    let f = Fixture::new();
    let e = Rc::new(RefCell::new(vec![]));
    let out = e.clone();
    let c = calendar::SearchCalendarController::new(
        f.scope.peer_key(peer_from_user(7u64)),
        tl::mtp_input_peer_self(),
        SharedMediaType::Photo,
        Rc::new(UtcCalendar),
        Rc::new(move |v| out.borrow_mut().push(v)),
    );
    let date = UtcCalendar::stamp(2024, 3, 8, 0, 0, 0);
    c.month_days(date, Rc::new(|_| {}));
    let calendar::Effect::Send { key, .. } = e.borrow()[0].clone();
    let result = tl::mtp_messages_search_results_calendar(
        Default::default(),
        0,
        date,
        4,
        None,
        vec![],
        vec![],
        vec![],
        vec![],
    );
    c.reply(key, &result, BTreeMap::new());
    assert_eq!(e.borrow().len(), 2);
    c.reply(key, &result, BTreeMap::new());
    assert_eq!(e.borrow().len(), 2);
    c.month_days(date, Rc::new(|v| assert!(v.is_empty())));
}
/// Captured typed server replies use the actual Sender/Instance parser, without network or fake core records.
#[test]
fn sender_memory_server_tones_calendar_and_resolve_requests() {
    use tg_mtproto::{
        mtp_instance::{get_next_request_id, Fields, Instance, Mode},
        mtproto_dc_options::Environment,
        mtproto_response::Response,
        sender::Sender,
    };
    let instance = Instance::new(Mode::Normal, Fields::new(Environment::Test));
    let sender = Sender::new(instance.clone());
    let replies = Arc::new(Mutex::new(vec![]));
    let received = replies.clone();
    let id = get_next_request_id();
    let request = ai::Request::Refresh(tl::AicomposeGetTones::new(123));
    ai::send(
        &sender,
        id,
        request,
        Arc::new(move |v| received.lock().unwrap().push(v)),
    );
    let server_reply = tl::mtp_aicompose_tones(456, vec![tone(7)], vec![]);
    instance.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&server_reply),
    });
    assert_eq!(
        replies.lock().unwrap()[0].as_ref().unwrap(),
        &tg_tl::serialize(&server_reply)
    );
    let id = get_next_request_id();
    let received = replies.clone();
    d::send_request(
        &sender,
        id,
        tl::MessagesGetMessages::new(vec![tl::mtp_input_message_id(1)]),
        Arc::new(move |v| received.lock().unwrap().push(v)),
    );
    instance.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&tl::mtp_rpc_error(400, b"MESSAGE_ID_INVALID".to_vec())),
    });
    assert_eq!(
        replies.lock().unwrap()[1].as_ref().unwrap_err().kind,
        "MESSAGE_ID_INVALID"
    );
    assert_eq!(replies.lock().unwrap()[1].as_ref().unwrap_err().code, 400);
    let id = get_next_request_id();
    let received = replies.clone();
    d::send_request(
        &sender,
        id,
        tl::MessagesGetSearchResultsCalendar::new(
            Default::default(),
            tl::mtp_input_peer_self(),
            None,
            tg_core::data::data_search_controller::prepare_search_filter(Some(
                SharedMediaType::Photo,
            )),
            0,
            123,
        ),
        Arc::new(move |v| received.lock().unwrap().push(v)),
    );
    let calendar_reply = tl::mtp_messages_search_results_calendar(
        Default::default(),
        0,
        0,
        0,
        None,
        vec![],
        vec![],
        vec![],
        vec![],
    );
    instance.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&calendar_reply),
    });
    assert_eq!(
        replies.lock().unwrap()[2].as_ref().unwrap(),
        &tg_tl::serialize(&calendar_reply)
    );
}
#[test]
fn real_file_loader_bridge_and_document_filter_duplicate_removal() {
    use tg_core::storage::file_download::*;
    let f = Fixture::new();
    let (m, _) = f.manager();
    let mut loader = FileLoader::new(
        LoaderConfig {
            session: f.scope.id(),
            filename: Default::default(),
            full_size: 8,
            load_size: 8,
            location_type: LocationType::Document,
            to_cache: LoadToCacheSetting::CacheAsWell,
            from_cloud: LoadFromCloudSetting::CloudOrLocal,
            auto_loading: false,
            cache_tag: 0,
        },
        Default::default(),
        None,
    )
    .unwrap();
    let a = f.object(1);
    let mut b = f.object(2);
    b.document = a.document;
    m.add_loading(a, "p".into(), 8, false);
    m.add_loading(b, "p".into(), 8, false);
    loader.write_result_part(0, b"abc");
    m.check_file_loader(a.item, true, false, &loader, None);
    assert_eq!(m.loading_progress().ready, 3);
    let hit = Cell::new(0);
    m.check_document(a.document.unwrap(), |_| hit.set(1));
    assert_eq!(hit.get(), 1);
    m.add_loaded(f.loaded(a, 8), 8);
    m.check_document(a.document.unwrap(), |_| hit.set(2));
    assert_eq!(hit.get(), 1);
    assert!(m.loaded_has_non_cloud_file(|_| false));
}
#[test]
fn start_date_clock_change_zero_and_peer_access_hash_real_variants() {
    let f = Fixture::new();
    let time = Rc::new(Cell::new(0));
    let t = time.clone();
    let m = d::DownloadManager::new(
        Rc::new(move || t.get()),
        Rc::new(|e| assert!(!matches!(e, d::Effect::Cancel(_)))),
        &f.host,
    );
    assert_eq!(m.compute_next_start_date(), 1);
    assert_eq!(m.compute_next_start_date(), 2);
    time.set(1);
    assert_eq!(m.compute_next_start_date(), 1000);
    time.set(0);
    assert_eq!(m.compute_next_start_date(), 0);
    let peers = Peers::new(
        &f.scope,
        Changes::with_queue(&f.scope, tg_runtime::MainQueue::new()),
    );
    let user = peers.ensure_user(UserId::new(1));
    let channel = peers.ensure_channel(ChannelId::new(2));
    let chat = peers.ensure_peer(peer_from_chat(3u64));
    peers.set_user_access_hash(user, 55);
    peers.set_channel_access_hash(channel, 66);
    assert_eq!(d::peer_access_hash(&peers, user.peer()), 55);
    assert_eq!(d::peer_access_hash(&peers, channel.peer()), 66);
    assert_eq!(d::peer_access_hash(&peers, chat), 0);
}
#[test]
fn stop_consumes_reentrant_additions_and_external_lookup_checks_first_item() {
    let f = Fixture::new();
    let (m, _) = f.manager();
    let one = f.object(1);
    let two = f.object(2);
    let w = Rc::downgrade(&m);
    let called = Rc::new(Cell::new(0));
    let n = called.clone();
    m.add_loading_external(
        one,
        "one".into(),
        1,
        Rc::new(move || {
            let n = n.clone();
            w.upgrade().unwrap().add_loading_external(
                two,
                "two".into(),
                2,
                Rc::new(move || n.set(n.get() + 1)),
            );
        }),
    );
    m.loading_stop(None, |_| false);
    assert_eq!(called.get(), 1);
    assert!(m.loading_list().is_empty());
    m.add_loading(one, "normal".into(), 1, false);
    let n = called.clone();
    m.add_loading_external(
        one,
        "external".into(),
        2,
        Rc::new(move || n.set(n.get() + 1)),
    );
    assert!(m.loading_external_state(one.item).is_none());
    m.update_loading_external(one.item, 99, 99);
    assert_eq!(m.loading_progress().total, 3);
    m.loading_stop(None, |_| false);
    assert_eq!(called.get(), 2);
    assert_eq!(m.loading_progress().total, 0);
}
