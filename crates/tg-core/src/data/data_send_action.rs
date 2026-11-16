//! Manager scheduling only. Painter timing/text/animation bodies belong to18–20/stage5.
use super::{data_msg_id::MsgId, data_store_keys::*};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use tg_rpl::{EventStream, Producer};
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ActionKey {
    pub history: PeerKey,
    pub root_id: MsgId,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimationUpdate {
    pub thread: ActionKey,
    pub rect: [i32; 4],
    pub text_updated: bool,
}
pub struct SendActionManager {
    actions: RefCell<BTreeMap<ActionKey, i64>>,
    now: Rc<dyn Fn() -> i64>,
    animation: EventStream<AnimationUpdate>,
    speaking: EventStream<PeerKey>,
}
impl SendActionManager {
    pub fn new(now: Rc<dyn Fn() -> i64>) -> Self {
        Self {
            actions: RefCell::default(),
            now,
            animation: EventStream::new(),
            speaking: EventStream::new(),
        }
    }
    /// Owner calls the real painter update first, and madeAction(when) before insertion.
    pub fn register_for(
        &self,
        key: ActionKey,
        is_self: bool,
        painter_update: impl FnOnce() -> bool,
        made_action: impl FnOnce(),
    ) -> bool {
        if is_self || !painter_update() {
            return false;
        }
        made_action();
        let mut a = self.actions.borrow_mut();
        if let std::collections::btree_map::Entry::Vacant(e) = a.entry(key) {
            e.insert((self.now)());
            true
        } else {
            false
        }
    }
    pub fn callback(&self, now: i64, mut update: impl FnMut(ActionKey, i64) -> bool) -> bool {
        let keys = self.actions.borrow().keys().copied().collect::<Vec<_>>();
        for k in keys {
            if !update(k, now) {
                self.actions.borrow_mut().remove(&k);
            }
        }
        !self.actions.borrow().is_empty()
    }
    pub fn started(&self, key: ActionKey) -> Option<i64> {
        self.actions.borrow().get(&key).copied()
    }
    pub fn animation_updated(&self) -> Producer<AnimationUpdate> {
        self.animation.events()
    }
    pub fn update_animation(&self, v: AnimationUpdate) {
        self.animation.fire(v)
    }
    pub fn speaking_animation_updated(&self) -> Producer<PeerKey> {
        self.speaking.events()
    }
    pub fn update_speaking_animation(&self, h: PeerKey) {
        self.speaking.fire(h)
    }
    pub fn clear(&self) {
        self.actions.borrow_mut().clear()
    }
}
