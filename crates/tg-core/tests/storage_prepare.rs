use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicI64, Ordering},
        mpsc, Arc, Mutex,
    },
    time::Duration,
};
use tg_core::{
    data::{
        data_media_types::MediaSize,
        data_msg_id::{FullMsgId, MsgId},
        data_peer_id::{peer_from_user, UserId},
    },
    storage::{file_upload as u, localimageloader as l, storage_media_prepare as p},
};
use tg_scheme as tl;
fn msg(n: i64) -> FullMsgId {
    FullMsgId::new(peer_from_user(UserId::new(1)), MsgId::new(n))
}
fn image(w: i32, h: i32, animated: bool) -> p::PreparedFileInformation {
    p::PreparedFileInformation {
        filemime: "image/png".into(),
        media: p::MediaFacts::Image(p::ImageFacts {
            dimensions: MediaSize::new(w, h),
            animated,
            encoded: Vec::new(),
            format: "png".into(),
            animated_edit_scene: false,
        }),
    }
}
fn video() -> p::VideoFacts {
    p::VideoFacts {
        is_gifv: false,
        is_webm_sticker: false,
        supports_streaming: true,
        has_audio: true,
        duration: 2501,
        thumbnail: MediaSize::new(640, 480),
        gif: false,
        quality: 720,
        cover: 1234,
    }
}
fn file(kind: p::FileType) -> p::PreparedFile {
    let mut f = p::PreparedFile::new("x.bin");
    f.kind = kind;
    f.information = Some(p::PreparedFileInformation {
        filemime: "application/octet-stream".into(),
        media: p::MediaFacts::None,
    });
    f
}
fn args(kind: l::SendMediaType) -> l::FileLoadArgs {
    l::FileLoadArgs {
        descriptor: l::FilePrepareDescriptor::new(123),
        dc_id: 4,
        now: 100,
        filepath: PathBuf::new(),
        content: vec![3; 100],
        filename: "x.bin".into(),
        display_name: String::new(),
        pixel_file: None,
        archive: None,
        filemime: "application/octet-stream".into(),
        kind,
        information: Some(p::PreparedFileInformation {
            filemime: "application/octet-stream".into(),
            media: p::MediaFacts::None,
        }),
        duration: 2501,
        waveform: vec![0, 1, 31],
        force_file: false,
        video_edited: false,
        send_large_photos: false,
        album_present: false,
        photo: None,
        thumbnail: None,
        video_cover: None,
    }
}
fn ready(task: &mut l::FileLoadTask) -> Arc<l::FilePrepareResult> {
    match task.process() {
        l::ProcessOutcome::Ready(r) => r,
        l::ProcessOutcome::Needs(n) => panic!("unexpected requirement {n:?}"),
    }
}
fn list(types: &[p::FileType]) -> p::PreparedList {
    p::PreparedList {
        files: types.iter().copied().map(file).collect(),
        ..Default::default()
    }
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "storage-prepare-{}-{}",
            std::process::id(),
            l::TaskId::allocate().0
        ));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn native_original_hash_parts_and_pure_rules() {
    let mut state = l::FilePrepareResult::new(l::FilePrepareDescriptor::new(1));
    let mut current = None;
    let mut count = 0;
    for line in include_str!("fixtures/storage-prepare/native.jsonl").lines() {
        let r: serde_json::Value = serde_json::from_str(line).unwrap();
        match r["op"].as_str().unwrap() {
            "parts" => {
                let n = r["size"].as_u64().unwrap() as usize;
                if current != Some(n) {
                    current = Some(n);
                    state = l::FilePrepareResult::new(l::FilePrepareDescriptor::new(1));
                }
                let data: Vec<u8> = (0..n).map(|i| (17 * i + 3) as u8).collect();
                let b = match r["step"].as_i64().unwrap() {
                    0 | 3 => data.as_slice(),
                    1 => &[],
                    2 => b"abc",
                    _ => unreachable!(),
                };
                state.set_file_data(b);
                state.set_thumb_data(b);
                assert_eq!(state.partssize, r["partssize"].as_i64().unwrap());
                assert_eq!(String::from_utf8(state.filemd5.clone()).unwrap(), r["md5"]);
                assert_eq!(
                    String::from_utf8(state.thumbmd5.clone()).unwrap(),
                    r["thumbmd5"]
                );
                for (actual, key) in [
                    (&state.fileparts, "fileparts"),
                    (&state.thumbparts, "thumbparts"),
                ] {
                    let expected = r[key].as_array().unwrap();
                    assert_eq!(actual.len(), expected.len());
                    for (b, e) in actual.iter().zip(expected) {
                        assert_eq!(b.len() as u64, e["size"].as_u64().unwrap());
                        assert_eq!(String::from_utf8(l::md5_hex(b)).unwrap(), e["md5"]);
                    }
                }
                assert_eq!(
                    state.thumbbytes.len() as u64,
                    r["thumbbytes_size"].as_u64().unwrap()
                );
                assert_eq!(
                    String::from_utf8(l::md5_hex(&state.thumbbytes)).unwrap(),
                    r["thumbbytes_md5"]
                );
            }
            "thumb" => assert_eq!(
                l::file_thumbnail_upload_required(
                    r["mime"].as_str().unwrap(),
                    r["size"].as_i64().unwrap()
                ),
                r["required"].as_bool().unwrap()
            ),
            "mp4" => assert_eq!(l::mp4_file_name(r["name"].as_str().unwrap()), r["result"]),
            "dimensions" => assert_eq!(
                p::validate_thumb_dimensions(
                    r["w"].as_i64().unwrap() as i32,
                    r["h"].as_i64().unwrap() as i32
                ),
                r["valid"].as_bool().unwrap()
            ),
            "group" => {
                let t = [
                    p::FileType::None,
                    p::FileType::Photo,
                    p::FileType::Video,
                    p::FileType::Music,
                    p::FileType::File,
                ][r["type"].as_u64().unwrap() as usize];
                assert_eq!(
                    p::group_type_for_file(
                        t,
                        r["group"].as_bool().unwrap(),
                        r["photos"].as_bool().unwrap()
                    ) as i64,
                    r["result"].as_i64().unwrap()
                );
            }
            "side" => assert_eq!(
                i64::from(l::photo_side_limit(r["large"].as_bool().unwrap())),
                r["result"].as_i64().unwrap()
            ),
            _ => panic!(),
        }
        count += 1;
    }
    assert_eq!(count, 160);
}
#[test]
fn thumbnail_rounding_jpeg_threshold_and_document_layout() {
    assert_eq!(
        l::thumbnail_dimensions(MediaSize::new(641, 481)),
        Some(MediaSize::new(320, 240))
    );
    assert_eq!(
        l::thumbnail_dimensions(MediaSize::new(1, 20)),
        Some(MediaSize::new(1, 20))
    );
    assert_eq!(l::thumbnail_dimensions(MediaSize::new(1, 21)), None);
    let s = MediaSize::new(100, 100);
    assert_eq!(
        l::photo_jpeg_plan(s, 5000, "jpeg", true),
        l::JpegPlan::PreserveProgressive
    );
    assert_eq!(
        l::photo_jpeg_plan(s, 5000, "jpeg", false),
        l::JpegPlan::TryMakeProgressive
    );
    assert_eq!(
        l::photo_jpeg_plan(s, 5001, "jpeg", true),
        l::JpegPlan::Reencode
    );
    assert_eq!(
        l::photo_jpeg_plan(s, 0, "jpeg", true),
        l::JpegPlan::Reencode
    );
    assert_eq!(
        l::photo_jpeg_plan(s, 100, "JPEG", true),
        l::JpegPlan::Reencode
    );
    let r = l::FilePrepareResult::new(l::FilePrepareDescriptor::new(1));
    for size in [1, 1048576, 33554432, 33554433, 2_000 * 1048576] {
        r.set_filesize(size);
        assert_eq!(r.document_part_layout(), u::part_layout(size));
    }
}
#[test]
fn mime_probe_order_and_exact_mime_vs_extension() {
    assert_eq!(
        l::probe_plan("", "image/jpeg"),
        vec![l::MediaProbe::Song, l::MediaProbe::Image]
    );
    assert_eq!(
        l::probe_plan("a.mp4", "video/mp4"),
        vec![l::MediaProbe::Video, l::MediaProbe::Image]
    );
    assert_eq!(l::probe_plan("a.TGS", "x"), vec![l::MediaProbe::Lottie]);
    let mut mime = "other".into();
    assert!(l::accept_video_facts("a.MP4", video(), &mut mime).is_some());
    assert_eq!(mime, "video/mp4");
    assert!(l::accept_song_facts(p::SongFacts {
        duration: -1,
        title: String::new(),
        performer: String::new(),
        cover: MediaSize::new(1, 1)
    })
    .is_none());
    let song = l::accept_song_facts(p::SongFacts {
        duration: 0,
        title: String::new(),
        performer: String::new(),
        cover: MediaSize::new(1, 21),
    })
    .unwrap();
    assert!(song.cover.is_empty());
    let p::MediaFacts::Image(i) = image(30, 30, false).media else {
        panic!()
    };
    assert!(l::fill_image_information(i, "image/png".into()).is_some());
    assert!(l::song_probe_allowed("", "anything"));
    assert!(l::song_probe_allowed("a.MP3", "audio/mpeg"));
    assert!(!l::song_probe_allowed("a.bin", "audio/mpeg"));
    assert!(l::song_probe_allowed("a.bin", "audio/mp3"));
    assert!(l::is_video_file("a.WEBM", "other"));
    assert!(!l::is_video_file("", "video/webm"));
    assert!(!l::is_video_file("a.bin", "VIDEO/MP4"));
    assert!(l::is_video_file("a.bin", "video/quicktime"));
    let mut v = video();
    assert!(l::video_probe_accepted("a.mp4", &v));
    v.thumbnail = MediaSize::new(1, 21);
    assert!(!l::video_probe_accepted("a.mp4", &v));
    v = video();
    assert!(l::video_conversion_required(
        "video/webm",
        100,
        &v,
        false,
        false
    ));
    assert!(!l::video_conversion_required(
        "video/webm",
        1000 * 1048576,
        &v,
        false,
        false
    ));
    assert!(!l::video_conversion_required(
        "video/webm",
        100,
        &v,
        true,
        false
    ));
    v.is_webm_sticker = true;
    assert!(!l::video_conversion_required(
        "video/webm",
        100,
        &v,
        false,
        false
    ));
}
#[test]
fn media_metadata_types_dimensions_and_preview_boundary() {
    let mut f = file(p::FileType::File);
    f.information = Some(image(2560, 1920, false));
    f.prepare_details(1280);
    assert_eq!(f.kind, p::FileType::Photo);
    assert_eq!(f.shown_dimensions, MediaSize::new(1280, 960));
    assert!(f.can_use_high_quality_photo());
    assert!(!f.is_sticker());
    f.information = Some(image(1280, 1, false));
    f.prepare_details(1280);
    assert_eq!(f.kind, p::FileType::Photo); // invalid dimensions leave prior type untouched.
    f.information = Some(image(1280, 1, true));
    f.prepare_details(1280);
    assert_eq!(f.kind, p::FileType::None);
    f.information = Some(p::PreparedFileInformation {
        filemime: "video/mp4".into(),
        media: p::MediaFacts::Video(video()),
    });
    f.prepare_details(1280);
    assert_eq!(f.kind, p::FileType::Video);
    assert!(f.is_video_file());
    assert!(!f.is_gifv());
    assert!(f.can_edit_video());
    assert_eq!(f.video_quality(), 720);
    if let Some(p::PreparedFileInformation {
        media: p::MediaFacts::Video(v),
        ..
    }) = &mut f.information
    {
        v.gif = true;
    }
    assert!(f.sends_video_as_gif());
}
#[test]
fn group_ranges_all_kinds_slowmode_gifs_and_none_runs() {
    use p::FileType::*;
    let way = p::SendFilesWay::default();
    let l = list(&[Photo, Video, Music, Music, File, Photo, None, None]);
    let ranges = p::group_ranges(&l.files, way, false);
    assert_eq!(
        ranges
            .iter()
            .map(|r| (r.from, r.till, r.kind))
            .collect::<Vec<_>>(),
        vec![
            (0, 2, p::AlbumType::PhotoVideo),
            (2, 4, p::AlbumType::Music),
            (4, 5, p::AlbumType::None),
            (5, 6, p::AlbumType::None),
            (6, 8, p::AlbumType::None)
        ]
    );
    let mut way = way;
    way.set_group_files(false);
    assert_eq!(p::group_ranges(&l.files, way, false).len(), 1);
    assert_eq!(p::group_ranges(&l.files, way, true).len(), 5);
    let groups = p::divide_by_groups(list(&[Photo; 21]), p::SendFilesWay::default(), false);
    assert_eq!(
        groups
            .iter()
            .map(|g| g.list.files.len())
            .collect::<Vec<_>>(),
        vec![10, 10, 1]
    );
    let bundle = p::prepare_files_bundle(groups, p::SendFilesWay::default(), true);
    assert_eq!(bundle.total_count, 21);
    assert!(bundle.ctrl_shift_enter);
    let mut l = list(&[Photo, Photo, Photo]);
    l.files[1].animation_job_present = true;
    assert_eq!(
        p::group_ranges(&l.files, p::SendFilesWay::default(), false).len(),
        3
    );
}
#[test]
fn prepared_list_options_and_reorder_drop_deferred_fields() {
    use p::FileType::*;
    let mut l = list(&[Photo, Photo]);
    assert!(l.can_be_sent_in_slowmode());
    assert!(l.has_group_option(false));
    assert!(l.has_send_images_as_photos_option(false));
    assert!(l.can_add_caption(true));
    assert!(l.can_move_caption(true, true));
    assert!(l.can_change_price(true, true));
    assert!(l.has_spoiler_menu(true));
    assert!(!l.has_spoiler_menu(false));
    assert!(!l.has_group_option(true));
    assert!(!list(&[File, Video]).can_be_sent_in_slowmode());
    assert!(!list(&[Music, Photo]).can_be_sent_in_slowmode());
    assert!(list(&[Music, Music]).can_be_sent_in_slowmode());
    assert!(!list(&[Photo; 11]).can_be_sent_in_slowmode());
    l.files[1].path = "a.TGS".into();
    assert!(!l.can_add_caption(true));
    l.files[1].path = "x".into();
    l.files[1].information = Some(image(2560, 1, false));
    assert!(l.can_have_editor_hint_label());
    assert!(!l.has_sticker());
    l.files[1].information.as_mut().unwrap().filemime = "image/webp".into();
    assert!(l.has_sticker());
    assert!(!l.can_add_caption(false));
    l.override_send_images_as_photos = Some(false);
    l.files_to_process.push_back(file(File));
    l.files[0].id = 1;
    l.files[1].id = 2;
    let r = p::PreparedList::reordered(l, &[1, 0]);
    assert_eq!(r.files[0].id, 2);
    assert!(r.files_to_process.is_empty());
    assert!(r.override_send_images_as_photos.is_none());
    let mut l = list(&[Photo; 9]);
    l.merge_to_end(list(&[Photo, Photo]), true);
    assert_eq!(l.files.len(), 10);
    l.merge_to_end(
        p::PreparedList::error(p::PrepareError::Directory, "bad"),
        false,
    );
    assert_eq!(l.error, p::PrepareError::Directory);
    assert!(p::PreparedList::default().has_spoiler_menu(false));
    assert!(!p::insert_text_on_image_cancel("data:image/png"));
    assert!(p::insert_text_on_image_cancel("Data:image"));
}
#[test]
fn send_way_serialization_and_compressed_sticker_dependencies() {
    assert_eq!(l::SendMediaType::Audio as u8, 1);
    assert_eq!(l::SendMediaType::File as u8, 3);
    let mut sticker_way = p::SendFilesWay::default();
    let plain = sticker_way;
    sticker_way.set_has_compressed_stickers(true);
    assert_ne!(sticker_way, plain);
    assert_eq!(sticker_way.serialize(), plain.serialize());
    for n in 0..8 {
        let w = p::SendFilesWay::from_serialized(n).unwrap();
        assert_eq!(w.serialize(), n);
    }
    assert!(p::SendFilesWay::from_serialized(-1).is_none());
    assert!(p::SendFilesWay::from_serialized(8).is_none());
    let mut w = p::SendFilesWay::default();
    w.set_has_compressed_stickers(true);
    w.set_send_images_as_photos(false);
    assert!(!w.group_files());
    w.set_group_files(true);
    assert!(w.send_images_as_photos());
    w.set_has_compressed_stickers(false);
    w.set_send_large_photos(false);
    assert_eq!(w.serialize(), 0);
}
#[test]
fn filesystem_validation_errors_callbacks_and_deferred_eleven() {
    let dir = Temp::new();
    let empty = dir.0.join("empty");
    std::fs::write(&empty, []).unwrap();
    let good = dir.0.join("good");
    std::fs::write(&good, [1]).unwrap();
    let big = dir.0.join("big");
    let b = std::fs::File::create(&big).unwrap();
    b.set_len((l::FILE_SIZE_LIMIT + 1) as u64).unwrap();
    assert_eq!(
        p::prepare_media_list(std::slice::from_ref(&dir.0), false, None).error,
        p::PrepareError::Directory
    );
    assert_eq!(
        p::prepare_media_list(std::slice::from_ref(&empty), false, None).error,
        p::PrepareError::EmptyFile
    );
    assert_eq!(
        p::prepare_media_list(&[dir.0.join("missing")], false, None).error,
        p::PrepareError::EmptyFile
    );
    let r = p::prepare_media_list(std::slice::from_ref(&big), false, None);
    assert_eq!(r.error, p::PrepareError::TooLargeFile);
    assert_eq!(r.files[0].size, l::FILE_SIZE_LIMIT + 1);
    assert_eq!(
        p::prepare_media_list(std::slice::from_ref(&big), true, None).error,
        p::PrepareError::None
    );
    b.set_len(l::FILE_SIZE_PREMIUM_LIMIT as u64).unwrap();
    assert_eq!(
        p::prepare_media_list(std::slice::from_ref(&big), true, None).error,
        p::PrepareError::None
    );
    b.set_len((l::FILE_SIZE_PREMIUM_LIMIT + 1) as u64).unwrap();
    assert_eq!(
        p::prepare_media_list(std::slice::from_ref(&big), true, None).error,
        p::PrepareError::TooLargeFile
    );
    let mut errors = Vec::new();
    let r = p::prepare_media_list(
        &[empty, good.clone(), big],
        false,
        Some(&mut |e| errors.push(e.error)),
    );
    assert_eq!(
        errors,
        vec![p::PrepareError::EmptyFile, p::PrepareError::TooLargeFile]
    );
    assert_eq!(r.files.len(), 1);
    let r = p::prepare_media_list(&vec![good; 11], false, None);
    assert_eq!(r.files.len(), 10);
    assert_eq!(r.files_to_process.len(), 1);
    let mut errors = Vec::new();
    let r = p::prepare_media_urls(
        &[
            p::LocalUrl::NonLocal("https://x".into()),
            p::LocalUrl::Local(dir.0.join("good")),
        ],
        false,
        Some(&mut |e| errors.push(e.error)),
    );
    assert_eq!(r.files.len(), 1);
    assert_eq!(errors, [p::PrepareError::NonLocalUrl]);
}
#[test]
fn drag_facts_local_forward_size_gif_and_editor_rules() {
    let mut f = p::DragFileFacts {
        local: true,
        directory: false,
        path: "a.jpg".into(),
        size: p::READ_BYTES_LIMIT,
        mime: "image/jpeg".into(),
        image_readable: true,
    };
    assert_eq!(
        p::compute_mime_data_state(false, false, &[f.clone()]),
        p::MimeDataState::PhotoFiles
    );
    f.size += 1;
    assert_eq!(
        p::compute_mime_data_state(false, false, &[f.clone()]),
        p::MimeDataState::MediaFiles
    );
    assert_eq!(
        p::compute_mime_data_state(true, true, &[f.clone()]),
        p::MimeDataState::None
    );
    assert_eq!(
        p::compute_mime_data_state(false, true, &[]),
        p::MimeDataState::Image
    );
    f.path = "a.gif".into();
    f.mime = "image/gif".into();
    assert_eq!(
        p::compute_mime_data_state(false, false, &[f.clone()]),
        p::MimeDataState::MediaFiles
    );
    f.local = false;
    assert_eq!(
        p::compute_mime_data_state(false, false, &[f.clone()]),
        p::MimeDataState::None
    );
    f.local = true;
    f.directory = true;
    assert_eq!(
        p::compute_mime_data_state(false, false, &[f.clone()]),
        p::MimeDataState::Folder
    );
    assert!(!p::validate_photo_editor_media_drag_data(
        false,
        &[f.clone(), f.clone()],
        true
    ));
    assert!(p::validate_photo_editor_media_drag_data(true, &[], false));
    assert!(!p::validate_edit_media_drag_data(
        true,
        &[],
        p::AlbumType::Music
    ));
    assert!(!p::photo_editor_read_allowed(
        &PathBuf::new(),
        p::READ_BYTES_LIMIT as usize + 1,
        0,
        false
    ));
}
#[test]
fn process_generic_voice_song_video_and_forced_file_attributes() {
    let mut task = l::FileLoadTask::new(args(l::SendMediaType::File));
    let r = ready(&mut task);
    assert!(matches!(r.document, tl::Document::Document(_)));
    assert_eq!(r.kind, l::SendMediaType::File);
    assert_eq!(task.fileid(), 123);
    assert!(task.peek_result().is_some());
    assert!(matches!(
        task.finish(false, false),
        l::FinishOutcome::SessionGone
    ));
    assert!(matches!(
        task.finish(true, false),
        l::FinishOutcome::SendConfirmed(_)
    ));
    let mut task = l::FileLoadTask::new(args(l::SendMediaType::Audio));
    let r = ready(&mut task);
    let d = r.document.c_document();
    assert_eq!(d.mime_type, b"audio/ogg");
    let tl::DocumentAttribute::DocumentAttributeAudio(a) = &d.attributes[0] else {
        panic!()
    };
    assert!(a.is_voice());
    assert_eq!(a.duration, 2);
    assert_eq!(d.attributes.len(), 1);
    let mut a = args(l::SendMediaType::File);
    a.information = Some(p::PreparedFileInformation {
        filemime: "audio/mp3".into(),
        media: p::MediaFacts::Song(p::SongFacts {
            duration: 2999,
            title: "t".into(),
            performer: "p".into(),
            cover: MediaSize::new(0, 0),
        }),
    });
    let r = ready(&mut l::FileLoadTask::new(a));
    assert!(
        matches!(&r.document.c_document().attributes[1],tl::DocumentAttribute::DocumentAttributeAudio(d)if d.duration==2)
    );
    for force in [false, true] {
        let mut a = args(l::SendMediaType::File);
        a.filemime = "video/mp4".into();
        a.force_file = force;
        a.information = Some(p::PreparedFileInformation {
            filemime: a.filemime.clone(),
            media: p::MediaFacts::Video(video()),
        });
        let r = ready(&mut l::FileLoadTask::new(a));
        assert_eq!(
            r.document
                .c_document()
                .attributes
                .iter()
                .any(|a| matches!(a, tl::DocumentAttribute::DocumentAttributeVideo(_))),
            !force
        );
    }
}
#[test]
fn process_image_photo_sticker_animation_and_missing_backend_boundaries() {
    let mut a = args(l::SendMediaType::Photo);
    a.information = None;
    assert!(matches!(
        l::FileLoadTask::new(a).process(),
        l::ProcessOutcome::Needs(l::PreparationNeed::MediaInformation)
    ));
    let mut a = args(l::SendMediaType::Photo);
    a.information = Some(image(640, 480, false));
    a.filemime = "image/png".into();
    assert!(matches!(
        l::FileLoadTask::new(a.clone()).process(),
        l::ProcessOutcome::Needs(l::PreparationNeed::ImageEncoding)
    ));
    a.photo = Some(l::EncodedPhoto {
        dimensions: MediaSize::new(640, 480),
        medium_dimensions: MediaSize::new(320, 240),
        jpeg: vec![1; 70_000],
    });
    let r = ready(&mut l::FileLoadTask::new(a));
    assert_eq!(r.kind, l::SendMediaType::Photo);
    assert_eq!(r.fileparts.len(), 3);
    assert_eq!(r.photo.c_photo().sizes.len(), 2);
    let mut a = args(l::SendMediaType::File);
    a.filemime = "image/webp".into();
    a.information = Some(image(512, 512, false));
    let r = ready(&mut l::FileLoadTask::new(a));
    assert!(r
        .document
        .c_document()
        .attributes
        .iter()
        .any(|a| matches!(a, tl::DocumentAttribute::DocumentAttributeSticker(_))));
    // Source only copies animated into its local flag for an existing file.
    let dir = Temp::new();
    let path = dir.0.join("a.gif");
    std::fs::write(&path, [1; 100]).unwrap();
    let mut a = args(l::SendMediaType::File);
    a.filemime = "image/gif".into();
    a.information = Some(image(30, 30, true));
    let r = ready(&mut l::FileLoadTask::new(a.clone()));
    assert!(!r
        .document
        .c_document()
        .attributes
        .iter()
        .any(|a| matches!(a, tl::DocumentAttribute::DocumentAttributeAnimated(_))));
    a.filepath = path;
    let r = ready(&mut l::FileLoadTask::new(a));
    assert!(r
        .document
        .c_document()
        .attributes
        .iter()
        .any(|a| matches!(a, tl::DocumentAttribute::DocumentAttributeAnimated(_))));
}
#[test]
fn process_directory_empty_limits_and_pixel_requirements() {
    let dir = Temp::new();
    let mut a = args(l::SendMediaType::File);
    a.filepath = dir.0.clone();
    let mut task = l::FileLoadTask::new(a);
    assert_eq!(ready(&mut task).filesize(), -1);
    assert!(matches!(task.finish(true, false), l::FinishOutcome::Empty));
    let mut a = args(l::SendMediaType::File);
    a.content.clear();
    let mut task = l::FileLoadTask::new(a);
    assert_eq!(ready(&mut task).filesize(), 0);
    let path = dir.0.join("large");
    std::fs::File::create(&path)
        .unwrap()
        .set_len((l::FILE_SIZE_LIMIT + 1) as u64)
        .unwrap();
    let mut a = args(l::SendMediaType::File);
    a.filepath = path;
    let mut task = l::FileLoadTask::new(a);
    ready(&mut task);
    assert!(matches!(
        task.finish(true, false),
        l::FinishOutcome::TooLarge(_)
    ));
    assert!(matches!(
        task.finish(true, true),
        l::FinishOutcome::SendConfirmed(_)
    ));
    let mut a = args(l::SendMediaType::File);
    a.content.clear();
    a.information = Some(image(20, 20, false));
    assert!(matches!(
        l::FileLoadTask::new(a.clone()).process(),
        l::ProcessOutcome::Needs(l::PreparationNeed::ImageEncoding)
    ));
    a.pixel_file = Some(vec![1; 200]);
    let r = ready(&mut l::FileLoadTask::new(a));
    assert_eq!(r.filemime, "image/png");
    assert_eq!(r.filesize(), 200);
}
#[test]
fn sending_album_music_take_media_and_caption_transfer_contract() {
    let r = Arc::new(l::FilePrepareResult::new(l::FilePrepareDescriptor::new(1)));
    let mut album = l::SendingAlbum::new(44);
    let t1 = l::TaskId::allocate();
    let t2 = l::TaskId::allocate();
    let mut i1 = l::AlbumItem::new(t1);
    i1.message_id = msg(1);
    let mut i2 = l::AlbumItem::new(t2);
    i2.message_id = msg(2);
    album.items = vec![i1, i2];
    album.music_prepared_batching = true;
    assert!(!album.prepared_music_ready());
    assert!(!album.finish_music_item(t1, r.clone()));
    assert!(album.prepared_music_sample().is_some());
    assert!(album.finish_music_item(t2, r));
    assert_eq!(album.take_prepared_music().len(), 2);
    assert!(!album.prepared_music_ready());
    let media = tl::mtp_input_single_media(
        tl::tl::Flags::from_raw(0),
        tl::mtp_input_media_empty(),
        7,
        b"x".to_vec(),
        None,
    );
    album.fill_media(msg(1), media.clone());
    album.refresh_media_caption(
        msg(1),
        tl::mtp_input_single_media(
            tl::tl::Flags::from_raw(0),
            tl::mtp_input_media_empty(),
            999,
            b"new".to_vec(),
            None,
        ),
    );
    assert_eq!(
        album.items[0]
            .media
            .as_ref()
            .unwrap()
            .c_input_single_media()
            .random_id,
        7
    );
    assert_eq!(
        album.items[0]
            .media
            .as_ref()
            .unwrap()
            .c_input_single_media()
            .message,
        b"new"
    );
    assert_eq!(album.items[0].random_id, 7);
    assert_eq!(album.remove_item(msg(1)), Some(msg(2)));
    album.remove_task(t2);
    assert!(album.items.is_empty());
}
#[test]
fn archive_preparation_real_writer_upload_bridge_and_temp_lease() {
    let dir = Temp::new();
    let path = dir.0.join("source");
    std::fs::write(&path, b"archive contents").unwrap();
    let mut a = args(l::SendMediaType::File);
    a.content.clear();
    a.information = None;
    a.archive = Some(tg_core::storage::storage_folder_archive::ArchiveJob {
        paths: vec![path],
        ..Default::default()
    });
    let mut task = l::FileLoadTask::new(a);
    let r = ready(&mut task);
    assert_eq!(r.filemime, "application/zip");
    assert!(r.has_archive_entries());
    let written = l::write_archive_preparation(&r, false, &dir.0.join("archives"), |_| {
        assert!(!r.has_archive_entries());
        true
    })
    .unwrap();
    assert_eq!(
        written.status,
        tg_core::storage::storage_folder_archive::Status::Done
    );
    let temp = written.path.clone();
    assert!(temp.exists());
    let mut uploader = u::Uploader::default();
    uploader.upload(msg(1), r.prepared_upload(), 1);
    uploader.take_effects();
    uploader.finish_preparation(
        msg(1),
        u::Preparation::Archive,
        Some(u::Content::Path(written.path)),
        2,
    );
    uploader.take_effects();
    assert_eq!(r.filesize(), written.size);
    assert_eq!(r.filepath(), temp);
    assert!(r.content().is_empty());
    assert!(!r.has_archive_entries());
    drop(task);
    drop(r);
    assert!(temp.exists());
    uploader.cancel_all(3);
    let mut work = uploader.take_effects();
    while !work.is_empty() {
        for e in work {
            uploader.apply_continuation(&e, 3);
        }
        work = uploader.take_effects();
    }
    assert!(!temp.exists());
}
struct TestTask {
    id: l::TaskId,
    process: Option<Box<dyn FnOnce() + Send>>,
    finish: Option<Box<dyn FnOnce() + Send>>,
}
impl l::Task for TestTask {
    fn id(&self) -> l::TaskId {
        self.id
    }
    fn process(&mut self) {
        self.process.take().unwrap()();
    }
    fn finish(mut self: Box<Self>) {
        self.finish.take().unwrap()();
    }
}
#[test]
fn task_queue_fifo_cancel_in_process_finish_reentrancy_and_virtual_stop() {
    let queue = Arc::new(l::TaskQueue::new(100));
    let main = tg_runtime::MainQueue::new();
    main.bind_current_thread();
    let (wake, wake_rx) = mpsc::channel();
    main.set_waker(Some(Arc::new(move || {
        let _ = wake.send(());
    })));
    let now = Arc::new(AtomicI64::new(10));
    let clock = now.clone();
    queue.bind_main_queue(main.clone(), Arc::new(move || clock.load(Ordering::SeqCst)));
    let (started, rx) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let (done, done_rx) = mpsc::channel();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let out = calls.clone();
    let out2 = calls.clone();
    let id = l::TaskId::allocate();
    queue.add_task(Box::new(TestTask {
        id,
        process: Some(Box::new(move || {
            started.send(()).unwrap();
            wait.recv().unwrap();
            out.lock().unwrap().push(1);
        })),
        finish: Some(Box::new(move || out2.lock().unwrap().push(9))),
    }));
    rx.recv_timeout(Duration::from_secs(10)).unwrap();
    queue.cancel_task(id);
    let q = queue.clone();
    let out = calls.clone();
    let out2 = calls.clone();
    queue.add_task(Box::new(TestTask {
        id: l::TaskId::allocate(),
        process: Some(Box::new(move || {
            out.lock().unwrap().push(2);
            done.send(()).unwrap();
        })),
        finish: Some(Box::new(move || {
            out2.lock().unwrap().push(3);
            q.cancel_task(l::TaskId(0));
        })),
    }));
    release.send(()).unwrap();
    done_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    // Finish-phase barrier, not process-entry: no sleep or enqueue/finish race.
    let last = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let finished = last.clone();
    let (tx, rx) = mpsc::channel();
    queue.add_task(Box::new(TestTask {
        id: l::TaskId::allocate(),
        process: Some(Box::new(move || {
            tx.send(()).unwrap();
        })),
        finish: Some(Box::new(move || {
            finished.store(true, Ordering::SeqCst);
        })),
    }));
    rx.recv_timeout(Duration::from_secs(10)).unwrap();
    while !last.load(Ordering::SeqCst) {
        main.process_all();
        if !last.load(Ordering::SeqCst) {
            wake_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        }
    }
    assert_eq!(*calls.lock().unwrap(), vec![1, 2, 3]);
    assert_eq!(queue.next_deadline(), Some(110));
    queue.advance_to(109);
    assert!(queue.worker_running());
    queue.advance_to(110);
    assert!(!queue.worker_running());
}
#[test]
fn file_load_work_runs_process_on_worker_and_finish_on_main() {
    let main_thread = std::thread::current().id();
    let queue = Arc::new(l::TaskQueue::new(0));
    let main = tg_runtime::MainQueue::new();
    main.bind_current_thread();
    queue.bind_main_queue(main.clone(), Arc::new(|| 0));
    let (tx, rx) = mpsc::channel();
    main.set_waker(Some(Arc::new(move || {
        let _ = tx.send(());
    })));
    let (done, got) = mpsc::channel();
    let task = l::FileLoadTask::new(args(l::SendMediaType::Audio));
    let task_id = task.args.descriptor.task_id;
    assert_eq!(
        queue.add_task(Box::new(task.into_work(move |_, result| {
            assert_eq!(std::thread::current().id(), main_thread);
            assert!(matches!(result, l::ProcessOutcome::Ready(_)));
            done.send(()).unwrap();
        }))),
        task_id
    );
    rx.recv_timeout(Duration::from_secs(10)).unwrap();
    main.process_all();
    got.recv_timeout(Duration::from_secs(10)).unwrap();
    queue.stop();
    assert!(!queue.worker_running());
}

