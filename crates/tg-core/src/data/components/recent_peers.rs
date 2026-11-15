//! Recent peers + weak thread history by real keys. Peer persistence is the
//! existing Serialize::writePeer/readPeer contract, never a new peer encoding.
use super::top_peers::PeerCodec;
use crate::data::data_store_keys::*;
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use tg_base::qt_stream::{Reader, Writer};
use tg_rpl::{EventStream, Producer};
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ThreadKey {
    History(PeerKey),
    Topic(TopicKey),
    Sublist(SublistKey),
}
pub trait RecentPeersOwner {
    fn read_search_suggestions(&self);
    fn write_search_suggestions_delayed(&self);
    /// Token owned by the actual Thread instance18/08/16. Recreating a key must
    /// issue a new token, so an expired weak reference never revives.
    fn thread_lifetime(&self, thread: ThreadKey) -> Weak<()>;
}
pub struct RecentPeers {
    scope: SessionScopeId,
    alive: Weak<crate::data::data_store_keys::ScopeState>,
    owner: Rc<dyn RecentPeersOwner>,
    list: RefCell<Vec<PeerKey>>,
    opens: RefCell<Vec<(ThreadKey, Weak<()>)>>,
    updates: EventStream<()>,
}
impl RecentPeers {
    pub fn new(scope: &SessionScope, owner: Rc<dyn RecentPeersOwner>) -> Self {
        Self {
            scope: scope.id(),
            alive: scope.downgrade(),
            owner,
            list: RefCell::default(),
            opens: RefCell::default(),
            updates: EventStream::new(),
        }
    }
    fn check(&self) {
        assert!(self.alive.upgrade().is_some(), "SessionScope ended");
    }
    pub fn list(&self) -> Vec<PeerKey> {
        self.check();
        self.owner.read_search_suggestions();
        self.list.borrow().clone()
    }
    pub fn updates(&self) -> Producer<()> {
        self.updates.events()
    }
    pub fn remove(&self, p: PeerKey) {
        self.check();
        assert_eq!(p.scope(), self.scope);
        let removed = {
            let mut v = self.list.borrow_mut();
            if let Some(i) = v.iter().position(|x| *x == p) {
                v.remove(i);
                true
            } else {
                false
            }
        };
        if removed {
            self.updates.fire(());
        }
        self.owner.write_search_suggestions_delayed();
    }
    pub fn bump(&self, p: PeerKey) {
        self.check();
        assert_eq!(p.scope(), self.scope);
        self.owner.read_search_suggestions();
        {
            let mut v = self.list.borrow_mut();
            if v.first() == Some(&p) {
                return;
            }
            let i = if let Some(i) = v.iter().position(|x| *x == p) {
                i
            } else {
                v.push(p);
                v.len() - 1
            };
            v[..=i].rotate_right(1);
        }
        self.updates.fire(());
        self.owner.write_search_suggestions_delayed();
    }
    pub fn clear(&self) {
        self.check();
        self.owner.read_search_suggestions();
        self.list.borrow_mut().clear();
        self.updates.fire(());
        self.owner.write_search_suggestions_delayed();
    }
    pub fn serialize(&self, app_version: u32, codec: &impl PeerCodec) -> Vec<u8> {
        let list = self.list();
        if list.is_empty() {
            return vec![];
        }
        let mut w = Writer::default();
        w.i32(app_version as i32);
        w.i32(list.len().min(48) as i32);
        for p in list.into_iter().take(48) {
            codec.write_peer(p, &mut w);
        }
        w.0
    }
    pub fn apply_local(&self, bytes: &[u8], codec: &impl PeerCodec) {
        self.check();
        self.list.borrow_mut().clear();
        let mut r = Reader::new(bytes);
        let (Some(version), Some(count)) = (r.i32(), r.i32()) else {
            return;
        };
        for _ in 0..count as u32 {
            let p = codec.read_peer(version as u32, &mut r);
            if let Some(p) = p {
                assert_eq!(p.scope(), self.scope);
                self.list.borrow_mut().push(p);
            } else {
                self.list.borrow_mut().clear();
                return;
            }
        }
    }
    pub fn collect_chat_open_history(&self) -> Vec<ThreadKey> {
        self.check();
        self.owner.read_search_suggestions();
        let list = self.opens.borrow().clone();
        list.into_iter()
            .filter(|(_, weak)| weak.upgrade().is_some())
            .map(|(key, _)| key)
            .collect()
    }
    pub fn chat_open_push(&self, t: ThreadKey) {
        self.check();
        let scope = match t {
            ThreadKey::History(p) => p.scope(),
            ThreadKey::Topic(t) => t.history.scope(),
            ThreadKey::Sublist(k) => k.scope(),
        };
        assert_eq!(scope, self.scope);
        let old = self.opens.borrow().clone();
        let mut v = old
            .into_iter()
            .filter(|(_, weak)| weak.upgrade().is_some())
            .collect::<Vec<_>>();
        if let Some(i) = v.iter().position(|(p, _)| *p == t) {
            v[..=i].rotate_right(1);
        } else {
            v.truncate(31);
            v.insert(0, (t, self.owner.thread_lifetime(t)));
        }
        *self.opens.borrow_mut() = v;
    }
    pub fn chat_open_remove(&self, t: ThreadKey) {
        self.check();
        self.opens.borrow_mut().retain(|(p, _)| *p != t);
    }
}
