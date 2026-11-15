//! Persistence-only slice of ui/image/image_location.cpp, needed by peer/document codecs.
//! Uses the existing data locations; no decoded pixels or loader objects.
use super::serialize_common::{ByteArrayReader as Reader, ByteArrayWriter as Writer};
use crate::data::{data_image_location::*, data_peer_id::*};

pub const MODERN_IMAGE_TAG: i32 = i32::MIN;
fn byte(r: &mut Reader<'_>) -> Option<u8> {
    Some(*r.raw(1)?.first()?)
}
fn short(r: &mut Reader<'_>) -> Option<u16> {
    Some(u16::from_be_bytes(r.raw(2)?.try_into().ok()?))
}

pub fn write_storage_file(location: &StorageFileLocation) -> Vec<u8> {
    if !location.valid() {
        return Vec::new();
    }
    let mut w = Writer::default();
    w.0.extend(location.dc_id.to_be_bytes());
    let in_message = location.in_message_peer_id.value != 0;
    w.0.push(location.kind as u8 | 0x20 | if in_message { 0x40 } else { 0 });
    w.0.push(location.size_letter);
    w.i32(if in_message {
        (location.in_message_peer_id.value >> 32) as i32
    } else {
        location.local_id
    });
    w.u64(location.id);
    w.u64(location.access_hash);
    w.u64(location.volume_id);
    w.i32(if in_message {
        location.in_message_peer_id.value as i32
    } else {
        0
    });
    w.i32(location.in_message_id as i32);
    w.nullable_bytes(
        (!location.file_reference.is_empty()).then_some(location.file_reference.as_slice()),
    );
    w.0
}
pub fn read_storage_file(bytes: &[u8]) -> Option<StorageFileLocation> {
    if bytes.is_empty() {
        return Some(StorageFileLocation::default());
    }
    let mut r = Reader::new(bytes);
    let dc_id = short(&mut r)?;
    let mut flags = byte(&mut r)?;
    if flags == 0x10 {
        return None;
    }
    let size_letter = byte(&mut r)?;
    let field1 = r.i32()?;
    let id = r.u64()?;
    let access_hash = r.u64()?;
    let volume_id = r.u64()?;
    let modern = flags & 0x20 != 0;
    let in_message = flags & 0x40 != 0;
    let (field2, msg) = if modern || flags & 8 != 0 {
        (r.i32()?, r.i32()? as u32)
    } else {
        (0, 0)
    };
    if modern {
        flags &= !0x20;
        if in_message {
            flags &= !0x40;
        }
    } else {
        flags &= !8;
    }
    let kind = match flags {
        0 => StorageFileType::Legacy,
        1 => StorageFileType::Encrypted,
        2 => StorageFileType::Document,
        3 => StorageFileType::Secure,
        4 => StorageFileType::Takeout,
        5 => StorageFileType::Photo,
        6 => StorageFileType::PeerPhoto,
        7 => StorageFileType::StickerSetThumb,
        8 => StorageFileType::GroupCallStream,
        _ => return None,
    };
    let mut result = StorageFileLocation {
        dc_id,
        kind,
        size_letter,
        local_id: field1,
        id,
        access_hash,
        volume_id,
        in_message_id: msg,
        file_reference: r.bytes()?,
        ..Default::default()
    };
    if modern {
        if in_message {
            result.local_id = 0;
            result.in_message_peer_id = PeerId {
                value: (u64::from(field1 as u32) << 32) | u64::from(field2 as u32),
            };
        }
    } else if kind == StorageFileType::PeerPhoto {
        result.id = deserialize_peer_id(id).value;
        result.local_id = 0;
        result.in_message_peer_id = if field2 > 0 {
            peer_from_user(UserId::new(field2 as u64))
        } else if field2 < 0 {
            peer_from_channel(ChannelId::new(i64::from(field2).unsigned_abs()))
        } else {
            PeerId::default()
        };
    }
    if kind == StorageFileType::StickerSetThumb && volume_id != 0 {
        return None;
    }
    result.valid().then_some(result)
}
pub fn write_download_location(location: &DownloadLocation) -> Option<Vec<u8>> {
    if let DownloadLocation::Storage(s) = location {
        return Some(write_storage_file(s));
    }
    // C++ std::get<StorageFileLocation> on an invalid non-storage variant is UB.
    if !location.valid() {
        return None;
    }
    let mut w = Writer(vec![0, 0, 0x10]);
    match location {
        DownloadLocation::Storage(_) => unreachable!(),
        DownloadLocation::Web { access_hash, url } => {
            w.0.push(0);
            w.bytes(url);
            w.u64(*access_hash);
        }
        DownloadLocation::Geo(g) => {
            w.0.push(1);
            w.u64(g.lat.to_bits());
            w.u64(g.lon.to_bits());
            w.u64(g.access);
            for v in [g.width, g.height, g.zoom, g.scale] {
                w.i32(v);
            }
        }
        DownloadLocation::Url(url) => {
            w.0.push(2);
            w.bytes(url.as_bytes());
        }
        DownloadLocation::InMemory(bytes) => {
            w.0.push(3);
            w.bytes(bytes);
        }
        DownloadLocation::AudioAlbumThumb { document_id } => {
            w.0.push(4);
            w.u64(*document_id);
        }
    }
    Some(w.0)
}
pub fn read_download_location(bytes: &[u8]) -> Option<DownloadLocation> {
    if bytes.get(..3) != Some(&[0, 0, 0x10]) {
        return Some(DownloadLocation::Storage(read_storage_file(bytes)?));
    }
    let mut r = Reader::new(&bytes[3..]);
    Some(match byte(&mut r)? {
        0 => DownloadLocation::Web {
            url: r.bytes()?,
            access_hash: r.u64()?,
        },
        1 => DownloadLocation::Geo(GeoPointLocation {
            lat: f64::from_bits(r.u64()?),
            lon: f64::from_bits(r.u64()?),
            access: r.u64()?,
            width: r.i32()?,
            height: r.i32()?,
            zoom: r.i32()?,
            scale: r.i32()?,
        }),
        2 => DownloadLocation::Url(String::from_utf8_lossy(&r.bytes()?).into_owned()),
        3 => DownloadLocation::InMemory(r.bytes()?),
        4 => DownloadLocation::AudioAlbumThumb {
            document_id: r.u64()?,
        },
        _ => return None,
    })
}
pub fn encode_image_location(location: &ImageLocation) -> Option<Vec<u8>> {
    let mut bytes = write_download_location(&location.file)?;
    if !bytes.is_empty() || location.width > 0 || location.height > 0 {
        bytes.extend(location.width.to_be_bytes());
        bytes.extend(location.height.to_be_bytes());
    }
    Some(bytes)
}
pub fn decode_image_location(bytes: &[u8]) -> Option<ImageLocation> {
    let file = read_download_location(bytes)?;
    if bytes.is_empty() {
        return Some(ImageLocation::new(file, 0, 0));
    }
    let mut r = Reader::new(bytes.get(bytes.len().checked_sub(8)?..)?);
    Some(ImageLocation::new(file, r.i32()?, r.i32()?))
}
pub fn write_image_location(w: &mut Writer, location: &ImageLocation) -> Option<()> {
    let bytes = encode_image_location(location)?;
    w.i32(MODERN_IMAGE_TAG);
    w.nullable_bytes((!bytes.is_empty()).then_some(bytes.as_slice()));
    Some(())
}
pub fn image_location_size(location: &ImageLocation) -> Option<usize> {
    Some(8 + encode_image_location(location)?.len())
}
pub fn read_image_location(app_version: i32, r: &mut Reader<'_>) -> Option<ImageLocation> {
    let width = r.i32()?;
    if width == MODERN_IMAGE_TAG {
        return decode_image_location(&r.bytes()?);
    }
    let height = r.i32()?;
    let dc = r.i32()?;
    let volume = r.u64()?;
    let local = r.i32()?;
    let secret = r.u64()?;
    let file_reference = if app_version >= 1003013 {
        r.bytes()?
    } else {
        Vec::new()
    };
    Some(ImageLocation::new(
        DownloadLocation::Storage(StorageFileLocation {
            dc_id: dc as u16,
            kind: StorageFileType::Legacy,
            volume_id: volume,
            local_id: local,
            access_hash: secret,
            file_reference,
            ..Default::default()
        }),
        width,
        height,
    ))
}
pub fn convert_to_modern_peer_photo(
    image: &ImageLocation,
    id: PeerId,
    access: u64,
    photo: u64,
) -> ImageLocation {
    let DownloadLocation::Storage(old) = &image.file else {
        return image.clone();
    };
    if !matches!(
        old.kind,
        StorageFileType::Legacy | StorageFileType::PeerPhoto
    ) {
        return image.clone();
    }
    let file = if photo == 0 {
        StorageFileLocation::default()
    } else {
        StorageFileLocation {
            kind: StorageFileType::PeerPhoto,
            id: id.value,
            access_hash: access,
            volume_id: photo,
            size_letter: b'a',
            local_id: 0,
            in_message_peer_id: PeerId::default(),
            in_message_id: 0,
            ..old.clone()
        }
    };
    ImageLocation::new(DownloadLocation::Storage(file), image.width, image.height)
}
/// Raw modern image record retains QByteArray nullness and file-reference bytes
/// exactly. The decoded model alone intentionally does not retain Qt nullness.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StoredImage(pub Option<Vec<u8>>);
impl StoredImage {
    pub fn from_location(location: &ImageLocation) -> Option<Self> {
        let bytes = encode_image_location(location)?;
        Some(Self((!bytes.is_empty()).then_some(bytes)))
    }
    pub fn location(&self) -> Option<ImageLocation> {
        decode_image_location(self.0.as_deref().unwrap_or_default())
    }
    pub fn write(&self, w: &mut Writer) {
        w.i32(MODERN_IMAGE_TAG);
        w.nullable_bytes(self.0.as_deref());
    }
    pub fn read(app: i32, r: &mut Reader<'_>) -> Option<Self> {
        let position = r.position;
        if r.i32()? == MODERN_IMAGE_TAG {
            let result = Self(r.nullable_bytes()?);
            result.location()?;
            Some(result)
        } else {
            r.position = position;
            Self::from_location(&read_image_location(app, r)?)
        }
    }
    pub fn size(&self) -> usize {
        8 + self.0.as_ref().map_or(0, Vec::len)
    }
}

pub fn storage_image_location_size(location: &ImageLocation) -> Option<usize> {
    if !matches!(location.file, DownloadLocation::Storage(_)) {
        return None;
    }
    image_location_size(location)
}
pub fn write_storage_image_location(w: &mut Writer, location: &ImageLocation) -> Option<()> {
    if !matches!(location.file, DownloadLocation::Storage(_)) {
        return None;
    }
    write_image_location(w, location)
}
pub fn read_storage_image_location(app: i32, r: &mut Reader<'_>) -> Option<ImageLocation> {
    let result = read_image_location(app, r)?;
    matches!(result.file, DownloadLocation::Storage(_)).then_some(result)
}