#[test]
fn album_membership_and_high_quality_predicates() {
    use p::{AlbumType as A, FileType as F};
    for kind in [F::None, F::Photo, F::Video, F::Music, F::File] {
        let f = file(kind);
        assert_eq!(
            f.can_be_in_album_type(A::PhotoVideo),
            matches!(kind, F::Photo | F::Video)
        );
        assert_eq!(
            f.can_be_in_album_type(A::File),
            matches!(kind, F::Photo | F::Video | F::File)
        );
        assert_eq!(f.can_be_in_album_type(A::Music), kind == F::Music);
    }
    assert_eq!(file(F::Video).album_type(false), A::PhotoVideo);
    assert_eq!(file(F::Photo).album_type(false), A::File);
    let mut f = file(F::Photo);
    f.information = Some(image(1280, 1280, false));
    f.original_dimensions = MediaSize::new(1280, 1280);
    assert!(!f.can_use_high_quality_photo());
    f.original_dimensions.width += 1;
    assert!(f.can_use_high_quality_photo());
    let mut prepared = list(&[]);
    prepared.files.push(f);
    assert!(prepared.has_send_large_photos_option(true));
    assert!(!prepared.has_send_large_photos_option(false));
    let bundle = p::make_single_file_bundle(list(&[F::File]));
    assert_eq!(bundle.total_count, 1);
    assert_eq!(bundle.groups[0].kind, A::None);
}

