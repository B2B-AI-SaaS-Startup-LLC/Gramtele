//! Saved gifts' metadata cache. Text/entities and Api::FromTL remain01/q1/24;
//! owner supplies actual parsed gift records, not substitute media/history models.
use crate::data::{data_star_gift::*, data_store_keys::*};
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    rc::Rc,
};
use tg_scheme as tl;
pub type Callback = Rc<dyn Fn(Vec<SavedStarGift>)>;
#[derive(Clone)]
pub enum Effect {
    Request {
        id: i32,
        peer: PeerKey,
        request: tl::PaymentsGetSavedStarGifts,
    },
    UpdatePinned {
        id: i32,
        peer: PeerKey,
        request: tl::PaymentsToggleStarGiftsPinnedToTop,
    },
    FetchGift {
        id: i32,
        peer: PeerKey,
        request: tl::PaymentsGetSavedStarGift,
    },
    GiftUpdate {
        peer: PeerKey,
        id: SavedStarGiftId,
        pinned: bool,
    },
    GiftError(String),
}
#[derive(Default)]
struct Entry {
    gifts: VecDeque<SavedStarGift>,
    last: i64,
    request: i32,
    // C++ captures the first completion in the request, separately from pendingCallbacks.
    first: Option<(bool, Callback)>,
    pending: Vec<(bool, Callback)>,
}
enum Transaction {
    Fetch {
        peer: PeerKey,
        input: tl::InputPeer,
        gifts: Vec<SavedStarGift>,
        ids: Vec<SavedStarGiftId>,
        manage: SavedStarGiftId,
    },
    Order {
        peer: PeerKey,
        gifts: Vec<SavedStarGift>,
        ids: Vec<SavedStarGiftId>,
        notify: Option<(SavedStarGiftId, bool)>,
    },
}
pub struct RecentSharedMediaGifts {
    transactions: RefCell<BTreeMap<i32, Transaction>>,
    scope: SessionScopeId,
    recent: RefCell<BTreeMap<PeerKey, Entry>>,
    now: Rc<dyn Fn() -> i64>,
    effect: Rc<dyn Fn(Effect)>,
}
impl RecentSharedMediaGifts {
    pub fn new(
        scope: SessionScopeId,
        now: Rc<dyn Fn() -> i64>,
        effect: Rc<dyn Fn(Effect)>,
    ) -> Rc<Self> {
        Rc::new(Self {
            transactions: RefCell::default(),
            scope,
            recent: RefCell::default(),
            now,
            effect,
        })
    }
    pub fn filter_gifts(g: &VecDeque<SavedStarGift>, only_pinned: bool) -> Vec<SavedStarGift> {
        g.iter()
            .filter(|g| !only_pinned || g.pinned)
            .take(if only_pinned { 6 } else { 3 })
            .cloned()
            .collect()
    }
    pub fn request(&self, p: PeerKey, input: tl::InputPeer, done: Callback, only_pinned: bool) {
        assert_eq!(p.scope(), self.scope);
        let now = (self.now)();
        let cached = {
            let mut r = self.recent.borrow_mut();
            if let Some(e) = r.get_mut(&p) {
                if e.last != 0 && e.last + 60000 > now {
                    Some(Self::filter_gifts(&e.gifts, only_pinned))
                } else if e.request != 0 {
                    e.pending.push((only_pinned, done.clone()));
                    return;
                } else {
                    None
                }
            } else {
                None
            }
        };
        if let Some(v) = cached {
            done(v);
            return;
        }
        let id = tg_mtproto::mtp_instance::get_next_request_id();
        {
            let mut r = self.recent.borrow_mut();
            let e = r.entry(p).or_default();
            e.request = id;
            e.first = Some((only_pinned, done));
        }
        (self.effect)(Effect::Request {
            id,
            peer: p,
            request: tl::PaymentsGetSavedStarGifts::new(Default::default(), input, None, vec![], 6),
        });
    }
    pub fn received(&self, p: PeerKey, gifts: Vec<SavedStarGift>) {
        let now = (self.now)();
        let first = {
            let mut r = self.recent.borrow_mut();
            let e = r.entry(p).or_default();
            e.last = now;
            e.request = 0;
            e.gifts = gifts.into();
            e.first.take()
        };
        if let Some((pin, done)) = first {
            let result = {
                let r = self.recent.borrow();
                Self::filter_gifts(&r.get(&p).unwrap().gifts, pin)
            };
            done(result);
        }
        loop {
            let next = {
                let mut r = self.recent.borrow_mut();
                let e = r.get_mut(&p).unwrap();
                if e.pending.is_empty() {
                    None
                } else {
                    Some(e.pending.remove(0))
                }
            };
            let Some((pin, cb)) = next else { break };
            let result = {
                let r = self.recent.borrow();
                Self::filter_gifts(&r.get(&p).unwrap().gifts, pin)
            };
            cb(result);
        }
    }
    /// Original request has no fail callback: request/pending remain latched on failure.
    pub fn clear_last_request_time(&self, p: PeerKey) {
        if let Some(e) = self.recent.borrow_mut().get_mut(&p) {
            e.last = 0
        }
    }
    pub fn update_pinned_order(
        &self,
        p: PeerKey,
        input: tl::InputPeer,
        ids: &[SavedStarGiftId],
        input_id: impl Fn(SavedStarGiftId) -> tl::InputSavedStarGift,
    ) {
        let id = tg_mtproto::mtp_instance::get_next_request_id();
        (self.effect)(Effect::UpdatePinned {
            id,
            peer: p,
            request: tl::PaymentsToggleStarGiftsPinnedToTop::new(
                input,
                ids.iter().copied().map(input_id).collect(),
            ),
        })
    }
    pub fn pinned_order_received(
        &self,
        p: PeerKey,
        gifts: &[SavedStarGift],
        ids: &[SavedStarGiftId],
        done: Option<&dyn Fn()>,
    ) {
        let ordered = ids
            .iter()
            .filter_map(|id| gifts.iter().find(|g| g.manage_id == *id).cloned())
            .collect();
        self.recent.borrow_mut().entry(p).or_default().gifts = ordered;
        if let Some(done) = done {
            done()
        }
    }
    pub fn toggle_ids(
        gifts: &[SavedStarGift],
        manage_id: SavedStarGiftId,
        pinned: bool,
        limit: usize,
    ) -> Vec<SavedStarGiftId> {
        let mut ids = vec![];
        for g in gifts {
            if g.pinned && g.manage_id != manage_id {
                ids.push(g.manage_id);
                if pinned && ids.len() >= limit.wrapping_sub(1) {
                    break;
                }
            }
        }
        if pinned {
            ids.push(manage_id)
        }
        ids
    }
    pub fn reorder_ids(
        gifts: &[SavedStarGift],
        old: i32,
        new: i32,
    ) -> Option<Vec<SavedStarGiftId>> {
        if old < 0
            || new < 0
            || old as usize >= gifts.len()
            || new as usize >= gifts.len()
            || old == new
        {
            return None;
        }
        let mut ids = gifts.iter().map(|g| g.manage_id).collect::<Vec<_>>();
        let v = ids.remove(old as usize);
        ids.insert(new as usize, v);
        Some(ids)
    }
}

