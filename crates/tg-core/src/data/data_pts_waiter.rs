//! PtsWaiter: Api::Updates owns timer scheduling and final update application.
use super::data_store_keys::ChannelKey;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use tg_scheme as tl;
#[derive(Clone)]
pub enum PtsUpdate {
    Update(tl::Update),
    Updates(tl::Updates),
}
#[derive(Clone)]
pub enum Effect {
    StartTimerFor {
        channel: Option<ChannelKey>,
        ms: i64,
    },
    ApplyUpdateNoPtsCheck(tl::Update),
    ApplyUpdatesNoPtsCheck(tl::Updates),
}
#[derive(Default)]
struct State {
    good: i32,
    last: i32,
    count: i32,
    apply_level: i32,
    requesting: bool,
    waiting_skipped: bool,
    waiting_short: bool,
    skipped_key: u32,
    queue: BTreeMap<u64, PtsUpdate>,
}
pub struct PtsWaiter {
    state: RefCell<State>,
    effect: Rc<dyn Fn(Effect)>,
}
impl PtsWaiter {
    pub const WAIT_FOR_SKIPPED_TIMEOUT: i64 = 1000;
    pub fn new(effect: Rc<dyn Fn(Effect)>) -> Self {
        Self {
            state: RefCell::default(),
            effect,
        }
    }
    pub fn init(&self, pts: i32) {
        {
            let mut s = self.state.borrow_mut();
            s.good = pts;
            s.last = pts;
            s.count = pts;
        }
        self.clear_skipped_updates();
    }
    pub fn inited(&self) -> bool {
        self.current() > 0
    }
    pub fn current(&self) -> i32 {
        self.state.borrow().good
    }
    pub fn set_requesting(&self, v: bool) {
        self.state.borrow_mut().requesting = v;
        if v {
            self.clear_skipped_updates();
        }
    }
    pub fn requesting(&self) -> bool {
        self.state.borrow().requesting
    }
    pub fn waiting_for_skipped(&self) -> bool {
        self.state.borrow().waiting_skipped
    }
    pub fn waiting_for_short_poll(&self) -> bool {
        self.state.borrow().waiting_short
    }
    pub fn set_waiting_for_skipped(&self, channel: Option<ChannelKey>, ms: i64) {
        if ms >= 0 {
            (self.effect)(Effect::StartTimerFor { channel, ms });
            self.state.borrow_mut().waiting_skipped = true;
        } else {
            self.state.borrow_mut().waiting_skipped = false;
            self.check_for_waiting(channel);
        }
    }
    pub fn set_waiting_for_short_poll(&self, channel: Option<ChannelKey>, ms: i64) {
        if ms >= 0 {
            (self.effect)(Effect::StartTimerFor { channel, ms });
            self.state.borrow_mut().waiting_short = true;
        } else {
            self.state.borrow_mut().waiting_short = false;
            self.check_for_waiting(channel);
        }
    }
    fn check_for_waiting(&self, channel: Option<ChannelKey>) {
        let waiting = {
            let s = self.state.borrow();
            s.waiting_skipped || s.waiting_short
        };
        if !waiting {
            (self.effect)(Effect::StartTimerFor { channel, ms: -1 });
        }
    }
    pub fn clear_skipped_updates(&self) {
        let mut s = self.state.borrow_mut();
        s.queue.clear();
        s.apply_level = 0;
    }
    fn enqueue(&self, pts: i32, data: PtsUpdate) {
        let mut s = self.state.borrow_mut();
        s.skipped_key = s.skipped_key.wrapping_add(1);
        let key = ((pts as u32 as u64) << 32) | u64::from(s.skipped_key);
        s.queue.entry(key).or_insert(data);
    }
    fn check(&self, channel: Option<ChannelKey>, pts: i32, count: i32) -> bool {
        if !self.inited() {
            self.init(pts);
            return true;
        }
        let wait = {
            let mut s = self.state.borrow_mut();
            s.last = s.last.max(pts);
            s.count = s.count.wrapping_add(count);
            if s.last == s.count {
                s.good = s.last;
                None
            } else {
                Some(if s.last < s.count {
                    1
                } else {
                    Self::WAIT_FOR_SKIPPED_TIMEOUT
                })
            }
        };
        if let Some(ms) = wait {
            self.set_waiting_for_skipped(channel, ms);
            count == 0
        } else {
            true
        }
    }
    pub fn updated(
        &self,
        channel: Option<ChannelKey>,
        pts: i32,
        count: i32,
        data: Option<PtsUpdate>,
    ) -> bool {
        {
            let s = self.state.borrow();
            if s.requesting || s.apply_level != 0 {
                return true;
            }
            if pts <= s.good && count > 0 {
                return false;
            }
        }
        if self.check(channel, pts, count) {
            return true;
        }
        if let Some(data) = data {
            self.enqueue(pts, data);
        }
        false
    }
    pub fn update_and_apply(
        &self,
        channel: Option<ChannelKey>,
        pts: i32,
        count: i32,
        data: Option<PtsUpdate>,
    ) -> bool {
        if !self.updated(channel, pts, count, data.clone()) {
            return false;
        }
        if let Some(data) = data {
            let immediate = {
                let s = self.state.borrow();
                !s.waiting_skipped || s.queue.is_empty()
            };
            if immediate {
                self.apply(data);
            } else {
                self.enqueue(pts, data);
                self.apply_skipped_updates(channel);
            }
        } else {
            self.apply_skipped_updates(channel);
        }
        true
    }
    fn apply(&self, data: PtsUpdate) {
        (self.effect)(match data {
            PtsUpdate::Update(u) => Effect::ApplyUpdateNoPtsCheck(u),
            PtsUpdate::Updates(u) => Effect::ApplyUpdatesNoPtsCheck(u),
        });
    }
    pub fn apply_skipped_updates(&self, channel: Option<ChannelKey>) {
        if !self.waiting_for_skipped() {
            return;
        }
        self.set_waiting_for_skipped(channel, -1);
        let queue = {
            let mut s = self.state.borrow_mut();
            if s.queue.is_empty() {
                return;
            }
            s.apply_level += 1;
            s.queue.values().cloned().collect::<Vec<_>>()
        };
        for u in queue {
            self.apply(u);
        }
        self.state.borrow_mut().apply_level -= 1;
        self.clear_skipped_updates();
    }
}
