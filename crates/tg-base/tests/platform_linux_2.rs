#![cfg(all(unix, not(target_os = "macos")))]
#[path = "support/private_bus.rs"]
mod private_bus;
#[path = "support/private_x11.rs"]
mod private_x11;
use private_bus::PrivateBus;
use private_x11::*;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};
use tg_base::platform::base_platform_system_media_controls::{Command, LoopStatus, PlaybackStatus};
use tg_base::platform::linux::{
    base_global_shortcuts_linux as shortcut, base_linux_xcb_library as xcb,
    base_linux_xcb_utilities as util, base_linux_xsettings as settings,
    base_process_linux as process, base_system_media_controls_linux as media,
};
use tg_runtime::MainQueue;
use zbus::blocking::{Connection, Proxy};
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .chunks_exact(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn value_json(name: &[u8], v: &settings::PropertyValue) -> serde_json::Value {
    let mut value = serde_json::json!({"name":hex(name),"serial":v.serial});
    match &v.value {
        settings::Value::Invalid => value["type"] = "invalid".into(),
        settings::Value::Integer(n) => {
            value["type"] = "int".into();
            value["value"] = (*n).into();
        }
        settings::Value::String(s) => {
            value["type"] = "string".into();
            value["value"] = hex(s).into();
        }
        settings::Value::Color { valid, rgba } => {
            value["type"] = "color".into();
            value["valid"] = (*valid).into();
            value["value"] = serde_json::json!(rgba);
        }
    }
    value
}
#[test]
fn native_xsettings_endian_padding_serial_partial_and_all_key_codes() {
    for line in include_str!("fixtures/platform-linux-2/native.jsonl").lines() {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        let actual = match row["op"].as_str().unwrap() {
            "parse" => {
                let mut state = BTreeMap::<Vec<u8>, settings::PropertyValue>::new();
                for blob in row["blobs"].as_array().unwrap() {
                    for record in settings::parse(&unhex(blob.as_str().unwrap())).records {
                        state
                            .entry(record.name.clone())
                            .or_default()
                            .update(&record);
                    }
                }
                serde_json::json!(state
                    .iter()
                    .map(|(k, v)| value_json(k, v))
                    .collect::<Vec<_>>())
            }
            "keys" => {
                serde_json::json!(tg_base::platform::linux::base_linux_shortcut_keys::KEY_TO_CODE)
            }
            "scale" => serde_json::json!(media::scale_milliseconds(
                row["value"].as_i64().unwrap() as i32
            )),
            "playback" => serde_json::json!(media::convert_playback_status(
                [
                    PlaybackStatus::Playing,
                    PlaybackStatus::Paused,
                    PlaybackStatus::Stopped
                ][row["value"].as_u64().unwrap() as usize]
            )),
            "loop" => serde_json::json!(media::convert_loop_status(
                [LoopStatus::None, LoopStatus::Track, LoopStatus::Playlist]
                    [row["value"].as_u64().unwrap() as usize]
            )),
            "command" => {
                serde_json::json!(media::event_to_command(row["value"].as_str().unwrap()) as u32)
            }
            "loop_command" => serde_json::json!(media::loop_status_to_command(
                row["value"].as_str().unwrap()
            ) as u32),
            "mouse_descriptor" => serde_json::json!(shortcut::SHIFT_MOUSE_BUTTON
                .wrapping_add(row["value"].as_u64().unwrap())
                .to_string()),
            "abi" => serde_json::json!([40, 40, 36, 44, 24, 32]),
            "mask" => {
                let mode = match row["mode"].as_u64().unwrap() {
                    0 => util::ChangeWindowEventMaskMode::Add,
                    1 => util::ChangeWindowEventMaskMode::Remove,
                    _ => util::ChangeWindowEventMaskMode::Replace,
                };
                serde_json::json!(util::event_mask_change(
                    row["old"].as_u64().unwrap() as u32,
                    row["mask"].as_u64().unwrap() as u32,
                    mode
                ))
            }
            _ => panic!("unexpected native row"),
        };
        assert_eq!(actual, row["expected"], "{line}");
    }
}
fn settings_blob(serial: i32, n: i32) -> Vec<u8> {
    let mut bytes = vec![0; 12];
    bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
    bytes.extend([0, 0, 1, 0, b'a', 0, 0, 0]);
    bytes.extend(serial.to_le_bytes());
    bytes.extend(n.to_le_bytes());
    bytes
}
#[test]
fn xsettings_manager_real_selection_property_update_and_callback_lease() {
    let display = PrivateX11::new();
    let connection = display.connect();
    let owner = create_window(&connection);
    let selection = connection.atom("_XSETTINGS_S0").unwrap();
    set_selection(&connection, owner, selection);
    let property = connection.atom("_XSETTINGS_SETTINGS").unwrap();
    assert!(connection.change_property(owner, property, property, 8, &settings_blob(1, 10)));
    let hub = util::EventHub::new(connection.clone());
    let state = settings::XSettings::instance(hub.clone());
    assert!(state.initialized());
    assert!(Rc::ptr_eq(
        &state,
        &settings::XSettings::instance(hub.clone())
    ));
    assert_eq!(state.setting(b"a"), settings::Value::Integer(10));
    let values = Rc::new(RefCell::new(vec![]));
    let out = values.clone();
    let weak = Rc::downgrade(&state);
    let lease =
        state.register_callback_for_property(b"a".to_vec(), move |connection, name, value| {
            assert!(connection.file_descriptor().is_some());
            assert_eq!(name, b"a");
            assert_eq!(weak.upgrade().unwrap().setting(name), *value);
            out.borrow_mut().push(value.clone());
        });
    assert!(connection.change_property(owner, property, property, 8, &settings_blob(2, 20)));
    connection.sync();
    hub.poll();
    assert_eq!(*values.borrow(), [settings::Value::Integer(20)]);
    assert!(connection.change_property(owner, property, property, 8, &settings_blob(2, 30)));
    connection.sync();
    hub.poll();
    assert_eq!(values.borrow().len(), 1);
    drop(lease);
    assert!(connection.change_property(owner, property, property, 8, &settings_blob(3, 40)));
    connection.sync();
    hub.poll();
    assert_eq!(values.borrow().len(), 1);
    assert_eq!(state.setting(b"a"), settings::Value::Integer(40));
    assert_eq!(state.setting(b"missing"), settings::Value::Invalid);
}
#[test]
fn xcb_real_wm_chunks_timestamp_masks_activation_and_idle() {
    let display = PrivateX11::new();
    let connection = display.connect();
    let root = connection.root_window().unwrap();
    let window = create_window(&connection);
    assert!(xcb::available());
    assert!(connection.file_descriptor().unwrap() >= 0);
    assert!(!connection.has_error());
    assert_eq!(
        connection.selection_owner(connection.atom("unused_selection").unwrap()),
        None
    );
    assert_eq!(connection.atom("é"), connection.atom("Ã")); // Source uses UTF-16 count for UTF-8 atom bytes.
    let supported = connection.atom("_NET_SUPPORTED").unwrap();
    let atoms = (1..=2050u32).flat_map(u32::to_ne_bytes).collect::<Vec<_>>();
    assert!(connection.change_property(root, supported, 4, 32, &atoms));
    assert_eq!(
        util::get_wm_supported(&connection, root),
        (1..=2050).collect::<Vec<_>>()
    );
    let check = connection.atom("_NET_SUPPORTING_WM_CHECK").unwrap();
    assert!(connection.change_property(root, check, 33, 32, &window.to_ne_bytes()));
    let name = connection.atom("_NET_WM_NAME").unwrap();
    let utf8 = connection.atom("UTF8_STRING").unwrap();
    assert!(connection.change_property(window, name, utf8, 8, b" Test  WM \n"));
    assert_eq!(util::window_manager(&connection), "Test WM");
    assert_eq!(
        util::get_supporting_wm_check(&connection, root),
        Some(window)
    );
    let real_atom = connection.atom("TestSupport").unwrap();
    assert!(connection.change_property(root, supported, 4, 32, &real_atom.to_ne_bytes()));
    assert!(util::is_supported_by_wm(&connection, "TestSupport"));
    let original = connection.window_attributes(window).unwrap().0;
    {
        let _lease = util::change_window_event_mask(
            connection.clone(),
            window,
            1,
            util::ChangeWindowEventMaskMode::Add,
            true,
        )
        .unwrap();
        assert_eq!(
            connection.window_attributes(window).unwrap().0,
            original | 1
        );
        let _noop = util::change_window_event_mask(
            connection.clone(),
            window,
            1 | (1 << 22),
            util::ChangeWindowEventMaskMode::Add,
            true,
        )
        .unwrap();
        assert_eq!(
            connection.window_attributes(window).unwrap().0,
            original | 1
        );
    }
    assert_eq!(connection.window_attributes(window).unwrap().0, original);
    let hub = util::EventHub::new(connection.clone());
    assert!(hub.timestamp().unwrap() > 0);
    let seen = Rc::new(RefCell::new(vec![]));
    let out = seen.clone();
    let _watch = hub
        .register(move |e| {
            if let Some(e) = e.filter(|e| e.kind() == 33) {
                out.borrow_mut().push(e.clone());
            }
        })
        .unwrap();
    let _mask = util::change_window_event_mask(
        connection.clone(),
        root,
        1 << 19,
        util::ChangeWindowEventMaskMode::Add,
        true,
    )
    .unwrap();
    assert!(process::xcb_activate_window(&hub, window, Some(42)));
    connection.sync();
    hub.poll();
    assert_eq!(connection.window_attributes(window).unwrap().1, 2);
    let e = seen.borrow();
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].u32(4), window);
    assert_eq!(e[0].u32(12), 1);
    assert_eq!(e[0].u32(20), 42);
    assert_eq!(e[0].u32(24), 0);
    assert!(connection.extension("MIT-SCREEN-SAVER").is_some());
    assert!(connection.screen_saver_idle().is_some());
    assert!(connection.reset_screen_saver());
    assert!(connection.screen_saver_idle().is_some());
    assert!(connection.extension("XWAYLAND").is_none());
    let facts = tg_base::platform::linux::base_info_linux::xcb_desktop_facts(&connection);
    assert!(facts.x11);
    assert!(!facts.xwayland);
    assert_eq!(facts.window_manager, "Test WM");
}
#[test]
fn shortcuts_reply_boundary_modifiers_keypad_and_real_record() {
    assert!(shortcut::is_keypad(0xff80));
    assert!(shortcut::is_keypad(0x1100ffff));
    assert!(!shortcut::is_keypad(0xffbe));
    assert_eq!(shortcut::compute_keysym(0x61, 0x41, 0), 0x41);
    assert_eq!(shortcut::compute_keysym(0x61, 0x42, 0xffff), 0x42);
    assert_eq!(shortcut::compute_keysym(0x61, 0xffb0, 0), 0xffb0);
    assert_eq!(shortcut::compute_keysym(0x61, 0x20, 0xffff), 0x61);
    assert_eq!(
        shortcut::key_name(0xffe4),
        shortcut::KeyName::Text("Right Ctrl".into())
    );
    assert_eq!(
        shortcut::key_name(shortcut::SHIFT_MOUSE_BUTTON + 2),
        shortcut::KeyName::Text("Mouse 2".into())
    );
    assert!(!shortcut::is_toggle_full_screen_key());
    assert!(
        shortcut::process_batch(&[key_event(4, 1, 0), key_event(2, 38, 0)], |_, _| 0x41).is_empty()
    );
    assert_eq!(
        shortcut::process_batch(&[key_event(2, 38, 0), key_event(5, 2, 0)], |_, _| 0x41),
        [(0x41, true), (shortcut::SHIFT_MOUSE_BUTTON + 2, false)]
    );
    let display = PrivateX11::new();
    let manager = shortcut::GlobalShortcuts::new(Some(display.display.clone()));
    assert!(!manager.available(false));
    assert!(manager.available(true));
    assert!(manager.allowed(true));
    let seen = Rc::new(RefCell::new(vec![]));
    let out = seen.clone();
    manager.start(move |key, down| out.borrow_mut().push((key, down)));
    let generator = display.connect();
    xtest_key(&generator, 38, true);
    xtest_key(&generator, 38, false);
    generator.sync();
    let deadline = Instant::now() + Duration::from_secs(5);
    while seen.borrow().len() < 2 {
        manager.poll();
        if seen.borrow().len() >= 2 {
            break;
        }
        assert!(Instant::now() < deadline, "actual XRecord events");
        let fd = manager.file_descriptor().unwrap();
        assert!(fd >= 0);
        generator.wait_readable(1);
    }
    assert_eq!(*seen.borrow(), [(0x41, true), (0x41, false)]);
    manager.stop();
    manager.poll();
    assert!(manager.available(true));
}
fn pump(q: &MainQueue, done: impl Fn() -> bool) {
    let (tx, rx) = mpsc::channel();
    q.set_waker(Some(Arc::new(move || {
        let _ = tx.send(());
    })));
    let deadline = Instant::now() + Duration::from_secs(7);
    while !done() {
        q.process_all();
        if done() {
            break;
        }
        rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("private bus/main queue completion");
    }
    q.set_waker(None);
}
fn rpc<T: Send + 'static>(
    q: &MainQueue,
    c: &Connection,
    f: impl FnOnce(Connection) -> T + Send + 'static,
) -> T {
    let (tx, rx) = mpsc::channel();
    let q2 = q.clone();
    let c = c.clone();
    let thread = std::thread::spawn(move || {
        tx.send(f(c)).ok();
        q2.post(|| {});
    });
    let value = Rc::new(RefCell::new(None));
    pump(q, || {
        if let Ok(v) = rx.try_recv() {
            *value.borrow_mut() = Some(v);
        }
        value.borrow().is_some()
    });
    thread.join().unwrap();
    value.take().unwrap()
}
fn proxy(c: &Connection, interface: &str) -> Proxy<'static> {
    Proxy::new_owned(
        c.clone(),
        "org.mpris.MediaPlayer2.tg_test",
        media::OBJECT_PATH,
        interface.to_owned(),
    )
    .unwrap()
}
#[test]
fn mpris_real_export_methods_properties_signals_reentrancy_and_unexport() {
    let bus = PrivateBus::new();
    let server = bus.connect();
    let client = bus.connect();
    let q = MainQueue::new();
    q.bind_current_thread();
    let controls = media::SystemMediaControls::new(
        Some(server.clone()),
        q.clone(),
        "tg.desktop".into(),
        media::bus_name("tg_test", None, None),
        false,
    );
    assert!(controls.init());
    assert!(media::SystemMediaControls::supported());
    assert!(controls.seeking_supported());
    assert!(controls.volume_supported());
    let commands = Rc::new(RefCell::new(vec![]));
    let out = commands.clone();
    let _commands = controls.command_requests().start_returning(
        move |c| out.borrow_mut().push(c),
        |_| {},
        || {},
    );
    let volumes = Rc::new(RefCell::new(vec![]));
    let out = volumes.clone();
    let weak = Rc::downgrade(&controls);
    let _volume = controls.volume_change_requests().start_returning(
        move |v| {
            out.borrow_mut().push(v);
            weak.upgrade().unwrap().set_volume(v);
        },
        |_| {},
        || {},
    );
    let seeks = Rc::new(RefCell::new(vec![]));
    let out = seeks.clone();
    let _seek =
        controls
            .seek_requests()
            .start_returning(move |v| out.borrow_mut().push(v), |_| {}, || {});
    controls.set_enabled(true);
    let wake = q.clone();
    std::thread::spawn(move || {
        wake.post(|| {});
    });
    pump(&q, || {
        controls.exported() || controls.last_error().is_some()
    });
    assert_eq!(controls.last_error(), None);
    assert!(controls.exported());
    assert_eq!(
        rpc(&q, &client, |c| proxy(&c, media::ROOT_INTERFACE)
            .get_property::<String>("DesktopEntry")
            .unwrap()),
        "tg.desktop"
    );
    assert!(rpc(&q, &client, |c| proxy(&c, media::ROOT_INTERFACE)
        .get_property::<bool>("CanRaise")
        .unwrap()));
    let changed = Rc::new(RefCell::new(vec![]));
    let out = changed.clone();
    let _watch = tg_base::platform::linux::base_linux_xdp_utilities::PropertyWatcher::new(
        &client,
        q.clone(),
        "org.mpris.MediaPlayer2.tg_test",
        media::OBJECT_PATH,
        media::PLAYER_INTERFACE.into(),
        move |changed, _| out.borrow_mut().push(changed),
    )
    .unwrap();
    rpc(&q, &client, |c| {
        proxy(&c, media::PLAYER_INTERFACE)
            .call::<_, _, ()>(
                "SetPosition",
                &(
                    zbus::zvariant::ObjectPath::try_from(media::TRACK_PATH).unwrap(),
                    0i64,
                ),
            )
            .unwrap()
    });
    assert!(seeks.borrow()[0].is_nan());
    rpc(&q, &client, |c| {
        proxy(&c, media::PLAYER_INTERFACE)
            .call::<_, _, ()>("Seek", &(1i64,))
            .unwrap()
    });
    assert_eq!(seeks.borrow()[1], f64::INFINITY);
    seeks.borrow_mut().clear();
    controls.set_duration(10000);
    controls.set_position(2000);
    controls.set_title("title".into());
    controls.set_artist("artist".into());
    controls.set_is_next_enabled(true);
    controls.set_is_previous_enabled(true);
    controls.set_is_play_pause_enabled(true);
    controls.set_playback_status(PlaybackStatus::Playing);
    controls.set_shuffle(true);
    controls.set_volume(0.5);
    controls.set_volume(0.5);
    q.process_all();
    assert_eq!(*volumes.borrow(), [0.5]);
    assert!(commands.borrow().is_empty());
    rpc(&q, &client, |c| {
        proxy(&c, media::PLAYER_INTERFACE)
            .call::<_, _, ()>("PlayPause", &())
            .unwrap()
    });
    assert_eq!(*commands.borrow(), [Command::PlayPause]);
    rpc(&q, &client, |c| {
        proxy(&c, media::PLAYER_INTERFACE)
            .set_property("LoopStatus", "Track")
            .unwrap()
    });
    assert_eq!(commands.borrow().last(), Some(&Command::LoopTrack));
    rpc(&q, &client, |c| {
        proxy(&c, media::PLAYER_INTERFACE)
            .set_property("LoopStatus", "Track")
            .unwrap()
    });
    assert_eq!(commands.borrow().len(), 2);
    rpc(&q, &client, |c| {
        proxy(&c, media::PLAYER_INTERFACE)
            .set_property("Shuffle", false)
            .unwrap()
    });
    assert_eq!(commands.borrow().last(), Some(&Command::Shuffle));
    rpc(&q, &client, |c| {
        proxy(&c, media::PLAYER_INTERFACE)
            .set_property("Volume", 0.75f64)
            .unwrap()
    });
    assert_eq!(*volumes.borrow(), [0.5, 0.75]);
    rpc(&q, &client, |c| {
        proxy(&c, media::PLAYER_INTERFACE)
            .call::<_, _, ()>("Seek", &(1000000i64,))
            .unwrap()
    });
    assert_eq!(*seeks.borrow(), [0.3]);
    rpc(&q, &client, |c| {
        proxy(&c, media::PLAYER_INTERFACE)
            .call::<_, _, ()>(
                "SetPosition",
                &(
                    zbus::zvariant::ObjectPath::try_from("/ignored_track").unwrap(),
                    4000000i64,
                ),
            )
            .unwrap()
    });
    assert_eq!(*seeks.borrow(), [0.3, 0.4]);
    let calls = Rc::new(Cell::new(0));
    let out = calls.clone();
    let weak = Rc::downgrade(&controls);
    let _position = controls.update_position_requests().start_returning(
        move |()| {
            out.set(out.get() + 1);
            weak.upgrade().unwrap().set_position(2500);
        },
        |_| {},
        || {},
    );
    assert_eq!(
        rpc(&q, &client, |c| proxy(&c, media::PLAYER_INTERFACE)
            .get_property::<i64>("Position")
            .unwrap()),
        2500000
    );
    assert_eq!(calls.get(), 1);
    assert!(rpc(&q, &client, |c| proxy(&c, media::PLAYER_INTERFACE)
        .call::<_, _, ()>("OpenUri", &("file:///ignored",)))
    .is_err());
    let objects = rpc(&q, &client, |c| {
        zbus::blocking::fdo::ObjectManagerProxy::builder(&c)
            .destination("org.mpris.MediaPlayer2.tg_test")
            .unwrap()
            .path("/org/mpris")
            .unwrap()
            .build()
            .unwrap()
            .get_managed_objects()
            .unwrap()
    });
    let metadata = rpc(&q, &client, |c| {
        proxy(&c, media::PLAYER_INTERFACE)
            .get_property::<std::collections::HashMap<String, zbus::zvariant::OwnedValue>>(
                "Metadata",
            )
            .unwrap()
    });
    assert_eq!(
        Vec::<String>::try_from(metadata["xesam:artist"].try_clone().unwrap()).unwrap(),
        ["artist"]
    );
    assert_eq!(
        i64::try_from(metadata["mpris:length"].try_clone().unwrap()).unwrap(),
        10000000
    );
    assert!(objects
        .contains_key(&zbus::zvariant::OwnedObjectPath::try_from(media::OBJECT_PATH).unwrap()));
    pump(&q, || !changed.borrow().is_empty());
    assert!(changed.borrow().iter().any(|v| v.contains_key("Metadata")));
    controls.set_thumbnail_jpeg(b"jpeg");
    assert_eq!(
        controls.metadata()["mpris:artUrl"],
        media::MetadataValue::Text("data:image/jpeg;base64,anBlZw==".into())
    );
    controls.clear_thumbnail();
    controls.clear_metadata();
    assert_eq!(controls.metadata().len(), 1);
    controls.set_is_stop_enabled(false);
    controls.set_playback_rate(2.);
    controls.update_display();
    controls.set_loop_status(LoopStatus::Playlist);
    controls.set_enabled(false);
    pump(&q, || !controls.exported());
    assert!(
        !tg_base::platform::linux::base_linux_dbus_utilities::name_has_owner(
            &client,
            "org.mpris.MediaPlayer2.tg_test"
        )
        .unwrap()
    );
}

