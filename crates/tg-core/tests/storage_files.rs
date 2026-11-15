use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    rc::Rc,
    time::{Duration, SystemTime},
};
use tg_core::{
    data::{
        data_file_origin::FileOrigin, data_image_location::*, data_peer_id::*, data_store_keys::*,
    },
    storage::{
        file_download::{self as download, *},
        file_download_mtproto::*,
        file_download_web::*,
        serialize_common::{ByteArrayReader as Reader, ByteArrayWriter as Writer},
        serialize_document::*,
        serialize_location::*,
        serialize_peer::{self as peer, PeerFields},
        storage_folder_archive::*,
        streamed_file_downloader::*,
    },
};
fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
fn rows() -> impl Iterator<Item = serde_json::Value> {
    include_str!("fixtures/storage-files/native.jsonl")
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
}
fn config(scope: &SessionScope, size: i64) -> LoaderConfig {
    LoaderConfig {
        session: scope.id(),
        filename: PathBuf::new(),
        load_size: size,
        full_size: size,
        location_type: LocationType::Unknown,
        to_cache: LoadToCacheSetting::CacheAsWell,
        from_cloud: LoadFromCloudSetting::CloudOrLocal,
        auto_loading: true,
        cache_tag: 7,
    }
}
fn file(scope: &SessionScope, size: i64) -> FileLoader {
    FileLoader::new(config(scope, size), Default::default(), None).unwrap()
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let mut random = [0; 16];
        tg_base::random::random_fill(&mut random);
        let name = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let p = std::env::temp_dir().join(format!("gramtele-storage-files-{name}"));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn native_peer_stream_versions_null_utf16_creator_and_phone_prefix() {
    let mut count = 0;
    for row in rows().filter(|r| r["op"] == "peer") {
        let bytes = unhex(row["hex"].as_str().unwrap());
        let app = row["app"].as_i64().unwrap() as i32;
        let mut r = Reader::new(&bytes);
        let p = peer::read_peer(app, &mut r).unwrap();
        assert!(r.at_end(), "{row}");
        assert_eq!(p.version, row["version"].as_i64().unwrap() as i32);
        assert_eq!(p.id.value & 0xffff, 123);
        if let PeerFields::User {
            phone,
            first,
            flags,
            supports_guest_chat,
            ..
        } = &p.fields
        {
            assert_eq!(peer::text(phone).unwrap(), "+123");
            assert_eq!(first.is_none(), row["nulls"].as_bool().unwrap());
            assert_eq!(*flags, if app >= 9012 { 17 } else { 0 });
            assert_eq!(*supports_guest_chat, if p.version > 2 { 1 } else { 0 });
            let mut peek = Reader::new(&bytes);
            assert_eq!(&peer::peek_user_phone(app, &mut peek).unwrap(), phone);
            assert!(peek.position < bytes.len());
        }
        if let PeerFields::Chat { creator, .. } = &p.fields {
            assert_eq!(creator.bare, 0x12345689abcdef);
        }
        if p.version == 3 {
            let mut w = Writer::default();
            peer::write_peer(&mut w, &p).unwrap();
            assert_eq!(w.0, bytes);
            assert_eq!(peer::peer_size(&p), Some(bytes.len()));
        }
        for n in 0..bytes.len() {
            assert!(
                peer::read_peer(app, &mut Reader::new(&bytes[..n])).is_none(),
                "{row} truncated{n}"
            );
        }
        count += 1;
    }
    assert_eq!(count, 48);
}
#[test]
fn native_document_current_bytes_legacy_reads_size_and_rewrite() {
    let mut count = 0;
    for row in rows().filter(|r| r["op"] == "document") {
        let bytes = unhex(row["hex"].as_str().unwrap());
        let app = row["app"].as_i64().unwrap() as i32;
        let mut r = Reader::new(&bytes);
        let d = DocumentRecord::read_from_stream(app, &mut r, None).unwrap();
        assert!(r.at_end(), "{row}");
        assert_eq!(d.kind, row["kind"].as_i64().unwrap() as i32);
        assert_eq!(d.size, 0xf0000001);
        assert_eq!(d.version, row["read_version"].as_i64().unwrap() as i32);
        assert_eq!(d.dc, row["read_dc"].as_i64().unwrap() as i32);
        assert_eq!(d.id, row["read_id"].as_f64().unwrap() as u64);
        assert_eq!(row["read_ok"], true);
        assert_eq!(d.version, row["version"].as_i64().unwrap().max(0) as i32);
        assert_eq!(d.name.is_none(), row["nulls"].as_bool().unwrap());
        let mut w = Writer::default();
        d.write_to_stream(&mut w).unwrap();
        let round =
            DocumentRecord::read_from_stream(2008007, &mut Reader::new(&w.0), None).unwrap();
        assert_eq!(round.duration, d.duration);
        assert_eq!(round.name, d.name);
        if d.version == 6 {
            assert_eq!(w.0, bytes);
            assert_eq!(
                bytes.len() - d.size_in_stream(),
                if d.kind == 3 { 16 } else { 4 }
            );
        }
        for n in 0..bytes.len() {
            assert!(
                DocumentRecord::read_from_stream(app, &mut Reader::new(&bytes[..n]), None)
                    .is_none(),
                "{row} truncated{n}"
            );
        }
        count += 1;
    }
    assert_eq!(count, 138);
}
#[test]
fn native_archive_names_estimates_http_url_and_zip_bytes() {
    let mut counts = BTreeMap::new();
    for row in rows() {
        match row["op"].as_str().unwrap() {
            "compressed" => assert_eq!(
                already_compressed_name(row["name"].as_str().unwrap()),
                row["value"].as_bool().unwrap()
            ),
            "dedup" => assert_eq!(
                deduped_name(&mut counts, row["name"].as_str().unwrap()),
                row["value"].as_str().unwrap()
            ),
            "estimate" => {
                let entries = ArchiveEntries {
                    total: 100,
                    list: vec![
                        ArchiveEntry {
                            absolute: PathBuf::new(),
                            name: String::new(),
                            modified: [0; 6],
                            size: 100,
                            directory: false,
                        },
                        ArchiveEntry {
                            absolute: PathBuf::new(),
                            name: String::new(),
                            modified: [0; 6],
                            size: 0,
                            directory: true,
                        },
                    ],
                };
                assert_eq!(
                    row["value"].as_f64().unwrap() as i64,
                    archive_size_estimate(&entries)
                );
            }
            "url" => assert_eq!(
                is_supported_web_url(row["url"].as_str().unwrap()),
                row["value"].as_bool().unwrap(),
                "{row}"
            ),
            "zip" => {
                let dir = Temp::new();
                let source = dir.0.join("input.txt");
                let bytes = b"hello archive\n".repeat(100);
                fs::write(&source, &bytes).unwrap();
                let entries = ArchiveEntries {
                    total: bytes.len() as i64,
                    list: vec![
                        ArchiveEntry {
                            absolute: source,
                            name: "root/input.txt".into(),
                            modified: [6, 4, 3, 2, 0, 2020],
                            size: bytes.len() as i64,
                            directory: false,
                        },
                        ArchiveEntry {
                            absolute: PathBuf::new(),
                            name: "root/empty/".into(),
                            modified: [6, 4, 3, 2, 0, 2020],
                            size: 0,
                            directory: true,
                        },
                    ],
                };
                let result = write_archive(entries, 1_000_000, &dir.0.join("output"), |_| true);
                assert_eq!(result.status, Status::Done);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    assert_eq!(
                        fs::metadata(&result.path).unwrap().permissions().mode() & 0o777,
                        0o600
                    );
                }
                assert_eq!(
                    fs::read(result.path).unwrap(),
                    unhex(row["hex"].as_str().unwrap())
                );
            }
            _ => {}
        }
    }
}
#[test]
fn image_location_variants_modern_legacy_and_corruption() {
    let storage = StorageFileLocation {
        dc_id: 4,
        kind: StorageFileType::Document,
        id: 99,
        size_letter: b'm',
        access_hash: 123,
        file_reference: vec![1, 2, 3],
        ..Default::default()
    };
    let locations = vec![
        DownloadLocation::Storage(storage.clone()),
        DownloadLocation::Web {
            url: b"https://a".to_vec(),
            access_hash: 12,
        },
        DownloadLocation::Geo(GeoPointLocation {
            lat: 1.5,
            lon: -3.,
            width: 32,
            height: 64,
            ..Default::default()
        }),
        DownloadLocation::Url("https://a/🙂".into()),
        DownloadLocation::InMemory(vec![0, 1, 0, 255]),
        DownloadLocation::AudioAlbumThumb { document_id: 123 },
    ];
    for location in locations {
        let input = ImageLocation::new(location, 32, 64);
        let encoded = encode_image_location(&input).unwrap();
        let out = decode_image_location(&encoded).unwrap();
        assert_eq!(out.file, input.file);
        assert_eq!((out.width, out.height), (32, 64));
    }
    for kind in [
        StorageFileType::Legacy,
        StorageFileType::Encrypted,
        StorageFileType::Document,
        StorageFileType::Secure,
        StorageFileType::Takeout,
        StorageFileType::Photo,
        StorageFileType::PeerPhoto,
        StorageFileType::StickerSetThumb,
        StorageFileType::GroupCallStream,
    ] {
        let location = StorageFileLocation {
            kind,
            volume_id: if kind == StorageFileType::StickerSetThumb {
                0
            } else {
                55
            },
            local_id: 7,
            ..storage.clone()
        };
        let bytes = write_storage_file(&location);
        assert_eq!(read_storage_file(&bytes).unwrap(), location);
        for n in 1..44 {
            assert!(read_storage_file(&bytes[..n.min(bytes.len())]).is_none());
        }
    }
    let mut s = Writer::default();
    for v in [20, 30, 4] {
        s.i32(v);
    }
    s.u64(55);
    s.i32(7);
    s.u64(88);
    s.bytes(&[9]);
    let legacy = read_image_location(1003013, &mut Reader::new(&s.0)).unwrap();
    let modern = convert_to_modern_peer_photo(&legacy, peer_from_channel(ChannelId::new(1)), 4, 77);
    let DownloadLocation::Storage(m) = modern.file else {
        panic!()
    };
    assert_eq!(
        (m.id, m.volume_id, m.local_id, m.size_letter),
        (peer_from_channel(ChannelId::new(1)).value, 77, 0, b'a')
    );
    assert_eq!(m.file_reference, vec![9]);
    assert!(decode_image_location(&[0, 0, 0x10, 255]).is_none());
    assert!(write_download_location(&DownloadLocation::Url(String::new())).is_none());
}
#[test]
fn stored_image_preserves_nonnull_empty_and_lone_utf16_peer_records() {
    let mut p = peer::read_peer(
        2008007,
        &mut Reader::new(&unhex(
            rows()
                .find(|r| r["op"] == "peer" && r["kind"] == 0 && r["version"] == 3)
                .unwrap()["hex"]
                .as_str()
                .unwrap(),
        )),
    )
    .unwrap();
    p.userpic = StoredImage(Some(Vec::new()));
    if let PeerFields::User { first, .. } = &mut p.fields {
        *first = Some(vec![0xd800, 0, 0xdc00]);
    }
    let mut w = Writer::default();
    peer::write_peer(&mut w, &p).unwrap();
    let out = peer::read_peer(2008007, &mut Reader::new(&w.0)).unwrap();
    assert_eq!(out, p);
    assert!(peer::text(match &out.fields {
        PeerFields::User { first, .. } => first,
        _ => panic!(),
    })
    .is_none());
}
#[test]
fn document_sticker_set_context_and_rejection_of_legacy_thumb() {
    let row = rows()
        .find(|r| r["op"] == "document" && r["kind"] == 3 && r["version"] == 4)
        .unwrap();
    let bytes = unhex(row["hex"].as_str().unwrap());
    let info = StickerSetInfo {
        id: 123,
        access_hash: 456,
        short_name: "set".into(),
    };
    assert!(
        DocumentRecord::read_from_stream(2008007, &mut Reader::new(&bytes), Some(&info)).is_none()
    );
    let special = StickerSetInfo {
        id: 0,
        access_hash: 0,
        short_name: String::new(),
    };
    let d = DocumentRecord::read_from_stream(2008007, &mut Reader::new(&bytes), Some(&special))
        .unwrap();
    assert!(d.attributes(Some(&special)).is_some());
    let mut d = d;
    d.dc = 0;
    d.access = 0;
    let mut w = Writer::default();
    d.write_to_stream(&mut w).unwrap();
    assert!(DocumentRecord::read_from_stream(2008007, &mut Reader::new(&w.0), None).is_none());
    d.access = 1;
    let legacy = ImageLocation::new(
        DownloadLocation::Storage(StorageFileLocation {
            dc_id: 4,
            volume_id: 5,
            local_id: 6,
            ..Default::default()
        }),
        1,
        1,
    );
    d.thumbnail = StoredImage::from_location(&legacy).unwrap();
    let mut w = Writer::default();
    d.write_to_stream(&mut w).unwrap();
    assert!(DocumentRecord::read_from_stream(2008007, &mut Reader::new(&w.0), None).is_none());
}
#[test]
fn peer_legacy_flag_masks_preserve_unrelated_bits_and_lastseen() {
    let row = rows()
        .find(|r| r["op"] == "peer" && r["kind"] == 0 && r["version"] == 3)
        .unwrap();
    let p = peer::read_peer(
        2008007,
        &mut Reader::new(&unhex(row["hex"].as_str().unwrap())),
    )
    .unwrap();
    assert_eq!(peer::restored_flags(&p, 2008007, 1 << 29), 17);
    assert_eq!(peer::restored_flags(&p, 2008006, 1 << 29), 1 << 29);
    assert!(peer::restored_lastseen(&p).is_some());
    assert_eq!(peer::convert_legacy_flags(0xff, 0, &[(1, 3)]), 0xf7);
    assert_eq!(peer::convert_legacy_flags(0, 2, &[(1, 3)]), 8);
}
#[test]
fn loader_local_cache_partial_and_ordered_completion_effects() {
    let s = SessionScope::new();
    let key = tg_cache::cache::storage_cache_types::Key::new(1, 2);
    let mut l = FileLoader::new(config(&s, 8), key, None).unwrap();
    l.start();
    assert_eq!(
        l.take_effects(),
        vec![
            Effect::ReadCache {
                key,
                decode_image: true
            },
            Effect::Progress
        ]
    );
    assert!(l.loading_local());
    l.start();
    assert!(l.take_effects().is_empty());
    l.local_loaded(b"partial:abc");
    assert_eq!(
        l.take_effects(),
        vec![Effect::StartLoading {
            partial: Some(b"partial:abc".to_vec())
        }]
    );
    assert_eq!(l.local_status(), LocalStatus::NotFound);
    assert!(l.write_result_part(0, b"abcdef"));
    assert!(l.finalize_result());
    assert_eq!(
        l.take_effects(),
        vec![
            Effect::CachePut {
                key,
                bytes: b"partial:abcdef".to_vec(),
                tag: 7
            },
            Effect::Done,
            Effect::NotifyDownloaderTaskFinished
        ]
    );
    assert_eq!(l.current_progress(), 1.);
}
#[test]
fn loader_missing_local_only_empty_cache_and_memory_owns_bytes() {
    let s = SessionScope::new();
    let mut c = config(&s, 8);
    c.from_cloud = LoadFromCloudSetting::LocalOnly;
    let mut l = FileLoader::new(c, Default::default(), None).unwrap();
    l.start();
    assert!(l.cancelled());
    assert_eq!(
        l.take_effects(),
        vec![
            Effect::CancelRequests,
            Effect::Done,
            Effect::ClearFileNameAfterUpdate
        ]
    );
    let mut l = file(&s, 8);
    l.start();
    assert_eq!(
        l.take_effects(),
        vec![Effect::StartLoading { partial: None }]
    );
    let mut bytes = vec![1, 2];
    l.finish_with_bytes(&bytes);
    bytes[0] = 9;
    assert_eq!(l.bytes(), &[1, 2]);
    assert_eq!(l.local_status(), LocalStatus::Loaded);
    assert_eq!(
        l.take_effects(),
        vec![Effect::Done, Effect::NotifyDownloaderTaskFinished]
    );
}
#[test]
fn loader_out_of_order_parts_overlap_and_progress_preserve_source_accounting() {
    let s = SessionScope::new();
    let mut l = file(&s, 10);
    assert!(l.write_result_part(5, b"56789"));
    assert_eq!(l.current_offset(), 5);
    assert_eq!(l.current_progress(), 0.5);
    assert!(l.write_result_part(0, b"01234"));
    assert_eq!(l.current_offset(), 10);
    assert_eq!(l.bytes(), b"0123456789");
    assert!(l.write_result_part(0, b"0"));
    assert_eq!(l.current_offset(), 11);
    assert_eq!(l.current_progress(), 1.);
    assert!(l.read_loaded_part_back(9, 2).is_empty());
    assert!(!l.write_result_part(-1, b"x"));
    assert!(!l.increase_load_size(10, false));
}
#[test]
fn loader_files_cancel_failure_and_filename_rules() {
    let s = SessionScope::new();
    let temp = Temp::new();
    let path = temp.0.join("result");
    let mut c = config(&s, 8);
    c.filename = path.clone();
    c.to_cache = LoadToCacheSetting::FileOnly;
    let mut l = FileLoader::new(c, Default::default(), Some((1, 2))).unwrap();
    l.start();
    l.take_effects();
    assert!(l.write_result_part(4, b"efgh"));
    assert!(l.write_result_part(0, b"abcd"));
    assert_eq!(l.read_loaded_part_back(0, 8), b"abcdefgh");
    l.cancel(FailureReason::OtherFailure);
    assert!(!path.exists());
    assert_eq!(
        l.take_effects(),
        vec![
            Effect::CancelRequests,
            Effect::Failed(download::Error {
                failure_reason: FailureReason::OtherFailure,
                started: true
            }),
            Effect::ClearFileNameAfterUpdate
        ]
    );
    let mut c = config(&s, 8);
    c.filename = temp.0.join("missing/child");
    c.to_cache = LoadToCacheSetting::FileOnly;
    let mut l = FileLoader::new(c, Default::default(), None).unwrap();
    l.start();
    assert!(l.cancelled());
    assert!(matches!(
        l.take_effects()
            .iter()
            .find(|e| matches!(e, Effect::Failed(_))),
        Some(Effect::Failed(download::Error {
            failure_reason: FailureReason::FileWriteFailure,
            started: false
        }))
    ));
    let mut l = file(&s, 8);
    assert!(l.set_file_name(path.clone()));
    assert!(!l.set_file_name(temp.0.join("other")));
    assert!(l.set_file_name(PathBuf::new()));
}
#[test]
fn loader_file_finalize_postprocess_location_and_partial_cache() {
    let s = SessionScope::new();
    let temp = Temp::new();
    let path = temp.0.join("result");
    let key = tg_cache::cache::storage_cache_types::Key::new(1, 2);
    let mut c = config(&s, 3);
    c.filename = path.clone();
    let mut l = FileLoader::new(c, key, Some((1, 2))).unwrap();
    l.start();
    l.take_effects();
    l.local_loaded(&[]);
    l.take_effects();
    l.write_result_part(0, b"abc");
    assert!(l.finalize_result());
    assert_eq!(fs::read(&path).unwrap(), b"abc");
    assert_eq!(
        l.take_effects(),
        vec![
            Effect::PostprocessDownloaded(path.clone()),
            Effect::WriteFileLocation { key: (1, 2), path },
            Effect::CachePut {
                key,
                bytes: b"abc".to_vec(),
                tag: 7
            },
            Effect::Done,
            Effect::NotifyDownloaderTaskFinished
        ]
    );
}
#[test]
fn mtproto_part_order_unknown_size_partial_resume_and_failure() {
    let s = SessionScope::new();
    let mut l = MtpFileLoader::new(file(&s, 0), DownloadLocation::default(), FileOrigin::None);
    assert_eq!(l.take_next_request_offset(false), Some(0));
    assert!(!l.ready_to_request(true));
    assert!(l.feed_part(0, b"abc", false));
    assert!(l.file.finished());
    assert_eq!(l.take_effects(), vec![TaskEffect::RemoveFromQueue]);
    let mut l = MtpFileLoader::new(
        file(&s, DOWNLOAD_PART_SIZE * 3),
        DownloadLocation::default(),
        FileOrigin::None,
    );
    let partial = [
        b"partial:".as_slice(),
        &vec![7; DOWNLOAD_PART_SIZE as usize + 1],
    ]
    .concat();
    l.start_loading_with_partial(&partial, false);
    assert_eq!(l.take_next_request_offset(true), Some(DOWNLOAD_PART_SIZE));
    assert_eq!(l.file.current_offset(), DOWNLOAD_PART_SIZE);
    assert!(l.feed_part(
        DOWNLOAD_PART_SIZE,
        &vec![8; DOWNLOAD_PART_SIZE as usize],
        true
    ));
    assert!(!l.file.finished());
    assert_eq!(
        l.take_next_request_offset(true),
        Some(2 * DOWNLOAD_PART_SIZE)
    );
    assert!(l.feed_part(
        2 * DOWNLOAD_PART_SIZE,
        &vec![9; DOWNLOAD_PART_SIZE as usize],
        false
    ));
    assert!(l.file.finished());
    let mut l = MtpFileLoader::new(file(&s, 10), DownloadLocation::default(), FileOrigin::None);
    assert!(!l.set_web_file_size_hook(11));
    assert!(l.file.cancelled());
    assert_eq!(l.take_effects(), vec![TaskEffect::CancelAllRequests]);
}
#[test]
fn streamed_window_duplicates_out_of_order_short_last_read_and_cancel() {
    let s = SessionScope::new();
    let mut l =
        StreamedFileDownloader::new(file(&s, 33 * DOWNLOAD_PART_SIZE + 3), 55, FileOrigin::None)
            .unwrap();
    l.start_loading();
    let e = l.take_effects();
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, ReaderEffect::LoadForDownloader { .. }))
            .count(),
        32
    );
    assert_eq!(
        e.last(),
        Some(&ReaderEffect::ContinueDownloaderFromMainThread)
    );
    assert!(l.save_part(33 * DOWNLOAD_PART_SIZE, b"end"));
    let e = l.take_effects();
    assert_eq!(
        e[0],
        ReaderEffect::DoneForDownloader {
            offset: 33 * DOWNLOAD_PART_SIZE
        }
    );
    assert!(l.save_part(33 * DOWNLOAD_PART_SIZE, b"end"));
    assert!(l.take_effects().is_empty());
    assert!(l.read_loaded_part(33 * DOWNLOAD_PART_SIZE).is_empty());
    assert!(l.save_part(0, &vec![1; DOWNLOAD_PART_SIZE as usize]));
    assert!(l.take_effects().contains(&ReaderEffect::LoadForDownloader {
        offset: 32 * DOWNLOAD_PART_SIZE
    }));
    l.reader_failed();
    assert!(l.file.cancelled());
    assert_eq!(l.take_effects(), vec![ReaderEffect::CancelForDownloader]);
}
#[test]
fn streamed_completion_and_reader_release() {
    let s = SessionScope::new();
    let mut l = StreamedFileDownloader::new(file(&s, 3), 1, FileOrigin::None).unwrap();
    l.start_loading();
    l.take_effects();
    assert!(l.save_part(0, b"abc"));
    assert!(l.file.finished());
    l.close();
    assert_eq!(
        l.take_effects(),
        vec![
            ReaderEffect::DoneForDownloader { offset: 0 },
            ReaderEffect::CancelForDownloader
        ]
    );
}
#[test]
fn web_queue_generation_limit_removal_and_stale_replies() {
    let mut m = WebLoadManager::default();
    for id in 1..=10 {
        m.enqueue(id, format!("https://a/{id}"), false, 0);
    }
    let e = m.take_effects();
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, WebEffect::Get { .. }))
            .count(),
        8
    );
    assert!(e.contains(&WebEffect::ScheduleReset { at: 200 }));
    m.advance_to(200);
    m.enqueue(11, "https://a/11".into(), false, 201);
    m.take_effects();
    m.failed(1, 1);
    let e = m.take_effects();
    assert!(matches!(&e[1], WebEffect::Get { id: 11, .. }));
    m.failed(1, 1);
    assert!(m.take_effects().is_empty());
    m.remove(2);
    assert!(m
        .take_effects()
        .iter()
        .any(|e| matches!(e, WebEffect::Get { id: 9, .. })));
    m.clear();
    assert_eq!(m.take_effects().len(), 8);
}
fn http_progress(id: u64, reply: u64, ready: i64, total: i64, bytes: &[u8]) -> HttpProgress<'_> {
    HttpProgress {
        id,
        reply,
        status: 200,
        ready,
        total,
        original_content_length: None,
        bytes,
        location: &[],
    }
}
#[test]
fn web_progress_sizes_statuses_streaming_and_failures() {
    let mut m = WebLoadManager::default();
    m.enqueue(1, "https://a".into(), false, 0);
    m.take_effects();
    m.progress(http_progress(1, 1, 2, 4, b"ab"));
    assert_eq!(
        m.take_effects(),
        vec![WebEffect::Update {
            id: 1,
            data: Update::Progress {
                ready: 2,
                total: 4,
                streamed: Vec::new()
            }
        }]
    );
    m.progress(http_progress(1, 1, 4, 4, b"cd"));
    assert_eq!(
        m.take_effects(),
        vec![
            WebEffect::Abort { reply: 1 },
            WebEffect::Update {
                id: 1,
                data: Update::Finished(b"abcd".to_vec())
            }
        ]
    );
    m.enqueue(2, "https://a".into(), true, 0);
    m.take_effects();
    m.progress(http_progress(2, 2, 3, 3, b"abc"));
    assert_eq!(
        m.take_effects(),
        vec![
            WebEffect::Update {
                id: 2,
                data: Update::Progress {
                    ready: 3,
                    total: 3,
                    streamed: b"abc".to_vec()
                }
            },
            WebEffect::Abort { reply: 2 },
            WebEffect::Update {
                id: 2,
                data: Update::Finished(Vec::new())
            }
        ]
    );
    for (id, total, status) in [(3, 0, 200), (4, MAX_WEB_FILE + 1, 200), (5, 9, 404)] {
        m.enqueue(id, "https://a".into(), true, 0);
        let reply = m
            .take_effects()
            .iter()
            .find_map(|e| {
                if let WebEffect::Get { reply, .. } = e {
                    Some(*reply)
                } else {
                    None
                }
            })
            .unwrap();
        let mut p = http_progress(id, reply, 0, total, b"");
        p.status = status;
        m.progress(p);
        assert!(m.take_effects().contains(&WebEffect::Update {
            id,
            data: Update::Error
        }));
    }
}
#[test]
fn web_redirect_limit_relative_scheme_rejection_and_adapter() {
    let mut m = WebLoadManager::default();
    m.enqueue(1, "https://a/path/file".into(), false, 0);
    m.take_effects();
    for reply in 1..=5 {
        m.redirect(1, reply, b"../next");
        let e = m.take_effects();
        assert!(matches!(&e[1],WebEffect::Get{url,..} if url=="https://a/next"));
    }
    m.redirect(1, 6, b"/last");
    assert_eq!(
        m.take_effects(),
        vec![
            WebEffect::Abort { reply: 6 },
            WebEffect::Update {
                id: 1,
                data: Update::Error
            }
        ]
    );
    m.enqueue(2, "file:///tmp/private".into(), false, 0);
    assert_eq!(
        m.take_effects(),
        vec![WebEffect::Update {
            id: 2,
            data: Update::Error
        }]
    );
    m.enqueue(3, "https://a".into(), false, 0);
    let reply = m
        .take_effects()
        .iter()
        .find_map(|e| {
            if let WebEffect::Get { reply, .. } = e {
                Some(*reply)
            } else {
                None
            }
        })
        .unwrap();
    m.redirect(3, reply, b"file:///private");
    assert!(m.take_effects().contains(&WebEffect::Update {
        id: 3,
        data: Update::Error
    }));
    assert!(http_client().is_ok());
}
#[test]
fn archive_gather_hidden_symlink_dedup_and_two_level_listing() {
    let temp = Temp::new();
    fs::create_dir_all(temp.0.join("sub/deep")).unwrap();
    fs::write(temp.0.join("a"), b"a").unwrap();
    fs::write(temp.0.join(".hidden"), b"h").unwrap();
    fs::write(temp.0.join("sub/b"), b"bb").unwrap();
    fs::write(temp.0.join("sub/deep/c"), b"ccc").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(temp.0.join("a"), temp.0.join("link")).unwrap();
    assert_eq!(
        single_folder_path(&[reqwest::Url::from_file_path(&temp.0).unwrap()]),
        Some(temp.0.clone())
    );
    assert!(single_folder_path(&[reqwest::Url::parse("https://a/").unwrap()]).is_none());
    assert!(single_folder_path(&[]).is_none());
    assert_eq!(archive_root_name(std::path::Path::new("/")), "Archive");
    assert_eq!(
        common_parent_name(&[temp.0.join("a"), temp.0.join("b")]),
        archive_root_name(&temp.0)
    );
    let job = ArchiveJob {
        folder: temp.0.clone(),
        root: "root".into(),
        paths: Vec::new(),
    };
    let entries = gather_archive_entries(&job).unwrap();
    assert_eq!(entries.total, 7);
    assert!(entries.list.iter().any(|e| e.name == "root/.hidden"));
    assert!(!entries.list.iter().any(|e| e.name == "root/link"));
    assert!(entries
        .list
        .iter()
        .any(|e| e.directory && e.name == "root/sub/deep/"));
    let files = folder_files_for_sending(&temp.0).unwrap();
    assert!(files.contains(&temp.0.join("a")));
    assert!(files.contains(&temp.0.join("sub/b")));
    assert!(!files.contains(&temp.0.join("sub/deep/c")));
    assert!(!files.contains(&temp.0.join(".hidden")));
    let entries = gather_archive_entries(&ArchiveJob {
        paths: vec![temp.0.join("a"), temp.0.join("a")],
        ..Default::default()
    })
    .unwrap();
    assert_eq!(entries.list[1].name, "a (2)");
    assert!(gather_archive_entries(&ArchiveJob {
        paths: vec![temp.0.join("missing")],
        ..Default::default()
    })
    .is_none());
}
#[test]
fn archive_cancellation_missing_files_size_limit_and_stale_boundary() {
    let temp = Temp::new();
    let path = temp.0.join("data.zip");
    fs::write(&path, vec![3; ARCHIVE_CHUNK_SIZE + 1]).unwrap();
    let entries = gather_archive_entries(&ArchiveJob {
        paths: vec![path.clone()],
        ..Default::default()
    })
    .unwrap();
    let output = temp.0.join("out");
    assert_eq!(
        write_archive(entries.clone(), 0, &output, |_| true).status,
        Status::TooLarge
    );
    assert_eq!(fs::read_dir(&output).unwrap().count(), 0);
    assert_eq!(
        write_archive(entries.clone(), i64::MAX, &output, |_| false).status,
        Status::Cancelled
    );
    fs::remove_file(path).unwrap();
    let result = write_archive(entries, 0, &output, |_| true);
    assert_eq!(result.status, Status::Done);
    assert!(result.size > 0);
    let modified = fs::metadata(&result.path).unwrap().modified().unwrap();
    assert_eq!(
        clear_stale_archive_files(&output, modified + Duration::from_secs(86400)).unwrap(),
        0
    );
    assert_eq!(
        clear_stale_archive_files(&output, modified + Duration::from_secs(86401)).unwrap(),
        1
    );
    assert_eq!(
        clear_stale_archive_files(&output, SystemTime::now()).unwrap(),
        0
    );
}

