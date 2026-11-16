use super::data_store_keys::*;
use std::collections::BTreeMap;
#[derive(Default)]
pub struct MediaRotation {
    photos: BTreeMap<PhotoRecordKey, i32>,
    documents: BTreeMap<DocumentRecordKey, i32>,
}
/// Caller excludes multiples of360: the original helper's negative-multiple
/// case violates its own Ensures, but set() never takes that branch.
pub fn normalize_rotation(v: i32) -> i32 {
    let r = i64::from(v) - ((i64::from(v) / 360) - i64::from(v < 0)) * 360;
    assert!((0..360).contains(&r));
    r as i32
}
impl MediaRotation {
    pub fn set_photo(&mut self, k: PhotoRecordKey, v: i32) {
        if v % 360 != 0 {
            self.photos.insert(k, normalize_rotation(v));
        } else {
            self.photos.remove(&k);
        }
    }
    pub fn get_photo(&self, k: PhotoRecordKey) -> i32 {
        self.photos.get(&k).copied().unwrap_or_default()
    }
    pub fn set_document(&mut self, k: DocumentRecordKey, v: i32) {
        if v % 360 != 0 {
            self.documents.insert(k, normalize_rotation(v));
        } else {
            self.documents.remove(&k);
        }
    }
    pub fn get_document(&self, k: DocumentRecordKey) -> i32 {
        self.documents.get(&k).copied().unwrap_or_default()
    }
}