impl RecentSharedMediaGifts {
    fn start_order(
        &self,
        p: PeerKey,
        input: tl::InputPeer,
        gifts: Vec<SavedStarGift>,
        ids: Vec<SavedStarGiftId>,
        notify: Option<(SavedStarGiftId, bool)>,
    ) {
        let request = tl::PaymentsToggleStarGiftsPinnedToTop::new(
            input.clone(),
            ids.iter()
                .map(|id| {
                    input_saved_star_gift_id(*id, |peer| {
                        assert_eq!(peer, p);
                        input.clone()
                    })
                })
                .collect(),
        );
        let id = tg_mtproto::mtp_instance::get_next_request_id();
        self.transactions.borrow_mut().insert(
            id,
            Transaction::Order {
                peer: p,
                gifts,
                ids,
                notify,
            },
        );
        (self.effect)(Effect::UpdatePinned {
            id,
            peer: p,
            request,
        });
    }
    pub fn toggle_pinned(
        self: &Rc<Self>,
        p: PeerKey,
        input: tl::InputPeer,
        manage: SavedStarGiftId,
        pinned: bool,
        limit: usize,
    ) {
        let weak = Rc::downgrade(self);
        let saved_input = input.clone();
        self.request(
            p,
            input,
            Rc::new(move |gifts| {
                let Some(s) = weak.upgrade() else { return };
                let ids = Self::toggle_ids(&gifts, manage, pinned, limit);
                if !pinned {
                    s.start_order(p, saved_input.clone(), gifts, ids, Some((manage, false)));
                    return;
                }
                let id = tg_mtproto::mtp_instance::get_next_request_id();
                let request = tl::PaymentsGetSavedStarGift::new(vec![input_saved_star_gift_id(
                    manage,
                    |peer| {
                        assert_eq!(peer, p);
                        saved_input.clone()
                    },
                )]);
                s.transactions.borrow_mut().insert(
                    id,
                    Transaction::Fetch {
                        peer: p,
                        input: saved_input.clone(),
                        gifts,
                        ids,
                        manage,
                    },
                );
                (s.effect)(Effect::FetchGift {
                    id,
                    peer: p,
                    request,
                });
            }),
            true,
        );
    }
    pub fn pin_gift_received(&self, id: i32, parsed: Option<SavedStarGift>) {
        let t = self.transactions.borrow_mut().remove(&id);
        let Some(Transaction::Fetch {
            peer,
            input,
            gifts,
            ids,
            manage,
        }) = t
        else {
            return;
        };
        if let Some(mut gift) = parsed {
            gift.pinned = true;
            let mut updated = gifts
                .into_iter()
                .filter(|g| g.pinned && g.manage_id != manage)
                .collect::<Vec<_>>();
            updated.push(gift);
            self.start_order(peer, input, updated, ids, Some((manage, true)));
        }
    }
    pub fn order_succeeded(&self, id: i32) {
        let t = self.transactions.borrow_mut().remove(&id);
        if let Some(Transaction::Order {
            peer,
            gifts,
            ids,
            notify,
        }) = t
        {
            self.pinned_order_received(peer, &gifts, &ids, None);
            if let Some((id, pinned)) = notify {
                (self.effect)(Effect::GiftUpdate { peer, id, pinned })
            }
        }
    }
    pub fn order_failed(&self, id: i32, error: String) {
        self.transactions.borrow_mut().remove(&id);
        (self.effect)(Effect::GiftError(error))
    }
    pub fn reorder_pinned(self: &Rc<Self>, p: PeerKey, input: tl::InputPeer, old: i32, new: i32) {
        let weak = Rc::downgrade(self);
        let saved_input = input.clone();
        self.request(
            p,
            input,
            Rc::new(move |gifts| {
                let Some(s) = weak.upgrade() else { return };
                if let Some(ids) = Self::reorder_ids(&gifts, old, new) {
                    s.start_order(p, saved_input.clone(), gifts, ids, None)
                }
            }),
            true,
        )
    }
}
/// Actual typed Sender calls; full Api::FromTL/text/credit parsing remains24/01.
pub type ReplyHandler = std::sync::Arc<dyn Fn(i32, Result<Vec<i32>, String>) + Send + Sync>;
pub fn send_request(sender: &tg_mtproto::sender::Sender, e: Effect, reply: ReplyHandler) {
    macro_rules! send {
        ($id:expr,$r:expr) => {{
            let id = $id;
            let fail = reply.clone();
            sender
                .request($r)
                .override_id(id)
                .done(move |v| reply(id, Ok(tl::tl::serialize(&v))))
                .fail(move |e| fail(id, Err(e.kind.clone())))
                .send();
        }};
    }
    match e {
        Effect::Request { id, request, .. } => send!(id, request),
        Effect::UpdatePinned { id, request, .. } => send!(id, request),
        Effect::FetchGift { id, request, .. } => send!(id, request),
        _ => panic!("owner UI effect is not a network request"),
    }
}