#[test]
fn optional_library_invalid_display_and_cached_headless_probes_in_child() {
    if let Some(mode) = std::env::var_os("TG_XCB_CHILD") {
        let present = mode == "present";
        if !present && xcb::available() {
            assert!(
                xcb::Connection::connect(Some("/tmp/tg-xcb-deliberately-invalid-display"))
                    .is_none()
            );
            return;
        }
        assert_eq!(xcb::available(), present);
        assert!(
            xcb::Connection::connect(Some("/tmp/tg-xcb-deliberately-invalid-display")).is_none()
        );
        if present {
            assert!(tg_base::platform::linux::base_info_linux::headless_is_x11());
            assert!(!tg_base::platform::linux::base_info_linux::headless_is_xwayland());
            assert!(shortcut::available(true));
            assert!(shortcut::allowed(true));
            shortcut::start(|_, _| {});
            shortcut::poll();
            shortcut::stop();
            assert!(!shortcut::available(false));
            let a = util::shared_connection().unwrap();
            let b = util::shared_connection().unwrap();
            assert!(Rc::ptr_eq(&a, &b));
            let bus = PrivateBus::new();
            let input =
                tg_base::platform::linux::base_last_input_linux::LastInput::platform(bus.connect());
            assert!(input.last_user_input_time(0).is_some());
        } else {
            assert!(!tg_base::platform::linux::base_info_linux::headless_is_x11());
            assert!(util::shared_connection().is_none());
        }
        return;
    }
    let display = PrivateX11::new();
    for present in [false, true] {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child
            .args([
                "--exact",
                "optional_library_invalid_display_and_cached_headless_probes_in_child",
                "--nocapture",
            ])
            .env("TG_XCB_CHILD", if present { "present" } else { "absent" })
            .env("DISPLAY", &display.display);
        // On distribution systems default system libxcb remains visible without LD path.
        // An unavailable-library run is only asserted when the dynamic loader lacks it.
        if !present {
            child.env_remove("LD_LIBRARY_PATH");
        }
        assert!(child.status().unwrap().success());
    }
}
#[test]
fn xsettings_negative_length_rejected_and_live_window_errors() {
    let mut data = settings_blob(1, 10);
    data[12] = 1;
    data[24..28].copy_from_slice(&(-1i32).to_le_bytes());
    assert_eq!(
        settings::parse(&data).error,
        Some(settings::ParseError::NegativeLength)
    );
    let mut x = PrivateX11::new();
    let c = x.connect();
    let hub = util::EventHub::new(c.clone());
    let state = settings::XSettings::new(hub);
    assert!(!state.initialized());
    assert!(c.window_attributes(0).is_none());
    assert!(!c.map_window(0));
    assert!(!c.change_property(0, 1, 19, 32, &[]));
    assert!(util::change_window_event_mask(
        c.clone(),
        0,
        1,
        util::ChangeWindowEventMaskMode::Add,
        true
    )
    .is_none());
    assert!(!c.change_property(c.root_window().unwrap(), 1, 19, 7, &[]));
    assert!(c.extension("TG_NO_SUCH_EXTENSION").is_none());
    assert!(c.send_raw_core(1, &[0], true).is_none());
    let hub = util::EventHub::new(c.clone());
    let errors = Rc::new(Cell::new(0));
    let count = errors.clone();
    let _lease = hub
        .register(move |event| {
            if event.is_none() {
                count.set(count.get() + 1);
            }
        })
        .unwrap();
    x.terminate();
    assert!(!c.sync());
    hub.poll();
    hub.poll();
    assert_eq!(errors.get(), 1);
    assert!(c.root_window().is_none());
    assert!(xcb::Record::new(c).is_none());
}
#[test]
fn mpris_unsupported_bus_source_noops_bus_names_and_zero_duration() {
    let q = MainQueue::new();
    q.bind_current_thread();
    let controls = media::SystemMediaControls::new(
        None,
        q.clone(),
        "desktop".into(),
        media::bus_name("app", Some("flatpak.App"), Some("snap")),
        true,
    );
    assert!(!controls.init());
    assert!(media::SystemMediaControls::supported());
    assert_eq!(
        media::bus_name("app", Some(""), Some("snap")),
        "org.mpris.MediaPlayer2."
    );
    assert_eq!(
        media::bus_name("app", None, Some("snap")),
        "org.mpris.MediaPlayer2.snap"
    );
    controls.set_enabled(true);
    controls.set_enabled(false);
    controls.set_is_stop_enabled(true);
    controls.set_playback_rate(9.);
    controls.update_display();
    assert!(!controls.exported());
    assert_eq!(controls.last_error(), None);
    controls.set_title("title\0ignored".into());
    controls.set_artist("artist\0ignored".into());
    assert_eq!(
        controls.metadata()["xesam:title"],
        media::MetadataValue::Text("title".into())
    );
    assert_eq!(
        controls.metadata()["xesam:artist"],
        media::MetadataValue::Strings(vec!["artist".into()])
    );
    controls.clear_metadata();
    assert_eq!(
        controls.metadata()["mpris:trackid"],
        media::MetadataValue::ObjectPath(media::TRACK_PATH.into())
    );
    assert_eq!(
        shortcut::process_batch(&[key_event(4, 255, 0)], |_, _| 0),
        [(154, true)]
    );
}

