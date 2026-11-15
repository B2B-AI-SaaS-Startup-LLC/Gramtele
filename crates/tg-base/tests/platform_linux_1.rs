#![cfg(all(unix, not(target_os = "macos")))]
#[path = "support/private_bus.rs"]
mod private_bus;
use private_bus::PrivateBus;
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap},
    rc::Rc,
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};
use tg_base::platform::{
    base_platform_info as model,
    linux::{
        base_battery_saving_linux as battery, base_info_linux as info,
        base_linux_dbus_utilities as bus, base_linux_library as lib,
        base_linux_xdp_utilities as portal, base_power_save_blocker_linux as power,
        base_url_scheme_linux as url,
    },
};
use tg_runtime::MainQueue;
use zbus::zvariant::{OwnedValue, Str};
fn pump(queue: &MainQueue, done: impl Fn() -> bool) {
    let (tx, rx) = mpsc::channel();
    queue.set_waker(Some(Arc::new(move || {
        let _ = tx.send(());
    })));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done() {
        queue.process_all();
        if done() {
            break;
        }
        rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("private D-Bus/owner queue completion");
    }
    queue.set_waker(None);
}
#[test]
fn native_pure_vectors() {
    for line in include_str!("fixtures/platform-linux-1/native.jsonl").lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let a = v["args"].as_array().unwrap();
        let s = |n: usize| a[n].as_str().unwrap();
        let actual = match v["op"].as_str().unwrap() {
            "model_ok" => serde_json::json!(model::is_device_model_ok(s(0))),
            "simplify" => serde_json::json!(model::simplify_device_model(s(0))),
            "product" => serde_json::json!(model::product_name_to_device_model(s(0))),
            "good" => serde_json::json!(model::simplify_good_device_model(
                s(0),
                &["notebook", "desktop", "mobile", "workstation", "pc"]
            )),
            "finalize" => serde_json::json!(model::finalize_device_model(s(0))),
            "chassis" => {
                serde_json::json!(info::chassis_type_to_string(a[0].as_u64().unwrap() as u32))
            }
            "version" => serde_json::json!(info::valid_libc_version(s(0))),
            "old_glibc" => serde_json::json!(info::glibc_less_228(s(0), s(1))),
            "split" => serde_json::json!(url::split_args(s(0))),
            "quote" => serde_json::json!(url::quote_arg(s(0))),
            "join" => serde_json::json!(url::join_args(
                &a.iter()
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect::<Vec<_>>()
            )),
            "desktop" => serde_json::json!(info::desktop_environment(s(0))),
            "dbus_request_path" => serde_json::json!(power::request_path(s(0), s(1)).unwrap()),
            "start_enum" => serde_json::json!([
                bus::StartReply::SUCCESS.0,
                bus::StartReply::ALREADY_RUNNING.0
            ]),
            "load_missing" => {
                serde_json::json!(lib::Library::load("/tg-native-no-such-library.so", 0).is_some())
            }
            "symbol_missing" => serde_json::json!(lib::Library::load("libc.so.6", 0)
                .unwrap()
                .has_symbol("tg_symbol_does_not_exist")),
            _ => panic!("unknown vector"),
        };
        assert_eq!(actual, v["expected"], "{line}");
    }
}
#[test]
fn model_selection_version_locale_and_platform_predicates() {
    assert_eq!(
        info::device_model_from_values("Desktop", "", "", "kvm", 30),
        "Desktop"
    );
    assert_eq!(
        info::device_model_from_values("a".repeat(16).as_str(), "Fam", "Board", "kvm", 30),
        "Fam Board"
    );
    assert_eq!(
        info::device_model_from_values("", "", "", " kvm \n", 30),
        "KVM"
    );
    assert_eq!(
        info::device_model_from_values("", "", "", "none", 30),
        "Tablet"
    );
    assert_eq!(info::device_model_from_values("", "", "", "", 0), "Desktop");
    let facts = info::DesktopFacts {
        window_manager: "KWin".into(),
        wayland: true,
        x11: true,
        xwayland: true,
    };
    assert_eq!(
        info::system_version_from_values(
            Some(("Linux", "1")),
            " GNOME: ",
            &facts,
            "glibc",
            "2.42",
            true
        ),
        "Linux GNOME Wayland glibc 2.42"
    );
    assert_eq!(
        info::system_version_from_values(None, "", &info::DesktopFacts::default(), "", "", false),
        "Unknown"
    );
    assert_eq!(
        info::system_version_from_values(Some(("BSD", "14")), "", &facts, "", "1.0", false),
        "BSD 14 KWin Wayland libc 1.0"
    );
    assert_eq!(info::system_country("sr_Latn_RS"), "RS");
    assert_eq!(info::system_language("sr_RS", &[]), "sr");
    assert_eq!(
        info::system_language("sr_RS", &["sr-Latn-RS".into()]),
        "sr-Latn-RS"
    );
    assert!(info::is_linux());
    assert!(!info::is_mac());
    assert!(!info::is_windows());
    assert!([
        info::is_windows_32_bit(),
        info::is_windows_64_bit(),
        info::is_windows_arm64(),
        info::is_windows_store_build(),
        info::is_windows7_or_greater(),
        info::is_windows8_or_greater(),
        info::is_windows8_point1_or_greater(),
        info::is_windows10_or_greater(),
        info::is_windows11_or_greater(),
        info::is_mac_store_build(),
        info::is_mac10_12_or_greater(),
        info::is_mac10_13_or_greater(),
        info::is_mac10_14_or_greater(),
        info::is_mac10_15_or_greater(),
        info::is_mac11_0_or_greater(),
        info::is_mac26_0_or_greater(),
        info::metal_supported()
    ]
    .into_iter()
    .all(|v| !v));
    assert_eq!(info::auto_update_key(), "linux");
    assert_eq!(
        info::why_system_becomes_outdated(),
        model::OutdateReason::IsOld
    );
    assert_eq!(
        info::auto_update_version(),
        if info::when_system_becomes_outdated().is_some() {
            2
        } else {
            6
        }
    );
    assert!(!info::get_libc_name().is_empty());
    assert!(info::valid_libc_version(&info::get_libc_version()));
    info::start(serde_json::json!({}));
    info::finish();
}
#[test]
fn private_bus_queries_raw_reply_async_and_watcher_lifetime() {
    let b = PrivateBus::new();
    let c = b.connect();
    let q = MainQueue::new();
    q.bind_current_thread();
    assert!(bus::name_has_owner(&c, bus::SERVICE).unwrap());
    assert!(!bus::name_has_owner(&c, "org.tg.Absent").unwrap());
    let names = bus::list_activatable_names(&c).unwrap();
    assert!(names.contains(&bus::SERVICE.to_owned()));
    assert!(names.contains(&"org.tg.Activatable".to_owned()));
    c.request_name("org.tg.Activatable").unwrap();
    // Real daemon's already-running return is2, not enum's named value1.
    assert_eq!(
        bus::start_service_by_name(&c, "org.tg.Activatable").unwrap(),
        bus::StartReply(2)
    );
    assert!(bus::start_service_by_name(&c, "org.tg.Absent").is_err());
    let result = Arc::new(Mutex::new(None));
    let out = result.clone();
    bus::start_service_by_name_async(
        c.clone(),
        "org.tg.Activatable".into(),
        q.clone(),
        move |finish| {
            *out.lock().unwrap() = Some(finish().unwrap());
        },
    );
    pump(&q, || result.lock().unwrap().is_some());
    assert_eq!(*result.lock().unwrap(), Some(bus::StartReply(2)));
    let closed = b.connect();
    closed.clone().close().unwrap();
    let called = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mark = called.clone();
    bus::start_service_by_name_async(
        closed,
        "org.tg.Activatable".into(),
        q.clone(),
        move |finish| {
            assert!(finish().is_err());
            mark.store(true, std::sync::atomic::Ordering::Release);
        },
    );
    assert!(called.load(std::sync::atomic::Ordering::Acquire));
    let events = Rc::new(RefCell::new(vec![]));
    let out = events.clone();
    let nested = q.clone();
    let watcher = bus::ServiceWatcher::new(&c, "org.tg.Test", q.clone(), move |n, o, v| {
        out.borrow_mut()
            .push((n.to_owned(), o.to_owned(), v.to_owned()));
        nested.process_all();
    })
    .unwrap();
    let owner = b.connect();
    owner.request_name("org.tg.Other").unwrap();
    owner.request_name("org.tg.Test").unwrap();
    pump(&q, || events.borrow().len() == 1);
    assert_eq!(events.borrow()[0].1, "");
    owner.release_name("org.tg.Test").unwrap();
    pump(&q, || events.borrow().len() == 2);
    assert_eq!(events.borrow()[1].2, "");
    drop(watcher);
    owner.request_name("org.tg.Test").unwrap();
    q.process_all();
    assert_eq!(events.borrow().len(), 2);
}
struct Idle;
#[zbus::interface(name = "org.gnome.Mutter.IdleMonitor")]
impl Idle {
    fn get_idletime(&self) -> u64 {
        25
    }
}
#[test]
fn last_input_xcb_priority_mutter_and_permanent_error_latch() {
    use tg_base::platform::linux::base_last_input_linux::LastInput;
    let b = PrivateBus::new();
    let c = b.connect();
    let service = zbus::blocking::connection::Builder::address(b.address.as_str())
        .unwrap()
        .name("org.gnome.Mutter.IdleMonitor")
        .unwrap()
        .serve_at("/org/gnome/Mutter/IdleMonitor/Core", Idle)
        .unwrap()
        .build()
        .unwrap();
    let input = LastInput::new(c.clone(), || None);
    assert_eq!(input.last_user_input_time(100), Some(75));
    let xcb = LastInput::new(c.clone(), || Some(4));
    assert_eq!(xcb.last_user_input_time(100), Some(4));
    service
        .release_name("org.gnome.Mutter.IdleMonitor")
        .unwrap();
    assert_eq!(input.last_user_input_time(100), None);
    service
        .request_name("org.gnome.Mutter.IdleMonitor")
        .unwrap();
    assert_eq!(input.last_user_input_time(100), None);
    assert_eq!(LastInput::new(c, || None).last_user_input_time(100), None);
    assert_eq!(xcb.last_user_input_time(100), Some(4));
}
#[test]
fn portal_response_parsing_and_parent_identifiers() {
    struct Parent(Option<String>, bool, u64);
    impl portal::ParentWindow for Parent {
        fn portal_identifier(&self) -> Option<String> {
            self.0.clone()
        }
        fn is_x11(&self) -> bool {
            self.1
        }
        fn native_window_id(&self) -> u64 {
            self.2
        }
    }
    assert_eq!(portal::parent_window_id(None), "");
    assert_eq!(
        portal::parent_window_id(Some(&Parent(None, true, 0xDEAD))),
        "x11:dead"
    );
    assert_eq!(
        portal::parent_window_id(Some(&Parent(Some("".into()), true, 42))),
        ""
    );
    assert_eq!(portal::parent_window_id(Some(&Parent(None, false, 42))), "");
    let m = zbus::Message::signal(
        portal::OBJECT_PATH,
        portal::SETTINGS_INTERFACE,
        "SettingChanged",
    )
    .unwrap()
    .build(&("group", "key", OwnedValue::from(true)))
    .unwrap();
    let (g, k, v) = portal::setting_changed(&m).unwrap();
    assert_eq!((g.as_str(), k.as_str()), ("group", "key"));
    assert!(bool::try_from(v).unwrap());
    let bad = zbus::Message::signal(
        portal::OBJECT_PATH,
        portal::SETTINGS_INTERFACE,
        "SettingChanged",
    )
    .unwrap()
    .build(&("bad", 1u32))
    .unwrap();
    assert!(portal::setting_changed(&bad).is_err());
    assert_eq!(power::handle_token(u32::MAX), "desktop_app4294967295");
    assert_eq!(
        power::request_path(":1.2.3", "token").unwrap(),
        "/org/freedesktop/portal/desktop/request/1_2.3/token"
    );
    assert!(power::request_path(":1", "x").is_none());
    let options = power::inhibit_options("x", "reason");
    assert_eq!(<&str>::try_from(&options["reason"]).unwrap(), "reason");
    assert_eq!(
        battery::value_from_property(
            battery::Driver::PowerProfiles,
            OwnedValue::from(Str::from("power-saver"))
        ),
        Some(true)
    );
    assert_eq!(
        battery::value_from_property(
            battery::Driver::PowerProfiles,
            OwnedValue::from(Str::from("balanced"))
        ),
        Some(false)
    );
    assert_eq!(
        battery::value_from_property(battery::Driver::Portal, OwnedValue::from(1u32)),
        None
    );
    assert_eq!(
        battery::value_from_property(battery::Driver::Dummy, OwnedValue::from(true)),
        None
    );
}
struct A11y;
#[zbus::interface(name = "org.a11y.Status")]
impl A11y {
    #[zbus(property)]
    fn screen_reader_enabled(&self) -> bool {
        false
    }
}
type ActivationSlot = Rc<RefCell<Option<Rc<dyn Fn(bool)>>>>;
struct Accessibility {
    x11: bool,
    always: bool,
    active: Cell<bool>,
    callback: ActivationSlot,
}
impl tg_base::platform::linux::base_screen_reader_state_linux::AccessibilityBackend
    for Accessibility
{
    fn is_x11(&self) -> bool {
        self.x11
    }
    fn always_on(&self) -> bool {
        self.always
    }
    fn accessible_active(&self) -> bool {
        self.active.get()
    }
    fn observe_activation(&self, callback: Rc<dyn Fn(bool)>) -> tg_rpl::Lifetime {
        *self.callback.borrow_mut() = Some(callback);
        let slot = self.callback.clone();
        let lifetime = tg_rpl::Lifetime::new();
        lifetime.add(move || {
            slot.borrow_mut().take();
        });
        lifetime
    }
}
#[test]
fn screen_reader_private_service_properties_owners_and_qt_lifetime_contract() {
    use tg_base::{
        platform::linux::base_screen_reader_state_linux as sr,
        screen_reader_state::ScreenReaderState,
    };
    let b = PrivateBus::new();
    let c = b.connect();
    let q = MainQueue::new();
    q.bind_current_thread();
    let qt = Rc::new(Accessibility {
        x11: true,
        always: false,
        active: Cell::new(false),
        callback: Rc::default(),
    });
    let state = sr::LinuxScreenReaderState::new(Some(c.clone()), qt, q.clone());
    assert!(!state.active());
    let service = zbus::blocking::connection::Builder::address(b.address.as_str())
        .unwrap()
        .name(sr::A11Y_SERVICE)
        .unwrap()
        .serve_at(sr::A11Y_PATH, A11y)
        .unwrap()
        .build()
        .unwrap();
    // Explicit owner roundtrip makes the initial Status proxy observable without sleeps.
    state.refresh_status_proxy();
    state.update_active();
    q.process_all();
    service
        .emit_signal(
            None::<&str>,
            sr::A11Y_PATH,
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
            &(
                sr::A11Y_INTERFACE,
                HashMap::from([("ScreenReaderEnabled", OwnedValue::from(true))]),
                Vec::<String>::new(),
            ),
        )
        .unwrap();
    pump(&q, || state.active());
    service.release_name(sr::A11Y_SERVICE).unwrap();
    pump(&q, || !state.active());
    let callback = Rc::default();
    let qt = Rc::new(Accessibility {
        x11: false,
        always: false,
        active: Cell::new(true),
        callback,
    });
    let state = sr::LinuxScreenReaderState::new(None, qt.clone(), q.clone());
    assert!(state.active());
    qt.callback.borrow().as_ref().unwrap()(false);
    assert!(!state.active());
    drop(state);
    assert!(qt.callback.borrow().is_none());
    let always = Rc::new(Accessibility {
        x11: true,
        always: true,
        active: Cell::new(false),
        callback: Rc::default(),
    });
    assert!(sr::LinuxScreenReaderState::new(None, always, q).active());
}
#[test]
fn battery_capabilities_and_power_timer_virtual_time() {
    use tg_base::{
        battery_saving::AbstractBatterySaving,
        platform::base_platform_power_save_blocker::PowerSaveBackend,
        power_save_blocker::PowerSaveBlockType as Ty,
        timer::{ManualClock, TimerHost},
    };
    let b = PrivateBus::new();
    let c = b.connect();
    let q = MainQueue::new();
    q.bind_current_thread();
    for missing in [
        battery::Capabilities {
            power_supply_entries: false,
            dup_default: true,
            get_enabled: true,
        },
        battery::Capabilities {
            power_supply_entries: true,
            dup_default: false,
            get_enabled: true,
        },
        battery::Capabilities {
            power_supply_entries: true,
            dup_default: true,
            get_enabled: false,
        },
    ] {
        assert_eq!(
            battery::LinuxBatterySaving::new(
                missing,
                battery::Driver::Dummy,
                None,
                q.clone(),
                Box::new(|| panic!("unsupported"))
            )
            .enabled(),
            None
        );
    }
    let full = battery::Capabilities {
        power_supply_entries: true,
        dup_default: true,
        get_enabled: true,
    };
    assert_eq!(
        battery::LinuxBatterySaving::new(
            full,
            battery::Driver::Dummy,
            None,
            q.clone(),
            Box::new(|| {})
        )
        .enabled(),
        Some(false)
    );
    struct Window(Rc<Cell<u32>>, Rc<Cell<u32>>);
    impl power::PowerWindowBackend for Window {
        fn parent_window_id(&self) -> String {
            self.1.set(self.1.get() + 1);
            String::new()
        }
        fn reset_xcb_screen_saver(&self) {
            self.0.set(self.0.get() + 1)
        }
    }
    let resets = Rc::new(Cell::new(0));
    let parents = Rc::new(Cell::new(0));
    let clock = ManualClock::new(0);
    let host = TimerHost::new(Arc::new(clock.clone()));
    let state = power::PowerController::new(
        c,
        q.clone(),
        Rc::new(Window(resets.clone(), parents.clone())),
        host.clone(),
    );
    let backend = state.backend();
    backend.block(Ty::PreventDisplaySleep, "test", None);
    q.process();
    assert_eq!(parents.get(), 0);
    q.process_all();
    assert_eq!(parents.get(), 1);
    clock.advance(9999);
    host.process();
    assert_eq!(resets.get(), 0);
    clock.advance(1);
    host.process();
    assert_eq!(resets.get(), 1);
    backend.block(Ty::PreventDisplaySleep, "changed ignored", None);
    q.process_all();
    clock.advance(10000);
    host.process();
    assert_eq!(resets.get(), 2);
    backend.unblock(Ty::PreventDisplaySleep, None);
    q.process_all();
    clock.advance(10000);
    host.process();
    assert_eq!(resets.get(), 2);
    backend.block(Ty::PreventAppSuspension, "test", None);
    q.process_all();
    clock.advance(10000);
    host.process();
    assert_eq!(resets.get(), 2);
    backend.unblock(Ty::PreventAppSuspension, None);
    q.process_all();
}
#[test]
fn launch_context_and_activation_null_are_not_process_environment_mutation() {
    use tg_base::platform::linux::{
        base_linux_app_launch_context::AppLaunchContext,
        base_linux_xdg_activation_token::ActivationTokenProvider,
    };
    struct Token(Option<String>);
    impl ActivationTokenProvider for Token {
        fn xdg_activation_token(&self) -> Option<String> {
            self.0.clone()
        }
        fn run_with_xdg_activation_token(&self, f: Box<dyn FnOnce(Option<String>)>) {
            f(self.0.clone());
        }
    }
    let before = std::env::var_os("PARENT_WINDOW_ID");
    let inherited = BTreeMap::from([("PARENT_WINDOW_ID".into(), "old".into())]);
    assert_eq!(
        AppLaunchContext::new("".into()).environment(inherited.clone()),
        inherited
    );
    let context = AppLaunchContext::new("x11:123".into());
    assert_eq!(
        context.environment(inherited)["PARENT_WINDOW_ID"],
        "x11:123"
    );
    assert_eq!(std::env::var_os("PARENT_WINDOW_ID"), before);
    assert_eq!(context.startup_notify_id(&Token(None)), None);
    assert_eq!(
        context.startup_notify_id(&Token(Some("".into()))),
        Some("".into())
    );
    assert_eq!(
        tg_base::platform::linux::base_process_linux::activation_message(7, Some(4)),
        [1, 7, 4, 0, 0]
    );
    assert!(tg_base::platform::linux::base_haptic_linux::is_swipe_back_enabled());
}

