//! Application-lifetime registry replacing global raw pointer registration.
//! Handles use safe shared slots, never expose a borrow across clear callbacks.
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::{Rc, Weak},
};
trait Clear {
    fn clear(&self);
}
struct Slot<T> {
    value: RefCell<Option<T>>,
    deleting: Cell<bool>,
}
impl<T> Clear for Slot<T> {
    fn clear(&self) {
        let value = self.value.borrow_mut().take();
        // C++ nulls its global pointer after delete returns. Keep its bool
        // observable during T::drop without exposing a half-destroyed &T.
        self.deleting.set(value.is_some());
        drop(value);
        self.deleting.set(false);
    }
}
#[derive(Default)]
pub struct GlobalStructures {
    slots: RefCell<BTreeMap<usize, Rc<dyn Clear>>>,
}
impl GlobalStructures {
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }
    pub fn clear_global_structures(&self) {
        let slots = std::mem::take(&mut *self.slots.borrow_mut());
        for slot in slots.into_values() {
            slot.clear();
        }
    }
}
impl Drop for GlobalStructures {
    fn drop(&mut self) {
        self.clear_global_structures();
    }
}
pub struct GlobalStructurePointer<T> {
    registry: Weak<GlobalStructures>,
    slot: Rc<Slot<T>>,
}
impl<T: 'static> GlobalStructurePointer<T> {
    pub fn new(registry: &Rc<GlobalStructures>) -> Self {
        Self {
            registry: Rc::downgrade(registry),
            slot: Rc::new(Slot {
                value: RefCell::new(None),
                deleting: Cell::new(false),
            }),
        }
    }
    pub fn create_if_null(&self, create: impl FnOnce() -> T) {
        let registry = self.registry.upgrade().expect("application registry ended");
        if self.is_present() {
            return;
        }
        let value = create();
        *self.slot.value.borrow_mut() = Some(value);
        registry
            .slots
            .borrow_mut()
            .insert(Rc::as_ptr(&self.slot) as usize, self.slot.clone());
    }
    pub fn is_present(&self) -> bool {
        self.slot.deleting.get() || self.slot.value.borrow().is_some()
    }
    pub fn with<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        read(
            self.slot
                .value
                .borrow()
                .as_ref()
                .expect("structure not created"),
        )
    }
    pub fn with_mut<R>(&self, write: impl FnOnce(&mut T) -> R) -> R {
        write(
            self.slot
                .value
                .borrow_mut()
                .as_mut()
                .expect("structure not created"),
        )
    }
}
