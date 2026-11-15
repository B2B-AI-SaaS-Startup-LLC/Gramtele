//! Sharing/retention policy over actual backend readers/documents. No streaming
//! loader or media document implementation is substituted here (stage7).
use super::{data_file_origin::FileOrigin, data_store_keys::*};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    rc::{Rc, Weak},
};
use tg_base::timer::{Timer, TimerHost};
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MediaKey {
    Document(DocumentRecordKey),
    Photo(PhotoRecordKey),
}
impl MediaKey {
    fn scope(self) -> SessionScopeId {
        match self {
            Self::Document(k) => k.scope(),
            Self::Photo(k) => k.scope(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QualityDescriptor {
    pub size_in_bytes: u32,
    pub height: u32,
}
pub trait StreamingBackend: 'static {
    type Reader: 'static;
    type Document: 'static;
    fn create_reader(
        &self,
        data: MediaKey,
        origin: FileOrigin,
        force_remote: bool,
    ) -> Option<Rc<Self::Reader>>;
    fn is_remote_loader(&self, reader: &Self::Reader) -> bool;
    fn create_document(
        &self,
        data: MediaKey,
        reader: Rc<Self::Reader>,
        other: Vec<QualityDescriptor>,
    ) -> Rc<Self::Document>;
    fn set_other_qualities(&self, document: &Self::Document, other: Vec<QualityDescriptor>);
    /// Actual original->resolveQualities(HistoryItem*) guard/list18–20.
    fn resolve_qualities(
        &self,
        original: DocumentRecordKey,
        context: MessageKey,
    ) -> Vec<DocumentRecordKey>;
    /// Actual DocumentData::size and resolveVideoQuality(), not loader estimates.
    fn quality_data(&self, quality: DocumentRecordKey) -> (i64, i32);
}
struct Maps<R, D> {
    readers: BTreeMap<MediaKey, Weak<R>>,
    documents: BTreeMap<MediaKey, Weak<D>>,
}
impl<R, D> Default for Maps<R, D> {
    fn default() -> Self {
        Self {
            readers: BTreeMap::new(),
            documents: BTreeMap::new(),
        }
    }
}
struct Kept<D> {
    document: Rc<D>,
    until: i64,
}
pub struct Streaming<B: StreamingBackend> {
    scope: SessionScopeId,
    alive: Weak<super::data_store_keys::ScopeState>,
    backend: Rc<B>,
    maps: RefCell<Maps<B::Reader, B::Document>>,
    kept: RefCell<Vec<Kept<B::Document>>>,
    host: Rc<TimerHost>,
    timer: Timer,
}
fn same_kind(a: MediaKey, b: MediaKey) -> bool {
    matches!(
        (a, b),
        (MediaKey::Document(_), MediaKey::Document(_)) | (MediaKey::Photo(_), MediaKey::Photo(_))
    )
}
/// Original helper replaces the requested entry (even expired), prunes OTHER
/// expired entries; caller inserts only if this key was absent.
fn prune_destroyed_and_set<T>(
    map: &mut BTreeMap<MediaKey, Weak<T>>,
    key: MediaKey,
    value: &Rc<T>,
) -> bool {
    let mut found = false;
    map.retain(|k, w| {
        if *k == key {
            *w = Rc::downgrade(value);
            found = true;
            true
        } else {
            !same_kind(*k, key) || w.upgrade().is_some()
        }
    });
    found
}
impl<B: StreamingBackend> Streaming<B> {
    pub fn new(scope: &SessionScope, backend: Rc<B>, host: Rc<TimerHost>) -> Rc<Self> {
        let id = scope.id();
        let alive = scope.downgrade();
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let w = weak.clone();
            Self {
                scope: id,
                alive,
                backend,
                maps: RefCell::default(),
                kept: RefCell::default(),
                host: host.clone(),
                timer: Timer::in_host(
                    &host,
                    Some(Rc::new(move || {
                        if let Some(s) = w.upgrade() {
                            s.clear_kept_alive();
                        }
                    })),
                ),
            }
        })
    }
    pub fn shared_reader(
        &self,
        key: MediaKey,
        origin: FileOrigin,
        force_remote: bool,
    ) -> Option<Rc<B::Reader>> {
        assert!(self.alive.upgrade().is_some(), "SessionScope ended");
        assert_eq!(key.scope(), self.scope);
        let existing = self.maps.borrow().readers.get(&key).and_then(Weak::upgrade);
        if let Some(r) = existing {
            if !force_remote || self.backend.is_remote_loader(&r) {
                return Some(r);
            }
        }
        let value = self.backend.create_reader(key, origin, force_remote)?;
        let mut maps = self.maps.borrow_mut();
        if !prune_destroyed_and_set(&mut maps.readers, key, &value) {
            maps.readers.insert(key, Rc::downgrade(&value));
        }
        Some(value)
    }
    pub fn shared_document(&self, key: MediaKey, origin: FileOrigin) -> Option<Rc<B::Document>> {
        self.document_impl(key, None, None, origin)
    }
    pub fn shared_quality_document(
        &self,
        quality: DocumentRecordKey,
        original: DocumentRecordKey,
        context: Option<MessageKey>,
        origin: FileOrigin,
    ) -> Option<Rc<B::Document>> {
        assert_eq!(original.scope(), self.scope);
        if let Some(c) = context {
            assert_eq!(c.scope(), self.scope);
        }
        self.document_impl(MediaKey::Document(quality), Some(original), context, origin)
    }
    fn document_impl(
        &self,
        key: MediaKey,
        original: Option<DocumentRecordKey>,
        context: Option<MessageKey>,
        origin: FileOrigin,
    ) -> Option<Rc<B::Document>> {
        assert!(self.alive.upgrade().is_some(), "SessionScope ended");
        assert_eq!(key.scope(), self.scope);
        let other = match (key, original, context) {
            (MediaKey::Document(q), Some(o), Some(c)) => self
                .backend
                .resolve_qualities(o, c)
                .into_iter()
                .filter(|video| *video != q)
                .filter_map(|video| {
                    assert_eq!(video.scope(), self.scope);
                    let (size, height) = self.backend.quality_data(video);
                    (height != 0).then_some(QualityDescriptor {
                        size_in_bytes: size as u32,
                        height: height as u32,
                    })
                })
                .collect(),
            _ => vec![],
        };
        let existing = self
            .maps
            .borrow()
            .documents
            .get(&key)
            .and_then(Weak::upgrade);
        if let Some(d) = existing {
            if !other.is_empty() {
                self.backend.set_other_qualities(&d, other);
            }
            return Some(d);
        }
        let reader = self.shared_reader(key, origin, false)?;
        let value = self.backend.create_document(key, reader, other);
        let mut maps = self.maps.borrow_mut();
        if !prune_destroyed_and_set(&mut maps.documents, key, &value) {
            maps.documents.insert(key, Rc::downgrade(&value));
        }
        Some(value)
    }
    pub fn keep_alive(&self, key: MediaKey) {
        assert!(self.alive.upgrade().is_some(), "SessionScope ended");
        assert_eq!(key.scope(), self.scope);
        let value = self
            .maps
            .borrow()
            .documents
            .get(&key)
            .and_then(Weak::upgrade);
        let Some(document) = value else {
            return;
        };
        let until = self.host.now() + 5000;
        let mut kept = self.kept.borrow_mut();
        if let Some(k) = kept.iter_mut().find(|k| Rc::ptr_eq(&k.document, &document)) {
            k.until = until;
        } else {
            kept.push(Kept { document, until });
        }
        drop(kept);
        if !self.timer.is_active() {
            self.timer.call_once(5000);
        }
    }
    pub fn clear_kept_alive(&self) {
        let now = self.host.now();
        let (expired, min) = {
            let mut v = self.kept.borrow_mut();
            let mut expired = vec![];
            let mut i = 0;
            while i < v.len() {
                if v[i].until <= now {
                    expired.push(v.remove(i));
                } else {
                    i += 1;
                }
            }
            let min = v.iter().map(|k| k.until - now).min();
            (expired, min)
        };
        drop(expired);
        if let Some(min) = min {
            self.timer.call_once(min);
        }
    }
}

impl<B: StreamingBackend> Drop for Streaming<B> {
    fn drop(&mut self) {
        // C++ destroys its timer before retained documents; keep backend alive while
        // their destructors run. A queued weak callback cannot resurrect this model.
        self.timer.cancel();
        let kept = std::mem::take(self.kept.get_mut());
        drop(kept);
    }
}