struct Snap {
    value: Mutex<String>,
    writes: Arc<Mutex<Vec<String>>>,
}
#[zbus::interface(name = "io.snapcraft.Settings")]
impl Snap {
    fn get_sub(&self, kind: &str, protocol: &str) -> String {
        assert_eq!((kind, protocol), ("default-url-scheme-handler", "tgtest"));
        self.value.lock().unwrap().clone()
    }
    fn set_sub(&self, kind: &str, protocol: &str, value: &str) {
        assert_eq!((kind, protocol), ("default-url-scheme-handler", "tgtest"));
        *self.value.lock().unwrap() = value.into();
        self.writes.lock().unwrap().push(value.into());
    }
}
struct Haptic;
#[zbus::interface(name = "org.sigxcpu.Feedback.Haptic")]
impl Haptic {
    fn vibrate(&self, app: &str, pattern: Vec<(f64, u32)>) -> bool {
        assert_eq!(app, "tg-test");
        assert_eq!(pattern, [(0.2, 5)]);
        true
    }
}
type ShownItems = Arc<Mutex<Vec<(Vec<String>, String)>>>;
struct FileManager(ShownItems);
#[zbus::interface(name = "org.freedesktop.FileManager1")]
impl FileManager {
    fn show_items(&self, uris: Vec<String>, startup: &str) {
        self.0.lock().unwrap().push((uris, startup.into()));
    }
}
#[test]
fn actual_private_bus_snap_haptic_and_file_manager_contracts() {
    let b = PrivateBus::new();
    let c = b.connect();
    let writes = Arc::new(Mutex::new(vec![]));
    let items = Arc::new(Mutex::new(vec![]));
    let service = zbus::blocking::connection::Builder::address(b.address.as_str())
        .unwrap()
        .name("io.snapcraft.Settings")
        .unwrap()
        .name("org.sigxcpu.Feedback")
        .unwrap()
        .name("org.freedesktop.FileManager1")
        .unwrap()
        .serve_at(
            "/io/snapcraft/Settings",
            Snap {
                value: Mutex::new("other.desktop".into()),
                writes: writes.clone(),
            },
        )
        .unwrap()
        .serve_at("/org/sigxcpu/Feedback", Haptic)
        .unwrap()
        .serve_at("/org/freedesktop/FileManager1", FileManager(items.clone()))
        .unwrap()
        .build()
        .unwrap();
    url::snap_default_handler(&c, "tgtest", "test").unwrap();
    url::snap_default_handler(&c, "tgtest", "test").unwrap();
    assert_eq!(*writes.lock().unwrap(), ["test.desktop"]);
    assert!(tg_base::platform::linux::base_haptic_linux::vibrate(&c, "tg-test").unwrap());
    tg_base::platform::linux::base_file_utilities_linux::dbus_show_in_folder(
        &c,
        "file:///tmp/a%20b",
        "token",
    )
    .unwrap();
    assert_eq!(
        *items.lock().unwrap(),
        [(vec!["file:///tmp/a%20b".into()], "token".into())]
    );
    service.release_name("org.sigxcpu.Feedback").unwrap();
    assert!(tg_base::platform::linux::base_haptic_linux::vibrate(&c, "tg-test").is_err());
}
struct Profiles;
#[zbus::interface(name = "org.freedesktop.UPower.PowerProfiles")]
impl Profiles {
    #[zbus(property)]
    fn active_profile(&self) -> &str {
        "balanced"
    }
}
#[test]
fn battery_private_power_profiles_changes_and_owner_loss() {
    use tg_base::battery_saving::AbstractBatterySaving;
    let b = PrivateBus::new();
    let c = b.connect();
    let q = MainQueue::new();
    q.bind_current_thread();
    let changes = Rc::new(Cell::new(0));
    let count = changes.clone();
    let service = zbus::blocking::connection::Builder::address(b.address.as_str())
        .unwrap()
        .name("org.freedesktop.UPower.PowerProfiles")
        .unwrap()
        .serve_at("/org/freedesktop/UPower/PowerProfiles", Profiles)
        .unwrap()
        .build()
        .unwrap();
    let state = battery::LinuxBatterySaving::new(
        battery::Capabilities {
            power_supply_entries: true,
            dup_default: true,
            get_enabled: true,
        },
        battery::Driver::PowerProfiles,
        Some(c),
        q.clone(),
        Box::new(move || count.set(count.get() + 1)),
    );
    assert_eq!(state.enabled(), Some(false));
    let emit = |v: &str| {
        service
            .emit_signal(
                None::<&str>,
                "/org/freedesktop/UPower/PowerProfiles",
                "org.freedesktop.DBus.Properties",
                "PropertiesChanged",
                &(
                    "org.freedesktop.UPower.PowerProfiles",
                    HashMap::from([("ActiveProfile", OwnedValue::from(Str::from(v)))]),
                    Vec::<String>::new(),
                ),
            )
            .unwrap()
    };
    emit("power-saver");
    pump(&q, || state.enabled() == Some(true));
    assert_eq!(changes.get(), 1);
    emit("balanced");
    pump(&q, || state.enabled() == Some(false));
    assert_eq!(changes.get(), 2);
    service
        .release_name("org.freedesktop.UPower.PowerProfiles")
        .unwrap();
    pump(&q, || changes.get() == 3);
    assert_eq!(state.enabled(), Some(false));
}
#[test]
fn folder_failure_falls_back_and_drop_cancels_owner_callbacks() {
    use tg_base::platform::linux::{
        base_file_utilities_linux as f, base_linux_xdg_activation_token::ActivationTokenProvider,
    };
    struct Context(Rc<RefCell<Vec<std::path::PathBuf>>>);
    impl ActivationTokenProvider for Context {
        fn xdg_activation_token(&self) -> Option<String> {
            None
        }
        fn run_with_xdg_activation_token(&self, f: Box<dyn FnOnce(Option<String>)>) {
            f(None)
        }
    }
    impl f::FolderContext for Context {
        fn parent_window_id(&self) -> String {
            String::new()
        }
        fn open_parent_directory(&self, p: &std::path::Path) {
            self.0.borrow_mut().push(p.to_owned())
        }
    }
    let b = PrivateBus::new();
    let c = b.connect();
    let q = MainQueue::new();
    q.bind_current_thread();
    let effects = Rc::new(RefCell::new(vec![]));
    let context = Rc::new(Context(effects.clone()));
    let path = b.root.join("no-such-file");
    let request = f::FolderRequest::start(c.clone(), q.clone(), context.clone(), path.clone());
    pump(&q, || effects.borrow().len() == 1);
    assert_eq!(effects.borrow()[0], b.root);
    drop(request);
    let request = f::FolderRequest::start(c, q.clone(), context, path);
    drop(request);
    q.process_all();
    assert_eq!(effects.borrow().len(), 1);
}
#[test]
#[should_panic(expected = "BundledResourcesPath not implemented.")]
fn bundled_resources_preserves_unexpected() {
    tg_base::platform::linux::base_file_utilities_linux::bundled_resources_path();
}
#[test]
fn url_gio_registration_in_child_with_private_xdg_directories() {
    use tg_base::platform::base_platform_url_scheme::UrlSchemeDescriptor;
    let mut desc = UrlSchemeDescriptor {
        executable: "/usr/bin/true".into(),
        arguments: "'a b' --flag".into(),
        protocol: "tg-rust-private-test".into(),
        display_app_name: "tg isolated test".into(),
        ..Default::default()
    };
    assert_eq!(
        url::handler_type(&desc),
        "x-scheme-handler/tg-rust-private-test"
    );
    assert_eq!(
        url::commandline(&desc, false),
        "/usr/bin/true 'a b' --flag --"
    );
    desc.executable = std::env::current_exe()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    if std::env::var_os("TG_LINUX_GIO_CHILD").is_some() {
        let root = std::env::var_os("TG_LINUX_GIO_CHILD").unwrap();
        assert!(std::env::var_os("XDG_DATA_HOME")
            .unwrap()
            .to_string_lossy()
            .starts_with(&*root.to_string_lossy()));
        assert!(!url::check_url_scheme(&desc));
        url::register_url_scheme(
            &desc,
            &url::UrlSchemeContext {
                is_snap: false,
                desktop_file_name: String::new(),
                executable_name: "test".into(),
                connection: None,
            },
        );
        if lib::Library::load("libgio-2.0.so.0", 0).is_some() {
            assert!(url::check_url_scheme(&desc));
            assert_eq!(
                lib::filename_to_uri(std::path::Path::new("/tmp/a b#é")).unwrap(),
                "file:///tmp/a%20b%23%C3%A9"
            );
            assert!(lib::filename_to_uri(std::path::Path::new("relative")).is_none());
        } else {
            assert!(!url::check_url_scheme(&desc));
        }
        url::unregister_url_scheme(&desc);
        assert!(!url::check_url_scheme(&desc));
        return;
    }
    let b = PrivateBus::new();
    for dir in ["config", "data", "cache", "runtime"] {
        std::fs::create_dir_all(b.root.join(dir)).unwrap();
    }
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "url_gio_registration_in_child_with_private_xdg_directories",
            "--nocapture",
        ])
        .env("TG_LINUX_GIO_CHILD", &b.root)
        .env("DBUS_SESSION_BUS_ADDRESS", &b.address)
        .env(
            "DBUS_SYSTEM_BUS_ADDRESS",
            format!("unix:path={}/no-system", b.root.display()),
        )
        .env("XDG_CONFIG_HOME", b.root.join("config"))
        .env("XDG_CONFIG_DIRS", b.root.join("config"))
        .env("XDG_DATA_HOME", b.root.join("data"))
        .env("XDG_DATA_DIRS", b.root.join("data"))
        .env("XDG_CACHE_HOME", b.root.join("cache"))
        .env("XDG_RUNTIME_DIR", b.root.join("runtime"))
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .status()
        .unwrap();
    assert!(status.success());
}
