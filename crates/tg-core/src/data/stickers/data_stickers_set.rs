use crate::data::{
    data_document::{sticker_set_identifier, DocumentData, StickerType, StickersType},
    data_image_location::*,
    data_media_location::*,
    data_media_store::Documents,
    data_store_keys::*,
    data_types::StickerSetIdentifier,
};
use tg_scheme as tl;
#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StickersSetFlag {
    Installed = 1,
    Archived = 2,
    Masks = 4,
    Official = 8,
    NotLoaded = 16,
    Featured = 32,
    Unread = 64,
    Special = 128,
    Emoji = 512,
    TextColor = 1024,
    ChannelStatus = 2048,
    AmCreator = 4096,
}
tg_base::flag_type!(StickersSetFlag:u16);
pub type StickersSetFlags = tg_base::flags::Flags<StickersSetFlag>;
pub type StickersSetsOrder = Vec<u64>;
pub type SavedGifs = Vec<DocumentRecordKey>;
pub type StickersPack = Vec<DocumentRecordKey>;
pub fn parse_stickers_set_flags(d: &tl::StickerSetData) -> StickersSetFlags {
    use StickersSetFlag as F;
    let mut r = StickersSetFlags::empty();
    for (flag, yes) in [
        (F::Installed, d.installed_date.is_some()),
        (F::Archived, d.is_archived()),
        (F::Official, d.is_official()),
        (F::Masks, d.is_masks()),
        (F::Emoji, d.is_emojis()),
        (F::TextColor, d.is_text_color()),
        (F::ChannelStatus, d.is_channel_emoji_status()),
        (F::AmCreator, d.is_creator()),
    ] {
        if yes {
            r |= flag;
        }
    }
    r
}
pub struct StickersSet {
    owner: SessionScopeId,
    pub id: u64,
    pub access_hash: u64,
    pub hash: u64,
    pub thumbnail_document_id: u64,
    pub title: String,
    pub short_name: String,
    pub count: i32,
    pub locked: i32,
    pub flags: StickersSetFlags,
    pub install_date: i32,
    pub covers: StickersPack,
    pub stickers: StickersPack,
    pub dates: Vec<i32>,
    thumbnail: CloudFileMetadata,
    thumbnail_type: StickerType,
}
impl StickersSet {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        owner: &SessionScope,
        id: u64,
        access_hash: u64,
        hash: u64,
        title: String,
        short_name: String,
        count: i32,
        flags: StickersSetFlags,
        install_date: i32,
    ) -> Self {
        Self {
            owner: owner.id(),
            id,
            access_hash,
            hash,
            title,
            short_name,
            count,
            flags,
            install_date,
            thumbnail_document_id: 0,
            locked: 0,
            covers: vec![],
            stickers: vec![],
            dates: vec![],
            thumbnail: Default::default(),
            thumbnail_type: Default::default(),
        }
    }
    pub fn owner(&self) -> SessionScopeId {
        self.owner
    }
    pub fn session(&self) -> SessionScopeId {
        self.owner
    }
    pub fn mtp_input(&self) -> tl::InputStickerSet {
        if self.id != 0 && self.access_hash != 0 {
            tl::mtp_input_sticker_set_id(self.id, self.access_hash)
        } else {
            tl::mtp_input_sticker_set_short_name(self.short_name.as_bytes().to_vec())
        }
    }
    pub fn identifier(&self) -> StickerSetIdentifier {
        StickerSetIdentifier {
            id: self.id,
            access_hash: self.access_hash,
            ..Default::default()
        }
    }
    pub fn kind(&self) -> StickersType {
        if (self.flags & StickersSetFlag::Emoji).any() {
            StickersType::Emoji
        } else if (self.flags & StickersSetFlag::Masks).any() {
            StickersType::Masks
        } else {
            StickersType::Stickers
        }
    }
    pub fn text_color(&self) -> bool {
        (self.flags & StickersSetFlag::TextColor).any()
    }
    pub fn channel_status(&self) -> bool {
        (self.flags & StickersSetFlag::ChannelStatus).any()
    }
    /// Model/cache metadata phase only. Actual loader/pixels remain storage3/4,UI5.
    pub fn set_thumbnail(
        &mut self,
        data: &ImageWithLocation,
        kind: StickerType,
    ) -> CloudMetadataUpdate {
        self.thumbnail_type = kind;
        self.thumbnail.update(data)
    }
    pub fn has_thumbnail(&self) -> bool {
        self.thumbnail.location.valid()
    }
    pub fn thumbnail_type(&self) -> StickerType {
        self.thumbnail_type
    }
    pub fn thumbnail_location(&self) -> &ImageLocation {
        &self.thumbnail.location
    }
    pub fn thumbnail_byte_size(&self) -> i32 {
        self.thumbnail.byte_size
    }
    pub fn thumbnail_big_file_base_cache_key(&self) -> CacheKey {
        match &self.thumbnail.location.file {
            DownloadLocation::Storage(s) => s.big_file_base_cache_key(),
            _ => CacheKey::default(),
        }
    }
    pub fn lookup_thumbnail_document(&self, docs: &Documents) -> Option<DocumentRecordKey> {
        if self.thumbnail_document_id != 0 {
            if let Some(k) = self.stickers.iter().find(|&&k| {
                assert_eq!(k.scope(), self.owner);
                docs.read(k, DocumentData::id).expect("document lifetime")
                    == self.thumbnail_document_id
            }) {
                return Some(*k);
            }
        }
        self.stickers
            .first()
            .or_else(|| self.covers.first())
            .copied()
    }
}
pub fn input_sticker_set(id: StickerSetIdentifier) -> tl::InputStickerSet {
    if id.empty() {
        tl::mtp_input_sticker_set_empty()
    } else if id.id != 0 {
        tl::mtp_input_sticker_set_id(id.id, id.access_hash)
    } else {
        tl::mtp_input_sticker_set_short_name(id.short_name.into_bytes())
    }
}
pub fn from_input_set(id: &tl::InputStickerSet) -> StickerSetIdentifier {
    sticker_set_identifier(id)
}