#[test]
fn xcb_resource_connection_retained_until_object_destruction_and_mask_modes() {
    let x = PrivateX11::new();
    let c = x.connect();
    let weak = Rc::downgrade(&c);
    let dropped = Rc::new(Cell::new(false));
    struct Resource(std::rc::Weak<xcb::Connection>, Rc<Cell<bool>>);
    impl Drop for Resource {
        fn drop(&mut self) {
            assert!(self.0.upgrade().is_some());
            self.1.set(true);
        }
    }
    let resource = util::ObjectWithConnection::new(c.clone(), |c| {
        assert!(c.root_window().is_some());
        Some(Resource(weak.clone(), dropped.clone()))
    });
    assert!(resource.get().is_some());
    assert!(Rc::ptr_eq(&resource.connection(), &c));
    drop(c);
    assert!(weak.upgrade().is_some());
    drop(resource);
    assert!(dropped.get());
    assert!(weak.upgrade().is_none());
    let c = x.connect();
    let window = create_window(&c);
    let _lease = util::change_window_event_mask(
        c.clone(),
        window,
        3,
        util::ChangeWindowEventMaskMode::Replace,
        false,
    )
    .unwrap();
    assert_eq!(c.window_attributes(window).unwrap().0, 3);
    {
        let _remove = util::change_window_event_mask(
            c.clone(),
            window,
            1,
            util::ChangeWindowEventMaskMode::Remove,
            true,
        )
        .unwrap();
        assert_eq!(c.window_attributes(window).unwrap().0, 2);
    }
    assert_eq!(c.window_attributes(window).unwrap().0, 3);
    let event_hub = util::EventHub::new(c.clone());
    let count = Rc::new(Cell::new(0));
    let out = count.clone();
    let life = event_hub.register(move |_| out.set(out.get() + 1)).unwrap();
    let _mask = util::change_window_event_mask(
        c.clone(),
        window,
        1 << 22,
        util::ChangeWindowEventMaskMode::Replace,
        false,
    )
    .unwrap();
    let atom = c.atom("test").unwrap();
    assert!(c.change_property(window, atom, 19, 32, &1u32.to_ne_bytes()));
    c.sync();
    event_hub.poll();
    assert!(count.get() > 0);
    let before = count.get();
    drop(life);
    assert!(c.change_property(window, atom, 19, 32, &2u32.to_ne_bytes()));
    c.sync();
    event_hub.poll();
    assert_eq!(count.get(), before);
}