#[test]
fn upload_projection_is_real_parts_with_thumbnail_and_temp_cleanup() {
    let mut a = args(l::SendMediaType::File);
    a.information = Some(image(500, 500, false));
    a.filemime = "image/png".into();
    a.display_name = "override.png".into();
    let mut task = l::FileLoadTask::new(a);
    let r = ready(&mut task);
    let d = r.document.c_document();
    assert!(
        matches!(&d.attributes[0],tl::DocumentAttribute::DocumentAttributeFilename(n)if n.file_name==b"override.png")
    );
    assert!(d.thumbs.is_none());
    assert_eq!(r.thumbnail_dimensions, Some(MediaSize::new(320, 320)));
    // Source flags=0 hides the provided thumb vector from getter and wire.
    assert_eq!(tg_tl::serialize(&r.document)[1], 0);
    let upload = r.prepared_upload();
    assert_eq!(upload.parts.len(), 0);
    assert!(upload.preparation_lease.is_some());
    assert_eq!(upload.filename, "x.bin");
    let mut a = args(l::SendMediaType::File);
    a.filemime = "application/x-tgsticker".into();
    a.information = Some(image(500, 500, true));
    assert!(matches!(
        l::FileLoadTask::new(a.clone()).process(),
        l::ProcessOutcome::Needs(l::PreparationNeed::ImageEncoding)
    ));
    a.thumbnail = Some(l::EncodedThumbnail {
        id: 77,
        dimensions: MediaSize::new(320, 320),
        bytes: vec![5; 32769],
        good_bytes: vec![1],
    });
    let r = ready(&mut l::FileLoadTask::new(a));
    assert_eq!(r.thumbparts.len(), 2);
    assert_eq!(r.thumbname, "thumb.webp");
    let upload = r.prepared_upload();
    assert_eq!(upload.parts.len(), 2);
    assert_eq!(upload.parts_id, 77);
}

