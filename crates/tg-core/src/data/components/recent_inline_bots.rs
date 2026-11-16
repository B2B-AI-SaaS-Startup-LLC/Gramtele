use crate::data::data_store_keys::*;
use std::{cell::RefCell, rc::Rc};
use tg_rpl::{EventStream, Producer};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    ReadRecentHashtagsAndBots,
    WriteRecentHashtagsAndBots,
}
pub struct RecentInlineBots {
    scope: SessionScopeId,
    list: RefCell<Vec<UserKey>>,
    updates: EventStream<()>,
    effect: Rc<dyn Fn(Effect)>,
}
impl RecentInlineBots {
    pub fn new(scope: SessionScopeId, effect: Rc<dyn Fn(Effect)>) -> Self {
        Self {
            scope,
            list: RefCell::default(),
            updates: EventStream::new(),
            effect,
        }
    }
    pub fn list(&self) -> Vec<UserKey> {
        (self.effect)(Effect::ReadRecentHashtagsAndBots);
        self.list.borrow().clone()
    }
    pub fn updates(&self) -> Producer<()> {
        self.updates.events()
    }
    pub fn bump(&self, k: UserKey) {
        assert_eq!(k.scope(), self.scope);
        (self.effect)(Effect::ReadRecentHashtagsAndBots);
        {
            let mut l = self.list.borrow_mut();
            if l.first() == Some(&k) {
                return;
            }
            if let Some(i) = l.iter().position(|v| *v == k) {
                l[..=i].rotate_right(1)
            } else {
                if l.len() >= 10 {
                    l.pop();
                }
                l.insert(0, k)
            }
        }
        self.updates.fire(());
        (self.effect)(Effect::WriteRecentHashtagsAndBots)
    }
    pub fn remove(&self, k: UserKey) {
        let removed = {
            let mut l = self.list.borrow_mut();
            if let Some(i) = l.iter().position(|v| *v == k) {
                l.remove(i);
                true
            } else {
                false
            }
        };
        if removed {
            self.updates.fire(());
            (self.effect)(Effect::WriteRecentHashtagsAndBots)
        }
    }
    pub fn apply_local(&self, list: Vec<UserKey>) {
        for k in &list {
            assert_eq!(k.scope(), self.scope)
        }
        *self.list.borrow_mut() = list
    }
}
