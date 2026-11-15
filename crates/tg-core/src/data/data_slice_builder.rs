//! Shared implementation of the two source slice builders, preserving their
//! different removal/refill policies. No storage lists are duplicated here.
use super::{
    data_messages::{LoadDirection, MessagePosition, MAX_MESSAGE_POSITION, MIN_MESSAGE_POSITION},
    data_msg_id::MsgId,
};
use std::{cell::RefCell, rc::Rc};
use tg_base::flat_set::FlatSet;
use tg_rpl::{EventStream, Producer};
pub trait SliceKey: Copy + Ord + 'static {
    fn present(self) -> bool;
    fn old_valid(self) -> bool;
    fn range_min() -> Self;
    fn range_max() -> Self;
}
impl SliceKey for MsgId {
    fn present(self) -> bool {
        !self.is_zero()
    }
    fn old_valid(self) -> bool {
        self.bare >= 0
    }
    fn range_min() -> Self {
        0.into()
    }
    fn range_max() -> Self {
        super::data_msg_id::SERVER_MAX_MSG_ID
    }
}
impl SliceKey for MessagePosition {
    fn present(self) -> bool {
        self.is_present()
    }
    fn old_valid(self) -> bool {
        self != Self {
            full_id: Default::default(),
            date: -1,
        }
    }
    fn range_min() -> Self {
        MIN_MESSAGE_POSITION
    }
    fn range_max() -> Self {
        MAX_MESSAGE_POSITION
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AroundData<K> {
    pub around_id: K,
    pub direction: LoadDirection,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SliceData<K> {
    pub ids: FlatSet<K>,
    pub count: Option<i32>,
    pub before: Option<i32>,
    pub after: Option<i32>,
}
struct State<K> {
    ids: Vec<K>,
    count: Option<i32>,
    before: Option<i32>,
    after: Option<i32>,
}
pub struct SliceBuilder<K: SliceKey> {
    key: K,
    limit_before: i32,
    limit_after: i32,
    refill_on_remove: bool,
    state: RefCell<State<K>>,
    insufficient: EventStream<AroundData<K>>,
}
impl<K: SliceKey + Default> SliceBuilder<K> {
    pub fn new(key: K, before: i32, after: i32, refill_on_remove: bool) -> Rc<Self> {
        assert!(before >= 0 && after >= 0);
        Rc::new(Self {
            key,
            limit_before: before,
            limit_after: after,
            refill_on_remove,
            state: RefCell::new(State {
                ids: vec![],
                count: None,
                before: None,
                after: None,
            }),
            insufficient: EventStream::new(),
        })
    }
    pub fn insufficient_around(&self) -> Producer<AroundData<K>> {
        self.insufficient.events()
    }
    pub fn snapshot(&self) -> SliceData<K> {
        let s = self.state.borrow();
        SliceData {
            ids: s.ids.iter().copied().collect(),
            count: s.count,
            before: s.before,
            after: s.after,
        }
    }
    pub fn apply_initial(
        &self,
        count: Option<i32>,
        ids: &FlatSet<K>,
        before: Option<i32>,
        after: Option<i32>,
    ) -> bool {
        self.merge(count, ids, before, after);
        true
    }
    pub fn apply_update(
        &self,
        count: Option<i32>,
        ids: Option<&FlatSet<K>>,
        from: K,
        till: K,
    ) -> bool {
        let intersects = {
            let s = self.state.borrow();
            ids.is_some()
                && from <= s.ids.last().copied().unwrap_or(self.key)
                && s.ids.first().copied().unwrap_or(self.key) <= till
        };
        if !intersects && count.is_none() {
            return false;
        }
        let empty = FlatSet::default();
        self.merge(
            count,
            if intersects { ids.unwrap() } else { &empty },
            (from == K::range_min()).then_some(0),
            (till == K::range_max()).then_some(0),
        );
        true
    }
    pub fn merge(
        &self,
        count: Option<i32>,
        ids: &FlatSet<K>,
        before: Option<i32>,
        after: Option<i32>,
    ) {
        {
            let mut s = self.state.borrow_mut();
            if ids.is_empty() {
                if let Some(count_value) = count.filter(|_| s.count != count) {
                    s.count = count;
                    if count_value <= s.ids.len() as i32 {
                        s.count = Some(s.ids.len() as i32);
                        s.before = Some(0);
                        s.after = Some(0);
                    }
                }
            } else {
                if count.is_some() {
                    s.count = count;
                }
                let old_min = s.ids.first().copied();
                let old_max = s.ids.last().copied();
                s.ids.extend(ids.iter().copied());
                s.ids.sort_unstable();
                s.ids.dedup();
                s.before = if let Some(n) = before {
                    Some((n - s.ids.binary_search(ids.front().unwrap()).unwrap() as i32).max(0))
                } else if let (Some(old), Some(n)) = (old_min.filter(|k| k.old_valid()), s.before) {
                    Some((n - s.ids.binary_search(&old).unwrap() as i32).max(0))
                } else {
                    None
                };
                s.after = if let Some(n) = after {
                    Some(
                        (n - (s.ids.len() - s.ids.binary_search(ids.back().unwrap()).unwrap() - 1)
                            as i32)
                            .max(0),
                    )
                } else if let (Some(old), Some(n)) = (old_max.filter(|k| k.old_valid()), s.after) {
                    Some((n - (s.ids.len() - s.ids.binary_search(&old).unwrap() - 1) as i32).max(0))
                } else {
                    None
                };
            }
            if let Some(c) = s.count {
                if let (Some(n), None) = (s.before, s.after) {
                    s.after = Some(c - n - s.ids.len() as i32);
                } else if let (None, Some(n)) = (s.before, s.after) {
                    s.before = Some(c - n - s.ids.len() as i32);
                }
            }
        }
        self.check_insufficient();
    }
    pub fn remove_one(&self, id: K) -> bool {
        let changed = {
            let mut s = self.state.borrow_mut();
            let mut changed = false;
            if let Some(c) = &mut s.count {
                if *c > 0 {
                    *c -= 1;
                    changed = true;
                }
            }
            if let Ok(i) = s.ids.binary_search(&id) {
                s.ids.remove(i);
                changed = true;
            } else if !s.ids.is_empty() {
                if s.ids[0] > id && s.before.is_some_and(|n| n > 0) {
                    s.before = s.before.map(|n| n - 1);
                    changed = true;
                } else if *s.ids.last().unwrap() < id && s.after.is_some_and(|n| n > 0) {
                    s.after = s.after.map(|n| n - 1);
                    changed = true;
                }
            }
            changed
        };
        if changed && self.refill_on_remove {
            self.check_insufficient();
        }
        changed
    }
    pub fn remove_all(&self) -> bool {
        let mut s = self.state.borrow_mut();
        s.ids.clear();
        s.count = Some(0);
        s.before = Some(0);
        s.after = Some(0);
        true
    }
    pub fn invalidated(&self) -> bool {
        {
            let mut s = self.state.borrow_mut();
            s.ids.clear();
            s.count = None;
            s.before = None;
            s.after = None;
        }
        self.check_insufficient();
        false
    }
    pub fn invalidate_bottom(&self) -> bool {
        {
            let mut s = self.state.borrow_mut();
            s.count = None;
            s.after = None;
        }
        self.check_insufficient();
        true
    }
    fn request(&self, before: bool) {
        let around = {
            let s = self.state.borrow();
            if s.ids.is_empty() {
                AroundData {
                    around_id: self.key,
                    direction: LoadDirection::Around,
                }
            } else if before {
                AroundData {
                    around_id: s.ids[0],
                    direction: LoadDirection::Before,
                }
            } else {
                AroundData {
                    around_id: *s.ids.last().unwrap(),
                    direction: LoadDirection::After,
                }
            }
        };
        self.insufficient.fire(around);
    }
    pub fn check_insufficient(&self) {
        if !self.key.present() {
            if self.state.borrow().count.is_none() {
                self.insufficient.fire(AroundData {
                    around_id: K::default(),
                    direction: LoadDirection::Around,
                });
            }
            return;
        }
        let (begin, end) = {
            let s = self.state.borrow();
            let p = s.ids.partition_point(|k| *k < self.key) as i32;
            (
                p - self.limit_before,
                s.ids.len() as i32 - p - self.limit_after - 1,
            )
        };
        let mut requested = false;
        let request_before = {
            let mut s = self.state.borrow_mut();
            if begin > 0 {
                s.ids.drain(..begin as usize);
                s.before = s.before.map(|v| v + begin);
                false
            } else {
                begin < 0 && s.before.is_none_or(|v| v > 0)
            }
        };
        if request_before {
            requested = true;
            self.request(true);
        }
        let request_after = {
            let mut s = self.state.borrow_mut();
            if end > 0 {
                let keep = s.ids.len() - end as usize;
                s.ids.truncate(keep);
                s.after = s.after.map(|v| v + end);
                false
            } else {
                end < 0 && s.after.is_none_or(|v| v > 0)
            }
        };
        if request_after {
            requested = true;
            self.request(false);
        }
        if !requested && self.state.borrow().count.is_none() {
            self.insufficient.fire(AroundData {
                around_id: K::default(),
                direction: LoadDirection::Around,
            });
        }
    }
}
