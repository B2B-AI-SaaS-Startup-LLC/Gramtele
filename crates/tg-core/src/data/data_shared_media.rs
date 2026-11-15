use super::{
    data_messages::LoadDirection, data_msg_id::*, data_peer_id::PeerId, data_sparse_ids::*,
};
use crate::storage::storage_shared_media::*;
use std::{cell::RefCell, rc::Rc};
use tg_rpl::{Lifetime, Producer};
pub fn shared_media_overview_type(t: SharedMediaType) -> Option<SharedMediaType> {
    use SharedMediaType::*;
    match t {
        Photo | Video | MusicFile | File | RoundVoiceFile | Link | Poll => Some(t),
        _ => None,
    }
}
pub fn shared_media_allow_search(t: SharedMediaType) -> bool {
    use SharedMediaType::*;
    matches!(t, MusicFile | File | Link | Poll)
}
pub type RequestMedia = dyn Fn(SharedMediaKey, MsgId, LoadDirection);
pub fn shared_media_viewer(
    storage: Rc<RefCell<SharedMedia>>,
    key: SharedMediaKey,
    before: i32,
    after: i32,
    request: Rc<RequestMedia>,
) -> Producer<SparseIdsSlice> {
    assert!(is_server_msg_id(key.message_id.into()) || key.message_id == 0);
    assert!(key.message_id != 0 || (before == 0 && after == 0));
    Producer::new(move |c| {
        let life = Lifetime::new();
        let builder = Rc::new(SparseIdsSliceBuilder::new(
            key.message_id.into(),
            before,
            after,
        ));
        let k = key.clone();
        let r = request.clone();
        builder.insufficient_around().start(
            move |u| r(k.clone(), u.around_id, u.direction),
            |_| {},
            || {},
            &life,
        );
        let stream = storage.borrow().slice_updated();
        let b = builder.clone();
        let out = c.clone();
        let k = key.clone();
        stream.start(
            move |u| {
                if u.peer_id == k.peer_id
                    && u.topic_root_id == k.topic_root_id
                    && u.monoforum_peer_id == k.monoforum_peer_id
                    && u.type_ == k.type_
                    && b.apply_update(&u.data)
                {
                    out.put_next(b.snapshot());
                }
            },
            |_| {},
            || {},
            &life,
        );
        let stream = storage.borrow().one_removed();
        let b = builder.clone();
        let out = c.clone();
        let k = key.clone();
        stream.start(
            move |u| {
                if u.peer_id == k.peer_id
                    && u.types.test(k.type_)
                    && b.remove_one(u.message_id.into())
                {
                    out.put_next(b.snapshot());
                }
            },
            |_| {},
            || {},
            &life,
        );
        let stream = storage.borrow().all_removed();
        let b = builder.clone();
        let out = c.clone();
        let k = key.clone();
        stream.start(
            move |u| {
                if u.peer_id == k.peer_id
                    && (u.topic_root_id == 0 || u.topic_root_id == k.topic_root_id)
                    && (u.monoforum_peer_id == 0 || u.monoforum_peer_id == k.monoforum_peer_id)
                    && u.types.test(k.type_)
                    && b.remove_all()
                {
                    out.put_next(b.snapshot());
                }
            },
            |_| {},
            || {},
            &life,
        );
        let stream = storage.borrow().bottom_invalidated();
        let b = builder.clone();
        let out = c.clone();
        let k = key.clone();
        stream.start(
            move |u| {
                if u.peer_id == k.peer_id && b.invalidate_bottom() {
                    out.put_next(b.snapshot());
                }
            },
            |_| {},
            || {},
            &life,
        );
        let query = storage.borrow().query(SharedMediaQuery {
            key: key.clone(),
            limit_before: before,
            limit_after: after,
        });
        let b = builder.clone();
        query.start(
            move |r| {
                if b.apply_initial(&r) {
                    c.put_next(b.snapshot());
                }
            },
            |_| {},
            move || builder.check_insufficient(),
            &life,
        );
        life
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SharedMediaMergedKey {
    pub merged_key: Key,
    pub type_: SharedMediaType,
}
pub fn shared_media_merged_viewer(
    storage: Rc<RefCell<SharedMedia>>,
    key: SharedMediaMergedKey,
    before: i32,
    after: i32,
    request: Rc<RequestMedia>,
) -> Producer<SparseIdsMergedSlice> {
    SparseIdsMergedSlice::create_viewer(
        key.merged_key,
        before,
        after,
        Rc::new(
            move |peer: PeerId, topic: MsgId, mono: PeerId, id: MsgId, b, a| {
                shared_media_viewer(
                    storage.clone(),
                    SharedMediaKey {
                        peer_id: peer.value,
                        topic_root_id: topic.bare,
                        monoforum_peer_id: mono.value,
                        message_id: id.bare,
                        type_: key.type_,
                    },
                    b,
                    a,
                    request.clone(),
                )
            },
        ),
    )
}

use super::{
    data_media_store::Photos, data_peer_id::PeerId as WirePeerId, data_session::Peers,
    data_store_keys::PhotoRecordKey,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MediaValue {
    Message(FullMsgId),
    Photo(PhotoRecordKey),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum UniversalMsgId {
    Message(MsgId),
    Photo(PhotoRecordKey),
}
#[derive(Clone, Copy, Debug)]
pub struct WithLastKey {
    pub merged_key: Key,
    pub type_: SharedMediaType,
    pub universal_id: UniversalMsgId,
}
impl WithLastKey {
    fn order_tuple(
        self,
    ) -> (
        PeerId,
        MsgId,
        PeerId,
        PeerId,
        SharedMediaType,
        UniversalMsgId,
    ) {
        (
            self.merged_key.peer_id,
            self.merged_key.topic_root_id,
            self.merged_key.monoforum_peer_id,
            self.merged_key.migrated_peer_id,
            self.type_,
            self.universal_id,
        )
    }
}
impl PartialEq for WithLastKey {
    fn eq(&self, other: &Self) -> bool {
        self.order_tuple() == other.order_tuple()
    }
}
impl Eq for WithLastKey {}
impl Ord for WithLastKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.order_tuple().cmp(&other.order_tuple())
    }
}
impl PartialOrd for WithLastKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl WithLastKey {
    pub fn viewer_key(self) -> Key {
        Key::new(
            self.merged_key.peer_id,
            self.merged_key.topic_root_id,
            self.merged_key.monoforum_peer_id,
            self.merged_key.migrated_peer_id,
            match self.universal_id {
                UniversalMsgId::Message(m) => m,
                UniversalMsgId::Photo(_) => MsgId::new(SERVER_MAX_MSG_ID.bare - 1),
            },
        )
    }
    pub fn ending_key(self) -> Key {
        Key {
            universal_id: MsgId::new(SERVER_MAX_MSG_ID.bare - 1),
            ..self.viewer_key()
        }
    }
    pub fn compute_id(self) -> MediaValue {
        match self.universal_id {
            UniversalMsgId::Message(m) => MediaValue::Message(
                Key {
                    universal_id: m,
                    ..self.merged_key
                }
                .compute_id(),
            ),
            UniversalMsgId::Photo(k) => MediaValue::Photo(k),
        }
    }
}
pub fn last_peer_photo_id(peers: &Peers, id: WirePeerId) -> Option<u64> {
    let key = peers.peer_loaded(id)?;
    peers.read(key, |p| {
        (!p.userpic_photo_unknown()).then(|| p.userpic_photo_id())
    })
}
pub fn last_full_msg_id(slice: &SparseIdsMergedSlice) -> Option<FullMsgId> {
    if slice.full_count() == Some(0) {
        Some(FullMsgId::default())
    } else if slice.size() == 0 || slice.skipped_after() != Some(0) {
        None
    } else {
        Some(slice.at(slice.size() - 1))
    }
}
#[derive(Clone)]
pub struct SharedMediaWithLastSlice {
    slice: SparseIdsMergedSlice,
    last_photo_id: Option<u64>,
    isolated_last_photo: Option<bool>,
    reversed: bool,
    photos: Rc<Photos>,
}
impl SharedMediaWithLastSlice {
    /// The required lookup is precisely message→media→photo→id (zero if absent).
    /// It does not construct History/HistoryItem models (packages18–20).
    pub fn new(
        key: WithLastKey,
        slice: SparseIdsMergedSlice,
        ending: Option<SparseIdsMergedSlice>,
        last_photo_id: Option<u64>,
        photos: Rc<Photos>,
        message_photo_id: impl Fn(FullMsgId) -> u64,
    ) -> Self {
        assert!(
            matches!(key.universal_id, UniversalMsgId::Message(_))
                || key.type_ == SharedMediaType::ChatPhoto
        );
        let isolated = if key.type_ != SharedMediaType::ChatPhoto || last_photo_id == Some(0) {
            Some(false)
        } else {
            last_photo_id.and_then(|photo| {
                last_full_msg_id(ending.as_ref().unwrap_or(&slice))
                    .map(|m| photo != message_photo_id(m))
            })
        };
        Self {
            slice,
            last_photo_id,
            isolated_last_photo: isolated,
            reversed: false,
            photos,
        }
    }
    fn isolated_in_slice(&self) -> bool {
        self.slice.skipped_after() != Some(0)
    }
    fn last_photo_skip(&self) -> Option<i32> {
        self.isolated_last_photo.map(i32::from)
    }
    pub fn full_count(&self) -> Option<i32> {
        Some(self.slice.full_count()? + self.last_photo_skip()?)
    }
    fn skipped_after_impl(&self) -> Option<i32> {
        if self.isolated_in_slice() {
            Some(self.slice.skipped_after()? + self.last_photo_skip()?)
        } else {
            self.last_photo_skip().map(|_| 0)
        }
    }
    pub fn skipped_before(&self) -> Option<i32> {
        if self.reversed {
            self.skipped_after_impl()
        } else {
            self.slice.skipped_before()
        }
    }
    pub fn skipped_after(&self) -> Option<i32> {
        if self.reversed {
            self.slice.skipped_before()
        } else {
            self.skipped_after_impl()
        }
    }
    pub fn size(&self) -> usize {
        self.slice.size()
            + usize::from(!self.isolated_in_slice() && self.last_photo_skip() == Some(1))
    }
    pub fn at(&self, index: usize) -> MediaValue {
        assert!(index < self.size());
        let index = if self.reversed {
            self.size() - 1 - index
        } else {
            index
        };
        if index < self.slice.size() {
            MediaValue::Message(self.slice.at(index))
        } else {
            MediaValue::Photo(self.photos.ensure_photo(self.last_photo_id.unwrap()))
        }
    }
    pub fn index_of(&self, value: MediaValue) -> Option<i32> {
        let result = match value {
            MediaValue::Message(id) => self.slice.index_of(id),
            MediaValue::Photo(k) => {
                let id = self.photos.read(k, |p| p.id()).expect("photo lifetime");
                if self.isolated_in_slice() || self.last_photo_id != Some(id) {
                    None
                } else {
                    self.last_photo_skip()
                        .map(|n| self.slice.size() as i32 - 1 + n)
                }
            }
        };
        if let Some(i) = result {
            assert!(
                i >= 0 && (i as usize) < self.size(),
                "source reports fatal DebugInfo for out-of-range index"
            );
        }
        if self.reversed {
            result.map(|n| self.size() as i32 - 1 - n)
        } else {
            result
        }
    }
    pub fn distance(&self, a: WithLastKey, b: WithLastKey) -> Option<i32> {
        Some(self.index_of(b.compute_id())? - self.index_of(a.compute_id())?)
    }
    pub fn reverse(&mut self) {
        self.reversed = !self.reversed;
    }
}

/// Actual owner calls used by WithLastViewer. Scheduled/SavedMusic implementations
/// require History (18–20) / SavedMusic; their bodies are deliberately not faked.
pub trait WithLastSource {
    fn merged_viewer(
        &self,
        key: SharedMediaMergedKey,
        before: i32,
        after: i32,
    ) -> Producer<SparseIdsMergedSlice>;
    fn scheduled_viewer(
        &self,
        key: SharedMediaMergedKey,
        before: i32,
        after: i32,
    ) -> Producer<SparseIdsMergedSlice>;
    fn saved_music_viewer(
        &self,
        key: SharedMediaMergedKey,
        before: i32,
        after: i32,
    ) -> Producer<SparseIdsMergedSlice>;
    fn last_peer_photo_id(&self, peer: PeerId) -> Option<u64>;
    fn message_photo_id(&self, message: FullMsgId) -> u64;
    fn photos(&self) -> Rc<Photos>;
}
pub fn shared_media_with_last_viewer(
    source: Rc<dyn WithLastSource>,
    key: WithLastKey,
    before: i32,
    after: i32,
) -> Producer<SharedMediaWithLastSlice> {
    tg_rpl::deferred(move || {
        let viewer_key = SharedMediaMergedKey {
            merged_key: key.viewer_key(),
            type_: key.type_,
        };
        let make = source.clone();
        let build = move |slice, ending| {
            SharedMediaWithLastSlice::new(
                key,
                slice,
                ending,
                make.last_peer_photo_id(key.merged_key.peer_id),
                make.photos(),
                |m| make.message_photo_id(m),
            )
        };
        if matches!(key.universal_id, UniversalMsgId::Photo(_)) {
            source
                .merged_viewer(viewer_key, before, after)
                .map(move |s| build(s, None))
        } else if key.merged_key.topic_root_id == SparseIdsMergedSlice::SCHEDULED_TOPIC_ID {
            source
                .scheduled_viewer(viewer_key, before, after)
                .map(move |s| build(s, None))
        } else if key.merged_key.topic_root_id == SparseIdsMergedSlice::SAVED_MUSIC_TOPIC_ID {
            source
                .saved_music_viewer(viewer_key, before, after)
                .map(move |s| build(s, None))
        } else {
            let ending = SharedMediaMergedKey {
                merged_key: key.ending_key(),
                type_: key.type_,
            };
            tg_rpl::combine((
                source.merged_viewer(viewer_key, before, after),
                source.merged_viewer(ending, 1, 1),
            ))
            .map(move |(s, e)| build(s, Some(e)))
        }
    })
}
pub fn shared_media_with_last_reversed_viewer(
    source: Rc<dyn WithLastSource>,
    key: WithLastKey,
    before: i32,
    after: i32,
) -> Producer<SharedMediaWithLastSlice> {
    shared_media_with_last_viewer(source, key, before, after).map(|mut s| {
        s.reverse();
        s
    })
}
impl SharedMediaWithLastSlice {
    pub fn from_key(
        key: WithLastKey,
        last_photo_id: Option<u64>,
        photos: Rc<Photos>,
        message_photo_id: impl Fn(FullMsgId) -> u64,
    ) -> Self {
        Self::new(
            key,
            SparseIdsMergedSlice::new(key.viewer_key()),
            matches!(key.universal_id, UniversalMsgId::Message(_))
                .then(|| SparseIdsMergedSlice::new(key.ending_key())),
            last_photo_id,
            photos,
            message_photo_id,
        )
    }
}
