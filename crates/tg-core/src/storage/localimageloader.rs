//! Encoded/scalar preparation and serial worker queue. Pixel/codec/editor work
//! is an explicit stage5/7 input boundary, never a fabricated decoder result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SendMediaType {
    Photo = 0,
    Audio = 1,
    Round = 2,
    File = 3,
    ThemeFile = 4,
    Secure = 5,
    SecondaryFile = 6,
}
impl From<SendMediaType> for file_upload::UploadKind {
    fn from(kind: SendMediaType) -> Self {
        match kind {
            SendMediaType::Photo => Self::Photo,
            SendMediaType::Audio => Self::Audio,
            SendMediaType::Round => Self::Round,
            SendMediaType::File => Self::File,
            SendMediaType::ThemeFile => Self::ThemeFile,
            SendMediaType::Secure => Self::Secure,
            SendMediaType::SecondaryFile => Self::SecondaryFile,
        }
    }
}
impl SendMediaType {
    pub fn is_document(self) -> bool {
        file_upload::UploadKind::from(self).is_document()
    }
}

use super::{
    file_upload::{self, Content, PreparedUpload},
    storage_media_prepare::{self as prepare, MediaFacts, PreparedFileInformation, VideoFacts},
};
use crate::{
    core::mime_type as mime,
    data::{
        data_media_types::{document_waveform_encode_5bit, good_sticker_dimensions, MediaSize},
        data_msg_id::FullMsgId,
    },
};
use md5::{Digest, Md5};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Condvar, Mutex,
    },
};
use tg_scheme as tl;
pub const FILE_SIZE_LIMIT: i64 = 2_000 * 1024 * 1024;
pub const FILE_SIZE_PREMIUM_LIMIT: i64 = 4_000 * 1024 * 1024;
pub const PHOTO_UPLOAD_PART_SIZE: usize = 32 * 1024;
pub const THUMBNAIL_QUALITY: i32 = 95;
pub fn photo_side_limit(large: bool) -> i32 {
    if large {
        2560
    } else {
        1280
    }
}
pub fn md5_hex(bytes: &[u8]) -> Vec<u8> {
    Md5::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
        .into_bytes()
}
pub fn file_thumbnail_upload_required(mime: &str, size: i64) -> bool {
    size > 5 * 1024 * 1024
        || ![
            "image/jpeg",
            "image/gif",
            "image/png",
            "image/webp",
            "video/mp4",
        ]
        .contains(&mime.to_lowercase().as_str())
}
pub fn mp4_file_name(name: &str) -> String {
    let stem = name
        .rfind('.')
        .filter(|&i| i > 0)
        .map_or(name, |i| &name[..i]);
    format!("{stem}.mp4")
}
pub fn thumbnail_dimensions(size: MediaSize) -> Option<MediaSize> {
    if !prepare::validate_thumb_dimensions(size.width, size.height) {
        return None;
    }
    if size.width <= 320 && size.height <= 320 {
        return Some(size);
    }
    Some(if size.width > size.height {
        MediaSize::new(
            320,
            (320. * f64::from(size.height) / f64::from(size.width)).round() as i32,
        )
    } else {
        MediaSize::new(
            (320. * f64::from(size.width) / f64::from(size.height)).round() as i32,
            320,
        )
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JpegPlan {
    PreserveProgressive,
    TryMakeProgressive,
    Reencode,
}
pub fn photo_jpeg_plan(size: MediaSize, bytes: usize, format: &str, progressive: bool) -> JpegPlan {
    if bytes != 0
        && (bytes as i64) <= i64::from(size.width) * i64::from(size.height) * 4 / 8
        && format == "jpeg"
    {
        if progressive {
            JpegPlan::PreserveProgressive
        } else {
            JpegPlan::TryMakeProgressive
        }
    } else {
        JpegPlan::Reencode
    }
}
pub fn is_video_file(path: &str, mime: &str) -> bool {
    check_mime_or_extensions(
        path,
        mime,
        &["video/mp4", "video/quicktime"],
        &[".mp4", ".mov", ".m4v", ".webm"],
    )
}
pub fn check_mime_or_extensions(
    path: &str,
    mime: &str,
    mimes: &[&str],
    extensions: &[&str],
) -> bool {
    mimes.contains(&mime)
        || extensions
            .iter()
            .any(|ext| mime::qt_ascii_ci_suffix(path, ext))
}
pub fn song_probe_allowed(path: &str, mime: &str) -> bool {
    path.is_empty()
        || check_mime_or_extensions(
            path,
            mime,
            &[
                "audio/mp3",
                "audio/m4a",
                "audio/aac",
                "audio/ogg",
                "audio/flac",
                "audio/opus",
            ],
            &[".mp3", ".m4a", ".aac", ".ogg", ".flac", ".opus", ".oga"],
        )
}
pub fn video_probe_accepted(path: &str, video: &VideoFacts) -> bool {
    video.duration >= 0
        && prepare::validate_thumb_dimensions(video.thumbnail.width, video.thumbnail.height)
        && (!path.is_empty() || !video.thumbnail.is_empty())
}
pub fn video_conversion_required(
    mime: &str,
    size: i64,
    video: &VideoFacts,
    force: bool,
    edited: bool,
) -> bool {
    let gif = video.gif && !force;
    let convert = (!mime::is_mime_sent_as_video(mime)
        || (gif && video.is_gifv && mime != "video/mp4"))
        && !video.is_webm_sticker
        && size < 1000 * 1024 * 1024;
    !force && !video.thumbnail.is_empty() && (convert || edited)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct TaskId(pub u64);
impl TaskId {
    pub fn allocate() -> Self {
        static IDS: AtomicU64 = AtomicU64::new(1);
        Self(IDS.fetch_add(1, Ordering::Relaxed))
    }
}
#[derive(Clone, Debug)]
pub struct FilePrepareDescriptor {
    pub task_id: TaskId,
    pub id: u64,
    pub kind: SendMediaType,
    pub spoiler: bool,
}
impl FilePrepareDescriptor {
    pub fn new(id: u64) -> Self {
        Self {
            task_id: TaskId::allocate(),
            id,
            kind: SendMediaType::File,
            spoiler: false,
        }
    }
}
#[derive(Default)]
struct PreparedContent {
    filepath: PathBuf,
    bytes: Vec<u8>,
    size: i64,
}
pub struct FilePrepareResult {
    materialized: Mutex<PreparedContent>,
    pub task_id: TaskId,
    pub id: u64,
    pub kind: SendMediaType,
    pub filename: String,
    pub filemime: String,
    pub fileparts: Vec<Vec<u8>>,
    pub filemd5: Vec<u8>,
    pub partssize: i64,
    pub thumbnail_dimensions: Option<MediaSize>,
    pub thumb_id: u64,
    pub thumbname: String,
    pub thumbparts: Vec<Vec<u8>>,
    pub thumbbytes: Vec<u8>,
    pub thumbmd5: Vec<u8>,
    pub photo: tl::Photo,
    pub document: tl::Document,
    pub spoiler: bool,
    pub force_file: bool,
    pub good_thumbnail_bytes: Vec<u8>,
    pub attached_stickers: Vec<tl::InputDocument>,
    pub video_cover: Option<Arc<FilePrepareResult>>,
    pub transcoded_temp_path: Mutex<PathBuf>,
    archive_entries: Mutex<Option<super::storage_folder_archive::ArchiveEntries>>,
    pub preparing: Option<file_upload::Preparation>,
}
impl FilePrepareResult {
    pub fn new(d: FilePrepareDescriptor) -> Self {
        Self {
            materialized: Mutex::new(PreparedContent::default()),
            task_id: d.task_id,
            id: d.id,
            kind: d.kind,
            spoiler: d.spoiler,
            filename: String::new(),
            filemime: String::new(),
            fileparts: Vec::new(),
            filemd5: Vec::new(),
            partssize: 0,
            thumbnail_dimensions: None,
            thumb_id: 0,
            thumbname: String::new(),
            thumbparts: Vec::new(),
            thumbbytes: Vec::new(),
            thumbmd5: Vec::new(),
            photo: tl::mtp_photo_empty(0),
            document: tl::mtp_document_empty(0),
            force_file: false,
            good_thumbnail_bytes: Vec::new(),
            attached_stickers: Vec::new(),
            video_cover: None,
            transcoded_temp_path: Mutex::new(PathBuf::new()),
            archive_entries: Mutex::new(None),
            preparing: None,
        }
    }
    /// Source appends; repeated calls intentionally do NOT clear old parts/hash.
    pub fn set_file_data(&mut self, data: &[u8]) {
        self.partssize = data.len() as i64;
        if !data.is_empty() {
            self.fileparts
                .extend(data.chunks(PHOTO_UPLOAD_PART_SIZE).map(<[u8]>::to_vec));
            self.filemd5 = md5_hex(data);
        }
    }
    pub fn set_thumb_data(&mut self, data: &[u8]) {
        if !data.is_empty() {
            self.thumbbytes = data.to_vec();
            self.thumbparts
                .extend(data.chunks(PHOTO_UPLOAD_PART_SIZE).map(<[u8]>::to_vec));
            self.thumbmd5 = md5_hex(data);
        }
    }
    pub fn filesize(&self) -> i64 {
        self.materialized.lock().unwrap().size
    }
    pub fn filepath(&self) -> PathBuf {
        self.materialized.lock().unwrap().filepath.clone()
    }
    pub fn content(&self) -> Vec<u8> {
        self.materialized.lock().unwrap().bytes.clone()
    }
    pub fn set_filesize(&self, size: i64) {
        self.materialized.lock().unwrap().size = size;
    }
    pub fn has_archive_entries(&self) -> bool {
        self.archive_entries.lock().unwrap().is_some()
    }
    /// Shared prepared result changes at the same owner callback as in C++.
    pub fn materialize(&self, content: &Content, size: i64) {
        let mut data = self.materialized.lock().unwrap();
        data.size = size;
        match content {
            Content::Bytes(bytes) => {
                data.bytes = bytes.clone();
                data.filepath = PathBuf::new();
            }
            Content::Path(path) => {
                data.bytes.clear();
                data.filepath = path.clone();
                self.set_transcoded_temp_path(path.clone());
            }
        }
    }
    pub fn set_transcoded_temp_path(&self, path: PathBuf) {
        *self.transcoded_temp_path.lock().unwrap() = path;
    }
    pub fn document_part_layout(&self) -> file_upload::PartLayout {
        file_upload::part_layout(self.filesize())
    }
    pub fn prepared_upload(self: &Arc<Self>) -> PreparedUpload {
        let data = self.materialized.lock().unwrap();
        PreparedUpload {
            id: self.id,
            kind: self.kind.into(),
            filename: self.filename.clone(),
            size: data.size,
            content: if data.bytes.is_empty() {
                Content::Path(data.filepath.clone())
            } else {
                Content::Bytes(data.bytes.clone())
            },
            parts: if self.kind.is_document() {
                self.thumbparts.clone()
            } else {
                self.fileparts.clone()
            },
            parts_id: if self.kind.is_document() {
                self.thumb_id
            } else {
                self.id
            },
            parts_name: self.thumbname.clone(),
            parts_md5: self.thumbmd5.clone(),
            file_md5: self.filemd5.clone(),
            parts_size: self.partssize,
            preparing: self.preparing,
            video_cover: self
                .video_cover
                .as_ref()
                .map(|c| Box::new(c.prepared_upload())),
            attached_stickers: self.attached_stickers.clone(),
            force_file: self.force_file,
            preparation_lease: Some(self.clone()),
        }
    }
}
impl Drop for FilePrepareResult {
    fn drop(&mut self) {
        let path = self.transcoded_temp_path.lock().unwrap();
        if !path.as_os_str().is_empty() {
            let _ = std::fs::remove_file(&*path);
        }
    }
}
/// Results of real pixel/codec work, not a pixel/backend implementation.
#[derive(Clone, Debug)]
pub struct EncodedThumbnail {
    pub id: u64,
    pub dimensions: MediaSize,
    pub bytes: Vec<u8>,
    pub good_bytes: Vec<u8>,
}
#[derive(Clone, Debug)]
pub struct EncodedPhoto {
    pub dimensions: MediaSize,
    pub medium_dimensions: MediaSize,
    pub jpeg: Vec<u8>,
}
#[derive(Clone, Debug)]
pub struct FileLoadArgs {
    pub descriptor: FilePrepareDescriptor,
    pub dc_id: i32,
    pub now: i32,
    pub filepath: PathBuf,
    pub content: Vec<u8>,
    /// Actual filename policy (core/file_utilities33); no guessed MIME sniffing/date.
    pub filename: String,
    pub display_name: String,
    pub pixel_file: Option<Vec<u8>>,
    pub archive: Option<super::storage_folder_archive::ArchiveJob>,
    pub filemime: String,
    pub kind: SendMediaType,
    pub information: Option<PreparedFileInformation>,
    pub duration: i64,
    pub waveform: Vec<i8>,
    pub force_file: bool,
    /// Snapshot of actual Editor::VideoEdited; editor geometry remains5/7.
    pub video_edited: bool,
    pub send_large_photos: bool,
    pub album_present: bool,
    pub photo: Option<EncodedPhoto>,
    pub thumbnail: Option<EncodedThumbnail>,
    pub video_cover: Option<Arc<FilePrepareResult>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreparationNeed {
    MediaInformation,
    ImageEncoding,
    VideoEncoding,
}
#[derive(Debug)]
pub enum ProcessOutcome {
    Ready(Arc<FilePrepareResult>),
    Needs(PreparationNeed),
}
pub struct FileLoadTask {
    pub args: FileLoadArgs,
    result: Option<Arc<FilePrepareResult>>,
}
impl FileLoadTask {
    pub fn new(args: FileLoadArgs) -> Self {
        Self { args, result: None }
    }
    pub fn fileid(&self) -> u64 {
        self.args.descriptor.id
    }
    pub fn peek_result(&self) -> Option<&Arc<FilePrepareResult>> {
        self.result.as_ref()
    }
    pub fn process(&mut self) -> ProcessOutcome {
        let a = &self.args;
        let mut r = FilePrepareResult::new(a.descriptor.clone());
        r.kind = SendMediaType::File; // descriptor in process() defaults to File until success.
        let metadata = std::fs::metadata(&a.filepath).ok();
        if metadata.as_ref().is_some_and(|m| m.is_dir()) {
            r.set_filesize(-1);
            return self.complete(r);
        }
        let voice = a.kind == SendMediaType::Audio;
        let round = a.kind == SendMediaType::Round;
        if metadata.is_some() {
            assert!(!voice && !round, "Source voice/round are memory-only");
        }
        let mut size = metadata
            .as_ref()
            .map_or(a.content.len() as i64, |m| m.len() as i64);
        let mut filename = metadata
            .as_ref()
            .and_then(|_| a.filepath.file_name())
            .map_or_else(|| a.filename.clone(), |n| n.to_string_lossy().into_owned());
        let mut filemime = if voice {
            "audio/ogg"
        } else if round {
            "video/mp4"
        } else if metadata.is_some() {
            a.information
                .as_ref()
                .map_or(a.filemime.as_str(), |i| i.filemime.as_str())
        } else {
            &a.filemime
        };
        if metadata.is_none() {
            if let Some(job) = &a.archive {
                if let Some(entries) = super::storage_folder_archive::gather_archive_entries(job) {
                    size = super::storage_folder_archive::archive_size_estimate(&entries);
                    *r.archive_entries.get_mut().unwrap() = Some(entries);
                } else {
                    size = 0;
                }
                filemime = "application/zip";
                filename = if a.display_name.is_empty() {
                    "Archive.zip".into()
                } else {
                    a.display_name.clone()
                };
                r.preparing = Some(file_upload::Preparation::Archive);
            }
        }
        let media = if voice || (metadata.is_none() && a.archive.is_some()) {
            &MediaFacts::None
        } else if let Some(i) = &a.information {
            &i.media
        } else {
            return ProcessOutcome::Needs(PreparationNeed::MediaInformation);
        };
        let image = if let MediaFacts::Image(i) = media {
            Some(i)
        } else {
            None
        };
        let mut kind = a.kind;
        let mut content = a.content.clone();
        if size == 0 && image.is_some_and(|i| !i.dimensions.is_empty()) {
            if kind == SendMediaType::Photo {
                let i = image.unwrap();
                if prepare::validate_thumb_dimensions(i.dimensions.width, i.dimensions.height) {
                    size = -1;
                    filemime = "image/jpeg";
                } else {
                    kind = SendMediaType::File;
                }
            }
            if kind == SendMediaType::File {
                let Some(png) = &a.pixel_file else {
                    return ProcessOutcome::Needs(PreparationNeed::ImageEncoding);
                };
                content = png.clone();
                size = content.len() as i64;
                filemime = "image/png";
            }
        }
        r.set_filesize(size.min(i64::from(u32::MAX)));
        if size == 0 || size > FILE_SIZE_PREMIUM_LIMIT {
            return self.complete(r);
        }
        let mut attrs = vec![tl::mtp_document_attribute_filename(
            if a.display_name.is_empty() {
                filename.as_bytes().to_vec()
            } else {
                a.display_name.as_bytes().to_vec()
            },
        )];
        let mut is_sticker = false;
        match media {
            MediaFacts::Song(s) if !round => {
                attrs.push(tl::mtp_document_attribute_audio(
                    tl::tl::Flags::from_raw(3),
                    (s.duration / 1000) as i32,
                    Some(s.title.as_bytes().to_vec()),
                    Some(s.performer.as_bytes().to_vec()),
                    None,
                ));
            }
            MediaFacts::Video(v) => {
                if video_conversion_required(filemime, size, v, a.force_file, a.video_edited)
                    && !round
                {
                    return ProcessOutcome::Needs(PreparationNeed::VideoEncoding);
                }
                if round || !a.force_file {
                    let gif = v.gif && !a.force_file;
                    if !a.album_present
                        && ((round && v.is_gifv) || (!round && gif && filemime == "video/mp4"))
                    {
                        attrs.push(tl::mtp_document_attribute_animated());
                    }
                    let start = if round {
                        0
                    } else {
                        v.cover.clamp(0, v.duration.max(0))
                    };
                    let flags = u64::from(round)
                        | if v.supports_streaming { 2 } else { 0 }
                        | if !round && gif { 8 } else { 0 }
                        | if start > 0 { 16 } else { 0 };
                    attrs.push(tl::mtp_document_attribute_video(
                        tl::tl::Flags::from_raw(flags),
                        v.duration as f64 / 1000.,
                        v.thumbnail.width,
                        v.thumbnail.height,
                        None,
                        (start > 0).then_some(start as f64 / 1000.),
                        None,
                    ));
                }
            }
            MediaFacts::Image(i) if !round => {
                let s = i.dimensions;
                attrs.push(tl::mtp_document_attribute_image_size(s.width, s.height));
                if prepare::validate_thumb_dimensions(s.width, s.height) {
                    is_sticker = mime::is_mime_sticker(filemime)
                        && size < 2 * 1024 * 1024
                        && (mime::is_mime_sticker_animated(filemime)
                            || (kind == SendMediaType::File
                                && good_sticker_dimensions(s.width, s.height)));
                    if is_sticker {
                        attrs.push(tl::mtp_document_attribute_sticker(
                            tl::tl::Flags::from_raw(0),
                            Vec::new(),
                            tl::mtp_input_sticker_set_empty(),
                            None,
                        ));
                    } else if metadata.is_some() && i.animated {
                        // Source copies isAnimation only in the existing-file branch.
                        // Memory-only Image facts do not set that local flag.
                        attrs.push(tl::mtp_document_attribute_animated());
                    } else if filemime.starts_with("image/") && kind != SendMediaType::File {
                        let Some(photo) = &a.photo else {
                            return ProcessOutcome::Needs(PreparationNeed::ImageEncoding);
                        };
                        let expected = prepare::prepare_shown_dimensions(
                            s,
                            photo_side_limit(a.send_large_photos),
                        );
                        assert_eq!(photo.dimensions, expected, "actual encoder dimensions");
                        r.photo = tl::mtp_photo(
                            tl::tl::Flags::from_raw(0),
                            r.id,
                            0,
                            Vec::new(),
                            a.now,
                            vec![
                                tl::mtp_photo_size(
                                    b"m".to_vec(),
                                    photo.medium_dimensions.width,
                                    photo.medium_dimensions.height,
                                    0,
                                ),
                                tl::mtp_photo_size(
                                    b"y".to_vec(),
                                    photo.dimensions.width,
                                    photo.dimensions.height,
                                    0,
                                ),
                            ],
                            None,
                            a.dc_id,
                        );
                        r.set_file_data(&photo.jpeg);
                        if size < 0 {
                            size = photo.jpeg.len() as i64;
                            r.set_filesize(size);
                        }
                    }
                }
            }
            _ => {}
        }
        if kind == SendMediaType::Photo && matches!(r.photo, tl::Photo::PhotoEmpty(_)) {
            kind = SendMediaType::File;
        }
        if voice {
            attrs = vec![tl::mtp_document_attribute_audio(
                tl::tl::Flags::from_raw(1028),
                (a.duration / 1000) as i32,
                None,
                None,
                Some(document_waveform_encode_5bit(&a.waveform)),
            )];
        }
        let thumbnail_source = match media {
            MediaFacts::Song(s) if !round => thumbnail_dimensions(s.cover),
            MediaFacts::Video(v) => thumbnail_dimensions(v.thumbnail),
            MediaFacts::Image(i) if !voice && !round => thumbnail_dimensions(i.dimensions),
            _ => None,
        };
        r.thumbname = if is_sticker {
            "thumb.webp"
        } else {
            "thumb.jpg"
        }
        .into();
        let theme = matches!(
            filemime,
            "application/x-tdesktop-theme" | "application/x-tgtheme-tdesktop"
        );
        if (thumbnail_source.is_some() && file_thumbnail_upload_required(filemime, size) || theme)
            && a.thumbnail.is_none()
        {
            return ProcessOutcome::Needs(PreparationNeed::ImageEncoding);
        }
        let thumb = if let Some(t) = &a.thumbnail {
            r.thumb_id = t.id;
            if file_thumbnail_upload_required(filemime, size) {
                r.set_thumb_data(&t.bytes);
            }
            r.good_thumbnail_bytes = t.good_bytes.clone();
            tl::mtp_photo_size(Vec::new(), t.dimensions.width, t.dimensions.height, 0)
        } else if let Some(size) = thumbnail_source {
            r.thumb_id = tg_base::random::random_value();
            tl::mtp_photo_size(Vec::new(), size.width, size.height, 0)
        } else {
            tl::mtp_photo_size_empty(Vec::new())
        };
        r.thumbnail_dimensions = match &thumb {
            tl::PhotoSize::PhotoSize(s) => Some(MediaSize::new(s.w, s.h)),
            _ => None,
        };
        if voice || kind != SendMediaType::Photo {
            // Source localimageloader.cpp creates flags=0: its provided vector
            // is hidden by vthumbs() and never serialized (generate_tl.py getter).
            // Actual thumb pixels/bytes remain separate FilePrepareResult fields.
            r.document = tl::mtp_document(
                tl::tl::Flags::from_raw(0),
                r.id,
                0,
                Vec::new(),
                a.now,
                filemime.as_bytes().to_vec(),
                size as u64,
                None,
                None,
                a.dc_id,
                attrs,
            );
            if !voice {
                kind = if round {
                    SendMediaType::Round
                } else {
                    SendMediaType::File
                };
            }
        }
        if filename.is_empty() {
            filename = a.filename.clone();
        }
        r.kind = kind;
        r.filename = filename;
        r.filemime = filemime.into();
        let data = r.materialized.get_mut().unwrap();
        data.filepath = a.filepath.clone();
        data.bytes = content;
        r.force_file = a.force_file;
        r.video_cover = a
            .video_cover
            .as_ref()
            .filter(|c| c.kind == SendMediaType::Photo && !c.fileparts.is_empty())
            .cloned();
        self.complete(r)
    }
    fn complete(&mut self, r: FilePrepareResult) -> ProcessOutcome {
        let r = Arc::new(r);
        self.result = Some(r.clone());
        ProcessOutcome::Ready(r)
    }
    pub fn finish(&self, session_alive: bool, premium: bool) -> FinishOutcome {
        if !session_alive {
            return FinishOutcome::SessionGone;
        }
        match &self.result {
            None => FinishOutcome::Empty,
            Some(r) if r.filesize() <= 0 => FinishOutcome::Empty,
            Some(r)
                if r.filesize() > FILE_SIZE_PREMIUM_LIMIT
                    || (!premium && r.filesize() > FILE_SIZE_LIMIT) =>
            {
                FinishOutcome::TooLarge(r.filesize())
            }
            Some(r) => FinishOutcome::SendConfirmed(r.clone()),
        }
    }
}
#[derive(Debug)]
pub enum FinishOutcome {
    SessionGone,
    Empty,
    TooLarge(i64),
    SendConfirmed(Arc<FilePrepareResult>),
}
pub struct AlbumItem {
    pub task_id: TaskId,
    pub random_id: u64,
    pub message_id: FullMsgId,
    pub media: Option<tl::InputSingleMedia>,
    pub prepared: Option<Arc<FilePrepareResult>>,
}
impl AlbumItem {
    pub fn new(task_id: TaskId) -> Self {
        Self {
            task_id,
            random_id: 0,
            message_id: FullMsgId::default(),
            media: None,
            prepared: None,
        }
    }
}
#[derive(Debug)]
pub struct SendingAlbum {
    pub group_id: u64,
    pub items: Vec<AlbumItem>,
    pub music_prepared_batching: bool,
    pub sent: bool,
}
impl SendingAlbum {
    pub fn new(group_id: u64) -> Self {
        Self {
            group_id,
            items: Vec::new(),
            music_prepared_batching: false,
            sent: false,
        }
    }
    pub fn prepared_music_batching(&self) -> bool {
        self.music_prepared_batching
    }
    pub fn prepared_music_ready(&self) -> bool {
        self.music_prepared_batching
            && !self.items.is_empty()
            && self.items.iter().all(|i| i.prepared.is_some())
    }
    pub fn prepared_music_sample(&self) -> Option<&Arc<FilePrepareResult>> {
        self.items.iter().find_map(|i| i.prepared.as_ref())
    }
    pub fn take_prepared_music(&mut self) -> Vec<Arc<FilePrepareResult>> {
        if !self.prepared_music_ready() {
            Vec::new()
        } else {
            self.items
                .iter_mut()
                .map(|i| i.prepared.take().unwrap())
                .collect()
        }
    }
    pub fn remove_task(&mut self, id: TaskId) {
        let i = self
            .items
            .iter()
            .position(|i| i.task_id == id)
            .expect("source task exists");
        self.items.remove(i);
    }
    // AyuGram: reverseLocalPremiumEmoji requires the actual History/lang owner.
    /// Trim/reverseLocalPremiumEmoji/EntitiesToMTP are done by real History/lang
    /// owner before this call; accepts the actual TL caption, no fake History.
    pub fn fill_media(&mut self, id: FullMsgId, media: tl::InputSingleMedia) {
        let i = self
            .items
            .iter_mut()
            .find(|i| i.message_id == id)
            .expect("album item");
        assert!(i.media.is_none());
        i.random_id = media.c_input_single_media().random_id;
        i.media = Some(media);
    }
    pub fn refresh_media_caption(&mut self, id: FullMsgId, media: tl::InputSingleMedia) {
        let i = self
            .items
            .iter_mut()
            .find(|i| i.message_id == id)
            .expect("album item");
        if let Some(old) = &i.media {
            let old = old.c_input_single_media();
            let caption = media.c_input_single_media();
            let entities = caption.entities.clone().filter(|v| !v.is_empty());
            i.media = Some(tl::mtp_input_single_media(
                tl::tl::Flags::from_raw(u64::from(entities.is_some())),
                old.media.clone(),
                old.random_id,
                caption.message.clone(),
                entities,
            ));
        }
    }
    /// Returns the new first message for actual History caption transfer.
    pub fn remove_item(&mut self, id: FullMsgId) -> Option<FullMsgId> {
        let i = self
            .items
            .iter()
            .position(|i| i.message_id == id)
            .expect("album item");
        let transfer = self.items.len() > 1 && i == 0;
        self.items.remove(i);
        transfer.then(|| self.items[0].message_id)
    }
    pub fn finish_music_item(&mut self, id: TaskId, result: Arc<FilePrepareResult>) -> bool {
        self.items
            .iter_mut()
            .find(|i| i.task_id == id)
            .expect("album task")
            .prepared = Some(result);
        self.prepared_music_ready()
    }
}
/// Real task objects run process on a worker, finish on the owner thread.
pub trait Task: Send + 'static {
    fn id(&self) -> TaskId;
    fn process(&mut self);
    fn finish(self: Box<Self>);
}
#[derive(Default)]
struct QueueState {
    pending: VecDeque<(TaskId, Box<dyn Task>)>,
    finished: VecDeque<(TaskId, Box<dyn Task>)>,
    active: Option<TaskId>,
    running: bool,
    stopping: bool,
}
struct SharedQueue {
    state: Mutex<QueueState>,
    wake: Condvar,
    stopped: Condvar,
    processed: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}
pub struct TaskQueue {
    shared: Arc<SharedQueue>,
    stop_timeout: i64,
    stop_at: Mutex<Option<i64>>,
}
impl TaskQueue {
    pub fn new(stop_timeout: i64) -> Self {
        Self {
            shared: Arc::new(SharedQueue {
                state: Mutex::new(QueueState::default()),
                wake: Condvar::new(),
                stopped: Condvar::new(),
                processed: Mutex::new(None),
            }),
            stop_timeout,
            stop_at: Mutex::new(None),
        }
    }
    pub fn add_task(&self, task: Box<dyn Task>) -> TaskId {
        let id = task.id();
        self.add_tasks(vec![task]);
        id
    }
    pub fn add_tasks(&self, tasks: Vec<Box<dyn Task>>) {
        *self.stop_at.lock().unwrap() = None;
        let shared = self.shared.clone();
        let start = {
            let mut s = shared.state.lock().unwrap();
            s.pending
                .extend(tasks.into_iter().map(|task| (task.id(), task)));
            let start = !s.running;
            s.running = true;
            start
        };
        if start {
            let worker = shared.clone();
            tg_runtime::async_(move || run_worker(worker));
        }
        shared.wake.notify_one();
    }
    pub fn cancel_task(&self, id: TaskId) {
        let mut s = self.shared.state.lock().unwrap();
        s.pending.retain(|t| t.0 != id);
        s.finished.retain(|t| t.0 != id);
        if s.active == Some(id) {
            s.active = None;
        }
    }
    pub fn on_task_processed(&self, now: i64) {
        loop {
            let task = self.shared.state.lock().unwrap().finished.pop_front();
            if let Some((_, task)) = task {
                task.finish();
            } else {
                break;
            }
        }
        let s = self.shared.state.lock().unwrap();
        if self.stop_timeout > 0 && s.pending.is_empty() && s.active.is_none() {
            *self.stop_at.lock().unwrap() = Some(now + self.stop_timeout);
        }
    }
    pub fn next_deadline(&self) -> Option<i64> {
        *self.stop_at.lock().unwrap()
    }
    pub fn advance_to(&self, now: i64) {
        let due = self.stop_at.lock().unwrap().is_some_and(|at| at <= now);
        if due {
            *self.stop_at.lock().unwrap() = None;
            self.stop();
        }
    }
    /// Real MainQueue bridge; callback executes without queue/store locks.
    pub fn bind_main_queue(
        self: &Arc<Self>,
        main: tg_runtime::MainQueue,
        now: Arc<dyn Fn() -> i64 + Send + Sync>,
    ) {
        let weak = Arc::downgrade(self);
        *self.shared.processed.lock().unwrap() = Some(Arc::new(move || {
            let weak = weak.clone();
            let now = now.clone();
            main.post(move || {
                if let Some(queue) = weak.upgrade() {
                    queue.on_task_processed(now());
                }
            });
        }));
    }
    pub fn worker_running(&self) -> bool {
        self.shared.state.lock().unwrap().running
    }
    /// Waits for the current process(), cancels pending finishes as original stop.
    pub fn stop(&self) {
        let mut s = self.shared.state.lock().unwrap();
        if s.running {
            s.stopping = true;
            self.shared.wake.notify_one();
            while s.running {
                s = self.shared.stopped.wait(s).unwrap();
            }
        }
        s.pending.clear();
        s.finished.clear();
        s.active = None;
        s.stopping = false;
    }
}
impl Drop for TaskQueue {
    fn drop(&mut self) {
        self.stop();
    }
}
struct WorkerExit(Arc<SharedQueue>);
impl Drop for WorkerExit {
    fn drop(&mut self) {
        let mut s = self.0.state.lock().unwrap();
        s.running = false;
        s.active = None;
        self.0.stopped.notify_all();
    }
}
fn run_worker(shared: Arc<SharedQueue>) {
    let _exit = WorkerExit(shared.clone());
    loop {
        let (id, mut task) = {
            let mut s = shared.state.lock().unwrap();
            while s.pending.is_empty() && !s.stopping {
                s = shared.wake.wait(s).unwrap();
            }
            if s.stopping {
                return;
            }
            let task = s.pending.pop_front().unwrap();
            s.active = Some(task.0);
            task
        };
        task.process();
        let notify = {
            let mut s = shared.state.lock().unwrap();
            if s.active == Some(id) {
                s.active = None;
                let notify = s.finished.is_empty();
                s.finished.push_back((id, task));
                notify
            } else {
                false
            }
        };
        if notify {
            let callback = shared.processed.lock().unwrap().clone();
            if let Some(callback) = callback {
                callback();
            }
        }
    }
}

pub struct FileLoadWork {
    task: FileLoadTask,
    outcome: Option<ProcessOutcome>,
    done: Box<dyn FnOnce(FileLoadTask, ProcessOutcome) + Send>,
}
impl FileLoadTask {
    pub fn into_work(
        self,
        done: impl FnOnce(FileLoadTask, ProcessOutcome) + Send + 'static,
    ) -> FileLoadWork {
        FileLoadWork {
            task: self,
            outcome: None,
            done: Box::new(done),
        }
    }
}
impl Task for FileLoadWork {
    fn id(&self) -> TaskId {
        self.task.args.descriptor.task_id
    }
    fn process(&mut self) {
        self.outcome = Some(self.task.process());
    }
    fn finish(self: Box<Self>) {
        let Self {
            task,
            outcome,
            done,
        } = *self;
        done(task, outcome.expect("task processed before finish"));
    }
}

impl std::fmt::Debug for FilePrepareResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilePrepareResult")
            .field("id", &self.id)
            .field("filename", &self.filename)
            .field("filesize", &self.filesize())
            .finish_non_exhaustive()
    }
}
impl std::fmt::Debug for AlbumItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlbumItem")
            .field("task_id", &self.task_id)
            .field("message_id", &self.message_id)
            .finish_non_exhaustive()
    }
}

