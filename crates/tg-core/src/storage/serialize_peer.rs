//! Serialized peer records, not substitute UserData models. Nullable UTF-16 is
//! retained at the disk boundary; applying full users remains package02/29.
use super::serialize_common::{ByteArrayReader as Reader, ByteArrayWriter as Writer};
pub use super::serialize_location::{
    image_location_size, read_image_location, write_image_location,
};
use crate::data::data_peer_id::*;
pub const VERSION_TAG: u64 = 0x77ff_ffff_ffff_ffff;
pub const VERSION: i32 = 3;
pub type QtString = Option<Vec<u16>>;
pub fn qt_string(s: &str) -> QtString {
    Some(s.encode_utf16().collect())
}
pub fn text(s: &QtString) -> Option<String> {
    String::from_utf16(s.as_deref().unwrap_or_default()).ok()
}
#[derive(Clone, Debug, PartialEq)]
pub struct PeerRecord {
    pub id: PeerId,
    pub version: i32,
    pub photo_id: u64,
    pub userpic: super::serialize_location::StoredImage,
    pub photo_has_video: i32,
    pub fields: PeerFields,
}
#[derive(Clone, Debug, PartialEq)]
pub enum PeerFields {
    User {
        first: QtString,
        last: QtString,
        phone: QtString,
        username: QtString,
        access: u64,
        flags: u32,
        inline_placeholder: QtString,
        lastseen: u32,
        contact: i32,
        bot_info_version: i32,
        supports_guest_chat: i32,
    },
    Chat {
        name: QtString,
        count: i32,
        date: i32,
        version: i32,
        creator: UserId,
        old_forbidden: u8,
        flags: u32,
        invite_link: QtString,
    },
    Channel {
        name: QtString,
        access: u64,
        date: i32,
        version: i32,
        old_forbidden: i32,
        flags: u32,
        invite_link: QtString,
    },
}
pub fn write_peer(w: &mut Writer, p: &PeerRecord) -> Option<()> {
    if p.id.value == 0 || p.id.value & (0x80u64 << 48) != 0 {
        return None;
    }
    let consistent = match p.fields {
        PeerFields::User { .. } => peer_is_user(p.id),
        PeerFields::Chat { .. } => peer_is_chat(p.id),
        PeerFields::Channel { .. } => peer_is_channel(p.id),
    };
    if !consistent {
        return None;
    }
    w.u64(serialize_peer_id(p.id));
    w.u64(VERSION_TAG);
    w.i32(VERSION);
    w.u64(p.photo_id);
    p.userpic.write(w);
    w.i32(p.photo_has_video);
    match &p.fields {
        PeerFields::User {
            first,
            last,
            phone,
            username,
            access,
            flags,
            inline_placeholder,
            lastseen,
            contact,
            bot_info_version,
            supports_guest_chat,
        } => {
            for s in [first, last, phone, username] {
                w.nullable_utf16(s.as_deref());
            }
            w.u64(*access);
            w.i32(*flags as i32);
            w.nullable_utf16(inline_placeholder.as_deref());
            w.i32(*lastseen as i32);
            w.i32(*contact);
            w.i32(*bot_info_version);
            w.i32(*supports_guest_chat);
        }
        PeerFields::Chat {
            name,
            count,
            date,
            version,
            creator,
            flags,
            invite_link,
            ..
        } => {
            w.nullable_utf16(name.as_deref());
            for v in [
                *count,
                *date,
                *version,
                creator.bare as i32,
                (((creator.bare >> 32) as u32) << 8) as i32,
                *flags as i32,
            ] {
                w.i32(v);
            }
            w.nullable_utf16(invite_link.as_deref());
        }
        PeerFields::Channel {
            name,
            access,
            date,
            flags,
            invite_link,
            ..
        } => {
            w.nullable_utf16(name.as_deref());
            w.u64(*access);
            for v in [*date, 0, 0, *flags as i32] {
                w.i32(v);
            }
            w.nullable_utf16(invite_link.as_deref());
        }
    }
    Some(())
}
pub fn peer_size(p: &PeerRecord) -> Option<usize> {
    let mut w = Writer::default();
    write_peer(&mut w, p)?;
    Some(w.0.len())
}
pub fn read_peer(app: i32, r: &mut Reader<'_>) -> Option<PeerRecord> {
    let id = deserialize_peer_id(r.u64()?);
    let tag = r.u64()?;
    if id.value == 0 {
        return None;
    }
    let (version, photo_id) = if tag == VERSION_TAG {
        (r.i32()?, r.u64()?)
    } else {
        (0, tag)
    };
    let userpic = super::serialize_location::StoredImage::read(app, r)?;
    let photo_has_video = if version > 0 { r.i32()? } else { 0 };
    let fields = if peer_is_user(id) {
        PeerFields::User {
            first: r.nullable_utf16()?,
            last: r.nullable_utf16()?,
            phone: r.nullable_utf16()?,
            username: r.nullable_utf16()?,
            access: r.u64()?,
            flags: if app >= 9012 { r.i32()? as u32 } else { 0 },
            inline_placeholder: if app >= 9016 {
                r.nullable_utf16()?
            } else {
                None
            },
            lastseen: r.i32()? as u32,
            contact: r.i32()?,
            bot_info_version: r.i32()?,
            supports_guest_chat: if version > 2 { r.i32()? } else { 0 },
        }
    } else if peer_is_chat(id) {
        let name = r.nullable_utf16()?;
        let count = r.i32()?;
        let date = r.i32()?;
        let version = r.i32()?;
        let f1 = r.i32()? as u32;
        let f2 = r.i32()? as u32;
        PeerFields::Chat {
            name,
            count,
            date,
            version,
            creator: UserId::new(u64::from(f1) | (u64::from(f2 >> 8) << 32)),
            old_forbidden: f2 as u8,
            flags: r.i32()? as u32,
            invite_link: r.nullable_utf16()?,
        }
    } else if peer_is_channel(id) {
        PeerFields::Channel {
            name: r.nullable_utf16()?,
            access: r.u64()?,
            date: r.i32()?,
            version: r.i32()?,
            old_forbidden: r.i32()?,
            flags: r.i32()? as u32,
            invite_link: r.nullable_utf16()?,
        }
    } else {
        return None;
    };
    Some(PeerRecord {
        id,
        version,
        photo_id,
        userpic,
        photo_has_video,
        fields,
    })
}
/// Consumes exactly the same prefix as C++; it does not consume user fields after phone.
pub fn peek_user_phone(app: i32, r: &mut Reader<'_>) -> Option<QtString> {
    let id = deserialize_peer_id(r.u64()?);
    let tag = r.u64()?;
    if id.value == 0 || !peer_is_user(id) {
        return None;
    }
    let version = if tag == VERSION_TAG {
        let v = r.i32()?;
        r.u64()?;
        v
    } else {
        0
    };
    read_image_location(app, r)?;
    if version > 0 {
        r.i32()?;
    }
    r.nullable_utf16()?;
    r.nullable_utf16()?;
    r.nullable_utf16()
}
/// The original conversions replace only their masks, retaining other current bits.
pub fn convert_legacy_flags(current: u64, saved: u32, pairs: &[(u8, u8)]) -> u64 {
    pairs.iter().fold(current, |out, &(from, to)| {
        (out & !(1u64 << to))
            | if saved & (1u32 << from) != 0 {
                1u64 << to
            } else {
                0
            }
    })
}