#[test]
fn temp_file_survives_ready_callback_then_final_prepared_lease_drops() {
    let dir = Temp::new();
    let path = dir.0.join("generated");
    std::fs::write(&path, b"temp").unwrap();
    let mut result = l::FilePrepareResult::new(l::FilePrepareDescriptor::new(123));
    result.materialize(&u::Content::Bytes(vec![1; 10]), 10);
    result.filename = "file".into();
    result.set_transcoded_temp_path(path.clone());
    let result = Arc::new(result);
    let mut uploader = u::Uploader::default();
    uploader.upload(msg(1), result.prepared_upload(), 1);
    drop(result);
    let id = uploader
        .take_effects()
        .into_iter()
        .find_map(|e| match e {
            u::Effect::Send { id, .. } => Some(id),
            _ => None,
        })
        .unwrap();
    uploader.part_loaded(id, true, 2);
    let events = u::UploadEvents::default();
    let life = tg_rpl::Lifetime::new();
    let seen = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed = seen.clone();
    let expected = path.clone();
    events.document_ready().on_next(
        move |_| {
            assert!(expected.exists());
            observed.store(true, Ordering::SeqCst);
        },
        &life,
    );
    let mut work = std::collections::VecDeque::from(uploader.take_effects());
    while let Some(e) = work.pop_front() {
        events.apply(&e);
        uploader.apply_continuation(&e, 2);
        for nested in uploader.take_effects().into_iter().rev() {
            work.push_front(nested);
        }
    }
    assert!(seen.load(Ordering::SeqCst));
    assert!(!path.exists());
}
