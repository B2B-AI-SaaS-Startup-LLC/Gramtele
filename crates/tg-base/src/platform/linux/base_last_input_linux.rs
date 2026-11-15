use super::base_linux_dbus_utilities as bus;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use zbus::blocking::Connection;
/// XCB provider is part2; None falls through to real Mutter D-Bus, first error latches.
static UNSUPPORTED: AtomicBool = AtomicBool::new(false);
pub struct LastInput {
    connection: Connection,
    xcb: Box<dyn Fn() -> Option<i64> + Send + Sync>,
}
impl LastInput {
    pub fn new(
        connection: Connection,
        xcb: impl Fn() -> Option<i64> + Send + Sync + 'static,
    ) -> Self {
        Self {
            connection,
            xcb: Box::new(xcb),
        }
    }
    pub fn last_user_input_time(&self, now: i64) -> Option<i64> {
        if let Some(result) = (self.xcb)() {
            return Some(result);
        }
        if UNSUPPORTED.load(Ordering::Acquire) {
            return None;
        }
        let result = (|| {
            let p = bus::proxy(
                &self.connection,
                "org.gnome.Mutter.IdleMonitor",
                "/org/gnome/Mutter/IdleMonitor/Core",
                "org.gnome.Mutter.IdleMonitor",
            )?;
            p.call::<_, _, u64>("GetIdletime", &())
        })();
        match result {
            Ok(idle) => Some(now.wrapping_sub(idle as i64)),
            Err(_) => {
                UNSUPPORTED.store(true, Ordering::Release);
                None
            }
        }
    }
    pub fn install(self: Arc<Self>) {
        crate::platform::base_platform_last_input::set_provider(Some(Box::new(move || {
            self.last_user_input_time(tg_runtime::now())
        })));
    }
}
