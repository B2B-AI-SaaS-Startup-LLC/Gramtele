//! Headless values/rules from storage_media_prepare and attach_prepare. Media facts
//! come from an actual inspector; there is no pixel object or decoder substitute.
use crate::{core::mime_type as mime, data::data_media_types::MediaSize};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};
pub const MAX_ALBUM_ITEMS: usize = 10;
pub const READ_BYTES_LIMIT: i64 = 64 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq)]
pub struct ImageFacts {
    pub dimensions: MediaSize,
    pub animated: bool,
    pub encoded: Vec<u8>,
    pub format: String,
    pub animated_edit_scene: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SongFacts {
    pub duration: i64,
    pub title: String,
    pub performer: String,
    pub cover: MediaSize,
}
#[derive(Clone, Debug, PartialEq)]
pub struct VideoFacts {
    pub is_gifv: bool,
    pub is_webm_sticker: bool,
    pub supports_streaming: bool,
    pub has_audio: bool,
    pub duration: i64,
    pub thumbnail: MediaSize,
    pub gif: bool,
    pub quality: i32,
    pub cover: i64,
}
#[derive(Clone, Debug, PartialEq)]
pub enum MediaFacts {
    None,
    Image(ImageFacts),
    Song(SongFacts),
    Video(VideoFacts),
}
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedFileInformation {
    pub filemime: String,
    pub media: MediaFacts,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FileType {
    None,
    Photo,
    Video,
    Music,
    #[default]
    File,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlbumType {
    #[default]
    None,
    PhotoVideo,
    Music,
    File,
}
/// Data projection of PreparedFile. Caption/Editor jobs and actual pixel slots
/// stay with their real owners (18–20/5/7); no stand-in UI object is constructed.
#[derive(Clone, Debug)]
pub struct PreparedFile {
    pub id: i64,
    pub path: PathBuf,
    pub display_name: String,
    pub content: Vec<u8>,
    pub size: i64,
    pub information: Option<PreparedFileInformation>,
    pub video_cover: Option<Box<PreparedFile>>,
    pub shown_dimensions: MediaSize,
    pub original_dimensions: MediaSize,
    pub kind: FileType,
    pub ttl_seconds: i64,
    pub spoiler: bool,
    pub send_large_photos: bool,
    /// Snapshot of whether the real Editor owner supplied an animation job.
    pub animation_job_present: bool,
    pub archive: Option<super::storage_folder_archive::ArchiveJob>,
}
impl PreparedFile {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let display_name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        Self {
            id: 0,
            path,
            display_name,
            content: Vec::new(),
            size: 0,
            information: None,
            video_cover: None,
            shown_dimensions: MediaSize::new(0, 0),
            original_dimensions: MediaSize::new(0, 0),
            kind: FileType::File,
            ttl_seconds: 0,
            spoiler: false,
            send_large_photos: false,
            animation_job_present: false,
            archive: None,
        }
    }
    pub fn can_be_in_album_type(&self, album: AlbumType) -> bool {
        can_be_in_album_type(self.kind, album)
    }
    pub fn album_type(&self, photos: bool) -> AlbumType {
        match self.kind {
            FileType::Photo => {
                if photos {
                    AlbumType::PhotoVideo
                } else {
                    AlbumType::File
                }
            }
            FileType::Video => AlbumType::PhotoVideo,
            FileType::Music => AlbumType::Music,
            FileType::File => AlbumType::File,
            FileType::None => AlbumType::None,
        }
    }
    pub fn is_sticker(&self) -> bool {
        let i = self
            .information
            .as_ref()
            .expect("Source information precondition");
        self.kind == FileType::Photo && mime::is_mime_sticker(&i.filemime)
    }
    pub fn is_video_file(&self) -> bool {
        self.kind == FileType::Video
            && matches!(&self.information.as_ref().expect("information").media,MediaFacts::Video(v)if !v.is_gifv)
    }
    pub fn is_gifv(&self) -> bool {
        self.kind == FileType::Video
            && matches!(&self.information.as_ref().expect("information").media,MediaFacts::Video(v)if v.is_gifv)
    }
    pub fn can_use_high_quality_photo(&self) -> bool {
        self.kind == FileType::Photo
            && self.information.is_some()
            && !self.is_sticker()
            && (self.original_dimensions.width > 1280 || self.original_dimensions.height > 1280)
    }
    pub fn video_quality(&self) -> i32 {
        if let Some(PreparedFileInformation {
            media: MediaFacts::Video(v),
            ..
        }) = &self.information
        {
            v.quality
        } else {
            0
        }
    }
    pub fn can_edit_video(&self) -> bool {
        let i = self.information.as_ref().expect("information");
        self.kind == FileType::Video
            && matches!(&i.media,MediaFacts::Video(v)if !v.is_webm_sticker)
            && (!self.path.as_os_str().is_empty() || !self.content.is_empty())
    }
    pub fn sends_video_as_gif(&self) -> bool {
        matches!(&self.information,Some(PreparedFileInformation{media:MediaFacts::Video(v),..})if v.has_audio && v.gif)
    }
    pub fn has_animated_edit_scene(&self) -> bool {
        matches!(&self.information,Some(PreparedFileInformation{media:MediaFacts::Image(i),..})if i.animated_edit_scene)
    }
    /// Metadata mutation only; pixel preview creation remains stage5.
    pub fn prepare_details(&mut self, side_limit: i32) {
        let i = self
            .information
            .as_ref()
            .expect("real inspection must finish first");
        match &i.media {
            MediaFacts::Image(image) => {
                self.original_dimensions = image.dimensions;
                if valid_photo_for_album(image, &i.filemime) {
                    self.shown_dimensions = prepare_shown_dimensions(image.dimensions, side_limit);
                    self.kind = FileType::Photo;
                } else if image.animated {
                    self.kind = FileType::None;
                }
            }
            MediaFacts::Video(video) if valid_video_for_album(video) => {
                self.original_dimensions = video.thumbnail;
                self.shown_dimensions = prepare_shown_dimensions(video.thumbnail, side_limit);
                self.kind = FileType::Video;
                if let Some(PreparedFileInformation {
                    media: MediaFacts::Video(v),
                    ..
                }) = &mut self.information
                {
                    v.gif = !v.has_audio;
                }
            }
            MediaFacts::Song(_) => self.kind = FileType::Music,
            _ => {}
        }
    }
}
pub fn validate_thumb_dimensions(width: i32, height: i32) -> bool {
    // i64 avoids C++ signed-overflow UB for hostile dimensions.
    width > 0
        && height > 0
        && i64::from(width) <= 20 * i64::from(height)
        && i64::from(height) <= 20 * i64::from(width)
}
pub fn valid_photo_for_album(image: &ImageFacts, mime: &str) -> bool {
    !image.animated
        && (mime.is_empty() || mime.starts_with("image/"))
        && validate_thumb_dimensions(image.dimensions.width, image.dimensions.height)
}
pub fn valid_video_for_album(video: &VideoFacts) -> bool {
    validate_thumb_dimensions(video.thumbnail.width, video.thumbnail.height)
}
pub fn prepare_shown_dimensions(size: MediaSize, limit: i32) -> MediaSize {
    if size.width > limit || size.height > limit {
        size.fit(MediaSize::new(limit, limit))
    } else {
        size
    }
}
pub fn can_be_in_album_type(kind: FileType, album: AlbumType) -> bool {
    assert_ne!(album, AlbumType::None);
    match album {
        AlbumType::PhotoVideo => matches!(kind, FileType::Photo | FileType::Video),
        AlbumType::Music => kind == FileType::Music,
        AlbumType::File => matches!(kind, FileType::Photo | FileType::Video | FileType::File),
        AlbumType::None => unreachable!(),
    }
}
pub fn insert_text_on_image_cancel(text: &str) -> bool {
    !text.is_empty() && !text.starts_with("data:image")
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PrepareError {
    #[default]
    None,
    NonLocalUrl,
    Directory,
    EmptyFile,
    TooLargeFile,
}
#[derive(Clone, Debug, Default)]
pub struct PreparedList {
    pub error: PrepareError,
    pub error_data: String,
    pub files: Vec<PreparedFile>,
    pub files_to_process: VecDeque<PreparedFile>,
    pub override_send_images_as_photos: Option<bool>,
}
impl PreparedList {
    pub fn error(error: PrepareError, data: impl Into<String>) -> Self {
        Self {
            error,
            error_data: data.into(),
            ..Default::default()
        }
    }
    pub fn reordered(mut list: Self, order: &[usize]) -> Self {
        assert_eq!(list.error, PrepareError::None);
        assert_eq!(list.files.len(), order.len());
        let mut files = list.files.drain(..).map(Some).collect::<Vec<_>>();
        Self {
            error: list.error,
            error_data: list.error_data,
            files: order
                .iter()
                .map(|&n| files[n].take().expect("duplicate order index"))
                .collect(),
            ..Default::default()
        }
    }
    pub fn merge_to_end(&mut self, other: Self, cut: bool) {
        if self.error != PrepareError::None {
            return;
        }
        if other.error != PrepareError::None {
            self.error = other.error;
            self.error_data = other.error_data;
            return;
        }
        for file in other.files {
            if cut && self.files.len() == MAX_ALBUM_ITEMS {
                break;
            }
            self.files.push(file);
        }
    }
    pub fn can_be_sent_in_slowmode(&self) -> bool {
        self.can_be_sent_in_slowmode_with(&Self::default())
    }
    pub fn can_be_sent_in_slowmode_with(&self, other: &Self) -> bool {
        if !self.files_to_process.is_empty() || !other.files_to_process.is_empty() {
            return false;
        }
        let count = self.files.len() + other.files.len();
        if count < 2 {
            return true;
        }
        if count > MAX_ALBUM_ITEMS {
            return false;
        }
        let has = |t| self.files.iter().chain(&other.files).any(|f| f.kind == t);
        if has(FileType::None) {
            false
        } else if has(FileType::File) {
            !has(FileType::Music) && !has(FileType::Video)
        } else if has(FileType::Video) {
            !has(FileType::Music) && !has(FileType::File)
        } else if has(FileType::Music) {
            !has(FileType::Video) && !has(FileType::File) && !has(FileType::Photo)
        } else {
            true
        }
    }
    pub fn can_add_caption(&self, compress: bool) -> bool {
        self.files.last().is_some_and(|f| {
            !mime::qt_ascii_ci_suffix(&f.path.to_string_lossy(), ".tgs")
                && (compress
                    || !f
                        .information
                        .as_ref()
                        .is_some_and(|i| mime::is_mime_sticker(&i.filemime)))
        })
    }
    pub fn can_move_caption(&self, album: bool, compress: bool) -> bool {
        if !self.can_add_caption(compress) || self.files.len() > MAX_ALBUM_ITEMS {
            false
        } else if !album || !compress {
            self.files.len() == 1
        } else {
            self.files
                .iter()
                .all(|f| matches!(f.kind, FileType::Photo | FileType::Video))
        }
    }
    pub fn can_change_price(&self, album: bool, compress: bool) -> bool {
        self.can_move_caption(album, compress)
    }
    pub fn has_group_option(&self, slowmode: bool) -> bool {
        if slowmode || self.files.len() < 2 {
            return false;
        }
        self.files.windows(2).any(|f| {
            let (a, b) = (f[0].kind, f[1].kind);
            a != FileType::None
                && (a == b
                    || matches!(
                        (a, b),
                        (FileType::Photo, FileType::Video | FileType::File)
                            | (FileType::Video | FileType::File, FileType::Photo)
                    ))
        })
    }
    pub fn has_send_images_as_photos_option(&self, slowmode: bool) -> bool {
        if slowmode {
            let f = self.files.first().expect("Source nonempty precondition");
            self.files.len() == 1 && matches!(f.kind, FileType::Photo | FileType::Video)
        } else {
            self.files
                .iter()
                .any(|f| matches!(f.kind, FileType::Photo | FileType::Video))
        }
    }
    pub fn can_have_editor_hint_label(&self) -> bool {
        self.files.iter().any(|f| {
            f.kind == FileType::Photo
                && !mime::is_mime_sticker(&f.information.as_ref().expect("information").filemime)
        })
    }
    pub fn has_sticker(&self) -> bool {
        self.files.iter().any(PreparedFile::is_sticker)
    }
    pub fn has_spoiler_menu(&self, compress: bool) -> bool {
        self.files.iter().all(|f| f.kind == FileType::Video)
            || (compress
                && self
                    .files
                    .iter()
                    .all(|f| matches!(f.kind, FileType::Photo | FileType::Video)))
    }
    pub fn has_send_large_photos_option(&self, compress: bool) -> bool {
        compress
            && self
                .files
                .iter()
                .any(PreparedFile::can_use_high_quality_photo)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SendFilesWay(u8);
impl Default for SendFilesWay {
    fn default() -> Self {
        Self(11)
    }
}
impl SendFilesWay {
    pub fn group_files(self) -> bool {
        self.0 & 1 != 0
    }
    pub fn send_images_as_photos(self) -> bool {
        self.0 & 2 != 0
    }
    pub fn send_large_photos(self) -> bool {
        self.0 & 8 != 0
    }
    pub fn set_group_files(&mut self, v: bool) {
        if v {
            self.0 |= 1;
            if self.0 & 4 != 0 {
                self.set_send_images_as_photos(true);
            }
        } else {
            self.0 &= !1;
        }
    }
    pub fn set_send_images_as_photos(&mut self, v: bool) {
        if v {
            self.0 |= 2;
        } else {
            if self.0 & 4 != 0 {
                self.set_group_files(false);
            }
            self.0 &= !2;
        }
    }
    pub fn set_send_large_photos(&mut self, v: bool) {
        if v {
            self.0 |= 8;
        } else {
            self.0 &= !8;
        }
    }
    pub fn set_has_compressed_stickers(&mut self, v: bool) {
        if v {
            self.0 |= 4;
        } else {
            self.0 &= !4;
        }
    }
    pub fn serialize(self) -> i32 {
        let v = match (self.send_images_as_photos(), self.group_files()) {
            (true, true) => 0,
            (true, false) => 1,
            (false, true) => 3,
            (false, false) => 2,
        };
        v | if self.send_large_photos() { 4 } else { 0 }
    }
    pub fn from_serialized(value: i32) -> Option<Self> {
        if !(0..=7).contains(&value) {
            return None;
        }
        let mut r = Self::default();
        r.set_group_files(matches!(value & 3, 0 | 3));
        r.set_send_images_as_photos(matches!(value & 3, 0 | 1));
        r.set_send_large_photos(value & 4 != 0);
        Some(r)
    }
}
#[derive(Clone, Debug)]
pub struct PreparedGroup {
    pub list: PreparedList,
    pub kind: AlbumType,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroupRange {
    pub from: usize,
    pub till: usize,
    pub kind: AlbumType,
}
pub fn group_type_for_file(kind: FileType, group: bool, photos: bool) -> AlbumType {
    if !group {
        AlbumType::None
    } else {
        match kind {
            FileType::Music => AlbumType::Music,
            FileType::Photo | FileType::Video => {
                if photos {
                    AlbumType::PhotoVideo
                } else {
                    AlbumType::File
                }
            }
            FileType::File => AlbumType::File,
            FileType::None => AlbumType::None,
        }
    }
}
pub fn group_ranges(files: &[PreparedFile], way: SendFilesWay, slowmode: bool) -> Vec<GroupRange> {
    let mut result = Vec::new();
    if files.is_empty() {
        return result;
    }
    let mut from = 0;
    let mut kind = AlbumType::None;
    for (i, file) in files.iter().enumerate() {
        let next = if file.animation_job_present || file.sends_video_as_gif() {
            AlbumType::None
        } else {
            group_type_for_file(
                file.kind,
                way.group_files() || slowmode,
                way.send_images_as_photos(),
            )
        };
        let count = i - from;
        if (i > from && kind != next) || (kind != AlbumType::None && count == MAX_ALBUM_ITEMS) {
            result.push(GroupRange {
                from,
                till: i,
                kind: if count > 1 { kind } else { AlbumType::None },
            });
            from = i;
        }
        kind = next;
    }
    result.push(GroupRange {
        from,
        till: files.len(),
        kind: if files.len() - from > 1 {
            kind
        } else {
            AlbumType::None
        },
    });
    result
}
pub fn divide_by_groups(
    list: PreparedList,
    way: SendFilesWay,
    slowmode: bool,
) -> Vec<PreparedGroup> {
    let ranges = group_ranges(&list.files, way, slowmode);
    let mut files = list.files.into_iter();
    ranges
        .into_iter()
        .map(|r| PreparedGroup {
            kind: r.kind,
            list: PreparedList {
                files: files.by_ref().take(r.till - r.from).collect(),
                ..Default::default()
            },
        })
        .collect()
}
#[derive(Clone, Debug)]
pub struct PreparedBundle {
    pub groups: Vec<PreparedGroup>,
    pub way: SendFilesWay,
    pub total_count: usize,
    pub ctrl_shift_enter: bool,
}
pub fn prepare_files_bundle(
    groups: Vec<PreparedGroup>,
    way: SendFilesWay,
    ctrl_shift_enter: bool,
) -> PreparedBundle {
    let total_count = groups.iter().map(|g| g.list.files.len()).sum();
    PreparedBundle {
        groups,
        way,
        total_count,
        ctrl_shift_enter,
    }
}
pub fn make_single_file_bundle(list: PreparedList) -> PreparedBundle {
    let way = SendFilesWay::default();
    prepare_files_bundle(divide_by_groups(list, way, false), way, false)
}
/// Real filesystem phase. Too many files are deferred, NOT rejected by Source.
pub fn prepare_media_list(
    paths: &[PathBuf],
    premium: bool,
    mut error: Option<&mut dyn FnMut(&PreparedList)>,
) -> PreparedList {
    let mut result = PreparedList::default();
    for path in paths {
        let metadata = std::fs::metadata(path).ok();
        let size = metadata.as_ref().map_or(0, |m| m.len() as i64);
        let kind = if metadata.as_ref().is_some_and(|m| m.is_dir()) {
            PrepareError::Directory
        } else if !metadata.as_ref().is_some_and(|m| m.is_file())
            || size <= 0
            || std::fs::File::open(path).is_err()
        {
            PrepareError::EmptyFile
        } else if size > super::localimageloader::FILE_SIZE_PREMIUM_LIMIT
            || (!premium && size > super::localimageloader::FILE_SIZE_LIMIT)
        {
            PrepareError::TooLargeFile
        } else {
            PrepareError::None
        };
        if kind != PrepareError::None {
            let mut invalid = PreparedList::error(kind, path.to_string_lossy());
            if kind == PrepareError::TooLargeFile {
                let mut f = PreparedFile::new(path);
                f.size = size;
                invalid.files.push(f);
            }
            if let Some(callback) = &mut error {
                callback(&invalid);
                continue;
            } else {
                return invalid;
            }
        }
        let mut file = PreparedFile::new(path);
        file.size = size;
        if result.files.len() < MAX_ALBUM_ITEMS {
            result.files.push(file);
        } else {
            result.files_to_process.push_back(file);
        }
    }
    result
}
#[derive(Clone, Debug)]
pub enum LocalUrl {
    Local(PathBuf),
    NonLocal(String),
}
pub fn prepare_media_urls(
    urls: &[LocalUrl],
    premium: bool,
    mut callback: Option<&mut dyn FnMut(&PreparedList)>,
) -> PreparedList {
    let mut locals = Vec::new();
    for url in urls {
        match url {
            LocalUrl::Local(path) => locals.push(path.clone()),
            LocalUrl::NonLocal(url) => {
                let invalid = PreparedList::error(PrepareError::NonLocalUrl, url);
                if let Some(cb) = &mut callback {
                    cb(&invalid);
                } else {
                    return invalid;
                }
            }
        }
    }
    prepare_media_list(&locals, premium, callback)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MimeDataState {
    None,
    Files,
    FilesArchive,
    FilesArchiveOnly,
    PhotoFiles,
    PhotoFilesArchive,
    MediaFiles,
    MediaFilesArchive,
    Image,
    Media,
    Folder,
    FolderArchiveOnly,
}
/// Caller supplies actual QMimeData/inspector facts, including image readability.
#[derive(Clone, Debug)]
pub struct DragFileFacts {
    pub local: bool,
    pub directory: bool,
    pub path: String,
    pub size: i64,
    pub mime: String,
    pub image_readable: bool,
}
pub fn compute_mime_data_state(
    forward: bool,
    has_image: bool,
    files: &[DragFileFacts],
) -> MimeDataState {
    if forward {
        return MimeDataState::None;
    }
    if has_image {
        return MimeDataState::Image;
    }
    if files.is_empty() {
        return MimeDataState::None;
    }
    let (mut small, mut media) = (true, true);
    for f in files {
        if !f.local {
            return MimeDataState::None;
        }
        if f.directory {
            return if files.len() == 1 {
                MimeDataState::Folder
            } else {
                MimeDataState::None
            };
        }
        if f.size > super::localimageloader::FILE_SIZE_PREMIUM_LIMIT {
            return MimeDataState::None;
        }
        if small
            && (f.size > READ_BYTES_LIMIT
                || f.mime == "image/gif"
                || !mime::file_is_image(&f.path, &f.mime)
                || !f.image_readable)
        {
            small = false;
        }
        if media
            && !matches!(
                mime::detect_name_type(&f.path),
                mime::NameType::Image | mime::NameType::Video
            )
        {
            media = false;
        }
    }
    if small {
        MimeDataState::PhotoFiles
    } else if media {
        MimeDataState::MediaFiles
    } else if files.len() > 1 {
        MimeDataState::FilesArchive
    } else {
        MimeDataState::Files
    }
}
pub fn validate_photo_editor_media_drag_data(
    has_image: bool,
    files: &[DragFileFacts],
    with_video: bool,
) -> bool {
    if files.len() > 1 {
        false
    } else if has_image {
        true
    } else {
        files.first().is_some_and(|f| {
            f.local
                && ((with_video && super::localimageloader::is_video_file(&f.path, &f.mime))
                    || (mime::file_is_image(&f.path, &f.mime) && f.image_readable))
        })
    }
}
pub fn validate_edit_media_drag_data(
    has_image: bool,
    files: &[DragFileFacts],
    album: AlbumType,
) -> bool {
    if files.len() > 1 {
        false
    } else if has_image {
        album != AlbumType::Music
    } else if album == AlbumType::PhotoVideo && !files.is_empty() {
        files[0].local && mime::is_mime_accepted_for_photo_video_album(&files[0].mime)
    } else {
        true
    }
}
pub fn photo_editor_read_allowed(
    path: &Path,
    content_len: usize,
    known_file_size: i64,
    video: bool,
) -> bool {
    if path.as_os_str().is_empty() && content_len > READ_BYTES_LIMIT as usize {
        return false;
    }
    !(video && !path.as_os_str().is_empty() && known_file_size > READ_BYTES_LIMIT)
}
