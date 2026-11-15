use super::data_abstract_sparse_ids::AbstractSparseIds;
use crate::storage::storage_user_photos::*;
use std::{cell::RefCell, collections::VecDeque, rc::Rc};
use tg_rpl::{EventStream, Lifetime, Producer};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserPhotosSlice {
    key: UserPhotosKey,
    pub data: AbstractSparseIds<VecDeque<u64>>,
}
impl UserPhotosSlice {
    pub fn new(key: UserPhotosKey) -> Self {
        Self::from_ids(key, VecDeque::new(), None, None, None)
    }
    pub fn from_ids(
        key: UserPhotosKey,
        ids: VecDeque<u64>,
        count: Option<i32>,
        before: Option<i32>,
        after: Option<i32>,
    ) -> Self {
        Self {
            key,
            data: AbstractSparseIds::new(ids, count, before, after),
        }
    }
    pub fn key(&self) -> UserPhotosKey {
        self.key
    }
    pub fn distance(&self, a: UserPhotosKey, b: UserPhotosKey) -> Option<i32> {
        if a.user_id != self.key.user_id || b.user_id != self.key.user_id {
            None
        } else {
            self.data.distance(a.photo_id, b.photo_id)
        }
    }
    pub fn reverse(&mut self) {
        self.data.reverse();
    }
}
#[derive(Default)]
struct State {
    ids: VecDeque<u64>,
    count: Option<i32>,
    before: Option<i32>,
    after: i32,
}
pub struct UserPhotosSliceBuilder {
    key: UserPhotosKey,
    before: i32,
    after: i32,
    state: RefCell<State>,
    insufficient: EventStream<u64>,
}
impl UserPhotosSliceBuilder {
    pub fn new(key: UserPhotosKey, before: i32, after: i32) -> Self {
        assert!(before >= 0 && after >= 0);
        Self {
            key,
            before,
            after,
            state: RefCell::default(),
            insufficient: EventStream::new(),
        }
    }
    pub fn apply_result(&self, u: &UserPhotosResult) -> bool {
        self.merge(u.count, &u.photo_ids, u.skipped_after);
        true
    }
    pub fn apply_update(&self, u: &UserPhotosSliceUpdate) -> bool {
        if u.user_id != self.key.user_id {
            return false;
        }
        self.merge(u.count, &u.photo_ids, 0);
        true
    }
    fn merge(&self, count: Option<i32>, ids: &VecDeque<u64>, after: i32) {
        {
            let mut s = self.state.borrow_mut();
            if ids.is_empty() {
                if s.count != count {
                    s.count = count;
                    if s.count.is_some_and(|n| n <= s.ids.len() as i32) {
                        s.count = Some(s.ids.len() as i32);
                        s.before = Some(0);
                        s.after = 0;
                    }
                }
            } else {
                if count.is_some() {
                    s.count = count;
                }
                s.after = after;
                s.ids = ids.clone();
                if let Some(c) = s.count {
                    s.before = Some(c - s.after - s.ids.len() as i32);
                }
            }
        }
        self.check_insufficient_photos();
    }
    pub fn check_insufficient_photos(&self) {
        let request = {
            let mut s = self.state.borrow_mut();
            let p = s
                .ids
                .iter()
                .position(|id| *id == self.key.photo_id)
                .unwrap_or(s.ids.len()) as i32;
            let begin = p - self.before;
            let end = s.ids.len() as i32 - p - self.after - 1;
            if end > 0 {
                let keep = s.ids.len() - end as usize;
                s.ids.truncate(keep);
                s.after += end;
            }
            if begin > 0 {
                s.ids.drain(..begin as usize);
                s.before = s.before.map(|v| v + begin);
                None
            } else if begin < 0 && s.before.is_none_or(|n| n > 0) {
                Some(s.ids.front().copied().unwrap_or(0))
            } else {
                None
            }
        };
        if let Some(id) = request {
            self.insufficient.fire(id);
        }
    }
    pub fn insufficient_photos_around(&self) -> Producer<u64> {
        self.insufficient.events()
    }
    pub fn snapshot(&self) -> UserPhotosSlice {
        let s = self.state.borrow();
        UserPhotosSlice::from_ids(self.key, s.ids.clone(), s.count, s.before, Some(s.after))
    }
}
/// Starts with the real storage query and forwards insufficiency to Api::PeerPhoto.
pub fn user_photos_viewer(
    storage: Rc<RefCell<UserPhotos>>,
    key: UserPhotosKey,
    before: i32,
    after: i32,
    request: Rc<dyn Fn(u64)>,
) -> Producer<UserPhotosSlice> {
    Producer::new(move |c| {
        let life = Lifetime::new();
        let builder = Rc::new(UserPhotosSliceBuilder::new(key, before, after));
        let send = request.clone();
        builder
            .insufficient_photos_around()
            .start(move |id| send(id), |_| {}, || {}, &life);
        let updates = storage.borrow().slice_updated();
        let b = builder.clone();
        let out = c.clone();
        updates.start(
            move |u| {
                if b.apply_update(&u) {
                    out.put_next(b.snapshot());
                }
            },
            |_| {},
            || {},
            &life,
        );
        let query = storage.borrow().query(UserPhotosQuery {
            key,
            limit_before: before,
            limit_after: after,
        });
        let b = builder.clone();
        query.start(
            move |r| {
                if b.apply_result(&r) {
                    c.put_next(b.snapshot());
                }
            },
            |_| {},
            move || builder.check_insufficient_photos(),
            &life,
        );
        life
    })
}
pub fn user_photos_reversed_viewer(
    storage: Rc<RefCell<UserPhotos>>,
    key: UserPhotosKey,
    before: i32,
    after: i32,
    request: Rc<dyn Fn(u64)>,
) -> Producer<UserPhotosSlice> {
    user_photos_viewer(storage, key, before, after, request).map(|mut s| {
        s.reverse();
        s
    })
}
pub fn sync_user_fallback_photo_viewer(storage: &UserPhotos, user_id: u64) -> Option<u64> {
    let value = Rc::new(RefCell::new(None));
    let v = value.clone();
    let life = Lifetime::new();
    storage
        .query(UserPhotosQuery {
            key: UserPhotosKey {
                user_id,
                photo_id: 0,
                back: true,
            },
            limit_before: 1,
            limit_after: 1,
        })
        .start(
            move |r| {
                if let Some(id) = r.photo_ids.front() {
                    *v.borrow_mut() = Some(*id);
                }
            },
            |_| {},
            || {},
            &life,
        );
    let r = *value.borrow();
    r
}
