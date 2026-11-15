use super::base_linux_dbus_utilities as bus;
use std::collections::HashMap;
use tg_runtime::MainQueue;
use zbus::{blocking::Connection, zvariant::OwnedValue, Message};
pub const SERVICE: &str = "org.freedesktop.portal.Desktop";
pub const OBJECT_PATH: &str = "/org/freedesktop/portal/desktop";
pub const REQUEST_INTERFACE: &str = "org.freedesktop.portal.Request";
pub const SETTINGS_INTERFACE: &str = "org.freedesktop.portal.Settings";
/// Provided by the real QWindow/native-services layer5; empty Some is significant.
pub trait ParentWindow {
    fn portal_identifier(&self) -> Option<String>;
    fn is_x11(&self) -> bool;
    fn native_window_id(&self) -> u64;
}
pub fn parent_window_id(window: Option<&dyn ParentWindow>) -> String {
    let Some(window) = window else {
        return String::new();
    };
    if let Some(id) = window.portal_identifier() {
        return id;
    }
    if window.is_x11() {
        format!("x11:{:x}", window.native_window_id())
    } else {
        String::new()
    }
}
pub fn read_setting(connection: &Connection, group: &str, key: &str) -> bus::Result<OwnedValue> {
    bus::proxy(connection, SERVICE, OBJECT_PATH, SETTINGS_INTERFACE)?.call("ReadOne", &(group, key))
}
pub fn read_setting_typed<T>(connection: &Connection, group: &str, key: &str) -> bus::Result<T>
where
    T: TryFrom<OwnedValue>,
    T::Error: Into<zbus::Error>,
{
    T::try_from(read_setting(connection, group, key)?).map_err(Into::into)
}
pub fn setting_changed(message: &Message) -> bus::Result<(String, String, OwnedValue)> {
    message.body().deserialize()
}
pub struct SettingWatcher {
    _signal: bus::SignalSubscription,
}
impl SettingWatcher {
    pub fn new(
        connection: &Connection,
        queue: MainQueue,
        callback: impl Fn(&str, &str, OwnedValue) + 'static,
    ) -> bus::Result<Self> {
        let rule = bus::signal_rule(SERVICE, OBJECT_PATH, SETTINGS_INTERFACE, "SettingChanged")?;
        Ok(Self {
            _signal: bus::SignalSubscription::new(connection, rule, queue, move |message| {
                if let Ok((group, key, value)) = setting_changed(&message) {
                    callback(&group, &key, value);
                }
            })?,
        })
    }
    pub fn typed<T: TryFrom<OwnedValue> + 'static>(
        connection: &Connection,
        queue: MainQueue,
        callback: impl Fn(&str, &str, T) + 'static,
    ) -> bus::Result<Self> {
        Self::new(connection, queue, move |g, k, v| {
            if let Ok(value) = T::try_from(v) {
                callback(g, k, value);
            }
        })
    }
    pub fn notify(
        connection: &Connection,
        queue: MainQueue,
        callback: impl Fn(&str, &str) + 'static,
    ) -> bus::Result<Self> {
        Self::new(connection, queue, move |g, k, _| callback(g, k))
    }
    pub fn for_key(
        connection: &Connection,
        queue: MainQueue,
        group: String,
        key: String,
        callback: impl Fn(OwnedValue) + 'static,
    ) -> bus::Result<Self> {
        Self::new(connection, queue, move |g, k, value| {
            if g == group && k == key {
                callback(value);
            }
        })
    }
    pub fn for_key_typed<T: TryFrom<OwnedValue> + 'static>(
        connection: &Connection,
        queue: MainQueue,
        group: String,
        key: String,
        callback: impl Fn(T) + 'static,
    ) -> bus::Result<Self> {
        Self::for_key(connection, queue, group, key, move |value| {
            if let Ok(value) = T::try_from(value) {
                callback(value);
            }
        })
    }
    pub fn for_key_notify(
        connection: &Connection,
        queue: MainQueue,
        group: String,
        key: String,
        callback: impl Fn() + 'static,
    ) -> bus::Result<Self> {
        Self::for_key(connection, queue, group, key, move |_| callback())
    }
}
pub struct PropertyWatcher {
    _signal: bus::SignalSubscription,
}
impl PropertyWatcher {
    pub fn new(
        connection: &Connection,
        queue: MainQueue,
        service: &str,
        path: &str,
        interface: String,
        callback: impl Fn(HashMap<String, OwnedValue>, Vec<String>) + 'static,
    ) -> bus::Result<Self> {
        let rule = bus::signal_rule(
            service,
            path,
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
        )?;
        Ok(Self {
            _signal: bus::SignalSubscription::new(connection, rule, queue, move |message| {
                if let Ok((iface, changed, invalidated)) =
                    message
                        .body()
                        .deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
                {
                    if iface == interface {
                        callback(changed, invalidated);
                    }
                }
            })?,
        })
    }
}
