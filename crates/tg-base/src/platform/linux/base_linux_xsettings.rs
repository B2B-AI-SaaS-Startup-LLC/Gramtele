use super::base_linux_xcb_utilities::{
    change_window_event_mask, ChangeWindowEventMaskMode, EventHub,
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::{Rc, Weak},
};
use tg_rpl::Lifetime;
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Value {
    #[default]
    Invalid,
    Integer(i32),
    String(Vec<u8>),
    Color {
        valid: bool,
        rgba: [u8; 4],
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Setting {
    pub name: Vec<u8>,
    pub serial: i32,
    pub value: Value,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    Header,
    ByteOrder,
    Truncated,
    NegativeLength,
    Overflow,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parsed {
    pub header_serial: Option<u32>,
    pub records: Vec<Setting>,
    pub error: Option<ParseError>,
}
struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
    little: bool,
}
impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], ParseError> {
        let end = self.offset.checked_add(n).ok_or(ParseError::Overflow)?;
        let data = self
            .bytes
            .get(self.offset..end)
            .ok_or(ParseError::Truncated)?;
        self.offset = end;
        Ok(data)
    }
    fn u16(&mut self) -> Result<u16, ParseError> {
        let little = self.little;
        let b = self.take(2)?.try_into().unwrap();
        Ok(if little {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })
    }
    fn u32(&mut self) -> Result<u32, ParseError> {
        let little = self.little;
        let b = self.take(4)?.try_into().unwrap();
        Ok(if little {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    }
    fn padded(&mut self, n: usize) -> Result<Vec<u8>, ParseError> {
        let value = self.take(n)?.to_vec();
        self.offset = self
            .offset
            .checked_add((4 - n % 4) % 4)
            .ok_or(ParseError::Overflow)?;
        Ok(value)
    }
    fn setting(&mut self) -> Result<Setting, ParseError> {
        let type_ = self.take(2)?[0];
        let len = self.u16()? as usize;
        let name = self.padded(len)?;
        let serial = self.u32()? as i32;
        let value = match type_ {
            0 => Value::Integer(self.u32()? as i32),
            1 => {
                let n = self.u32()? as i32;
                if n < 0 {
                    return Err(ParseError::NegativeLength);
                }
                Value::String(self.padded(n as usize)?)
            }
            2 => {
                let rgba = [self.u16()?, self.u16()?, self.u16()?, self.u16()?];
                let valid = rgba.iter().all(|v| *v <= 255);
                Value::Color {
                    valid,
                    rgba: if valid {
                        rgba.map(|v| v as u8)
                    } else {
                        [0, 0, 0, 0]
                    },
                }
            }
            _ => Value::Invalid,
        };
        Ok(Setting {
            name,
            serial,
            value,
        })
    }
}
/// Header serial is ignored by C++; per-property serial is a signed32-bit value.
pub fn parse(bytes: &[u8]) -> Parsed {
    let mut result = Parsed {
        header_serial: None,
        records: vec![],
        error: None,
    };
    if bytes.len() < 12 {
        result.error = Some(ParseError::Header);
        return result;
    }
    if bytes[0] > 1 {
        result.error = Some(ParseError::ByteOrder);
        return result;
    }
    let mut r = Reader {
        bytes,
        offset: 4,
        little: bytes[0] == 0,
    };
    result.header_serial = Some(r.u32().unwrap());
    let count = r.u32().unwrap();
    for _ in 0..count {
        match r.setting() {
            Ok(value) => result.records.push(value),
            Err(e) => {
                result.error = Some(e);
                break;
            }
        }
    }
    result
}
type Callback = Rc<dyn Fn(&super::base_linux_xcb_library::Connection, &[u8], &Value)>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropertyValue {
    pub serial: i32,
    pub value: Value,
}
impl Default for PropertyValue {
    fn default() -> Self {
        Self {
            serial: -1,
            value: Value::Invalid,
        }
    }
}
impl PropertyValue {
    pub fn update(&mut self, record: &Setting) -> bool {
        if record.serial <= self.serial {
            return false;
        }
        self.serial = record.serial;
        self.value = record.value.clone();
        true
    }
}
#[derive(Default)]
struct Entry {
    value: PropertyValue,
    callbacks: BTreeMap<u64, Callback>,
}
pub struct XSettings {
    hub: Rc<EventHub>,
    window: u32,
    values: RefCell<BTreeMap<Vec<u8>, Entry>>,
    next: Cell<u64>,
    initialized: Cell<bool>,
    guards: RefCell<Vec<Lifetime>>,
}
thread_local! {static INSTANCE:RefCell<Weak<XSettings>>=const{RefCell::new(Weak::new())};}
impl XSettings {
    pub fn instance(hub: Rc<EventHub>) -> Rc<Self> {
        INSTANCE.with(|slot| {
            if let Some(value) = slot.borrow().upgrade() {
                return value;
            }
            let value = Self::new(hub);
            *slot.borrow_mut() = Rc::downgrade(&value);
            value
        })
    }
    pub fn new(hub: Rc<EventHub>) -> Rc<Self> {
        let c = hub.connection();
        let window = c
            .atom("_XSETTINGS_S0")
            .and_then(|a| c.selection_owner(a))
            .unwrap_or(0);
        let this = Rc::new(Self {
            hub,
            window,
            values: RefCell::default(),
            next: Cell::new(0),
            initialized: Cell::new(false),
            guards: RefCell::default(),
        });
        if window == 0 {
            return this;
        }
        let weak = Rc::downgrade(&this);
        let Some(event) = this.hub.register(move |event| {
            let Some(this) = weak.upgrade() else {
                return;
            };
            if event.is_some_and(|e| e.kind() == 28 && e.u32(4) == this.window) {
                this.populate(&this.get_settings());
            }
        }) else {
            return this;
        };
        this.guards.borrow_mut().push(event);
        let Some(mask) = change_window_event_mask(
            c,
            window,
            (1 << 17) | (1 << 22),
            ChangeWindowEventMaskMode::Add,
            true,
        ) else {
            return this;
        };
        this.guards.borrow_mut().push(mask);
        this.populate(&this.get_settings());
        this.initialized.set(true);
        this
    }
    pub fn initialized(&self) -> bool {
        self.initialized.get()
    }
    pub fn setting(&self, name: &[u8]) -> Value {
        self.values
            .borrow_mut()
            .entry(name.to_vec())
            .or_default()
            .value
            .value
            .clone()
    }
    pub fn get_settings(&self) -> Vec<u8> {
        let c = self.hub.connection();
        let Some(atom) = c.atom("_XSETTINGS_SETTINGS") else {
            return vec![];
        };
        let mut data = vec![];
        while let Some(p) = c.property(self.window, atom, atom, (data.len() / 4) as u32, 8192) {
            data.extend(p.data);
            if p.bytes_after == 0 {
                break;
            }
        }
        data
    }
    pub fn populate(&self, bytes: &[u8]) {
        for record in parse(bytes).records {
            let callbacks = {
                let mut values = self.values.borrow_mut();
                let entry = values.entry(record.name.clone()).or_default();
                if !entry.value.update(&record) {
                    continue;
                }
                entry.callbacks.values().cloned().collect::<Vec<_>>()
            };
            for callback in callbacks {
                callback(&self.hub.connection(), &record.name, &record.value);
            }
        }
    }
    pub fn register_callback_for_property(
        self: &Rc<Self>,
        name: Vec<u8>,
        callback: impl Fn(&super::base_linux_xcb_library::Connection, &[u8], &Value) + 'static,
    ) -> Lifetime {
        let id = self.next.get();
        self.next.set(id + 1);
        self.values
            .borrow_mut()
            .entry(name.clone())
            .or_default()
            .callbacks
            .insert(id, Rc::new(callback));
        let this = self.clone();
        let life = Lifetime::new();
        life.add(move || {
            this.values
                .borrow_mut()
                .entry(name)
                .or_default()
                .callbacks
                .remove(&id);
        });
        life
    }
}