pub const USER_FLAG_CONVERSIONS: &[(u8, u8)] = &[
    (13, 2),
    (17, 3),
    (24, 4),
    (26, 5),
    (21, 6),
    (23, 10),
    (11, 0),
    (12, 1),
];
pub const CHAT_FLAG_CONVERSIONS: &[(u8, u8)] = &[(2, 0), (0, 2), (5, 3), (31, 4), (23, 5), (24, 6)];
pub const CHANNEL_FLAG_CONVERSIONS: &[(u8, u8)] = &[
    (5, 10),
    (7, 6),
    (19, 7),
    (25, 8),
    (8, 9),
    (26, 11),
    (6, 12),
    (11, 5),
    (20, 18),
    (22, 19),
    (23, 3),
    (24, 4),
    (31, 2),
    (2, 0),
    (0, 1),
];
/// Model-ready flags; channel serialization intentionally truncates high flags
/// to quint32, including AyuNoForwards at63, exactly writePeer.cpp.
pub fn restored_flags(record: &PeerRecord, app: i32, current: u64) -> u64 {
    let (saved, table, forbidden) = match &record.fields {
        PeerFields::User { flags, .. } => (*flags, USER_FLAG_CONVERSIONS, false),
        PeerFields::Chat {
            flags,
            old_forbidden,
            ..
        } => (*flags, CHAT_FLAG_CONVERSIONS, *old_forbidden == 1),
        PeerFields::Channel {
            flags,
            old_forbidden,
            ..
        } => (*flags, CHANNEL_FLAG_CONVERSIONS, *old_forbidden != 0),
    };
    if app >= 2008007 {
        return u64::from(saved);
    }
    let mut result = if matches!(record.fields, PeerFields::Chat { .. }) && app < 9012 {
        current | u64::from(saved == 1)
    } else {
        convert_legacy_flags(current, saved, table)
    };
    if forbidden {
        result |= if matches!(record.fields, PeerFields::Chat { .. }) {
            1 << 4
        } else {
            1 << 2
        };
    }
    result
}
pub fn restored_lastseen(
    record: &PeerRecord,
) -> Option<crate::data::data_lastseen_status::LastseenStatus> {
    let PeerFields::User { lastseen, .. } = record.fields else {
        return None;
    };
    Some(if record.version > 1 {
        crate::data::data_lastseen_status::LastseenStatus::from_serialized(lastseen)
    } else {
        crate::data::data_lastseen_status::LastseenStatus::from_legacy(lastseen as i32)
    })
}
/// Current Chat/Channel snapshots use real models. Users require actual02 fields;
/// no dummy UserData is synthesized to satisfy Serialize::writePeer.
pub fn snapshot_group(
    peers: &crate::data::data_session::Peers,
    key: crate::data::data_store_keys::PeerKey,
) -> Option<PeerRecord> {
    use crate::data::data_store_keys::{ChannelKey, ChatKey};
    let fields = if let Ok(k) = ChatKey::try_from(key) {
        peers.read_chat(k, |c| PeerFields::Chat {
            name: qt_string(c.peer.raw_name()),
            count: c.count,
            date: c.date,
            version: c.version(),
            creator: c.creator,
            old_forbidden: 0,
            flags: c.flags().value() as u32,
            invite_link: qt_string(c.invite_link()),
        })
    } else if let Ok(k) = ChannelKey::try_from(key) {
        peers.read_channel(k, |c| PeerFields::Channel {
            name: qt_string(c.peer.raw_name()),
            access: c.access_hash(),
            date: c.date,
            version: 0,
            old_forbidden: 0,
            flags: c.flags().value() as u32,
            invite_link: qt_string(c.invite_link()),
        })
    } else {
        return None;
    };
    peers.read(key, |p| {
        Some(PeerRecord {
            id: key.id(),
            version: VERSION,
            photo_id: p.userpic_photo_id(),
            userpic: super::serialize_location::StoredImage::from_location(p.userpic_location())?,
            photo_has_video: i32::from(p.userpic_has_video()),
            fields,
        })
    })
}
/// Own mutation phase. Name-index/fork policy comes from the real owner. The
/// normal/full-loaded short circuit still consumes bytes in read_peer first.
pub fn apply_group(
    peers: &crate::data::data_session::Peers,
    record: &PeerRecord,
    app: i32,
    filter_zalgo: bool,
    index: &impl Fn(crate::data::data_store_keys::PeerKey, &str) -> crate::data::data_session::NameIndex,
) -> Option<crate::data::data_store_keys::PeerKey> {
    use crate::data::{
        data_channel::ChannelDataFlags,
        data_chat::ChatDataFlags,
        data_peer::{LoadedStatus, PeerMutation},
        data_store_keys::{ChannelKey, ChatKey},
    };
    if matches!(record.fields, PeerFields::User { .. }) {
        return None;
    }
    let location = record.userpic.location()?;
    let (name, link) = match &record.fields {
        PeerFields::Chat {
            name, invite_link, ..
        }
        | PeerFields::Channel {
            name, invite_link, ..
        } => (text(name)?, text(invite_link)?),
        _ => return None,
    };
    if let Some(key) = peers.peer_loaded(record.id) {
        return Some(key);
    }
    let key = peers.ensure_peer(record.id);
    peers.update(key, |p| {
        p.set_loaded_status(LoadedStatus::Normal);
        PeerMutation::quiet(())
    });
    let access = match &record.fields {
        PeerFields::Chat {
            count,
            date,
            creator,
            ..
        } => {
            let k = ChatKey::try_from(key).ok()?;
            peers.set_chat_name(k, &name, filter_zalgo, || index(key, &name));
            peers.edit_chat(k, |c| {
                c.count = *count;
                c.date = *date;
                c.version = 0;
            });
            let flags = restored_flags(
                record,
                app,
                peers.read_chat(k, |c| c.flags().value() as u32 as u64),
            );
            peers.set_chat_flags(k, ChatDataFlags::from_raw(flags as i32));
            peers.edit_chat(k, |c| c.creator = *creator);
            peers.set_chat_invite_link(k, &link);
            0
        }
        PeerFields::Channel { access, date, .. } => {
            let k = ChannelKey::try_from(key).ok()?;
            peers.set_channel_name(k, &name, "", filter_zalgo, || index(key, &name));
            peers.set_channel_access_hash(k, *access);
            peers.edit_channel(k, |c| c.date = *date);
            let flags = restored_flags(record, app, peers.read_channel(k, |c| c.flags().value()));
            peers.set_channel_flags(k, ChannelDataFlags::from_raw(flags));
            peers.set_channel_invite_link(k, &link);
            *access
        }
        _ => return None,
    };
    let location = super::serialize_location::convert_to_modern_peer_photo(
        &location,
        record.id,
        access,
        record.photo_id,
    );
    peers.update(key, |p| {
        p.set_userpic(record.photo_id, location, record.photo_has_video == 1)
    });
    Some(key)
}
