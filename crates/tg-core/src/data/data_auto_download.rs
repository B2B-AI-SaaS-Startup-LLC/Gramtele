//! data_auto_download.h/.cpp: limits and v1/v2/v3 Qt stream compatibility.
use super::{
    data_document::DocumentData,
    data_peer::PeerData,
    data_peer_fields::PeerFields,
    data_peer_id::{deserialize_peer_id, serialize_peer_id, PeerId},
    data_photo::{PhotoData, PhotoSize},
};
use std::collections::BTreeMap;
use tg_base::qt_stream::{Reader, Writer};
pub const MAX_BYTES_LIMIT: i64 = 8000 * 512 * 1024;
const DEFAULT_MAX_SIZE: i64 = 12 * 1024 * 1024;
const DEFAULT_AUTO_PLAY_SIZE: i64 = 50 * 1024 * 1024;
#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    User,
    Group,
    Channel,
}
pub const SOURCES: [Source; 3] = [Source::User, Source::Group, Source::Channel];
#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    Photo,
    AutoPlayVideo,
    VoiceMessage,
    AutoPlayVideoMessage,
    Music,
    AutoPlayGIF,
    File,
}
pub const TYPES: [Type; 7] = [
    Type::Photo,
    Type::AutoPlayVideo,
    Type::VoiceMessage,
    Type::AutoPlayVideoMessage,
    Type::Music,
    Type::AutoPlayGIF,
    Type::File,
];
pub const AUTO_PLAY_TYPES: [Type; 3] = [
    Type::AutoPlayVideo,
    Type::AutoPlayVideoMessage,
    Type::AutoPlayGIF,
];
pub const STREAMED_TYPES: [Type; 2] = [Type::VoiceMessage, Type::Music];
#[repr(i8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Override {
    #[default]
    Default,
    ForceAllow,
    ForceDeny,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Single {
    limit: i32,
}
impl Default for Single {
    fn default() -> Self {
        Self { limit: -1 }
    }
}
impl Single {
    pub fn set_bytes_limit(&mut self, value: i64) {
        assert!((0..=MAX_BYTES_LIMIT).contains(&value));
        self.limit = value as u32 as i32;
    }
    pub fn has_value(&self) -> bool {
        self.limit != -1
    }
    pub fn bytes_limit(&self) -> i64 {
        assert!(self.has_value());
        self.limit as u32 as i64
    }
    pub fn should_download(&self, size: i64) -> bool {
        let n = self.bytes_limit();
        n > 0 && size <= n
    }
    pub fn serialize(&self) -> i32 {
        self.limit
    }
    pub fn set_from_serialized(&mut self, value: i32) -> bool {
        if value != -1 && value as u32 as i64 > MAX_BYTES_LIMIT {
            return false;
        }
        self.limit = value;
        true
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Set {
    data: [Single; 7],
}
impl Set {
    pub fn set_bytes_limit(&mut self, t: Type, n: i64) {
        self.data[t as usize].set_bytes_limit(n);
    }
    pub fn has_value(&self, t: Type) -> bool {
        self.data[t as usize].has_value()
    }
    pub fn should_download(&self, t: Type, n: i64) -> bool {
        self.data[t as usize].should_download(n)
    }
    pub fn bytes_limit(&self, t: Type) -> i64 {
        self.data[t as usize].bytes_limit()
    }
    pub fn serialize(&self, t: Type) -> i32 {
        self.data[t as usize].serialize()
    }
    pub fn set_from_serialized(&mut self, t: Type, n: i32) -> bool {
        self.data[t as usize].set_from_serialized(n)
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Full {
    data: [Set; 3],
    peer_overrides: BTreeMap<PeerId, Override>,
}
fn default_limit(s: Source, t: Type) -> i64 {
    match t {
        Type::AutoPlayVideo | Type::AutoPlayVideoMessage | Type::AutoPlayGIF => {
            DEFAULT_AUTO_PLAY_SIZE
        }
        Type::File | Type::Music if s == Source::Channel => 0,
        _ => DEFAULT_MAX_SIZE,
    }
}
impl Full {
    pub fn set_bytes_limit(&mut self, s: Source, t: Type, n: i64) {
        self.data[s as usize].set_bytes_limit(t, n);
    }
    pub fn bytes_limit(&self, s: Source, t: Type) -> i64 {
        let set = &self.data[s as usize];
        if set.has_value(t) {
            set.bytes_limit(t)
        } else {
            default_limit(s, t)
        }
    }
    pub fn should_download(&self, s: Source, t: Type, n: i64) -> bool {
        !STREAMED_TYPES.contains(&t) && self.bytes_limit(s, t) > 0 && n <= self.bytes_limit(s, t)
    }
    pub fn set_peer_override(&mut self, p: PeerId, v: Override) {
        if v == Override::Default {
            self.peer_overrides.remove(&p);
        } else {
            self.peer_overrides.insert(p, v);
        }
    }
    pub fn peer_override(&self, p: PeerId) -> Override {
        self.peer_overrides.get(&p).copied().unwrap_or_default()
    }
    pub fn enumerate_peer_overrides(&self, mut callback: impl FnMut(PeerId, Override)) {
        for (&p, &v) in &self.peer_overrides {
            callback(p, v);
        }
    }
    pub fn serialize(&self) -> Vec<u8> {
        let mut w = Writer(vec![3]);
        for set in &self.data {
            for t in TYPES {
                w.i32(set.serialize(t));
            }
        }
        w.i32(self.peer_overrides.len() as i32);
        for (&p, &v) in &self.peer_overrides {
            w.u64(serialize_peer_id(p));
            w.0.push(v as i8 as u8);
        }
        w.0
    }
    pub fn set_from_serialized(&mut self, bytes: &[u8]) -> bool {
        let Some(&version) = bytes.first() else {
            return false;
        };
        if !(1..=3).contains(&version) {
            return false;
        }
        let mut r = Reader::new(&bytes[1..]);
        let mut temp = Self::default();
        // Original data_auto_download.cpp Full::setFromSerialized does not check
        // stream.status after limits. QDataStream consumes a short read and zeros
        // subsequent integers; a truncated tail can therefore be accepted.
        let mut failed = false;
        for s in SOURCES {
            for t in TYPES {
                let n = if failed {
                    0
                } else {
                    match r.i32() {
                        Some(n) => n,
                        None => {
                            failed = true;
                            r.position = r.data.len();
                            0
                        }
                    }
                };
                if !temp.data[s as usize].set_from_serialized(t, n) {
                    return false;
                }
            }
        }
        if version == 1 {
            for s in SOURCES {
                for t in AUTO_PLAY_TYPES {
                    temp.set_bytes_limit(s, t, temp.bytes_limit(s, t).max(DEFAULT_AUTO_PLAY_SIZE));
                }
            }
        }
        if version >= 3 && !r.at_end() {
            let Some(count) = r.i32() else {
                return false;
            };
            if count < 0 {
                return false;
            }
            for _ in 0..count {
                let Some(p) = r.u64() else {
                    return false;
                };
                let Some(v) = r.raw(1).map(|s| s[0]) else {
                    return false;
                };
                if let Some(v) = match v {
                    1 => Some(Override::ForceAllow),
                    2 => Some(Override::ForceDeny),
                    _ => None,
                } {
                    temp.set_peer_override(deserialize_peer_id(p), v);
                }
            }
        }
        *self = temp;
        true
    }
    pub fn full_disabled() -> Self {
        let mut r = Self::default();
        for s in SOURCES {
            set_disabled_for_source(&mut r, s);
        }
        r
    }
    /// Applies only the legacy zero-limit effect; no SessionSettings stand-in.
    /// Other SessionEffects remain owned by the actual settings lifecycle.
    pub fn apply_settings_effect(
        &mut self,
        effect: &crate::storage::details::storage_settings_scheme::SessionEffect,
    ) -> bool {
        use crate::storage::details::storage_settings_scheme::{
            AutoDownloadSource as S, AutoDownloadType as T, SessionEffect,
        };
        let SessionEffect::AutoDownloadBytesLimit(s, t) = effect else {
            return false;
        };
        let s = match s {
            S::User => Source::User,
            S::Group => Source::Group,
            S::Channel => Source::Channel,
        };
        let t = match t {
            T::Photo => Type::Photo,
            T::VoiceMessage => Type::VoiceMessage,
            T::AutoPlayGIF => Type::AutoPlayGIF,
            T::AutoPlayVideoMessage => Type::AutoPlayVideoMessage,
            T::AutoPlayVideo => Type::AutoPlayVideo,
        };
        self.set_bytes_limit(s, t, 0);
        true
    }
    fn force_allowed(&self, s: Source, t: Type, n: i64) -> bool {
        if STREAMED_TYPES.contains(&t) {
            return false;
        }
        let own = self.bytes_limit(s, t);
        let limit = if own > 0 { own } else { default_limit(s, t) };
        limit > 0 && n <= limit
    }
}
pub fn set_defaults_for_source(data: &mut Full, s: Source) {
    for t in TYPES {
        data.set_bytes_limit(s, t, default_limit(s, t));
    }
}
pub fn set_disabled_for_source(data: &mut Full, s: Source) {
    for t in TYPES {
        data.set_bytes_limit(s, t, 0);
    }
}
pub fn has_enabled_types(data: &Full, s: Source) -> bool {
    TYPES
        .into_iter()
        .any(|t| !STREAMED_TYPES.contains(&t) && data.bytes_limit(s, t) > 0)
}
pub fn source_from_peer(peer: &PeerData, fields: &PeerFields<'_>) -> Source {
    if peer.is_user() {
        Source::User
    } else if peer.is_chat() || peer.is_megagroup(fields) {
        Source::Group
    } else {
        Source::Channel
    }
}
fn auto_play_type(doc: &DocumentData) -> Type {
    if doc.is_video_file() {
        Type::AutoPlayVideo
    } else if doc.is_video_message() {
        Type::AutoPlayVideoMessage
    } else {
        Type::AutoPlayGIF
    }
}
pub fn should_from_source(data: &Full, s: Source, doc: &DocumentData) -> bool {
    if doc.sticker().is_some() || doc.is_gifv() {
        true
    } else if doc.is_voice_message()
        || doc.is_video_message()
        || doc.is_song()
        || doc.is_video_file()
    {
        false
    } else {
        data.should_download(s, Type::File, doc.size)
    }
}
pub fn should_document(
    data: &Full,
    peer: &PeerData,
    fields: &PeerFields<'_>,
    doc: &DocumentData,
) -> bool {
    if doc.sticker().is_some() {
        return true;
    }
    let v = data.peer_override(peer.id());
    let s = source_from_peer(peer, fields);
    if v == Override::ForceDeny {
        false
    } else if doc.is_gifv() {
        true
    } else if v == Override::ForceAllow {
        !(doc.is_voice_message() || doc.is_video_message() || doc.is_song() || doc.is_video_file())
            && data.force_allowed(s, Type::File, doc.size)
    } else {
        should_from_source(data, s, doc)
    }
}
pub fn should_document_any_source(data: &Full, doc: &DocumentData) -> bool {
    SOURCES
        .into_iter()
        .any(|s| should_from_source(data, s, doc))
}
pub fn should_photo(
    data: &Full,
    peer: &PeerData,
    fields: &PeerFields<'_>,
    photo: &PhotoData,
) -> bool {
    let s = source_from_peer(peer, fields);
    let n = i64::from(photo.image_byte_size(PhotoSize::Large));
    match data.peer_override(peer.id()) {
        Override::ForceDeny => false,
        Override::ForceAllow => data.force_allowed(s, Type::Photo, n),
        Override::Default => data.should_download(s, Type::Photo, n),
    }
}
pub fn should_auto_play_document(
    data: &Full,
    peer: &PeerData,
    fields: &PeerFields<'_>,
    doc: &DocumentData,
) -> bool {
    if doc.sticker().is_some() {
        return true;
    }
    let s = source_from_peer(peer, fields);
    let t = auto_play_type(doc);
    match data.peer_override(peer.id()) {
        Override::ForceDeny => false,
        Override::ForceAllow => data.force_allowed(s, t, doc.size),
        Override::Default => data.should_download(s, t, doc.size),
    }
}
pub fn should_auto_play_photo(
    data: &Full,
    peer: &PeerData,
    fields: &PeerFields<'_>,
    photo: &PhotoData,
) -> bool {
    if !photo.has_video() {
        return false;
    }
    let s = source_from_peer(peer, fields);
    let n = i64::from(photo.video_byte_size(PhotoSize::Large));
    match data.peer_override(peer.id()) {
        Override::ForceDeny => false,
        Override::ForceAllow => AUTO_PLAY_TYPES
            .into_iter()
            .any(|t| data.force_allowed(s, t, n)),
        Override::Default => AUTO_PLAY_TYPES
            .into_iter()
            .any(|t| data.should_download(s, t, n)),
    }
}
pub fn with_disabled_auto_play(data: &Full) -> Full {
    let mut r = data.clone();
    for s in SOURCES {
        for t in AUTO_PLAY_TYPES {
            r.set_bytes_limit(s, t, 0);
        }
    }
    r.peer_overrides.retain(|_, v| *v != Override::ForceAllow);
    r
}
