pub use super::base_linux_xcb_library::Connection as CustomConnection;
use super::base_linux_xcb_library::{Connection, Event, Property};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::{Rc, Weak},
};
use tg_rpl::Lifetime;
pub type SharedConnection = Rc<Connection>;
thread_local! {static SHARED:RefCell<Weak<Connection>>=const{RefCell::new(Weak::new())};}
pub fn shared_connection() -> Option<SharedConnection> {
    SHARED.with(|slot| {
        if let Some(c) = slot.borrow().upgrade() {
            return Some(c);
        }
        let c = Rc::new(Connection::connect(None)?);
        *slot.borrow_mut() = Rc::downgrade(&c);
        Some(c)
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeWindowEventMaskMode {
    Add,
    Remove,
    Replace,
}
/// Source deliberately treats any overlapping Add bit as a complete no-op.
pub fn event_mask_change(old: u32, mask: u32, mode: ChangeWindowEventMaskMode) -> Option<u32> {
    use ChangeWindowEventMaskMode::*;
    if (mode == Add && old & mask != 0) || (mode == Remove && old & mask == 0) || old == mask {
        return None;
    }
    Some(match mode {
        Add => old | mask,
        Remove => old & !mask,
        Replace => mask,
    })
}
pub fn change_window_event_mask(
    connection: Rc<Connection>,
    window: u32,
    mask: u32,
    mode: ChangeWindowEventMaskMode,
    revert: bool,
) -> Option<Lifetime> {
    if connection.has_error() {
        return None;
    }
    let old = connection.window_attributes(window).map_or(0, |v| v.0);
    let lifetime = Lifetime::new();
    let Some(new) = event_mask_change(old, mask, mode) else {
        return Some(Lifetime::from_fn(|| {}));
    };
    if !connection.change_event_mask(window, new) {
        return None;
    }
    if !revert {
        return Some(Lifetime::from_fn(|| {}));
    }
    if revert {
        lifetime.add(move || {
            if !connection.has_error() {
                connection.change_event_mask(window, old);
            }
        });
    }
    Some(lifetime)
}
pub fn property_u32s(property: &Property, type_: u32) -> Option<Vec<u32>> {
    (property.format == 32 && property.type_ == type_).then(|| {
        property
            .data
            .chunks_exact(4)
            .map(|b| u32::from_ne_bytes(b.try_into().unwrap()))
            .collect()
    })
}
pub fn get_wm_supported(connection: &Connection, root: u32) -> Vec<u32> {
    let Some(atom) = connection.atom("_NET_SUPPORTED") else {
        return vec![];
    };
    let mut result = vec![];
    let mut offset = 0;
    while let Some(p) = connection.property(root, atom, 4, offset, 1024) {
        let Some(values) = property_u32s(&p, 4) else {
            break;
        };
        offset += values.len() as u32;
        result.extend(values);
        if p.bytes_after == 0 {
            break;
        }
    }
    result
}
pub fn get_supporting_wm_check(connection: &Connection, root: u32) -> Option<u32> {
    let atom = connection.atom("_NET_SUPPORTING_WM_CHECK")?;
    property_u32s(&connection.property(root, atom, 33, 0, 1024)?, 33)?
        .first()
        .copied()
        .filter(|n| *n != 0)
}
pub fn is_supported_by_wm(connection: &Connection, name: &str) -> bool {
    let (Some(root), Some(atom)) = (connection.root_window(), connection.atom(name)) else {
        return false;
    };
    get_wm_supported(connection, root).contains(&atom)
}
pub fn window_manager(connection: &Connection) -> String {
    let Some(root) = connection.root_window() else {
        return String::new();
    };
    let (Some(name), Some(utf8), Some(window)) = (
        connection.atom("_NET_WM_NAME"),
        connection.atom("UTF8_STRING"),
        get_supporting_wm_check(connection, root),
    ) else {
        return String::new();
    };
    connection
        .property(window, name, utf8, 0, 1024)
        .filter(|p| p.format == 8 && p.type_ == utf8)
        .map(|p| {
            String::from_utf8_lossy(&p.data)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
}
type Handler = Rc<dyn Fn(Option<&Event>)>;
/// Actual XCB polling/dispatch; Qt socket/native-event adapter is a stage5 owner.
pub struct EventHub {
    connection: Rc<Connection>,
    handlers: RefCell<BTreeMap<u64, Handler>>,
    next: Cell<u64>,
    failed: Cell<bool>,
}
impl EventHub {
    pub fn new(connection: Rc<Connection>) -> Rc<Self> {
        Rc::new(Self {
            connection,
            handlers: RefCell::default(),
            next: Cell::new(0),
            failed: Cell::new(false),
        })
    }
    pub fn connection(&self) -> Rc<Connection> {
        self.connection.clone()
    }
    pub fn register(
        self: &Rc<Self>,
        callback: impl Fn(Option<&Event>) + 'static,
    ) -> Option<Lifetime> {
        if self.connection.has_error() {
            return None;
        }
        let id = self.next.get();
        self.next.set(id + 1);
        self.handlers.borrow_mut().insert(id, Rc::new(callback));
        let me = self.clone();
        let life = Lifetime::new();
        life.add(move || {
            me.handlers.borrow_mut().remove(&id);
        });
        Some(life)
    }
    fn dispatch(&self, event: Option<&Event>) {
        let end = self.next.get();
        let ids = self
            .handlers
            .borrow()
            .range(..end)
            .map(|(&id, _)| id)
            .collect::<Vec<_>>();
        for id in ids {
            let callback = self.handlers.borrow().get(&id).cloned();
            if let Some(f) = callback {
                f(event);
            }
        }
    }
    pub fn poll(&self) -> usize {
        if self.failed.get() {
            return 0;
        }
        let mut count = 0;
        while !self.connection.has_error() {
            let Some(e) = self.connection.poll_event() else {
                break;
            };
            self.dispatch(Some(&e));
            count += 1;
        }
        if self.connection.has_error() && !self.failed.replace(true) {
            self.dispatch(None);
        }
        count
    }
    /// Core timestamp protocol; the nested Qt dispatcher integration remains5.
    pub fn timestamp(self: &Rc<Self>) -> Option<u32> {
        let c = &self.connection;
        let window = c.root_window()?;
        let atom = c.atom("_DESKTOP_APP_GET_TIMESTAMP")?;
        let _mask = change_window_event_mask(
            c.clone(),
            window,
            1 << 22,
            ChangeWindowEventMaskMode::Add,
            true,
        )?;
        let result = Rc::new(Cell::new(None));
        let target = result.clone();
        let conn = c.clone();
        let _handler = self.register(move |event| {
            let Some(e) = event else {
                target.set(Some(0));
                return;
            };
            if e.kind() == 28 && e.u32(4) == window && e.u32(8) == atom {
                target.set(Some(e.u32(12)));
            }
            conn.sync();
        })?;
        if !c.change_property(window, atom, 19, 32, &[]) {
            return None;
        }
        loop {
            self.poll();
            if let Some(time) = result.get() {
                return (time != 0).then_some(time);
            }
            if c.has_error() {
                return None;
            }
            c.wait_readable(-1);
        }
    }
}

/// ObjectWithConnection: destroy the native object before its retained connection.
pub struct ObjectWithConnection<T> {
    object: Option<T>,
    connection: Rc<Connection>,
}
impl<T> ObjectWithConnection<T> {
    pub fn new(
        connection: Rc<Connection>,
        constructor: impl FnOnce(&Connection) -> Option<T>,
    ) -> Self {
        let object = if connection.has_error() {
            None
        } else {
            constructor(&connection)
        };
        Self { object, connection }
    }
    pub fn get(&self) -> Option<&T> {
        self.object.as_ref()
    }
    pub fn connection(&self) -> Rc<Connection> {
        self.connection.clone()
    }
}
impl<T> Drop for ObjectWithConnection<T> {
    fn drop(&mut self) {
        self.object.take();
    }
}
