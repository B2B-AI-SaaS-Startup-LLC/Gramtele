use super::{base_linux_dbus_utilities as bus, base_linux_xdp_utilities as portal};
use crate::{
    platform::base_platform_power_save_blocker::{PowerSaveBackend, WindowId},
    power_save_blocker::PowerSaveBlockType,
    timer::{Timer, TimerHost},
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::{Rc, Weak},
};
use tg_runtime::{MainQueue, MainRef};
use zbus::{
    blocking::Connection,
    zvariant::{OwnedObjectPath, OwnedValue, Str},
};
pub fn handle_token(random: u32) -> String {
    format!("desktop_app{random}")
}
pub fn request_path(unique_name: &str, token: &str) -> Option<String> {
    let mut name = unique_name.get(1..)?.to_owned();
    let dot = name.find('.')?;
    name.replace_range(dot..dot + 1, "_");
    Some(format!("{}/request/{name}/{token}", portal::OBJECT_PATH))
}
pub fn inhibit_options(token: &str, reason: &str) -> HashMap<String, OwnedValue> {
    HashMap::from([
        ("handle_token".into(), OwnedValue::from(Str::from(token))),
        ("reason".into(), OwnedValue::from(Str::from(reason))),
    ])
}
fn close(connection: Connection, path: String) {
    tg_runtime::async_(move || {
        if let Ok(p) = bus::proxy(
            &connection,
            portal::SERVICE,
            &path,
            portal::REQUEST_INTERFACE,
        ) {
            let _ = p.call::<_, _, ()>("Close", &());
        }
    });
}
struct PendingResponse {
    owner: Weak<PortalInhibit>,
    connection: Connection,
    path: String,
    subscription: RefCell<Option<bus::SignalSubscription>>,
}
pub struct PortalInhibit {
    connection: Connection,
    path: RefCell<Option<String>>,
    began: Cell<bool>,
    parent: Box<dyn Fn() -> String>,
}
impl PortalInhibit {
    pub fn new(
        connection: Connection,
        queue: MainQueue,
        flags: u32,
        description: String,
        parent: impl Fn() -> String + 'static,
        random: u32,
    ) -> Rc<Self> {
        let state = Rc::new(Self {
            connection,
            path: RefCell::default(),
            began: Cell::new(false),
            parent: Box::new(parent),
        });
        let target = MainRef::new(&state);
        let q = queue.clone();
        target.on_queue(&queue, move |state| {
            let Some(unique) = state.connection.unique_name() else {
                return;
            };
            let token = handle_token(random);
            let Some(path) = request_path(unique.as_str(), &token) else {
                return;
            };
            let Ok(rule) = bus::signal_rule(
                portal::SERVICE,
                &path,
                portal::REQUEST_INTERFACE,
                "Response",
            ) else {
                return;
            };
            let pending = Rc::new(PendingResponse {
                owner: Rc::downgrade(state),
                connection: state.connection.clone(),
                path: path.clone(),
                subscription: RefCell::default(),
            });
            let keep = pending.clone();
            let Ok(subscription) =
                bus::SignalSubscription::new(&state.connection, rule, q, move |message| {
                    if message
                        .body()
                        .deserialize::<(u32, HashMap<String, OwnedValue>)>()
                        .is_err()
                    {
                        return;
                    }
                    if keep.owner.upgrade().is_none() {
                        close(keep.connection.clone(), keep.path.clone());
                    }
                    keep.subscription.borrow_mut().take();
                })
            else {
                return;
            };
            *pending.subscription.borrow_mut() = Some(subscription);
            *state.path.borrow_mut() = Some(path);
            state.began.set(true);
            let conn = state.connection.clone();
            let parent = (state.parent)();
            tg_runtime::async_(move || {
                if let Ok(p) = bus::proxy(
                    &conn,
                    portal::SERVICE,
                    portal::OBJECT_PATH,
                    "org.freedesktop.portal.Inhibit",
                ) {
                    let _: bus::Result<OwnedObjectPath> = p.call(
                        "Inhibit",
                        &(parent, flags, inhibit_options(&token, &description)),
                    );
                }
            });
        });
        state
    }
    pub fn request_ready(&self) -> bool {
        self.began.get()
    }
}
impl Drop for PortalInhibit {
    fn drop(&mut self) {
        if let Some(path) = self.path.get_mut().take() {
            close(self.connection.clone(), path);
        }
    }
}
pub trait PowerWindowBackend {
    fn parent_window_id(&self) -> String;
    fn reset_xcb_screen_saver(&self);
}
pub struct PowerController {
    connection: Connection,
    queue: MainQueue,
    window: Rc<dyn PowerWindowBackend>,
    display: RefCell<Option<Rc<PortalInhibit>>>,
    suspend: RefCell<Option<Rc<PortalInhibit>>>,
    xcb: Timer,
}
#[derive(Clone)]
pub struct LinuxPowerSaveBackend {
    target: MainRef<PowerController>,
    queue: MainQueue,
}
impl PowerController {
    pub fn new(
        connection: Connection,
        queue: MainQueue,
        window: Rc<dyn PowerWindowBackend>,
        host: Rc<TimerHost>,
    ) -> Rc<Self> {
        let w = window.clone();
        let xcb = Timer::in_host(&host, Some(Rc::new(move || w.reset_xcb_screen_saver())));
        Rc::new(Self {
            connection,
            queue,
            window,
            display: RefCell::default(),
            suspend: RefCell::default(),
            xcb,
        })
    }
    pub fn backend(self: &Rc<Self>) -> LinuxPowerSaveBackend {
        LinuxPowerSaveBackend {
            target: MainRef::new(self),
            queue: self.queue.clone(),
        }
    }
    fn prevent(&self, ty: PowerSaveBlockType, enabled: bool, description: &str) {
        let slot = match ty {
            PowerSaveBlockType::PreventAppSuspension => &self.suspend,
            PowerSaveBlockType::PreventDisplaySleep => {
                if enabled {
                    if !self.xcb.is_active() {
                        self.xcb.call_each(10000);
                    }
                } else {
                    self.xcb.cancel();
                }
                &self.display
            }
        };
        if enabled {
            if slot.borrow().is_none() {
                let mut bytes = [0u8; 4];
                getrandom::fill(&mut bytes).expect("RandomValue<uint>");
                let window = self.window.clone();
                let request = PortalInhibit::new(
                    self.connection.clone(),
                    self.queue.clone(),
                    if ty == PowerSaveBlockType::PreventDisplaySleep {
                        8
                    } else {
                        4
                    },
                    description.into(),
                    move || window.parent_window_id(),
                    u32::from_ne_bytes(bytes),
                );
                *slot.borrow_mut() = Some(request);
            }
        } else {
            slot.borrow_mut().take();
        }
    }
}
impl PowerSaveBackend for LinuxPowerSaveBackend {
    fn block(&self, ty: PowerSaveBlockType, description: &str, _window: WindowId) {
        let text = description.to_owned();
        self.target
            .on_queue(&self.queue, move |c| c.prevent(ty, true, &text));
    }
    fn unblock(&self, ty: PowerSaveBlockType, _window: WindowId) {
        self.target
            .on_queue(&self.queue, move |c| c.prevent(ty, false, ""));
    }
}

pub struct XcbPowerWindowBackend {
    connection: Rc<super::base_linux_xcb_library::Connection>,
    parent: Box<dyn Fn() -> String>,
}
impl XcbPowerWindowBackend {
    pub fn new(
        connection: Rc<super::base_linux_xcb_library::Connection>,
        parent: impl Fn() -> String + 'static,
    ) -> Self {
        Self {
            connection,
            parent: Box::new(parent),
        }
    }
}
impl PowerWindowBackend for XcbPowerWindowBackend {
    fn parent_window_id(&self) -> String {
        (self.parent)()
    }
    fn reset_xcb_screen_saver(&self) {
        if !self.connection.has_error() {
            self.connection.reset_screen_saver();
        }
    }
}
