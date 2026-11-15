use super::{
    base_linux_shortcut_keys::KEY_TO_CODE,
    base_linux_xcb_library::{Connection, Event, Record},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
pub const SHIFT_MOUSE_BUTTON: u64 = u64::MAX - 100;
pub fn is_keypad(keysym: u32) -> bool {
    (0xff80..=0xffbd).contains(&keysym) || (0x11000000..=0x1100ffff).contains(&keysym)
}
pub fn skip_mouse_button(button: u8) -> bool {
    button == 1 || (button > 3 && button < 8)
}
pub fn compute_keysym(column0: u32, column1: u32, _state: u16) -> u32 {
    if is_keypad(column1) || (0x41..=0x5a).contains(&column1) {
        column1
    } else {
        column0
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyName {
    Text(String),
    Sequence { code: u32, keypad: bool },
}
pub fn key_name(descriptor: u64) -> KeyName {
    if descriptor > SHIFT_MOUSE_BUTTON {
        let button = descriptor - SHIFT_MOUSE_BUTTON;
        if matches!(button, 2 | 3 | 8..=31) {
            return KeyName::Text(format!("Mouse {button}"));
        }
    }
    let modifier = match descriptor {
        0xffe1 => Some("Shift"),
        0xffe2 => Some("Right Shift"),
        0xffe3 => Some("Ctrl"),
        0xffe4 => Some("Right Ctrl"),
        0xffe7 => Some("Meta"),
        0xffe8 => Some("Right Meta"),
        0xffe9 => Some("Alt"),
        0xffea => Some("Right Alt"),
        0xffeb => Some("Super"),
        0xffec => Some("Right Super"),
        _ => None,
    };
    if let Some(text) = modifier {
        return KeyName::Text(text.into());
    }
    if (0x20..=0x60).contains(&descriptor) {
        return KeyName::Sequence {
            code: descriptor as u32,
            keypad: false,
        };
    }
    match KEY_TO_CODE.binary_search_by_key(&descriptor, |v| v.0) {
        Ok(i) => KeyName::Sequence {
            code: KEY_TO_CODE[i].1,
            keypad: is_keypad(descriptor as u32),
        },
        Err(_) => KeyName::Text(format!("\\x{descriptor:x}")),
    }
}
/// Original always returns false; inspecting a QKeyEvent is unnecessary.
pub const fn is_toggle_full_screen_key() -> bool {
    false
}
/// Keep reply boundaries: a skipped mouse button drops the rest of that reply.
pub fn process_batch(events: &[Event], mut keysym: impl FnMut(u8, u16) -> u32) -> Vec<(u64, bool)> {
    let mut result = vec![];
    for event in events {
        let kind = event.0[0];
        let button = kind == 4 || kind == 5;
        if !button && kind != 2 && kind != 3 {
            continue;
        }
        let code = event.0[1];
        if button && skip_mouse_button(code) {
            return result;
        }
        result.push((
            if button {
                SHIFT_MOUSE_BUTTON.wrapping_add(code as u64)
            } else {
                keysym(code, event.u16(28)) as u64
            },
            kind == 2 || kind == 4,
        ));
    }
    result
}
type Callback = Rc<dyn Fn(u64, bool)>;
pub struct GlobalShortcuts {
    display: Option<String>,
    attempted: Cell<bool>,
    record: RefCell<Option<Record>>,
    callback: RefCell<Option<Callback>>,
}
impl GlobalShortcuts {
    pub fn new(display: Option<String>) -> Self {
        Self {
            display,
            attempted: Cell::new(false),
            record: RefCell::default(),
            callback: RefCell::default(),
        }
    }
    fn ensure(&self) {
        if self.attempted.replace(true) {
            return;
        }
        *self.record.borrow_mut() = Connection::connect(self.display.as_deref())
            .map(Rc::new)
            .and_then(Record::new);
    }
    pub fn available(&self, is_x11: bool) -> bool {
        if !is_x11 {
            return false;
        }
        self.ensure();
        self.record.borrow().as_ref().is_some_and(Record::available)
    }
    pub fn allowed(&self, is_x11: bool) -> bool {
        self.available(is_x11)
    }
    pub fn start(&self, callback: impl Fn(u64, bool) + 'static) {
        *self.callback.borrow_mut() = Some(Rc::new(callback));
        self.ensure();
    }
    pub fn stop(&self) {
        self.callback.borrow_mut().take();
        self.record.borrow_mut().take();
        self.attempted.set(false);
    }
    pub fn file_descriptor(&self) -> Option<i32> {
        self.record
            .borrow()
            .as_ref()?
            .connection()
            .file_descriptor()
    }
    /// Host socket notifier invokes poll; actual Qt notifier/native filter stays5.
    pub fn poll(&self) {
        let batches = {
            let record = self.record.borrow();
            let Some(record) = record.as_ref() else {
                return;
            };
            record
                .poll()
                .iter()
                .map(|events| {
                    process_batch(events, |code, state| {
                        compute_keysym(
                            record.keysym(code, 0).unwrap_or(0),
                            record.keysym(code, 1).unwrap_or(0),
                            state,
                        )
                    })
                })
                .collect::<Vec<_>>()
        };
        for batch in batches {
            for (code, down) in batch {
                let callback = self.callback.borrow().clone();
                if let Some(callback) = callback {
                    callback(code, down);
                }
            }
        }
    }
}

thread_local! { static MANAGER:RefCell<Option<Rc<GlobalShortcuts>>>=const{RefCell::new(None)}; }
fn ensure_x11_shortcut_manager() -> Rc<GlobalShortcuts> {
    MANAGER.with(|slot| {
        let mut slot = slot.borrow_mut();
        slot.get_or_insert_with(|| Rc::new(GlobalShortcuts::new(None)))
            .clone()
    })
}
/// The real QGui/native IsX11 fact is supplied by the stage5 platform entrypoint.
pub fn available(is_x11: bool) -> bool {
    is_x11 && ensure_x11_shortcut_manager().available(true)
}
pub fn allowed(is_x11: bool) -> bool {
    available(is_x11)
}
pub fn start(callback: impl Fn(u64, bool) + 'static) {
    ensure_x11_shortcut_manager().start(callback);
}
pub fn stop() {
    let manager = MANAGER.with(|slot| slot.borrow_mut().take());
    if let Some(manager) = manager {
        manager.stop();
    }
}
pub fn poll() {
    let manager = MANAGER.with(|slot| slot.borrow().clone());
    if let Some(manager) = manager {
        manager.poll();
    }
}
