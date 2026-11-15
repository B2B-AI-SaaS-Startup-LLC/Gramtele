use super::{base_linux_dbus_utilities as bus, base_linux_xdp_utilities::PropertyWatcher};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use tg_rpl::{Lifetime, Producer, Variable};
use tg_runtime::MainQueue;
use zbus::{blocking::Connection, proxy::MethodFlags, zvariant::OwnedValue};
pub const A11Y_SERVICE: &str = "org.a11y.Bus";
pub const A11Y_PATH: &str = "/org/a11y/bus";
pub const A11Y_INTERFACE: &str = "org.a11y.Status";
pub trait AccessibilityBackend {
    fn is_x11(&self) -> bool;
    fn always_on(&self) -> bool;
    fn accessible_active(&self) -> bool;
    fn observe_activation(&self, callback: Rc<dyn Fn(bool)>) -> Lifetime;
}
pub struct LinuxScreenReaderState {
    connection: Option<Connection>,
    backend: Rc<dyn AccessibilityBackend>,
    x11: bool,
    value: Variable<bool>,
    status: Cell<Option<bool>>,
    service: RefCell<Option<bus::ServiceWatcher>>,
    property: RefCell<Option<PropertyWatcher>>,
    observer: RefCell<Option<Lifetime>>,
    queue: MainQueue,
}
impl LinuxScreenReaderState {
    pub fn new(
        connection: Option<Connection>,
        backend: Rc<dyn AccessibilityBackend>,
        queue: MainQueue,
    ) -> Rc<Self> {
        let x11 = backend.is_x11();
        let initial = if x11 {
            backend.always_on()
        } else {
            backend.accessible_active()
        };
        let state = Rc::new(Self {
            connection,
            backend,
            x11,
            value: Variable::new(initial),
            status: Cell::new(None),
            service: RefCell::default(),
            property: RefCell::default(),
            observer: RefCell::default(),
            queue,
        });
        if !x11 {
            let weak = Rc::downgrade(&state);
            let guard = state.backend.observe_activation(Rc::new(move |v| {
                if let Some(s) = weak.upgrade() {
                    s.accessibility_active_changed(v);
                }
            }));
            *state.observer.borrow_mut() = Some(guard);
            return state;
        }
        if state.backend.always_on() {
            return state;
        }
        if let Some(connection) = &state.connection {
            let weak = Rc::downgrade(&state);
            *state.service.borrow_mut() = bus::ServiceWatcher::new(
                connection,
                A11Y_SERVICE,
                state.queue.clone(),
                move |_, _, _| {
                    if let Some(s) = weak.upgrade() {
                        s.refresh_status_proxy();
                        s.update_active();
                    }
                },
            )
            .ok();
            state.refresh_status_proxy();
            state.update_active();
        }
        state
    }
    pub fn accessibility_active_changed(&self, active: bool) {
        if !self.x11 {
            self.value.set(active);
        }
    }
    pub fn refresh_status_proxy(self: &Rc<Self>) {
        self.property.borrow_mut().take();
        self.status.set(None);
        let Some(conn) = &self.connection else {
            return;
        };
        if !self.x11
            || self.backend.always_on()
            || !bus::name_has_owner(conn, A11Y_SERVICE).unwrap_or(false)
        {
            return;
        }
        let Ok(proxy) = bus::proxy(
            conn,
            A11Y_SERVICE,
            A11Y_PATH,
            "org.freedesktop.DBus.Properties",
        ) else {
            return;
        };
        let value = proxy
            .call_with_flags::<_, _, OwnedValue>(
                "Get",
                MethodFlags::NoAutoStart.into(),
                &(A11Y_INTERFACE, "ScreenReaderEnabled"),
            )
            .ok()
            .flatten()
            .and_then(|v| bool::try_from(v).ok())
            .unwrap_or(false);
        self.status.set(Some(value));
        let weak = Rc::downgrade(self);
        *self.property.borrow_mut() = PropertyWatcher::new(
            conn,
            self.queue.clone(),
            A11Y_SERVICE,
            A11Y_PATH,
            A11Y_INTERFACE.into(),
            move |mut changed, invalidated| {
                if let Some(s) = weak.upgrade() {
                    if let Some(v) = changed.remove("ScreenReaderEnabled") {
                        s.status.set(Some(bool::try_from(v).unwrap_or(false)));
                        s.update_active();
                    } else if invalidated.iter().any(|n| n == "ScreenReaderEnabled") {
                        s.status.set(Some(false));
                        s.update_active();
                    }
                }
            },
        )
        .ok();
    }
    pub fn update_active(&self) {
        let v = if !self.x11 {
            self.backend.accessible_active()
        } else if self.backend.always_on() {
            true
        } else {
            self.status.get().unwrap_or(false)
        };
        self.value.set(v);
    }
}
impl crate::screen_reader_state::ScreenReaderState for LinuxScreenReaderState {
    fn active(&self) -> bool {
        self.value.current()
    }
    fn active_value(&self) -> Producer<bool> {
        self.value.value()
    }
}

impl Drop for LinuxScreenReaderState {
    fn drop(&mut self) {
        self.observer.get_mut().take();
        self.property.get_mut().take();
        self.service.get_mut().take();
    }
}
