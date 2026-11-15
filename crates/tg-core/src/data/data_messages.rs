use super::{data_msg_id::*, data_peer_id::PeerId, data_slice_builder::*};
use std::{cell::RefCell, cmp::Ordering, rc::Rc};
use tg_base::flat_set::FlatSet;
use tg_rpl::{EventStream, Lifetime, Producer};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum LoadDirection {
    #[default]
    Around,
    Before,
    After,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MessagePosition {
    pub full_id: FullMsgId,
    pub date: i32,
}
impl MessagePosition {
    pub fn is_present(self) -> bool {
        !self.full_id.msg.is_zero()
    }
}
impl Ord for MessagePosition {
    fn cmp(&self, o: &Self) -> Ordering {
        (self.date, self.full_id).cmp(&(o.date, o.full_id))
    }
}
impl PartialOrd for MessagePosition {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MessagesRange {
    pub from: MessagePosition,
    pub till: MessagePosition,
}
pub const MIN_DATE: i32 = 0;
pub const MAX_DATE: i32 = i32::MAX;
pub const MIN_MESSAGE_POSITION: MessagePosition = MessagePosition {
    full_id: FullMsgId {
        peer: PeerId::new(0),
        msg: MsgId::new(1),
    },
    date: MIN_DATE,
};
pub const MAX_MESSAGE_POSITION: MessagePosition = MessagePosition {
    full_id: FullMsgId {
        peer: PeerId::new(0),
        msg: MsgId::new(SERVER_MAX_MSG_ID.bare - 1),
    },
    date: MAX_DATE,
};
pub const FULL_MESSAGES_RANGE: MessagesRange = MessagesRange {
    from: MIN_MESSAGE_POSITION,
    till: MAX_MESSAGE_POSITION,
};
pub const UNREAD_MESSAGE_POSITION: MessagePosition = MessagePosition {
    full_id: FullMsgId {
        peer: PeerId::new(0),
        msg: SHOW_AT_UNREAD_MSG_ID,
    },
    date: MIN_DATE,
};
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MessagesSlice {
    pub ids: Vec<FullMsgId>,
    pub nearest_to_around: FullMsgId,
    pub skipped_before: Option<i32>,
    pub skipped_after: Option<i32>,
    pub full_count: Option<i32>,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct MessagesQuery {
    pub around_id: MessagePosition,
    pub limit_before: i32,
    pub limit_after: i32,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MessagesResult {
    pub count: Option<i32>,
    pub skipped_before: Option<i32>,
    pub skipped_after: Option<i32>,
    pub message_ids: FlatSet<MessagePosition>,
}
#[derive(Clone, Debug, Default)]
pub struct MessagesSliceUpdate {
    pub messages: Option<FlatSet<MessagePosition>>,
    pub range: MessagesRange,
    pub count: Option<i32>,
}
struct Slice {
    messages: FlatSet<MessagePosition>,
    range: MessagesRange,
}
#[derive(Default)]
struct State {
    count: Option<i32>,
    slices: Vec<Slice>,
}
#[derive(Default)]
pub struct MessagesList {
    state: RefCell<State>,
    updated: EventStream<MessagesSliceUpdate>,
}
impl MessagesList {
    fn add_range(
        &self,
        messages: impl IntoIterator<Item = MessagePosition>,
        range: MessagesRange,
        count: Option<i32>,
        increment: bool,
    ) {
        assert!(range.from <= range.till);
        assert!(count.is_none() || !increment);
        let update = {
            let mut s = self.state.borrow_mut();
            let from = s.slices.partition_point(|v| v.range.till < range.from);
            let till = s.slices.partition_point(|v| v.range.from <= range.till);
            let messages: FlatSet<_> = messages.into_iter().collect();
            let added;
            if from < till {
                // Source uniteAndAdd subtracts only the first old slice size.
                // Keep the overcount when addNew unites multiple ranges.
                let was = s.slices[from].messages.len();
                let mut joined = Slice { messages, range };
                for old in s.slices.drain(from..till) {
                    joined.messages.extend(old.messages.iter().copied());
                    joined.range.from = joined.range.from.min(old.range.from);
                    joined.range.till = joined.range.till.max(old.range.till);
                }
                added = joined.messages.len() as i32 - was as i32;
                s.slices.insert(from, joined);
            } else {
                added = messages.len() as i32;
                s.slices.insert(from, Slice { messages, range });
            }
            if count.is_some() {
                s.count = count;
            } else if increment && added > 0 {
                s.count = s.count.map(|n| n + added);
            }
            if s.slices.len() == 1 && s.slices[0].range == FULL_MESSAGES_RANGE {
                s.count = Some(s.slices[0].messages.len() as i32);
            }
            MessagesSliceUpdate {
                messages: Some(s.slices[from].messages.clone()),
                range: s.slices[from].range,
                count: s.count,
            }
        };
        self.updated.fire(update);
    }
    pub fn add_one(&self, id: MessagePosition) {
        self.add_range([id], MessagesRange { from: id, till: id }, None, true);
    }
    pub fn add_new(&self, id: MessagePosition) {
        self.add_range(
            [id],
            MessagesRange {
                from: id,
                till: MAX_MESSAGE_POSITION,
            },
            None,
            true,
        );
    }
    pub fn add_slice(&self, ids: Vec<MessagePosition>, range: MessagesRange, count: Option<i32>) {
        self.add_range(ids, range, count, false);
    }
    pub fn remove_one(&self, id: MessagePosition) {
        let update = {
            let mut s = self.state.borrow_mut();
            let i = s.slices.partition_point(|v| v.range.till < id);
            s.count = s.count.map(|n| n - 1);
            if i < s.slices.len() && s.slices[i].range.from <= id {
                s.slices[i].messages.remove(&id);
                Some(MessagesSliceUpdate {
                    messages: Some(s.slices[i].messages.clone()),
                    range: s.slices[i].range,
                    count: s.count,
                })
            } else {
                None
            }
        };
        if let Some(u) = update {
            self.updated.fire(u);
        }
    }
    pub fn remove_less_than(&self, id: MessagePosition) {
        let mut s = self.state.borrow_mut();
        let mut removed = 0;
        while let Some(first) = s.slices.first_mut() {
            if first.range.till <= id {
                removed += first.messages.len() as i32;
                s.slices.remove(0);
            } else {
                if first.range.from <= id {
                    first.range.from = MIN_MESSAGE_POSITION;
                    let n = first.messages.lower_bound(&id);
                    removed += n as i32;
                    first.messages.erase_range(0..n);
                }
                break;
            }
        }
        if removed != 0 {
            s.count = s.count.map(|n| n - removed);
        }
    }
    pub fn invalidate(&self) {
        *self.state.borrow_mut() = State::default();
    }
    pub fn invalidate_bottom(&self) {
        let mut s = self.state.borrow_mut();
        if let Some(last) = s.slices.last_mut() {
            if last.range.till == MAX_MESSAGE_POSITION {
                last.range.till = last.messages.back().copied().unwrap_or(last.range.from);
            }
        }
        s.count = None;
    }
    pub fn empty(&self) -> bool {
        self.state
            .borrow()
            .slices
            .iter()
            .all(|s| s.messages.is_empty())
    }
    pub fn slice_updated(&self) -> Producer<MessagesSliceUpdate> {
        self.updated.events()
    }
    pub fn snapshot(&self, q: MessagesQuery) -> MessagesResult {
        if !q.around_id.is_present() {
            return MessagesResult::default();
        }
        assert!(q.limit_before >= 0 && q.limit_after >= 0);
        let s = self.state.borrow();
        let Some(slice) = s.slices.iter().find(|v| v.range.till >= q.around_id) else {
            return MessagesResult::default();
        };
        if slice.range.from > q.around_id {
            return MessagesResult::default();
        }
        let p = slice.messages.lower_bound(&q.around_id);
        let before = p.min(q.limit_before as usize);
        let after = (slice.messages.len() - p).min(q.limit_after as usize + 1);
        let ids = slice.messages.as_slice()[p - before..p + after]
            .iter()
            .copied()
            .collect::<FlatSet<_>>();
        let mut r = MessagesResult {
            count: s.count,
            skipped_before: (slice.range.from == MIN_MESSAGE_POSITION)
                .then_some((p - before) as i32),
            skipped_after: (slice.range.till == MAX_MESSAGE_POSITION)
                .then_some((slice.messages.len() - p - after) as i32),
            message_ids: ids,
        };
        if let Some(c) = r.count {
            if let (None, Some(n)) = (r.skipped_before, r.skipped_after) {
                r.skipped_before = Some(c - n - r.message_ids.len() as i32);
            } else if let (Some(n), None) = (r.skipped_before, r.skipped_after) {
                r.skipped_after = Some(c - n - r.message_ids.len() as i32);
            }
        }
        r
    }
    pub fn query(self: &Rc<Self>, q: MessagesQuery) -> Producer<MessagesResult> {
        let this = self.clone();
        Producer::new(move |c| {
            let r = this.snapshot(q);
            if r.count.is_some() || !r.message_ids.is_empty() {
                c.put_next(r);
            }
            c.put_done();
            Lifetime::new()
        })
    }
    pub fn viewer(self: &Rc<Self>, q: MessagesQuery) -> Producer<MessagesResult> {
        let this = self.clone();
        tg_rpl::single(self.snapshot(q))
            .then(self.slice_updated().map(move |_| this.snapshot(q)))
            .filter(|r| r.count.is_some() || !r.message_ids.is_empty())
    }
}
pub type MessagesAroundData = AroundData<MessagePosition>;
pub struct MessagesSliceBuilder {
    inner: Rc<SliceBuilder<MessagePosition>>,
    key: MessagePosition,
}
impl MessagesSliceBuilder {
    pub fn new(key: MessagePosition, before: i32, after: i32) -> Self {
        Self {
            inner: SliceBuilder::new(key, before, after, false),
            key,
        }
    }
    pub fn apply_initial(&self, r: &MessagesResult) -> bool {
        self.inner
            .apply_initial(r.count, &r.message_ids, r.skipped_before, r.skipped_after)
    }
    pub fn apply_update(&self, u: &MessagesSliceUpdate) -> bool {
        self.inner
            .apply_update(u.count, u.messages.as_ref(), u.range.from, u.range.till)
    }
    pub fn remove_one(&self, id: MessagePosition) -> bool {
        self.inner.remove_one(id)
    }
    pub fn remove_all(&self) -> bool {
        self.inner.remove_all()
    }
    pub fn invalidated(&self) -> bool {
        self.inner.invalidated()
    }
    pub fn bottom_invalidated(&self) -> bool {
        self.inner.invalidate_bottom()
    }
    pub fn check_insufficient(&self) {
        self.inner.check_insufficient();
    }
    pub fn insufficient_around(&self) -> Producer<MessagesAroundData> {
        self.inner.insufficient_around()
    }
    pub fn snapshot(&self) -> MessagesSlice {
        let s = self.inner.snapshot();
        MessagesSlice {
            ids: s.ids.iter().map(|p| p.full_id).collect(),
            nearest_to_around: s
                .ids
                .iter()
                .find(|p| **p >= self.key)
                .or_else(|| s.ids.back())
                .map(|p| p.full_id)
                .unwrap_or_default(),
            full_count: s.count,
            skipped_before: s.before,
            skipped_after: s.after,
        }
    }
}
