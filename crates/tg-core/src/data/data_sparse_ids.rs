use super::{
    data_abstract_sparse_ids::AbstractSparseIds,
    data_msg_id::*,
    data_peer_id::PeerId,
    data_slice_builder::{AroundData, SliceBuilder},
};
use crate::storage::storage_sparse_ids_list::{SparseIdsListResult, SparseIdsSliceUpdate};
use std::rc::Rc;
use tg_base::flat_set::FlatSet;
use tg_rpl::Producer;
pub type SparseIdsSlice = AbstractSparseIds<FlatSet<MsgId>>;
pub type SparseUnsortedIdsSlice = AbstractSparseIds<Vec<MsgId>>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub peer_id: PeerId,
    pub topic_root_id: MsgId,
    pub monoforum_peer_id: PeerId,
    pub migrated_peer_id: PeerId,
    pub universal_id: MsgId,
}
impl Key {
    pub fn new(
        peer_id: PeerId,
        topic_root_id: MsgId,
        monoforum_peer_id: PeerId,
        migrated_peer_id: PeerId,
        universal_id: MsgId,
    ) -> Self {
        Self {
            peer_id,
            topic_root_id,
            monoforum_peer_id,
            migrated_peer_id: if !topic_root_id.is_zero() || !monoforum_peer_id.is_zero() {
                PeerId::default()
            } else {
                migrated_peer_id
            },
            universal_id,
        }
    }
    pub fn part_key(self) -> MsgId {
        if self.universal_id.bare < 0 {
            1.into()
        } else {
            self.universal_id
        }
    }
    pub fn migrated_key(self) -> MsgId {
        if self.universal_id.bare < 0 {
            SERVER_MAX_MSG_ID + self.universal_id
        } else if self.universal_id.bare > 0 {
            MsgId::new(SERVER_MAX_MSG_ID.bare - 1)
        } else {
            0.into()
        }
    }
    pub fn compute_id(self) -> FullMsgId {
        if self.universal_id.bare >= 0 {
            FullMsgId::new(self.peer_id, self.universal_id)
        } else {
            FullMsgId::new(self.migrated_peer_id, SERVER_MAX_MSG_ID + self.universal_id)
        }
    }
}
#[derive(Clone, Debug)]
pub struct SparseIdsMergedSlice {
    pub key: Key,
    part: SparseIdsSlice,
    migrated: Option<SparseIdsSlice>,
    unsorted: Option<SparseUnsortedIdsSlice>,
}
impl SparseIdsMergedSlice {
    pub const SCHEDULED_TOPIC_ID: MsgId = SCHEDULED_MAX_MSG_ID;
    pub const SAVED_MUSIC_TOPIC_ID: MsgId = MsgId::new(SCHEDULED_MAX_MSG_ID.bare + 1);
    pub fn new(key: Key) -> Self {
        Self::from_parts(
            key,
            SparseIdsSlice::default(),
            (!key.migrated_peer_id.is_zero()).then(SparseIdsSlice::default),
        )
    }
    pub fn from_parts(key: Key, part: SparseIdsSlice, migrated: Option<SparseIdsSlice>) -> Self {
        Self {
            key,
            part,
            migrated,
            unsorted: None,
        }
    }
    pub fn from_unsorted(key: Key, unsorted: SparseUnsortedIdsSlice) -> Self {
        Self {
            key,
            part: Default::default(),
            migrated: None,
            unsorted: Some(unsorted),
        }
    }
    fn isolated_in_part(&self) -> bool {
        is_server_msg_id(self.key.universal_id)
            && (self.migrated.is_none() || self.part.skipped_before() != Some(0))
    }
    fn isolated_in_migrated(&self) -> bool {
        is_server_msg_id(SERVER_MAX_MSG_ID + self.key.universal_id)
            && self
                .migrated
                .as_ref()
                .expect("source requires migrated slice for negative universal ID")
                .skipped_after()
                != Some(0)
    }
    fn migrated_size(&self) -> usize {
        if self.isolated_in_part() {
            0
        } else {
            self.migrated
                .as_ref()
                .expect("source dereferences migrated slice outside part isolation")
                .size()
        }
    }
    pub fn full_count(&self) -> Option<i32> {
        if let Some(u) = &self.unsorted {
            return u.full_count();
        }
        Some(self.part.full_count()? + self.migrated.as_ref().map_or(Some(0), |m| m.full_count())?)
    }
    pub fn skipped_before(&self) -> Option<i32> {
        if let Some(u) = &self.unsorted {
            return u.skipped_before();
        }
        Some(
            if self.isolated_in_migrated() {
                0
            } else {
                self.part.skipped_before()?
            } + if let Some(m) = &self.migrated {
                if self.isolated_in_part() {
                    m.full_count()?
                } else {
                    m.skipped_before()?
                }
            } else {
                0
            },
        )
    }
    pub fn skipped_after(&self) -> Option<i32> {
        if let Some(u) = &self.unsorted {
            return u.skipped_after();
        }
        Some(
            if self.isolated_in_migrated() {
                self.part.full_count()?
            } else {
                self.part.skipped_after()?
            } + if self.isolated_in_part() {
                0
            } else {
                self.migrated
                    .as_ref()
                    .expect("migrated slice required")
                    .skipped_after()?
            },
        )
    }
    pub fn index_of(&self, id: FullMsgId) -> Option<i32> {
        if let Some(u) = &self.unsorted {
            return u.index_of(id.msg);
        }
        if id.peer == self.key.peer_id {
            self.part
                .index_of(id.msg)
                .map(|n| n + self.migrated_size() as i32)
        } else if self.isolated_in_part() {
            None
        } else if id.peer == self.key.migrated_peer_id {
            self.migrated.as_ref().and_then(|m| m.index_of(id.msg))
        } else {
            None
        }
    }
    pub fn size(&self) -> usize {
        if let Some(u) = &self.unsorted {
            return u.size();
        }
        self.migrated_size()
            + if self.isolated_in_migrated() {
                0
            } else {
                self.part.size()
            }
    }
    pub fn at(&self, index: usize) -> FullMsgId {
        assert!(index < self.size());
        if let Some(u) = &self.unsorted {
            return FullMsgId::new(self.key.peer_id, u.at(index));
        }
        let n = self.migrated_size();
        if index < n {
            FullMsgId::new(
                self.key.migrated_peer_id,
                self.migrated.as_ref().unwrap().at(index),
            )
        } else {
            FullMsgId::new(self.key.peer_id, self.part.at(index - n))
        }
    }
    pub fn distance(&self, a: Key, b: Key) -> Option<i32> {
        Some(self.index_of(b.compute_id())? - self.index_of(a.compute_id())?)
    }
    pub fn nearest(&self, id: MsgId) -> Option<FullMsgId> {
        if let Some(u) = &self.unsorted {
            return if u.index_of(id).is_some() {
                Some(FullMsgId::new(self.key.peer_id, id))
            } else if u.size() > 0 {
                Some(FullMsgId::new(self.key.peer_id, u.at(u.size() / 2)))
            } else {
                None
            };
        }
        if is_server_msg_id(id) {
            if let Some(n) = self.part.nearest(id) {
                return Some(FullMsgId::new(self.key.peer_id, n));
            }
            if self.isolated_in_part() {
                return None;
            }
            self.migrated
                .as_ref()
                .unwrap()
                .nearest(MsgId::new(SERVER_MAX_MSG_ID.bare - 1))
                .map(|n| FullMsgId::new(self.key.migrated_peer_id, n))
        } else {
            if let Some(n) = self
                .migrated
                .as_ref()
                .and_then(|m| m.nearest(id + SERVER_MAX_MSG_ID))
            {
                return Some(FullMsgId::new(self.key.migrated_peer_id, n));
            }
            if self.isolated_in_migrated() {
                return None;
            }
            self.part
                .nearest(0.into())
                .map(|n| FullMsgId::new(self.key.peer_id, n))
        }
    }
    pub fn create_viewer(
        key: Key,
        before: i32,
        after: i32,
        viewer: Rc<SimpleViewer>,
    ) -> Producer<Self> {
        assert!(
            key.topic_root_id.is_zero()
                || (key.monoforum_peer_id.is_zero() && key.migrated_peer_id.is_zero())
        );
        assert!(
            key.monoforum_peer_id.is_zero()
                || (key.topic_root_id.is_zero() && key.migrated_peer_id.is_zero())
        );
        assert!(
            is_server_msg_id(key.universal_id)
                || key.universal_id.is_zero()
                || (is_server_msg_id(SERVER_MAX_MSG_ID + key.universal_id)
                    && !key.migrated_peer_id.is_zero())
        );
        assert!(!key.universal_id.is_zero() || (before == 0 && after == 0));
        tg_rpl::deferred(move || {
            let part = viewer(
                key.peer_id,
                key.topic_root_id,
                key.monoforum_peer_id,
                key.part_key(),
                before,
                after,
            );
            if key.migrated_peer_id.is_zero() {
                part.map(move |p| Self::from_parts(key, p, None))
            } else {
                let m = viewer(
                    key.migrated_peer_id,
                    0.into(),
                    PeerId::default(),
                    key.migrated_key(),
                    before,
                    after,
                );
                tg_rpl::combine((part, m)).map(move |(p, m)| Self::from_parts(key, p, Some(m)))
            }
        })
    }
}
pub type SimpleViewer = dyn Fn(PeerId, MsgId, PeerId, MsgId, i32, i32) -> Producer<SparseIdsSlice>;
pub type SparseIdsAroundData = AroundData<MsgId>;
pub struct SparseIdsSliceBuilder {
    inner: Rc<SliceBuilder<MsgId>>,
}
impl SparseIdsSliceBuilder {
    pub fn new(key: MsgId, before: i32, after: i32) -> Self {
        Self {
            inner: SliceBuilder::new(key, before, after, true),
        }
    }
    pub fn apply_initial(&self, r: &SparseIdsListResult) -> bool {
        let ids = r.message_ids.iter().map(|&n| MsgId::new(n)).collect();
        self.inner
            .apply_initial(r.count, &ids, r.skipped_before, r.skipped_after)
    }
    pub fn apply_update(&self, u: &SparseIdsSliceUpdate) -> bool {
        let ids = u
            .messages
            .as_ref()
            .map(|ids| ids.iter().map(|&n| MsgId::new(n)).collect());
        self.inner.apply_update(
            u.count,
            ids.as_ref(),
            u.range.from.into(),
            u.range.till.into(),
        )
    }
    pub fn remove_one(&self, id: MsgId) -> bool {
        self.inner.remove_one(id)
    }
    pub fn remove_all(&self) -> bool {
        self.inner.remove_all()
    }
    pub fn invalidate_bottom(&self) -> bool {
        self.inner.invalidate_bottom()
    }
    pub fn check_insufficient(&self) {
        self.inner.check_insufficient();
    }
    pub fn insufficient_around(&self) -> Producer<SparseIdsAroundData> {
        self.inner.insufficient_around()
    }
    pub fn snapshot(&self) -> SparseIdsSlice {
        let s = self.inner.snapshot();
        SparseIdsSlice::new(s.ids, s.count, s.before, s.after)
    }
}
