//! Qt_5_1 document persistence, including pre-tag and v0..v6 records.
//! Disk DTOs retain null/empty UTF-16 and bytes; no artificial session model.
use super::{
    serialize_common::{ByteArrayReader as Reader, ByteArrayWriter as Writer},
    serialize_location::StoredImage,
    serialize_peer::{qt_string, text, QtString},
};
use crate::data::{
    data_document::{DocumentData, StickersType},
    data_media_types::DocumentType,
};
pub const VERSION_TAG: i32 = i32::MAX;
pub const VERSION: i32 = 6;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StickerRecord {
    pub alt: QtString,
    pub set_type: i32,
    pub premium: i32,
    pub use_text_color: i32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentRecord {
    pub id: u64,
    pub access: u64,
    pub date: i32,
    pub file_reference: Option<Vec<u8>>,
    pub version: i32,
    pub name: QtString,
    pub mime: QtString,
    pub dc: i32,
    pub size: u32,
    pub width: i32,
    pub height: i32,
    pub kind: i32,
    pub sticker: Option<StickerRecord>,
    pub duration: i64,
    pub thumbnail: StoredImage,
    pub thumbnail_byte_size: i32,
    pub video_thumbnail: StoredImage,
    pub video_thumbnail_byte_size: i32,
    pub inline_thumbnail_is_path: i32,
    pub inline_thumbnail: Option<Vec<u8>>,
}
#[derive(Clone, Debug)]
pub struct StickerSetInfo {
    pub id: u64,
    pub access_hash: u64,
    pub short_name: String,
}
impl DocumentRecord {
    pub fn from_document(d: &DocumentData) -> Option<Self> {
        Some(Self {
            id: d.id(),
            access: d.access_hash,
            date: d.date,
            file_reference: (!d.file_reference.is_empty()).then(|| d.file_reference.clone()),
            version: VERSION,
            name: qt_string(d.filename()),
            mime: qt_string(d.mime_string()),
            dc: d.dc_id(),
            size: d.size as u32,
            width: d.dimensions.width,
            height: d.dimensions.height,
            kind: d.kind as i32,
            sticker: d.sticker().map(|s| StickerRecord {
                alt: qt_string(&s.alt),
                set_type: match s.set_type {
                    StickersType::Emoji => 3,
                    StickersType::Masks => 4,
                    StickersType::Stickers if s.set.id != 0 => 1,
                    _ => 0,
                },
                premium: i32::from(d.is_premium_sticker() || d.is_premium_emoji()),
                use_text_color: i32::from(d.uses_text_color()),
            }),
            duration: if d.has_duration() { d.duration() } else { -1 },
            thumbnail: StoredImage::from_location(d.thumbnail_location())?,
            thumbnail_byte_size: d.thumbnail_byte_size(),
            video_thumbnail: StoredImage::from_location(d.video_thumbnail_location())?,
            video_thumbnail_byte_size: d.video_thumbnail_byte_size(),
            inline_thumbnail_is_path: i32::from(d.inline_thumbnail_is_path()),
            inline_thumbnail: (!d.inline_thumbnail_bytes().is_empty())
                .then(|| d.inline_thumbnail_bytes().to_vec()),
        })
    }
    pub fn write_to_stream(&self, w: &mut Writer) -> Option<()> {
        if (self.kind == DocumentType::Sticker as i32) != self.sticker.is_some() {
            return None;
        }
        w.u64(self.id);
        w.u64(self.access);
        w.i32(self.date);
        w.nullable_bytes(self.file_reference.as_deref());
        w.i32(VERSION_TAG);
        w.i32(VERSION);
        w.nullable_utf16(self.name.as_deref());
        w.nullable_utf16(self.mime.as_deref());
        for value in [
            self.dc,
            self.size as i32,
            self.width,
            self.height,
            self.kind,
        ] {
            w.i32(value);
        }
        if let Some(s) = &self.sticker {
            w.nullable_utf16(s.alt.as_deref());
            w.i32(s.set_type);
        }
        w.u64(self.duration as u64);
        if let Some(s) = &self.sticker {
            w.i32(s.premium);
            w.i32(s.use_text_color);
        }
        self.thumbnail.write(w);
        w.i32(self.thumbnail_byte_size);
        self.video_thumbnail.write(w);
        w.i32(self.video_thumbnail_byte_size);
        w.i32(self.inline_thumbnail_is_path);
        w.nullable_bytes(self.inline_thumbnail.as_deref());
        Some(())
    }
    /// Original reserve estimate (not actual written length). Source omits the
    /// modern 64-bit duration and sticker premium/text-color fields, cpp:277.
    pub fn size_in_stream(&self) -> usize {
        let string = |s: &QtString| 4 + s.as_ref().map_or(0, |s| s.len() * 2);
        20 + 4
            + self.file_reference.as_ref().map_or(0, Vec::len)
            + 8
            + string(&self.name)
            + string(&self.mime)
            + 20
            + self.sticker.as_ref().map_or(4, |s| string(&s.alt) + 4)
            + self.thumbnail.size()
            + 4
            + self.video_thumbnail.size()
            + 4
            + 4
            + 4
            + self.inline_thumbnail.as_ref().map_or(0, Vec::len)
    }
    pub fn read_from_stream(
        app: i32,
        r: &mut Reader<'_>,
        info: Option<&StickerSetInfo>,
    ) -> Option<Self> {
        let id = r.u64()?;
        let access = r.u64()?;
        let date = r.i32()?;
        let file_reference = if app >= 1003013 {
            r.nullable_bytes()?
        } else {
            None
        };
        let version = if app >= 9061 && r.i32()? == VERSION_TAG {
            r.i32()?
        } else {
            0
        };
        let name = r.nullable_utf16()?;
        let mime = r.nullable_utf16()?;
        let dc = r.i32()?;
        let size = r.i32()? as u32;
        let width = r.i32()?;
        let height = r.i32()?;
        let kind = r.i32()?;
        let mut duration = -1;
        let sticker = if kind == DocumentType::Sticker as i32 {
            let alt = r.nullable_utf16()?;
            let set_type = r.i32()?;
            let mut premium = 0;
            let mut use_text_color = 0;
            if version >= 6 {
                duration = r.u64()? as i64;
                premium = r.i32()?;
                use_text_color = r.i32()?;
            } else if version >= 3 {
                duration = legacy_duration(r.i32()?);
                if version >= 4 {
                    premium = r.i32()?;
                    if version >= 5 {
                        use_text_color = r.i32()?;
                    }
                }
            }
            if set_type == 3 && version < 5 && info.is_some_and(|i| !special_set(i.id)) {
                return None;
            }
            Some(StickerRecord {
                alt,
                set_type,
                premium,
                use_text_color,
            })
        } else {
            duration = if version >= 6 {
                r.u64()? as i64
            } else {
                legacy_duration(r.i32()?)
            };
            None
        };
        let thumbnail = StoredImage::read(app, r)?;
        let (thumbnail_byte_size, video_thumbnail, video_thumbnail_byte_size) = if version >= 1 {
            (r.i32()?, StoredImage::read(app, r)?, r.i32()?)
        } else {
            (0, StoredImage::default(), 0)
        };
        let (inline_thumbnail_is_path, inline_thumbnail) = if version >= 2 {
            (r.i32()?, r.nullable_bytes()?)
        } else {
            (0, None)
        };
        let thumb = thumbnail.location()?;
        if dc == 0 && access == 0 {
            return None;
        }
        if thumb.valid()
            && !matches!(&thumb.file,crate::data::data_image_location::DownloadLocation::Storage(s) if s.kind==crate::data::data_image_location::StorageFileType::Document && s.size_letter!=0)
        {
            return None;
        }
        Some(Self {
            id,
            access,
            date,
            file_reference,
            version,
            name,
            mime,
            dc,
            size,
            width,
            height,
            kind,
            sticker,
            duration,
            thumbnail,
            thumbnail_byte_size,
            video_thumbnail,
            video_thumbnail_byte_size,
            inline_thumbnail_is_path,
            inline_thumbnail,
        })
    }
    /// Attributes follow readFromStreamHelper, rather than restoring lossy
    /// song/voice fields which were never persisted by the original.
    pub fn attributes(
        &self,
        info: Option<&StickerSetInfo>,
    ) -> Option<Vec<tg_scheme::DocumentAttribute>> {
        use tg_scheme::*;
        let mut out = Vec::new();
        let name = text(&self.name)?;
        if !name.is_empty() {
            out.push(mtp_document_attribute_filename(name.into_bytes()));
        }
        if let Some(s) = &self.sticker {
            let alt = text(&s.alt)?;
            if s.set_type == 0 {
                out.push(mtp_document_attribute_sticker(
                    tg_scheme::tl::Flags::from_raw(0),
                    alt.into_bytes(),
                    mtp_input_sticker_set_empty(),
                    None,
                ));
            } else if let Some(i) = info {
                let kind = if special_set(i.id) { 0 } else { s.set_type };
                let set = if matches!(kind, 1 | 3 | 4) {
                    mtp_input_sticker_set_id(i.id, i.access_hash)
                } else {
                    mtp_input_sticker_set_empty()
                };
                if kind == 3 {
                    if self.version < 5 {
                        return None;
                    }
                    out.push(mtp_document_attribute_custom_emoji(
                        tg_scheme::tl::Flags::from_raw(
                            u64::from(s.premium == 0) | (u64::from(s.use_text_color != 0) << 1),
                        ),
                        alt.into_bytes(),
                        set,
                    ));
                } else {
                    out.push(mtp_document_attribute_sticker(
                        tg_scheme::tl::Flags::from_raw(if kind == 4 { 2 } else { 0 }),
                        alt.into_bytes(),
                        set,
                        None,
                    ));
                }
            }
        } else if self.kind == DocumentType::Animated as i32 {
            out.push(mtp_document_attribute_animated());
        }
        if self.width > 0 && self.height > 0 {
            if self.duration >= 0 {
                out.push(mtp_document_attribute_video(
                    tg_scheme::tl::Flags::from_raw(
                        if self.kind == DocumentType::RoundVideo as i32 {
                            1
                        } else {
                            0
                        },
                    ),
                    self.duration as f64 / 1000.,
                    self.width,
                    self.height,
                    None,
                    None,
                    None,
                ));
            } else {
                out.push(mtp_document_attribute_image_size(self.width, self.height));
            }
        }
        Some(out)
    }
}
fn legacy_duration(value: i32) -> i64 {
    if value < 0 {
        i64::from(value)
    } else {
        i64::from(value.wrapping_mul(1000))
    }
}
fn special_set(id: u64) -> bool {
    matches!(
        id,
        0 | 0xffff_ffff_ffff_ffff
            | 0xffff_ffff_ffff_fffc
            | 0xffff_ffff_ffff_fff9
            | 0xffff_ffff_ffff_fffa
            | 0xffff_ffff_ffff_fff8
    )
}

impl DocumentRecord {
    pub fn apply_to_document(
        &self,
        document: &mut DocumentData,
        info: Option<&StickerSetInfo>,
        database: &impl crate::core::mime_type::MimeDatabase,
        files: &dyn crate::core::file_location::FileLocations,
    ) -> Option<Vec<crate::data::data_document::DocumentEffect>> {
        if document.id() != self.id {
            return None;
        }
        let attributes = self.attributes(info)?;
        let mime = text(&self.mime)?;
        let thumbnail = crate::data::data_media_location::ImageWithLocation {
            location: self.thumbnail.location()?,
            bytes_count: self.thumbnail_byte_size,
            ..Default::default()
        };
        let video = crate::data::data_media_location::ImageWithLocation {
            location: self.video_thumbnail.location()?,
            bytes_count: self.video_thumbnail_byte_size,
            ..Default::default()
        };
        if self.date == 0 {
            return Some(Vec::new());
        }
        document.date = self.date;
        document.set_mime_string(&mime);
        let mut effects = document.update_thumbnails(
            self.inline_thumbnail.as_deref().unwrap_or_default(),
            self.inline_thumbnail_is_path == 1,
            &thumbnail,
            &video,
            self.sticker.as_ref().is_some_and(|s| s.premium == 1),
        );
        document.size = i64::from(self.size);
        effects.extend(document.set_attributes(&attributes, database));
        document.recount_is_image();
        if self.dc != 0 && self.access != 0 {
            effects.extend(document.set_remote_location(
                self.dc,
                self.access,
                self.file_reference.as_deref().unwrap_or_default(),
                files,
            ));
        }
        Some(effects)
    }
}