#[test]
fn native_image_location_storage_wire_and_stream_wrappers() {
    for row in rows().filter(|r| r["op"] == "location") {
        let bytes = unhex(row["hex"].as_str().unwrap());
        if row["kind"] == 0 && row["in_message"] == true {
            assert!(decode_image_location(&bytes).is_none());
            continue;
        }
        let image = decode_image_location(&bytes).unwrap();
        let DownloadLocation::Storage(s) = &image.file else {
            panic!()
        };
        assert_eq!(s.kind as i64, row["kind"].as_i64().unwrap());
        assert_eq!(
            s.in_message_peer_id.value,
            if row["in_message"] == true {
                0x200000000002a
            } else {
                0
            }
        );
        assert_eq!(encode_image_location(&image).unwrap(), bytes);
        let mut w = Writer::default();
        write_storage_image_location(&mut w, &image).unwrap();
        assert_eq!(w.0.len(), storage_image_location_size(&image).unwrap());
        assert_eq!(
            read_storage_image_location(2008007, &mut Reader::new(&w.0)).unwrap(),
            image
        );
    }
}
#[test]
fn group_peer_codec_applies_real_store_loaded_gate_version_reset_and_truncation() {
    use tg_core::data::{
        data_changes::Changes,
        data_channel::ChannelDataFlags,
        data_session::{NameIndex, Peers},
    };
    let s = SessionScope::new();
    let q = tg_runtime::MainQueue::new();
    let changes = Changes::with_queue(&s, q);
    let peers = Peers::new(&s, changes);
    let make = |kind| {
        let row = rows()
            .find(|r| {
                r["op"] == "peer" && r["kind"] == kind && r["version"] == 3 && r["nulls"] == false
            })
            .unwrap();
        peer::read_peer(
            2008007,
            &mut Reader::new(&unhex(row["hex"].as_str().unwrap())),
        )
        .unwrap()
    };
    let mut p = make(1);
    let index = |_key, _name: &str| NameIndex::from_words(Vec::<Vec<u16>>::new());
    let key = peer::apply_group(&peers, &p, 2008007, false, &index).unwrap();
    let k = ChatKey::try_from(key).unwrap();
    peers.read_chat(k, |c| {
        assert_eq!(c.version(), 0);
        assert_eq!(c.count, 23);
        assert_eq!(c.creator.bare, 0x12345689abcdef);
    });
    if let PeerFields::Chat { count, .. } = &mut p.fields {
        *count = 99;
    }
    assert_eq!(
        peer::apply_group(&peers, &p, 2008007, false, &index),
        Some(key)
    );
    assert_eq!(peers.read_chat(k, |c| c.count), 23);
    let p = make(2);
    let key = peer::apply_group(&peers, &p, 2008007, false, &index).unwrap();
    let k = ChannelKey::try_from(key).unwrap();
    assert_eq!(peers.read_channel(k, |c| c.access_hash()), 99);
    peers.set_channel_flags(k, ChannelDataFlags::from_raw((1 << 63) | 1));
    let snapshot = peer::snapshot_group(&peers, key).unwrap();
    assert!(matches!(
        snapshot.fields,
        PeerFields::Channel { flags: 1, .. }
    ));
    assert!(peer::apply_group(&peers, &make(0), 2008007, false, &index).is_none());
}
struct FileServices;
impl tg_core::core::file_location::FileLocations for FileServices {
    fn inspect(
        &self,
        location: &tg_core::core::file_location::FileLocation,
    ) -> tg_core::core::file_location::FileInfo {
        let metadata = fs::metadata(location.name()).ok();
        tg_core::core::file_location::FileInfo {
            path: location.fname.clone(),
            exists: metadata.is_some(),
            readable: File::open(location.name()).is_ok(),
            modified_ms: None,
            size: metadata.map_or(-1, |m| m.len() as i64),
        }
    }
    fn read(&self, _: (u64, u64)) -> tg_core::core::file_location::FileLocation {
        Default::default()
    }
}
use std::fs::File;
struct MimeRegistry;
impl tg_core::core::mime_type::MimeDatabase for MimeRegistry {
    fn for_name(&self, name: &str) -> tg_core::core::mime_type::MimeType {
        let row = rows()
            .find(|r| r["op"] == "registry" && r["query"] == name)
            .unwrap();
        tg_core::core::mime_type::MimeType {
            name: row["name"].as_str().unwrap().into(),
            glob_patterns: row["globs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect(),
            filter_string: row["filter"].as_str().unwrap().into(),
        }
    }
}
#[test]
fn document_codec_applies_real_documents_attributes_sizes_remote_and_effects() {
    use tg_core::data::data_media_store::Documents;
    let s = SessionScope::new();
    let documents = Documents::new(&s, Rc::new(FileServices));
    let row = rows()
        .find(|r| {
            r["op"] == "document" && r["kind"] == 4 && r["version"] == 6 && r["nulls"] == false
        })
        .unwrap();
    let p = DocumentRecord::read_from_stream(
        2008007,
        &mut Reader::new(&unhex(row["hex"].as_str().unwrap())),
        None,
    )
    .unwrap();
    let key = documents
        .apply_serialized_document(&p, None, &MimeRegistry)
        .unwrap();
    documents
        .read(key, |d| {
            assert_eq!(d.id(), 123);
            assert_eq!(d.size, 0xf0000001);
            assert_eq!(d.duration(), 12345);
            assert_eq!(d.dc_id(), 4);
            // The real model rejects animation+unknown .bin MIME, as Source
            // setattributes does; deserialization does not force saved type.
            assert_eq!(d.kind, tg_core::data::data_media_types::DocumentType::File);
            assert_eq!(d.filename(), "файл 😀.bin");
            assert!(DocumentRecord::from_document(d).is_some());
        })
        .unwrap();
    let mut corrupt = p.clone();
    corrupt.name = Some(vec![0xd800]);
    assert!(documents
        .apply_serialized_document(&corrupt, None, &MimeRegistry)
        .is_none());
    assert_eq!(documents.read(key, |d| d.size), Some(p.size as i64));
}
#[test]
fn loader_factory_selection_overrides_and_memory_start() {
    let s = SessionScope::new();
    let mut c = config(&s, 4);
    c.filename = PathBuf::from("ignored-for-web");
    let FileDownload::Web(web) = create_file_loader(
        c,
        DownloadLocation::Url("https://a".into()),
        FileOrigin::None,
    )
    .unwrap() else {
        panic!()
    };
    assert!(web.file.file_name().as_os_str().is_empty());
    assert_eq!(web.file.full_size(), 0);
    assert!(!web.stream_loading());
    assert_eq!(
        loader_kind(&DownloadLocation::Url("a".into())),
        LoaderKind::Web
    );
    let FileDownload::Memory(mut memory) = create_file_loader(
        config(&s, 4),
        DownloadLocation::InMemory(b"data".to_vec()),
        FileOrigin::None,
    )
    .unwrap() else {
        panic!()
    };
    memory.start_loading();
    assert_eq!(memory.file.bytes(), b"data");
    assert!(memory.file.finished());
    let location = DownloadLocation::Storage(StorageFileLocation {
        dc_id: 4,
        kind: StorageFileType::Document,
        id: 55,
        ..Default::default()
    });
    let mut c = config(&s, 4);
    c.location_type = LocationType::Document;
    let FileDownload::Mtproto(mtp) = create_file_loader(c, location, FileOrigin::None).unwrap()
    else {
        panic!()
    };
    assert_eq!(mtp.obj_id(), 55);
    assert_eq!(
        mtp.file.file_location_key,
        Some(((0x4e45abe9u64 << 32) | 4, 55))
    );
}

#[test]
fn loader_rpl_dispatch_releases_borrow_before_callbacks() {
    use std::cell::{Cell, RefCell};
    let scope = SessionScope::new();
    let loader = Rc::new(RefCell::new(file(&scope, 3)));
    let updates = LoaderUpdates::default();
    let life = tg_rpl::Lifetime::new();
    let seen = Rc::new(Cell::new(0));
    let s = seen.clone();
    let l = loader.clone();
    updates.updates().start(
        move |_| {
            assert!(!l.borrow().finished());
            l.borrow_mut().permit_load_from_cloud();
            s.set(s.get() + 1);
        },
        |_| {},
        || {},
        &life,
    );
    loader.borrow_mut().notify_about_progress();
    let effects = loader.borrow_mut().take_effects();
    for e in &effects {
        assert!(updates.apply(e));
    }
    assert_eq!(seen.get(), 1);
}
#[path = "../../tg-cache/tests/support/database.rs"]
mod cache_support;
#[test]
fn loader_effects_use_real_encrypted_cache() {
    use std::sync::mpsc;
    let host = cache_support::Host::new();
    let temp = cache_support::Temp::new("file-loader");
    let db = tg_cache::cache::Database::with_environment(
        &temp.0,
        cache_support::settings("loader"),
        host.env.clone(),
    );
    assert!(cache_support::open(&db, cache_support::key()).is_ok());
    let key = tg_cache::cache::Key::new(123, 456);
    assert!(apply_cache_put(
        &Effect::CachePut {
            key,
            bytes: b"partial:data".to_vec(),
            tag: 7
        },
        &db
    ));
    let (tx, rx) = mpsc::channel();
    read_local_cache(&db, key, move |bytes| tx.send(bytes).unwrap());
    assert_eq!(rx.recv().unwrap(), b"partial:data");
}

#[test]
fn web_loader_streams_to_real_file_detaches_and_ignores_late_updates() {
    let s = SessionScope::new();
    let temp = Temp::new();
    let path = temp.0.join("web");
    let mut c = config(&s, 0);
    c.filename = path.clone();
    c.to_cache = LoadToCacheSetting::FileOnly;
    let mut file = FileLoader::new(c, Default::default(), None).unwrap();
    file.start();
    file.take_effects();
    let mut web = WebFileLoader::new(file, "https://a".into(), WebRequestType::OnlySize);
    web.start_loading();
    assert_eq!(
        web.take_effects(),
        vec![WebLoaderEffect::Enqueue {
            url: "https://a".into(),
            stream: true
        }]
    );
    web.apply_update(Update::Progress {
        ready: 3,
        total: 3,
        streamed: b"abc".to_vec(),
    });
    assert_eq!(web.current_offset(), 3);
    web.apply_update(Update::Finished(Vec::new()));
    assert_eq!(fs::read(path).unwrap(), b"abc");
    assert_eq!(web.take_effects(), vec![WebLoaderEffect::Remove]);
    assert!(web.file.finished());
    web.apply_update(Update::Error);
    assert!(!web.file.cancelled());
}

#[test]
fn web_base_virtual_offset_progress_and_started_error() {
    let scope = SessionScope::new();
    let mut web = WebFileLoader::new(
        file(&scope, 0),
        "https://a".into(),
        WebRequestType::FullLoad,
    );
    web.start_loading();
    web.apply_update(Update::Progress {
        ready: 3,
        total: 6,
        streamed: Vec::new(),
    });
    assert_eq!(web.file.current_progress(), 0.5);
    assert_eq!(web.file.current_offset(), 3);
    web.file.take_effects();
    web.apply_update(Update::Error);
    assert!(web
        .file
        .take_effects()
        .contains(&Effect::Failed(download::Error {
            failure_reason: FailureReason::OtherFailure,
            started: true
        })));
}

#[test]
fn loader_cancel_update_observes_old_filename_then_clears_after_callback() {
    use std::cell::RefCell;
    let scope = SessionScope::new();
    let temp = Temp::new();
    let filename = temp.0.join("target");
    let mut c = config(&scope, 0);
    c.filename = filename.clone();
    let loader = Rc::new(RefCell::new(
        FileLoader::new(c, Default::default(), None).unwrap(),
    ));
    let updates = LoaderUpdates::default();
    let life = tg_rpl::Lifetime::new();
    let l = loader.clone();
    let observed = filename.clone();
    updates.updates().start(
        |_| {},
        |_| {},
        move || assert_eq!(l.borrow().file_name(), observed),
        &life,
    );
    loader.borrow_mut().cancel(FailureReason::NoFailure);
    let effects = loader.borrow_mut().take_effects();
    for e in effects {
        if !updates.apply(&e) {
            loader.borrow_mut().apply_post_update(&e);
        }
    }
    assert!(loader.borrow().file_name().as_os_str().is_empty());
}
