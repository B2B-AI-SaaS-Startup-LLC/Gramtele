//! WallPaper data and exact Qt_5_1 persistence. Pixel ownership/rendering is5.
use super::{
    data_file_origin::{FileOrigin, FileOriginWallpaper},
    data_peer_colors::Color,
    data_peer_id::UserId,
    data_store_keys::*,
};
use std::collections::BTreeMap;
use tg_base::qt_stream::{Reader, Writer};
use tg_scheme as tl;
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WallPaperFlag {
    Pattern = 1,
    Default = 2,
    Creator = 4,
    Dark = 8,
}
tg_base::flag_type!(WallPaperFlag:i32);
pub type WallPaperFlags = tg_base::flags::Flags<WallPaperFlag>;
const VERSION_TAG: i32 = i32::MAX;
pub const DEFAULT_BACKGROUND: u64 = 5933856211186221059;
pub const LEGACY2_BACKGROUND: u64 = 5947530738516623361;
pub const LEGACY3_BACKGROUND: u64 = 5778236420632084488;
pub const LEGACY4_BACKGROUND: u64 = 5945087215657811969;
pub const fn from_legacy_background_id(id: i32) -> u64 {
    0xffffffff00000000 | (id as u32 as u64)
}
#[derive(Clone, Debug)]
pub struct WallPaper {
    id: u64,
    access_hash: u64,
    owner_id: UserId,
    flags: WallPaperFlags,
    /// QString nullness and unpaired UTF-16 are retained for exact persistence.
    slug: Option<Vec<u16>>,
    emoji_id: String,
    background_colors: Vec<Color>,
    rotation: i32,
    intensity: i32,
    blurred: bool,
    document: Option<DocumentRecordKey>,
}
impl WallPaper {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            access_hash: 0,
            owner_id: UserId::new(0),
            flags: WallPaperFlags::empty(),
            slug: None,
            emoji_id: String::new(),
            background_colors: vec![],
            rotation: 0,
            intensity: 50,
            blurred: false,
            document: None,
        }
    }
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn access_hash(&self) -> u64 {
        self.access_hash
    }
    pub fn owner_id(&self) -> UserId {
        self.owner_id
    }
    pub fn flags(&self) -> WallPaperFlags {
        self.flags
    }
    pub fn slug(&self) -> String {
        String::from_utf16_lossy(self.slug.as_deref().unwrap_or_default())
    }
    pub fn emoji_id(&self) -> &str {
        &self.emoji_id
    }
    pub fn background_colors(&self) -> &[Color] {
        &self.background_colors
    }
    pub fn document(&self) -> Option<DocumentRecordKey> {
        self.document
    }
    pub fn is_pattern(&self) -> bool {
        (self.flags & WallPaperFlag::Pattern).any()
    }
    pub fn is_default(&self) -> bool {
        (self.flags & WallPaperFlag::Default).any()
    }
    pub fn is_creator(&self) -> bool {
        (self.flags & WallPaperFlag::Creator).any()
    }
    pub fn is_dark(&self) -> bool {
        (self.flags & WallPaperFlag::Dark).any()
    }
    pub fn is_blurred(&self) -> bool {
        self.blurred
    }
    pub fn pattern_intensity(&self) -> i32 {
        self.intensity
    }
    pub fn pattern_opacity(&self) -> f64 {
        f64::from(self.intensity) / 100.
    }
    pub fn gradient_rotation(&self) -> i32 {
        if self.background_colors.len() < 3 {
            self.rotation
        } else {
            0
        }
    }
    pub fn has_share_url(&self) -> bool {
        self.slug.as_ref().is_some_and(|s| !s.is_empty())
    }
    pub fn is_null(&self) -> bool {
        self.id == 0 && !self.has_share_url() && self.background_colors.is_empty()
    }
    pub fn equals(&self, p: &Self) -> bool {
        self.flags == p.flags
            && self.slug.as_deref().unwrap_or_default() == p.slug.as_deref().unwrap_or_default()
            && self.emoji_id == p.emoji_id
            && self.background_colors == p.background_colors
            && self.rotation == p.rotation
            && self.intensity == p.intensity
            && self.blurred == p.blurred
            && self.document == p.document
    }
    pub fn collect_share_params(&self) -> Vec<String> {
        let mut r = vec![];
        if self.is_pattern() {
            if !self.background_colors.is_empty() {
                r.push(format!(
                    "bg_color={}",
                    string_from_colors(&self.background_colors)
                ))
            }
            if self.intensity != 0 {
                r.push(format!("intensity={}", self.intensity))
            }
        }
        if self.rotation != 0 && self.background_colors.len() == 2 {
            r.push(format!("rotation={}", self.rotation))
        }
        if self.blurred {
            r.push("mode=blur".into())
        }
        r
    }
    pub fn key(&self) -> String {
        if self.is_null() {
            return String::new();
        }
        let slug = self.slug();
        let b = if !slug.is_empty() {
            format!("bg/{slug}")
        } else if self.id != 0 {
            self.id.to_string()
        } else {
            string_from_colors(&self.background_colors)
        };
        let mut p = self.collect_share_params();
        if self.document.is_some() && !self.is_pattern() {
            p.push(format!("&intensity={}", self.intensity))
        }
        if p.is_empty() {
            b
        } else {
            format!("{b}?{}", p.join("&"))
        }
    }
    pub fn share_url(&self, internal_link: impl FnOnce(&str) -> String) -> String {
        if !self.has_share_url() {
            return String::new();
        }
        let b = internal_link(&format!("bg/{}", self.slug()));
        let p = self.collect_share_params();
        if p.is_empty() {
            b
        } else {
            format!("{b}?{}", p.join("&"))
        }
    }
    pub fn file_origin(&self) -> FileOrigin {
        FileOrigin::Wallpaper(FileOriginWallpaper {
            paper_id: self.id,
            access_hash: self.access_hash,
            owner_id: self.owner_id,
            slug: self.slug(),
        })
    }
    pub fn mtp_input(&self, self_user: UserId) -> tl::InputWallPaper {
        if self.owner_id.bare != 0 && self.owner_id != self_user && self.has_share_url() {
            tl::mtp_input_wall_paper_slug(self.slug().into_bytes())
        } else {
            tl::mtp_input_wall_paper(self.id, self.access_hash)
        }
    }
    pub fn mtp_settings(&self) -> tl::WallPaperSettings {
        let mut f = tl::WallPaperSettingsData::F_INTENSITY | tl::WallPaperSettingsData::F_ROTATION;
        if self.blurred {
            f |= tl::WallPaperSettingsData::F_BLUR
        }
        if !self.emoji_id.is_empty() {
            f |= tl::WallPaperSettingsData::F_EMOTICON
        }
        let bits = [
            tl::WallPaperSettingsData::F_BACKGROUND_COLOR,
            tl::WallPaperSettingsData::F_SECOND_BACKGROUND_COLOR,
            tl::WallPaperSettingsData::F_THIRD_BACKGROUND_COLOR,
            tl::WallPaperSettingsData::F_FOURTH_BACKGROUND_COLOR,
        ];
        let c: Vec<_> = self
            .background_colors
            .iter()
            .take(4)
            .enumerate()
            .map(|(i, c)| {
                f |= bits[i];
                c.serialized() as i32
            })
            .collect();
        tl::mtp_wall_paper_settings(
            f,
            c.first().copied(),
            c.get(1).copied(),
            c.get(2).copied(),
            c.get(3).copied(),
            Some(self.intensity),
            Some(self.rotation),
            (!self.emoji_id.is_empty()).then(|| self.emoji_id.as_bytes().to_vec()),
        )
    }
    pub fn with_url_params(&self, p: &BTreeMap<String, String>) -> Self {
        let mut r = self.clone();
        r.blurred = p
            .get("mode")
            .is_some_and(|m| m.replace('+', " ").split(' ').any(|s| s == "blur"));
        r.background_colors = colors_from_string(&self.slug());
        r.intensity = 50;
        for name in ["bg_color", "gradient", "color", "slug"] {
            if r.background_colors.is_empty() {
                r.background_colors = colors_from_string(p.get(name).map_or("", String::as_str))
            }
        }
        if let Some(v) = p.get("intensity").and_then(|s| qt_int(s)) {
            if (-100..=100).contains(&v) {
                r.intensity = v
            }
        }
        r.rotation = (p
            .get("rotation")
            .and_then(|s| qt_int(s))
            .unwrap_or(0)
            .clamp(0, 315)
            / 45)
            * 45;
        r
    }
    pub fn with_blurred(&self, v: bool) -> Self {
        let mut r = self.clone();
        r.blurred = v;
        r
    }
    pub fn with_pattern_intensity(&self, v: i32) -> Self {
        let mut r = self.clone();
        r.intensity = v;
        r
    }
    pub fn with_gradient_rotation(&self, v: i32) -> Self {
        let mut r = self.clone();
        r.rotation = v;
        r
    }
    pub fn with_background_colors(&self, c: Vec<Color>) -> Self {
        let mut r = self.clone();
        r.background_colors = c;
        if !colors_from_string(&self.slug()).is_empty() {
            r.slug = Some(
                string_from_colors(&r.background_colors)
                    .encode_utf16()
                    .collect(),
            )
        }
        r
    }
    pub fn with_params_from(&self, o: &Self) -> Self {
        let mut r = self.clone();
        r.blurred = o.blurred;
        if !o.background_colors.is_empty() {
            r = r.with_background_colors(o.background_colors.clone())
        }
        r.intensity = o.intensity;
        if o.is_pattern() {
            r.flags |= WallPaperFlag::Pattern
        }
        r
    }
    pub fn create(
        owner: UserId,
        v: &tl::WallPaper,
        process_document: impl FnOnce(&tl::Document) -> Option<DocumentRecordKey>,
    ) -> Option<Self> {
        match v {
            tl::WallPaper::WallPaperNoFile(d) => Some(Self::create_no_file(d)),
            tl::WallPaper::WallPaper(d) => {
                let document = process_document(&d.document)?;
                let mut r = Self::new(d.id);
                r.document = Some(document);
                r.access_hash = d.access_hash;
                r.owner_id = owner;
                r.slug = Some(String::from_utf8_lossy(&d.slug).encode_utf16().collect());
                for (b, f) in [
                    (d.is_dark(), WallPaperFlag::Dark),
                    (d.is_pattern(), WallPaperFlag::Pattern),
                    (d.is_default(), WallPaperFlag::Default),
                    (d.is_creator(), WallPaperFlag::Creator),
                ] {
                    if b {
                        r.flags |= f
                    }
                }
                if let Some(tl::WallPaperSettings::WallPaperSettings(s)) = d.vsettings() {
                    r.blurred = s.is_blur();
                    r.intensity = s.vintensity().unwrap_or(50);
                    if r.is_pattern() {
                        r.background_colors = colors_from_mtp(s);
                        r.rotation = s.vrotation().unwrap_or(0)
                    }
                }
                Some(r)
            }
        }
    }
    pub fn create_no_file(d: &tl::WallPaperNoFileData) -> Self {
        let mut r = Self::new(d.id);
        if d.is_dark() {
            r.flags |= WallPaperFlag::Dark
        }
        if d.is_default() {
            r.flags |= WallPaperFlag::Default
        }
        if let Some(tl::WallPaperSettings::WallPaperSettings(s)) = d.vsettings() {
            r.blurred = s.is_blur();
            r.background_colors = colors_from_mtp(s);
            r.rotation = s.vrotation().unwrap_or(0);
            r.emoji_id = String::from_utf8_lossy(s.vemoticon().map_or(&[], Vec::as_slice)).into()
        }
        r
    }
    pub fn serialize(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.u64(self.id);
        w.u64(self.access_hash);
        w.i32(VERSION_TAG);
        w.i32(1);
        w.i32(self.flags.value());
        w.nullable_utf16(self.slug.as_deref());
        w.i32(i32::from(self.blurred));
        w.i32(self.background_colors.len() as i32);
        for c in &self.background_colors {
            w.i32(c.serialized() as i32)
        }
        w.i32(self.intensity);
        w.i32(self.rotation);
        w.u64(self.owner_id.bare);
        w.0
    }
    pub fn from_serialized(b: &[u8]) -> Option<Self> {
        let mut q = Reader::new(b);
        let mut r = Self::new(q.u64()?);
        r.access_hash = q.u64()?;
        let tag = q.i32()?;
        if tag == VERSION_TAG {
            let _version = q.i32()?;
            r.flags = WallPaperFlags::from_raw(q.i32()?);
            r.slug = q.nullable_utf16()?;
            r.blurred = q.i32()? == 1;
            let n = q.i32()?;
            if !(0..=4).contains(&n) {
                return None;
            }
            for _ in 0..n {
                let c = q.i32()? as u32;
                if c == u32::MAX {
                    return None;
                }
                r.background_colors.push(Color::rgb(c))
            }
            r.intensity = q.i32()?;
            r.rotation = q.i32()?;
            r.owner_id = UserId::new(q.u64()?);
        } else {
            r.slug = q.nullable_utf16()?;
            let settings = q.i32()?;
            let c = q.i32()? as u32;
            r.intensity = q.i32()?;
            if !q.at_end() {
                let lo = q.i32()? as u32 as u64;
                let hi = if !q.at_end() {
                    q.i32()? as u32 as u64
                } else {
                    0
                };
                r.owner_id = UserId::new(lo | (hi << 32));
            }
            r.flags = WallPaperFlags::from_raw(raw_from_legacy_flags(tag));
            r.blurred = settings & 2 != 0;
            if c != u32::MAX {
                r.background_colors.push(Color::rgb(c))
            }
        }
        if !(-100..=100).contains(&r.intensity) {
            return None;
        }
        Some(r)
    }
    pub fn from_legacy_serialized(
        id: u64,
        access: u64,
        flags: u32,
        slug: Option<Vec<u16>>,
    ) -> Self {
        let mut r = Self::new(id);
        r.access_hash = access;
        r.flags = WallPaperFlags::from_raw(raw_from_legacy_flags(flags as i32));
        r.slug = slug;
        if let Some(c) = color_from_string(&r.slug()) {
            r.background_colors.push(c)
        }
        r
    }
    pub fn from_legacy_id(id: i32) -> Self {
        let mut r = Self::new(from_legacy_background_id(id));
        if !is_custom_wall_paper(&r) {
            r.flags |= WallPaperFlag::Default
        }
        r
    }
    pub fn from_colors_slug(slug: &str) -> Option<Self> {
        let c = colors_from_string(slug);
        if c.is_empty() {
            return None;
        }
        let mut r = custom_wall_paper();
        r.slug = Some(slug.encode_utf16().collect());
        r.background_colors = c;
        Some(r)
    }
    pub fn from_emoji_id(emoji: &str) -> Self {
        let mut r = Self::new(0);
        r.emoji_id = emoji.into();
        r
    }
    pub fn construct_default() -> Self {
        let mut r = Self::new(DEFAULT_BACKGROUND).with_background_colors(
            [0xdbddbb, 0x6ba587, 0xd5d88d, 0x88b884]
                .map(Color::rgb)
                .to_vec(),
        );
        r.flags = WallPaperFlag::Default | WallPaperFlag::Pattern;
        r
    }
}
pub fn raw_from_legacy_flags(f: i32) -> i32 {
    let mut raw = 0;
    for (a, b) in [(1, 4), (2, 2), (8, 1), (16, 8)] {
        if f & a != 0 {
            raw |= b
        }
    }
    raw
}

