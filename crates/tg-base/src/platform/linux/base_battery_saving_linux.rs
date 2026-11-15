use super::{
    base_linux_dbus_utilities as bus,
    base_linux_library::{gio_power_capabilities, process_has_symbol},
    base_linux_xdp_utilities::PropertyWatcher,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use tg_runtime::MainQueue;
use zbus::{blocking::Connection, zvariant::OwnedValue};
#[derive(Clone, Copy, Debug)]
pub struct Capabilities {
    pub power_supply_entries: bool,
    pub dup_default: bool,
    pub get_enabled: bool,
}
impl Capabilities {
    pub fn detect() -> Self {
        let (dup, get) = gio_power_capabilities();
        Self {
            power_supply_entries: std::fs::read_dir("/sys/class/power_supply")
                .is_ok_and(|mut entries| entries.next().is_some()),
            dup_default: dup || process_has_symbol("g_power_profile_monitor_dup_default"),
            get_enabled: get
                || process_has_symbol("g_power_profile_monitor_get_power_saver_enabled"),
        }
    }
}
/// The actual GIO provider selection/sandbox fact is supplied by platform startup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Driver {
    PowerProfiles,
    Portal,
    Dummy,
}
impl Driver {
    fn endpoint(self) -> Option<(&'static str, &'static str, &'static str, &'static str)> {
        match self {
            Self::PowerProfiles => Some((
                "org.freedesktop.UPower.PowerProfiles",
                "/org/freedesktop/UPower/PowerProfiles",
                "org.freedesktop.UPower.PowerProfiles",
                "ActiveProfile",
            )),
            Self::Portal => Some((
                "org.freedesktop.portal.Desktop",
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.PowerProfileMonitor",
                "power-saver-enabled",
            )),
            Self::Dummy => None,
        }
    }
}
pub fn value_from_property(driver: Driver, value: OwnedValue) -> Option<bool> {
    match driver {
        Driver::PowerProfiles => String::try_from(value).ok().map(|v| v == "power-saver"),
        Driver::Portal => bool::try_from(value).ok(),
        Driver::Dummy => None,
    }
}
pub struct LinuxBatterySaving {
    capabilities: Capabilities,
    value: Cell<bool>,
    watchers: RefCell<Vec<PropertyWatcher>>,
    owner: RefCell<Option<bus::ServiceWatcher>>,
    changed: Box<dyn Fn()>,
}
impl LinuxBatterySaving {
    pub fn new(
        capabilities: Capabilities,
        driver: Driver,
        connection: Option<Connection>,
        queue: MainQueue,
        changed: Box<dyn Fn()>,
    ) -> Rc<Self> {
        let state = Rc::new(Self {
            capabilities,
            value: Cell::new(false),
            watchers: RefCell::default(),
            owner: RefCell::default(),
            changed,
        });
        if !capabilities.power_supply_entries || !capabilities.dup_default {
            return state;
        }
        let (Some(connection), Some((service, path, interface, property))) =
            (connection, driver.endpoint())
        else {
            return state;
        };
        let weak = Rc::downgrade(&state);
        if let Ok(watcher) = PropertyWatcher::new(
            &connection,
            queue.clone(),
            service,
            path,
            interface.into(),
            move |mut values, _| {
                if let Some(state) = weak.upgrade() {
                    if let Some(value) = values
                        .remove(property)
                        .and_then(|v| value_from_property(driver, v))
                    {
                        state.set(value, false);
                    }
                }
            },
        ) {
            state.watchers.borrow_mut().push(watcher);
        }
        let weak = Rc::downgrade(&state);
        let conn = connection.clone();
        *state.owner.borrow_mut() =
            bus::ServiceWatcher::new(&connection, service, queue, move |_, _, new| {
                if let Some(state) = weak.upgrade() {
                    if new.is_empty() {
                        state.set(false, true);
                    } else {
                        state.read(&conn, driver);
                    }
                }
            })
            .ok();
        state.read(&connection, driver);
        state
    }
    fn read(&self, connection: &Connection, driver: Driver) {
        if let Some((service, path, interface, property)) = driver.endpoint() {
            if !bus::name_has_owner(connection, service).unwrap_or(false) {
                return;
            }
            let value = bus::proxy(connection, service, path, "org.freedesktop.DBus.Properties")
                .and_then(|p| {
                    p.call_with_flags::<_, _, OwnedValue>(
                        "Get",
                        zbus::proxy::MethodFlags::NoAutoStart.into(),
                        &(interface, property),
                    )
                })
                .ok()
                .flatten()
                .and_then(|v| value_from_property(driver, v));
            if let Some(v) = value {
                self.set(v, false);
            }
        }
    }
    fn set(&self, value: bool, force: bool) {
        let was = self.value.replace(value);
        if was != value || force {
            (self.changed)();
        }
    }
}
impl crate::battery_saving::AbstractBatterySaving for LinuxBatterySaving {
    fn enabled(&self) -> Option<bool> {
        (self.capabilities.power_supply_entries
            && self.capabilities.dup_default
            && self.capabilities.get_enabled)
            .then(|| self.value.get())
    }
}
pub fn create_battery_saving(
    capabilities: Capabilities,
    driver: Driver,
    connection: Option<Connection>,
    queue: MainQueue,
) -> crate::battery_saving::HelperFactory {
    Box::new(move |changed| {
        LinuxBatterySaving::new(capabilities, driver, connection, queue, changed)
    })
}
