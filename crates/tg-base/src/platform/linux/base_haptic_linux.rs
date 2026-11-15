use super::base_linux_dbus_utilities as bus;
use zbus::blocking::Connection;
pub fn vibrate(connection: &Connection, desktop_file_name: &str) -> bus::Result<bool> {
    bus::proxy(
        connection,
        "org.sigxcpu.Feedback",
        "/org/sigxcpu/Feedback",
        "org.sigxcpu.Feedback.Haptic",
    )?
    .call("Vibrate", &(desktop_file_name, vec![(0.2f64, 5u32)]))
}
pub fn haptic(connection: Connection, desktop_file_name: String) {
    tg_runtime::async_(move || {
        let _ = vibrate(&connection, &desktop_file_name);
    });
}
pub const fn is_swipe_back_enabled() -> bool {
    true
}