pub fn color_from_string(s: &str) -> Option<Color> {
    if s.len() != 6 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        None
    } else {
        Some(Color::rgb(u32::from_str_radix(s, 16).ok()?))
    }
}
pub fn colors_from_string(s: &str) -> Vec<Color> {
    let n = s.len() / 6;
    if n == 0 || n > 4 || s.len() != n * 7 - 1 {
        return vec![];
    }
    let mut r = vec![];
    for i in 0..n {
        if i + 1 < n
            && s.as_bytes()[i * 7 + 6] != b'~'
            && (n > 2 || s.as_bytes()[i * 7 + 6] != b'-')
        {
            return vec![];
        }
        let Some(c) = s.get(i * 7..i * 7 + 6).and_then(color_from_string) else {
            return vec![];
        };
        r.push(c)
    }
    r
}
pub fn string_from_colors(c: &[Color]) -> String {
    assert!(!c.is_empty());
    c.iter()
        .map(|c| format!("{:06x}", c.serialized()))
        .collect::<Vec<_>>()
        .join(if c.len() > 2 { "~" } else { "-" })
}
pub fn qt_int(s: &str) -> Option<i32> {
    let s = s.trim();
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        None
    } else {
        s.parse().ok()
    }
}
pub fn theme_wall_paper() -> WallPaper {
    WallPaper::new(from_legacy_background_id(-2))
}
pub fn custom_wall_paper() -> WallPaper {
    WallPaper::new(from_legacy_background_id(-1))
}
pub fn legacy1_default_wall_paper() -> WallPaper {
    WallPaper::new(from_legacy_background_id(0))
}
pub fn default_wall_paper() -> WallPaper {
    WallPaper::construct_default()
}
pub fn is_theme_wall_paper(p: &WallPaper) -> bool {
    p.id == from_legacy_background_id(-2)
}
pub fn is_custom_wall_paper(p: &WallPaper) -> bool {
    p.id == from_legacy_background_id(-1)
}
pub fn is_legacy1_default_wall_paper(p: &WallPaper) -> bool {
    p.id == from_legacy_background_id(0)
}
pub fn is_legacy2_default_wall_paper(p: &WallPaper) -> bool {
    p.id == LEGACY2_BACKGROUND || p.id == from_legacy_background_id(105)
}
pub fn is_legacy3_default_wall_paper(p: &WallPaper) -> bool {
    p.id == LEGACY3_BACKGROUND
}
pub fn is_legacy4_default_wall_paper(p: &WallPaper) -> bool {
    p.id == LEGACY4_BACKGROUND
}
pub fn is_default_wall_paper(p: &WallPaper) -> bool {
    p.id == DEFAULT_BACKGROUND
}
pub fn is_cloud_wall_paper(p: &WallPaper) -> bool {
    ![105, -2, -1, 0, -999, -666, -665, -664]
        .iter()
        .any(|i| p.id == from_legacy_background_id(*i))
}
pub mod details {
    use super::*;
    pub fn uninitialized_wall_paper() -> WallPaper {
        WallPaper::new(from_legacy_background_id(-999))
    }
    pub fn is_uninitialized_wall_paper(p: &WallPaper) -> bool {
        p.id() == from_legacy_background_id(-999)
    }
    pub fn testing_theme_wall_paper() -> WallPaper {
        WallPaper::new(from_legacy_background_id(-666))
    }
    pub fn is_testing_theme_wall_paper(p: &WallPaper) -> bool {
        p.id() == from_legacy_background_id(-666)
    }
    pub fn testing_default_wall_paper() -> WallPaper {
        WallPaper::new(from_legacy_background_id(-665)).with_params_from(&default_wall_paper())
    }
    pub fn is_testing_default_wall_paper(p: &WallPaper) -> bool {
        p.id() == from_legacy_background_id(-665)
    }
    pub fn testing_editor_wall_paper() -> WallPaper {
        WallPaper::new(from_legacy_background_id(-664))
    }
    pub fn is_testing_editor_wall_paper(p: &WallPaper) -> bool {
        p.id() == from_legacy_background_id(-664)
    }
}
pub fn colors_from_mtp(d: &tl::WallPaperSettingsData) -> Vec<Color> {
    [
        d.vbackground_color(),
        d.vsecond_background_color(),
        d.vthird_background_color(),
        d.vfourth_background_color(),
    ]
    .into_iter()
    .take_while(Option::is_some)
    .map(|v| Color::rgb(v.unwrap() as u32))
    .collect()
}
/// Genuine media-store adapter. Property checking mutates the real document,
/// exactly like Session::processDocument + checkWallPaperProperties.
pub fn create_with_documents(
    documents: &super::data_media_store::Documents,
    database: &impl crate::core::mime_type::MimeDatabase,
    user: UserId,
    wire: &tl::WallPaper,
) -> Option<WallPaper> {
    WallPaper::create(user, wire, |v| {
        let key = documents.process_document(v, database);
        documents
            .edit(key, |d| d.check_wallpaper_properties())
            .then_some(key)
    })
}
