#[cfg(all(unix, not(target_os = "macos")))]
fn main() {
    use tg_base::platform::linux::{
        base_file_utilities_linux as files, base_linux_dbus_utilities as bus,
        base_linux_xdp_utilities as xdp, base_power_save_blocker_linux as power,
    };
    use zbus::blocking::connection::Builder;
    let address = std::env::args()
        .nth(1)
        .expect("explicit private bus address required");
    let c = Builder::address(address.as_str()).unwrap().build().unwrap();
    let q = tg_runtime::MainQueue::new();
    q.bind_current_thread();
    let _all = xdp::SettingWatcher::new(&c, q.clone(), |_, _, _| {}).unwrap();
    let _typed = xdp::SettingWatcher::typed::<(u32, u32)>(&c, q.clone(), |_, _, _| {}).unwrap();
    let _notify = xdp::SettingWatcher::notify(&c, q.clone(), |_, _| {}).unwrap();
    let _key =
        xdp::SettingWatcher::for_key(&c, q.clone(), "tg-test".into(), "missing".into(), |_| {})
            .unwrap();
    let _key_typed = xdp::SettingWatcher::for_key_typed::<u32>(
        &c,
        q.clone(),
        "tg-test".into(),
        "missing".into(),
        |_| {},
    )
    .unwrap();
    let _key_notify = xdp::SettingWatcher::for_key_notify(
        &c,
        q.clone(),
        "tg-test".into(),
        "missing".into(),
        || {},
    )
    .unwrap();
    println!(
        "ReadOne: {:?}",
        xdp::read_setting(&c, "org.freedesktop.appearance", "color-scheme")
    );
    println!(
        "ReadOne missing/typed: {:?}",
        xdp::read_setting_typed::<u32>(&c, "org.tg.Absent", "missing")
    );
    let responses = std::rc::Rc::new(std::cell::Cell::new(0));
    let count = responses.clone();
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender(xdp::SERVICE)
        .unwrap()
        .interface(xdp::REQUEST_INTERFACE)
        .unwrap()
        .member("Response")
        .unwrap()
        .build();
    let _response = bus::SignalSubscription::new(&c, rule, q.clone(), move |m| {
        let decoded = m.body().deserialize::<(
            u32,
            std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
        )>();
        println!("Response {}: {:?}", m.header().path().unwrap(), decoded);
        if decoded.is_ok() {
            count.set(count.get() + 1);
        }
    })
    .unwrap();
    let token = power::handle_token(123);
    let path = power::request_path(c.unique_name().unwrap().as_str(), &token).unwrap();
    let p = bus::proxy(
        &c,
        xdp::SERVICE,
        xdp::OBJECT_PATH,
        "org.freedesktop.portal.Inhibit",
    )
    .unwrap();
    let result: zbus::Result<zbus::zvariant::OwnedObjectPath> = p.call(
        "Inhibit",
        &(
            "",
            8u32,
            power::inhibit_options(&token, "headless isolated test"),
        ),
    );
    println!("Inhibit: {result:?}; predicted path: {path}");
    let fd = std::fs::File::open("/dev/null").unwrap();
    println!(
        "OpenDirectory: {:?}",
        files::portal_show_in_folder(&c, &fd, "", "")
    );
    let (send, receive) = std::sync::mpsc::channel();
    q.set_waker(Some(std::sync::Arc::new(move || {
        let _ = send.send(());
    })));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while responses.get() < 2 {
        q.process_all();
        if responses.get() >= 2
            || receive
                .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
                .is_err()
        {
            break;
        }
    }
    println!(
        "Response count: {} (backend acceptance remains unverified)",
        responses.get()
    );
}
#[cfg(not(all(unix, not(target_os = "macos"))))]
fn main() {}