/// Actual archive backend on the prepared snapshot. The caller executes this
/// on tg-runtime and posts its real result/progress back to the owner.
pub fn write_archive_preparation(
    result: &FilePrepareResult,
    premium: bool,
    directory: &std::path::Path,
    progress: impl FnMut(f64) -> bool,
) -> Option<super::storage_folder_archive::ArchiveWriteResult> {
    let entries = result.archive_entries.lock().unwrap().take()?;
    Some(super::storage_folder_archive::write_archive(
        entries,
        if premium {
            FILE_SIZE_PREMIUM_LIMIT
        } else {
            FILE_SIZE_LIMIT
        },
        directory,
        progress,
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaProbe {
    Song,
    Video,
    Image,
    Lottie,
}
pub fn probe_plan(path: &str, mime: &str) -> Vec<MediaProbe> {
    let mut steps = Vec::new();
    if song_probe_allowed(path, mime) {
        steps.push(MediaProbe::Song);
    }
    if is_video_file(path, mime) {
        steps.push(MediaProbe::Video);
    }
    steps.push(if mime::qt_ascii_ci_suffix(path, ".tgs") {
        MediaProbe::Lottie
    } else {
        MediaProbe::Image
    });
    steps
}
pub fn accept_song_facts(mut song: prepare::SongFacts) -> Option<prepare::SongFacts> {
    if song.duration < 0 {
        return None;
    }
    if !prepare::validate_thumb_dimensions(song.cover.width, song.cover.height) {
        song.cover = MediaSize::new(0, 0);
    }
    Some(song)
}
pub fn accept_video_facts(path: &str, video: VideoFacts, mime: &mut String) -> Option<VideoFacts> {
    if video.duration < 0
        || !prepare::validate_thumb_dimensions(video.thumbnail.width, video.thumbnail.height)
    {
        return None;
    }
    if mime::qt_ascii_ci_suffix(path, ".mp4") {
        *mime = "video/mp4".into();
    }
    Some(video)
}
pub fn fill_image_information(
    image: prepare::ImageFacts,
    mime: String,
) -> Option<PreparedFileInformation> {
    if image.dimensions.is_empty() {
        None
    } else {
        Some(PreparedFileInformation {
            filemime: mime,
            media: MediaFacts::Image(image),
        })
    }
}
