use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::{Arc, Mutex},
};
use tg_core::{
    data::{
        data_file_origin::{FileLocationId, FileOrigin, UpdatedFileReferences},
        data_image_location::*,
        data_msg_id::{FullMsgId, MsgId},
        data_peer_id::*,
        data_store_keys::SessionScope,
    },
    storage::{
        download_manager_mtproto as d,
        file_download::{
            FileLoader, LoadFromCloudSetting, LoadToCacheSetting, LoaderConfig, LocationType,
        },
        file_download_mtproto::MtpFileLoader,
        file_upload as u, transfer_transport as transport,
    },
};
use tg_mtproto::mtproto_response::Error;
use tg_scheme as tl;
fn message(n: i64) -> FullMsgId {
    FullMsgId::new(peer_from_user(UserId::new(1)), MsgId::new(n))
}
fn loader(scope: &SessionScope, size: i64) -> MtpFileLoader {
    let location = DownloadLocation::Storage(StorageFileLocation {
        dc_id: 4,
        kind: StorageFileType::Document,
        id: 55,
        access_hash: 99,
        file_reference: vec![1],
        ..Default::default()
    });
    let f = FileLoader::new(
        LoaderConfig {
            session: scope.id(),
            filename: Default::default(),
            load_size: size,
            full_size: size,
            location_type: LocationType::Document,
            to_cache: LoadToCacheSetting::CacheAsWell,
            from_cloud: LoadFromCloudSetting::CloudOrLocal,
            auto_loading: false,
            cache_tag: 1,
        },
        Default::default(),
        None,
    )
    .unwrap();
    MtpFileLoader::new(f, location, FileOrigin::None)
}
fn setup(size: i64) -> (SessionScope, d::DownloadManagerMtproto, d::TaskId) {
    let scope = SessionScope::new();
    let mut m = d::DownloadManagerMtproto::default();
    let id = m.insert(d::DownloadMtprotoTask::new(loader(&scope, size), 4));
    (scope, m, id)
}
fn ids(m: &mut d::DownloadManagerMtproto) -> Vec<i32> {
    m.take_effects()
        .iter()
        .filter_map(|e| {
            if let d::Effect::Send { id, .. } = e {
                Some(*id)
            } else {
                None
            }
        })
        .collect()
}
fn error(code: i32, kind: &str) -> Error {
    Error {
        code,
        kind: kind.into(),
        description: String::new(),
    }
}
fn hash(offset: i64, bytes: &[u8]) -> d::CdnHash {
    d::CdnHash {
        offset,
        limit: 1,
        hash: tg_base::openssl_help::sha256(bytes),
    }
}
fn bytes_reply(bytes: Vec<u8>) -> tl::UploadFile {
    tl::mtp_upload_file(tl::mtp_storage_file_unknown(), 0, bytes)
}
fn upload(size: usize, kind: u::UploadKind) -> u::PreparedUpload {
    u::PreparedUpload {
        id: 123,
        kind,
        filename: "file.bin".into(),
        size: size as i64,
        content: u::Content::Bytes(vec![3; size]),
        parts: Vec::new(),
        parts_id: 456,
        parts_name: "thumb.jpg".into(),
        parts_md5: b"thumb-md5".to_vec(),
        file_md5: b"photo-md5".to_vec(),
        parts_size: size as i64,
        preparing: None,
        video_cover: None,
        attached_stickers: Vec::new(),
        force_file: false,
    }
}
fn upload_effects(m: &mut u::Uploader, now: i64) -> Vec<u::Effect> {
    let mut work = std::collections::VecDeque::from(m.take_effects());
    let mut output = Vec::new();
    while let Some(effect) = work.pop_front() {
        if !m.apply_continuation(&effect, now) {
            output.push(effect);
        }
        for nested in m.take_effects().into_iter().rev() {
            work.push_front(nested);
        }
    }
    output
}
fn sent(m: &mut u::Uploader) -> Vec<(i32, i32, u::UploadRequest)> {
    upload_effects(m, 0)
        .into_iter()
        .filter_map(|e| {
            if let u::Effect::Send { id, dc, request } = e {
                Some((id, dc, request))
            } else {
                None
            }
        })
        .collect()
}
#[test]
fn native_cdn_hash_and_unsigned_short_layout() {
    let mut count = 0;
    for line in include_str!("fixtures/storage-transfer/native.jsonl").lines() {
        let r: serde_json::Value = serde_json::from_str(line).unwrap();
        if r["op"] == "layout" {
            let l = u::part_layout(r["size"].as_str().unwrap().parse().unwrap());
            assert_eq!(i64::from(l.size), r["part"].as_i64().unwrap());
            assert_eq!(i64::from(l.count), r["count"].as_i64().unwrap());
        } else {
            let n = r["size"].as_u64().unwrap() as usize;
            let bytes = (0..n).map(|i| (i * 17 + 3) as u8).collect::<Vec<_>>();
            let real = tg_base::openssl_help::sha256(&bytes);
            let hex = real.iter().map(|b| format!("{b:02x}")).collect::<String>();
            assert_eq!(hex, r["sha256"]);
            let mode = r["mode"].as_u64().unwrap();
            let mut hashes = BTreeMap::new();
            if mode != 0 {
                let mut digest = real;
                if mode == 2 {
                    digest[0] ^= 1;
                }
                hashes.insert(
                    131072,
                    d::CdnHash {
                        offset: 131072,
                        limit: r["limit"].as_i64().unwrap() as i32,
                        hash: digest,
                    },
                );
            }
            assert_eq!(
                d::check_cdn_file_hash(&hashes, 131072, &bytes) as u64,
                r["result"].as_u64().unwrap()
            );
        }
        count += 1;
    }
    assert_eq!(count, 76);
}
#[test]
fn queue_lifo_reprioritize_generation_and_high_priority_starvation() {
    let mut q = d::Queue::default();
    for (id, p) in [(1, 0), (2, 0), (3, 2), (4, 2)] {
        q.enqueue(d::TaskId(id), p);
    }
    assert_eq!(
        q.tasks,
        vec![
            (d::TaskId(4), 2),
            (d::TaskId(3), 2),
            (d::TaskId(2), 0),
            (d::TaskId(1), 0)
        ]
    );
    assert!(q.next_task(true, |id| id == d::TaskId(1)).is_none());
    assert_eq!(
        q.next_task(false, |id| id == d::TaskId(1)),
        Some(d::TaskId(1))
    );
    q.enqueue(d::TaskId(4), -1);
    q.reset_generation();
    assert_eq!(
        q.tasks.iter().map(|t| t.1).collect::<Vec<_>>(),
        vec![2, -1, -1, -1]
    );
    q.remove(d::TaskId(3));
    assert_eq!(q.next_task(true, |_| true), Some(d::TaskId(2)));
}
#[test]
fn download_window_order_cancel_midway_and_idle_stop_preserves_count() {
    let (_s, mut m, t) = setup(8 * 131072);
    m.enqueue(t, 0, 1);
    let requests = ids(&mut m);
    assert_eq!(requests.len(), 4);
    assert_eq!(m.balances[&4].total_requested, 4 * d::PART_SIZE);
    assert_eq!(
        m.tasks[&t]
            .sent
            .values()
            .map(|r| r.offset)
            .collect::<Vec<_>>(),
        vec![0, 131072, 262144, 393216]
    );
    m.cancel_request_for_offset(t, 131072, 2);
    assert!(!m.tasks[&t].have_sent_request_for_offset(131072));
    m.cancel_all_requests(t, 3);
    assert_eq!(m.balances[&4].total_requested, 0);
    m.take_effects();
    m.advance_to(15008);
    assert!(m
        .take_effects()
        .iter()
        .any(|e| matches!(e, d::Effect::StopSession(_))));
    assert_eq!(m.balances[&4].sessions.len(), 1);
}
#[test]
fn download_balance_growth_overload_timeout_removal_and_raw_restart_bug() {
    let (_s, mut m, t) = setup(0);
    m.enqueue(t, 0, 1);
    m.cancel_all_requests(t, 2);
    for n in 0..3 {
        m.request_succeeded(
            4,
            &d::RequestData {
                offset: 0,
                session: 0,
                amount: 4 * d::PART_SIZE,
                sent: 10 + n,
            },
            20 + n,
        );
    }
    assert_eq!(m.balances[&4].sessions.len(), 2);
    m.restart_by_timeout(tg_mtproto::facade::download_dc_id(4, 0), 30);
    assert_eq!(m.balances[&4].timeouts, 0);
    for _ in 0..4 {
        m.session_timed_out(4, 0, 40);
    }
    assert_eq!(m.balances[&4].sessions.len(), 1);
    assert_eq!(m.balances[&4].remove_times, 1);
    assert!(m
        .take_effects()
        .iter()
        .any(|e| matches!(e, d::Effect::KillSession(_))));
    let max = m.balances[&4].sessions[0].max_waited;
    m.request_succeeded(
        4,
        &d::RequestData {
            offset: 0,
            session: 0,
            amount: max + 1,
            sent: 50,
        },
        51,
    );
    assert_eq!(m.balances[&4].sessions[0].max_waited, max);
}
#[test]
fn download_flood_wait_is_delegated_and_reference_expired_refreshes_same_offset() {
    let (_s, mut m, t) = setup(0);
    m.enqueue(t, 0, 1);
    let id = ids(&mut m)[0];
    assert!(!m.part_failed(t, id, &error(420, "FLOOD_WAIT_2"), false, Some(&[1]), 2));
    assert_eq!(m.tasks[&t].sent.len(), 1);
    assert!(m.part_failed(
        t,
        id,
        &error(400, "FILE_REFERENCE_EXPIRED"),
        false,
        Some(&[1]),
        3
    ));
    assert!(
        matches!(&m.take_effects()[0],d::Effect::RefreshFileReference{current,..}if current==&[1])
    );
    let updates = UpdatedFileReferences {
        data: BTreeMap::from([(FileLocationId::Document(55), vec![2])]),
    };
    m.refresh_file_reference_from(t, id, &updates, &[1], 4);
    let next = ids(&mut m)[0];
    assert_ne!(next, id);
    assert_eq!(m.tasks[&t].sent[&next].offset, 0);
    let DownloadLocation::Storage(s) = &m.tasks[&t].loader.location else {
        panic!()
    };
    assert_eq!(s.file_reference, vec![2]);
    m.refresh_file_reference_from(t, next, &updates, &[2], 5);
    assert!(m.tasks[&t].loader.file.cancelled());
    assert!(m.tasks[&t].sent.is_empty());
}
#[test]
fn download_cdn_redirect_reupload_hash_mismatch_and_token_fallback() {
    let (_s, mut m, t) = setup(0);
    m.enqueue(t, 0, 1);
    let id = ids(&mut m)[0];
    m.normal_part_loaded(
        t,
        id,
        tl::mtp_upload_file_cdn_redirect(8, vec![9], vec![1; 32], vec![2; 16], Vec::new()),
        2,
    );
    let cdn = ids(&mut m)[0];
    m.cdn_part_loaded(t, cdn, tl::mtp_upload_cdn_file_reupload_needed(vec![4]), 3);
    let reup = ids(&mut m)[0];
    m.reupload_done(t, reup, vec![hash(0, b"other")], 4);
    let cdn = ids(&mut m)[0];
    let mut bytes = b"abc".to_vec();
    d::decrypt_cdn(&mut bytes, &[1; 32], &[2; 16], 0);
    m.cdn_part_loaded(t, cdn, tl::mtp_upload_cdn_file(bytes), 5);
    assert!(m.tasks[&t].loader.file.cancelled());
    assert!(m.tasks[&t].sent.is_empty());
    let (_s, mut m, t) = setup(0);
    m.enqueue(t, 0, 1);
    let id = ids(&mut m)[0];
    m.normal_part_loaded(
        t,
        id,
        tl::mtp_upload_file_cdn_redirect(8, vec![9], vec![1; 32], vec![2; 16], Vec::new()),
        2,
    );
    let cdn = ids(&mut m)[0];
    m.part_failed(t, cdn, &error(400, "FILE_TOKEN_INVALID"), true, None, 3);
    assert_eq!(m.tasks[&t].cdn_dc, 0);
    assert!(matches!(
        m.take_effects().last(),
        Some(d::Effect::Send {
            request: d::DownloadRequest::File { .. },
            ..
        })
    ));
}
#[test]
fn download_missing_hash_queues_real_origin_hash_request_then_finishes() {
    let (_s, mut m, t) = setup(0);
    m.enqueue(t, 0, 1);
    let id = ids(&mut m)[0];
    m.normal_part_loaded(
        t,
        id,
        tl::mtp_upload_file_cdn_redirect(8, vec![9], vec![1; 32], vec![2; 16], Vec::new()),
        2,
    );
    let cdn = ids(&mut m)[0];
    let mut bytes = b"abc".to_vec();
    d::decrypt_cdn(&mut bytes, &[1; 32], &[2; 16], 0);
    m.cdn_part_loaded(t, cdn, tl::mtp_upload_cdn_file(bytes), 3);
    let e = m.take_effects();
    let hash_id = e
        .iter()
        .find_map(|e| {
            if let d::Effect::Send {
                id,
                dc,
                request: d::DownloadRequest::Hashes { offset, .. },
                ..
            } = e
            {
                assert_eq!(*dc, tg_mtproto::facade::download_dc_id(4, 0));
                assert_eq!(*offset, 0);
                Some(*id)
            } else {
                None
            }
        })
        .unwrap();
    assert!(m.tasks[&t].have_sent_request_for_offset(0));
    m.hashes_done(t, hash_id, vec![hash(0, b"abc")], 4);
    assert!(m.tasks[&t].loader.file.finished());
    assert_eq!(m.tasks[&t].loader.file.bytes(), b"abc");
    assert!(!m.tasks[&t].have_sent_requests());
}
#[test]
fn download_hash_reply_without_needed_hash_and_bad_key_refuse() {
    let (_s, mut m, t) = setup(0);
    m.enqueue(t, 0, 1);
    let id = ids(&mut m)[0];
    m.normal_part_loaded(
        t,
        id,
        tl::mtp_upload_file_cdn_redirect(8, vec![], vec![1], vec![2; 16], Vec::new()),
        2,
    );
    assert!(m.tasks[&t].loader.file.cancelled());
    let (_s, mut m, t) = setup(0);
    m.enqueue(t, 0, 1);
    let id = ids(&mut m)[0];
    m.normal_part_loaded(
        t,
        id,
        tl::mtp_upload_file_cdn_redirect(8, vec![9], vec![1; 32], vec![2; 16], Vec::new()),
        2,
    );
    let cdn = ids(&mut m)[0];
    let mut bytes = b"abc".to_vec();
    d::decrypt_cdn(&mut bytes, &[1; 32], &[2; 16], 0);
    m.cdn_part_loaded(t, cdn, tl::mtp_upload_cdn_file(bytes), 3);
    let id = ids(&mut m)[0];
    m.hashes_done(t, id, vec![hash(131072, b"abc")], 4);
    assert!(m.tasks[&t].loader.file.cancelled());
}
#[test]
fn download_parent_loader_start_and_terminal_effect_order() {
    let (_s, mut m, t) = setup(0);
    m.start_task(t, 1);
    let request = ids(&mut m)[0];
    m.normal_part_loaded(t, request, bytes_reply(b"abc".to_vec()), 2);
    let effects = m.take_effects();
    let done = effects
        .iter()
        .position(|e| {
            matches!(
                e,
                d::Effect::File {
                    effect: tg_core::storage::file_download::Effect::Done,
                    ..
                }
            )
        })
        .unwrap();
    let finished = effects
        .iter()
        .position(|e| matches!(e, d::Effect::TaskFinished))
        .unwrap();
    let cont = effects
        .iter()
        .position(|e| matches!(e, d::Effect::Continue { .. }))
        .unwrap();
    assert!(done < finished && finished < cont);
    m.close_task(t, 3);
    m.pump(4, 4);
    assert!(m.tasks.is_empty());
}
#[test]
fn upload_small_file_md5_parts_and_terminal_ready() {
    let mut m = u::Uploader::default();
    m.upload(message(1), upload(100_000, u::UploadKind::File), 1);
    let requests = sent(&mut m);
    assert_eq!(requests.len(), 4);
    for (id, _, request) in &requests {
        let u::UploadRequest::Small(r) = request else {
            panic!()
        };
        assert_eq!(r.file_id, 123);
        m.part_loaded(*id, true, 2);
    }
    let effects = upload_effects(&mut m, 2);
    let ready = effects
        .iter()
        .find_map(|e| {
            if let u::Effect::Ready(r) = e {
                Some(r)
            } else {
                None
            }
        })
        .unwrap();
    let tl::InputFile::InputFile(file) = &ready.file else {
        panic!()
    };
    assert_eq!(file.parts, 4);
    use md5::{Digest, Md5};
    let expected = Md5::digest(vec![3; 100_000])
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert_eq!(file.md5_checksum, expected.as_bytes());
    assert!(m.queue.is_empty());
    assert_eq!(m.next_deadline(), Some(15002));
}
#[test]
fn upload_session_boundaries_pause_cancel_and_no_local_rpc_retry() {
    let mut m = u::Uploader::default();
    m.pause(message(99));
    m.upload(message(1), upload(2 * 1024 * 1024, u::UploadKind::File), 1);
    assert!(sent(&mut m).is_empty());
    m.unpause(2);
    let requests = sent(&mut m);
    assert_eq!(m.sent_per_dc[0], 512 * 1024);
    assert_eq!(m.next_deadline(), Some(202));
    m.advance_to(202);
    assert_eq!(m.sent_per_dc[0], 576 * 1024);
    m.take_effects();
    m.part_failed(requests[0].0, &error(420, "FLOOD_WAIT_2"), 203);
    assert!(m.queue.is_empty());
    let effects = upload_effects(&mut m, 203);
    assert!(m.requests.is_empty());
    assert!(effects
        .iter()
        .any(|e| matches!(e, u::Effect::Failed { .. })));
    m.advance_to(204);
    assert!(m.next_deadline().is_some());
}
#[test]
fn upload_sliced_resend_preserves_original_missing_part_number() {
    let mut m = u::Uploader::default();
    let mut file = upload(0, u::UploadKind::Photo);
    file.parts = vec![vec![1; 32], vec![2; 32]];
    m.upload(message(1), file, 1);
    let r = sent(&mut m);
    assert_eq!(r.len(), 2);
    assert!(matches!(&r[1].2,u::UploadRequest::Small(p)if p.file_part==1));
    let first_id = r[0].0;
    m.sent_per_dc.push(0);
    let moved = r[1].0;
    m.sent_per_dc[0] -= 32;
    m.sent_per_dc[1] = 32;
    m.requests.get_mut(&moved).unwrap().dc_index = 1;
    m.remove_dc_index();
    m.take_effects();
    m.maybe_send(2);
    assert!(matches!(&sent(&mut m)[0].2,u::UploadRequest::Small(p)if p.file_part==0));
    m.part_loaded(first_id, true, 3);
}
#[test]
fn upload_fast_sessions_grow_slow_sessions_move_pending_and_normalize() {
    let mut m = u::Uploader::default();
    assert_eq!(m.choose_dc_index(&BTreeSet::new(), 1), Some(0));
    m.sent_per_dc[0] = 100;
    m.fast.insert(0);
    assert_eq!(m.choose_dc_index(&BTreeSet::new(), 2), Some(1));
    m.sent_per_dc.fill(0);
    m.fast.clear();
    m.upload(message(1), upload(2 * 1024 * 1024, u::UploadKind::File), 3);
    let r = sent(&mut m);
    assert!(!r.is_empty());
    m.part_loaded(r[0].0, true, 9003);
    assert_eq!(m.sent_per_dc.len(), 1);
    assert_eq!(m.latest_removed, 9003);
    m.cancel_all(9004);
    upload_effects(&mut m, 9004);
    assert!(m.requests.is_empty());
    assert!(m.sent_per_dc.is_empty());
}
#[test]
fn upload_prepare_parallel_queues_and_late_cancel_path_cleanup() {
    let mut m = u::Uploader::default();
    for (n, kind) in [
        (1, u::Preparation::Archive),
        (2, u::Preparation::Archive),
        (3, u::Preparation::Transcode),
    ] {
        let mut f = upload(10, u::UploadKind::File);
        f.preparing = Some(kind);
        m.upload(message(n), f, 1);
    }
    let e = upload_effects(&mut m, 2);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, u::Effect::StartPreparation { .. }))
            .count(),
        2
    );
    m.cancel(message(1), 2);
    assert!(m
        .take_effects()
        .iter()
        .any(|e| matches!(e, u::Effect::CancelPreparation(_))));
    m.finish_preparation(
        message(1),
        u::Preparation::Archive,
        Some(u::Content::Path("does-not-exist".into())),
        3,
    );
    let e = upload_effects(&mut m, 2);
    assert!(e.iter().any(|e| matches!(e, u::Effect::RemoveTemporary(_))));
    assert!(e
        .iter()
        .any(|e| matches!(e,u::Effect::StartPreparation{item,..}if *item==message(2))));
    m.finish_preparation(
        message(2),
        u::Preparation::Archive,
        Some(u::Content::Bytes(vec![7; 20])),
        4,
    );
    assert!(!sent(&mut m).is_empty());
}
#[test]
fn upload_photo_extension_secondary_secure_and_nonpremium() {
    for kind in [
        u::UploadKind::Photo,
        u::UploadKind::SecondaryFile,
        u::UploadKind::Secure,
    ] {
        let mut m = u::Uploader::default();
        let mut f = upload(0, kind);
        f.parts = vec![vec![1; 10]];
        m.upload(message(1), f, 1);
        let id = sent(&mut m)[0].0;
        m.non_premium_delayed(id);
        m.part_loaded(id, true, 2);
        let e = upload_effects(&mut m, 2);
        assert!(e.iter().any(|e| matches!(e, u::Effect::NonPremiumDelay(_))));
        if kind == u::UploadKind::Secure {
            assert!(e
                .iter()
                .any(|e| matches!(e, u::Effect::SecureReady { parts: 1, .. })));
        } else {
            let ready = e
                .iter()
                .find_map(|e| {
                    if let u::Effect::Ready(r) = e {
                        Some(r)
                    } else {
                        None
                    }
                })
                .unwrap();
            let tl::InputFile::InputFile(f) = &ready.file else {
                panic!()
            };
            assert_eq!(
                &f.name,
                if kind == u::UploadKind::Photo {
                    b"file.bin.jpg".as_slice()
                } else {
                    b"file.bin"
                }
            );
        }
    }
}
#[test]
fn sender_adapters_register_real_rpc_ids_and_decode_replies_without_network() {
    use tg_mtproto::{
        mtp_instance::{Fields, Instance, Mode},
        mtproto_dc_options::Environment,
        mtproto_response::Response,
        sender::Sender,
    };
    let i = Instance::new(Mode::Normal, Fields::new(Environment::Test));
    let sender = Sender::new(i.clone());
    let (_s, mut m, t) = setup(0);
    m.enqueue(t, 0, 1);
    let effect = m
        .take_effects()
        .into_iter()
        .find(|e| matches!(e, d::Effect::Send { .. }))
        .unwrap();
    let d::Effect::Send { id, .. } = &effect else {
        panic!()
    };
    let id = *id;
    let replies = Arc::new(Mutex::new(Vec::new()));
    let out = replies.clone();
    assert!(transport::send_download(
        &sender,
        effect,
        UserId::new(1),
        None,
        Arc::new(move |t, id, r| out.lock().unwrap().push((t, id, r)))
    ));
    assert!(sender.pending(id));
    i.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&bytes_reply(b"abc".to_vec())),
    });
    let (t, id, r) = replies.lock().unwrap().pop().unwrap();
    m.handle_reply(t, id, r, 2);
    assert!(m.tasks[&t].loader.file.finished());
    let mut up = u::Uploader::default();
    up.upload(message(1), upload(1, u::UploadKind::File), 1);
    let effect = up
        .take_effects()
        .into_iter()
        .find(|e| matches!(e, u::Effect::Send { .. }))
        .unwrap();
    let u::Effect::Send { id, .. } = &effect else {
        panic!()
    };
    let id = *id;
    let replies = Arc::new(Mutex::new(Vec::new()));
    let out = replies.clone();
    assert!(transport::send_upload(
        &sender,
        effect,
        Arc::new(move |id, r| out.lock().unwrap().push((id, r)))
    ));
    i.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&true),
    });
    assert!(replies.lock().unwrap().pop().unwrap().1.unwrap());
}
#[test]
fn event_dispatch_releases_machine_borrow() {
    let machine = Rc::new(RefCell::new(u::Uploader::default()));
    let events = u::UploadEvents::default();
    let life = tg_rpl::Lifetime::new();
    let m = machine.clone();
    events
        .document_progress()
        .on_next(move |_| m.borrow_mut().pause(message(99)), &life);
    machine
        .borrow_mut()
        .upload(message(1), upload(1, u::UploadKind::File), 1);
    let r = sent(&mut machine.borrow_mut());
    machine.borrow_mut().part_loaded(r[0].0, true, 2);
    let effects = machine.borrow_mut().take_effects();
    for e in effects {
        events.apply(&e);
        machine.borrow_mut().apply_continuation(&e, 2);
    }
    assert_eq!(machine.borrow().paused, Some(message(99)));
}

#[test]
fn upload_progress_callback_cancels_before_completion_and_next_send() {
    let machine = Rc::new(RefCell::new(u::Uploader::default()));
    let events = u::UploadEvents::default();
    let life = tg_rpl::Lifetime::new();
    let model = machine.clone();
    events
        .document_progress()
        .on_next(move |_| model.borrow_mut().cancel(message(1), 3), &life);
    machine
        .borrow_mut()
        .upload(message(1), upload(2 * 1024 * 1024, u::UploadKind::File), 1);
    let r = sent(&mut machine.borrow_mut());
    machine.borrow_mut().part_loaded(r[0].0, true, 2);
    let mut work = std::collections::VecDeque::from(machine.borrow_mut().take_effects());
    let mut failed = 0;
    while let Some(e) = work.pop_front() {
        assert!(!matches!(e, u::Effect::Ready(_) | u::Effect::Send { .. }));
        failed += usize::from(matches!(e, u::Effect::Failed { .. }));
        events.apply(&e);
        machine.borrow_mut().apply_continuation(&e, 3);
        for nested in machine.borrow_mut().take_effects().into_iter().rev() {
            work.push_front(nested);
        }
    }
    assert_eq!(failed, 1);
    assert!(machine.borrow().queue.is_empty());
    assert!(machine.borrow().requests.is_empty());
}

#[test]
fn upload_big_threshold_capacity_and_bad_input() {
    let dir = std::env::temp_dir().join(format!("tg-transfer-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("sparse");
    let file = std::fs::File::create(&path).unwrap();
    file.set_len((u::USE_BIG_FILES_FROM + 1) as u64).unwrap();
    for (size, big) in [
        (u::USE_BIG_FILES_FROM, false),
        (u::USE_BIG_FILES_FROM + 1, true),
    ] {
        let mut m = u::Uploader::default();
        let mut f = upload(0, u::UploadKind::File);
        f.size = size;
        f.content = u::Content::Path(path.clone());
        m.upload(message(1), f, 1);
        let r = sent(&mut m);
        assert!(!r.is_empty());
        assert_eq!(matches!(r[0].2, u::UploadRequest::Big(_)), big);
        m.cancel_all(2);
        upload_effects(&mut m, 2);
    }
    // Sliced uploads refuse equality at 1MiB, document uploads allow it.
    let mut m = u::Uploader::default();
    let mut f = upload(0, u::UploadKind::Photo);
    f.parts = vec![vec![1; u::MAX_UPLOAD_PER_SESSION]];
    m.upload(message(1), f, 1);
    assert!(sent(&mut m).is_empty());
    for content in [
        u::Content::Path(dir.join("missing")),
        u::Content::Bytes(vec![1]),
    ] {
        let mut m = u::Uploader::default();
        let mut f = upload(0, u::UploadKind::File);
        f.content = content;
        f.size = 100_000;
        m.upload(message(1), f, 1);
        assert!(upload_effects(&mut m, 1)
            .iter()
            .any(|e| matches!(e, u::Effect::Failed { .. })));
        assert!(m.queue.is_empty());
        assert!(m.requests.is_empty());
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn upload_video_cover_real_photo_result_and_cancellation() {
    for cancel in [false, true] {
        let mut m = u::Uploader::default();
        let mut f = upload(1, u::UploadKind::File);
        let mut cover = upload(0, u::UploadKind::Photo);
        cover.id = 789;
        cover.parts = vec![vec![4; 10]];
        f.video_cover = Some(Box::new(cover));
        m.upload(message(1), f, 1);
        let part = sent(&mut m)[0].0;
        m.part_loaded(part, true, 2);
        let e = upload_effects(&mut m, 2);
        let (video, cover) = e
            .into_iter()
            .find_map(|e| match e {
                u::Effect::NeedCoverId { video, cover } => Some((video, *cover)),
                _ => None,
            })
            .unwrap();
        m.upload_video_cover(video, message(2), cover, 3);
        let id = sent(&mut m)[0].0;
        if cancel {
            m.cancel(message(1), 4);
            let e = upload_effects(&mut m, 4);
            assert!(e
                .iter()
                .any(|e| matches!(e,u::Effect::Failed{item,..} if *item==message(1))));
            assert!(m.queue.is_empty());
            assert!(m.requests.is_empty());
            m.part_loaded(id, true, 5); // Late network callback is harmless.
        } else {
            m.part_loaded(id, true, 4);
            assert!(upload_effects(&mut m, 4)
                .iter()
                .any(|e| matches!(e,u::Effect::UploadCover{video,..}if *video==message(1))));
            let photo = tl::mtp_photo(
                tl::tl::Flags::from_raw(0),
                999,
                888,
                vec![7],
                0,
                Vec::new(),
                None,
                4,
            );
            m.cover_uploaded(
                message(1),
                message(2),
                tl::mtp_message_media_photo(tl::tl::Flags::from_raw(1), Some(photo), None, None),
                5,
            );
            let e = upload_effects(&mut m, 5);
            assert!(e.iter().any(|e| matches!(e,u::Effect::Ready(v) if v.item==message(1) && v.video_cover.as_ref().is_some_and(|p|matches!(p,tl::InputPhoto::InputPhoto(p)if p.id==999)))));
        }
    }
}

#[test]
fn real_sender_flood_migrate_retry_and_nonpremium_notices() {
    use tg_mtproto::{
        mtp_instance::{Fields, Instance, Mode},
        mtproto_dc_options::Environment,
        mtproto_response::Response,
        sender::Sender,
    };
    let i = Instance::new(Mode::Normal, Fields::new(Environment::Test));
    let sender = Sender::new(i.clone());
    let (_s, mut down, _) = setup(0);
    let mut up = u::Uploader::default();
    let mut notices = transport::TransferNotices::new(&i);
    up.upload(message(1), upload(1, u::UploadKind::File), 1);
    let effect = up
        .take_effects()
        .into_iter()
        .find(|e| matches!(e, u::Effect::Send { .. }))
        .unwrap();
    let u::Effect::Send { id, .. } = effect else {
        unreachable!()
    };
    let replies = Arc::new(Mutex::new(Vec::new()));
    let out = replies.clone();
    transport::send_upload(
        &sender,
        effect,
        Arc::new(move |id, r| out.lock().unwrap().push((id, r))),
    );
    for (code, text) in [
        (303, "FILE_MIGRATE_2"),
        (420, "FLOOD_WAIT_2"),
        (420, "FLOOD_PREMIUM_WAIT_1"),
    ] {
        i.process_callback(&Response {
            request_id: id,
            outer_msg_id: 0,
            reply: tg_tl::serialize(&tl::mtp_rpc_error(code, text.as_bytes().to_vec())),
        });
        assert!(sender.pending(id));
        assert!(replies.lock().unwrap().is_empty());
        assert!(up.requests.contains_key(&id));
    }
    assert!(i
        .take_session_spawns()
        .iter()
        .any(|s| s.id == tg_mtproto::facade::upload_dc_id(0) + 2));
    assert!(i.next_delayed_deadline().is_some());
    notices.poll(&mut down, &mut up, 2);
    assert!(up.requests[&id].non_premium_delayed);
    let at = i.next_delayed_deadline().unwrap();
    i.check_delayed_requests(at);
    i.process_callback(&Response {
        request_id: id,
        outer_msg_id: 0,
        reply: tg_tl::serialize(&true),
    });
    let (id, result) = replies.lock().unwrap().pop().unwrap();
    up.part_loaded(id, result.unwrap(), at + 1);
    assert!(upload_effects(&mut up, at + 1)
        .iter()
        .any(|e| matches!(e, u::Effect::NonPremiumDelay(_))));
    assert!(up.queue.is_empty());
}

#[test]
fn download_redirect_resends_window_first_hash_wins_and_nonpremium() {
    let (_s, mut m, t) = setup(4 * 131072);
    m.enqueue(t, 0, 1);
    let requests = ids(&mut m);
    let first = hash(0, b"first");
    m.normal_part_loaded(
        t,
        requests[0],
        tl::mtp_upload_file_cdn_redirect(
            8,
            vec![9],
            vec![1; 32],
            vec![2; 16],
            vec![
                tl::mtp_file_hash(0, 1, first.hash.clone()),
                tl::mtp_file_hash(0, 1, vec![0; 32]),
            ],
        ),
        2,
    );
    let e = m.take_effects();
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, d::Effect::Cancel(_)))
            .count(),
        3
    );
    assert_eq!(e.iter().filter(|e|matches!(e,d::Effect::Send{request:d::DownloadRequest::Cdn{..},dc,..}if *dc==tg_mtproto::facade::download_dc_id(8,0))).count(),4);
    assert_eq!(m.tasks[&t].hashes[&0], first);
    assert_eq!(m.balances[&4].total_requested, 4 * d::PART_SIZE);
    let current = *m.tasks[&t].sent.keys().next().unwrap();
    m.non_premium_delayed(current);
    assert!(matches!(
        m.take_effects().as_slice(),
        [d::Effect::NonPremiumDelay(55)]
    ));
    m.close_task(t, 3);
    m.normal_part_loaded(t, current, bytes_reply(vec![1]), 4);
    assert!(m.tasks.is_empty());
}

#[test]
fn download_hash_callback_boundary_and_last_session_removal() {
    let (_s, mut m, t) = setup(2 * 131072);
    m.enqueue(t, 0, 1);
    let origin = ids(&mut m);
    m.normal_part_loaded(
        t,
        origin[0],
        tl::mtp_upload_file_cdn_redirect(8, vec![9], vec![1; 32], vec![2; 16], Vec::new()),
        2,
    );
    m.take_effects();
    let mut pending = m.tasks[&t]
        .sent
        .iter()
        .map(|(&id, r)| (id, r.offset))
        .collect::<Vec<_>>();
    pending.sort_by_key(|r| r.1);
    // Put one request in the last session before it becomes an unchecked part.
    m.balances
        .get_mut(&4)
        .unwrap()
        .sessions
        .push(d::SessionBalance::default());
    let r = m
        .tasks
        .get_mut(&t)
        .unwrap()
        .sent
        .get_mut(&pending[0].0)
        .unwrap();
    r.session = 1;
    m.balances.get_mut(&4).unwrap().sessions[0].requested -= d::PART_SIZE;
    m.balances.get_mut(&4).unwrap().sessions[1].requested = d::PART_SIZE;
    let data = vec![3; 131072];
    let mut encrypted = data.clone();
    d::decrypt_cdn(&mut encrypted, &[1; 32], &[2; 16], 0);
    m.cdn_part_loaded(t, pending[0].0, tl::mtp_upload_cdn_file(encrypted), 3);
    m.take_effects();
    // The hash request uses session1; removal cancels it, remaps unchecked data,
    // and posts another hash request on a remaining origin-DC session.
    for _ in 0..4 {
        m.session_timed_out(4, 1, 4);
    }
    assert_eq!(m.balances[&4].sessions.len(), 1);
    assert!(m.has_deferred_work());
    m.take_effects();
    m.advance_to(5);
    let hash_id = m
        .take_effects()
        .iter()
        .find_map(|e| match e {
            d::Effect::Send {
                id,
                dc,
                request: d::DownloadRequest::Hashes { .. },
                ..
            } => {
                assert_eq!(*dc, tg_mtproto::facade::download_dc_id(4, 0));
                Some(*id)
            }
            _ => None,
        })
        .unwrap();
    let mut encrypted = data.clone();
    d::decrypt_cdn(&mut encrypted, &[1; 32], &[2; 16], 131072);
    m.cdn_part_loaded(t, pending[1].0, tl::mtp_upload_cdn_file(encrypted), 6);
    m.take_effects();
    m.hashes_done(t, hash_id, vec![hash(0, &data), hash(131072, &data)], 7);
    assert!(!m.tasks[&t].loader.file.finished());
    let effects = m.take_effects();
    let continuation = effects
        .iter()
        .find(|e| matches!(e, d::Effect::ContinueHashes { .. }))
        .unwrap();
    assert_eq!(m.tasks[&t].loader.file.bytes().len(), 131072);
    // Simulate destruction by the progress subscriber before the second feed.
    m.close_task(t, 8);
    m.apply_continuation(continuation, 8);
    assert!(m.tasks.is_empty());
}

#[test]
fn upload_metadata_events_and_failure_ready_order() {
    assert_eq!(u::thumbnail_format("image/webp"), "WEBP");
    assert_eq!(u::thumbnail_format("video/webm"), "JPG");
    assert_eq!(u::thumbnail_format("image/jpeg"), "JPG");
    let mut m = u::Uploader::default();
    let mut f = upload(1, u::UploadKind::File);
    f.force_file = true;
    f.attached_stickers = vec![tl::mtp_input_document(10, 11, vec![12])];
    m.upload(message(1), f, 1);
    let id = sent(&mut m)[0].0;
    m.part_loaded(id, true, 2);
    let e = upload_effects(&mut m, 2);
    assert!(e
        .iter()
        .any(|e| matches!(e,u::Effect::Ready(r)if r.force_file && r.attached_stickers.len()==1)));
    assert_eq!(m.current_upload_id(), FullMsgId::default());
    m.advance_to(15002);
    assert!(m
        .take_effects()
        .iter()
        .any(|e| matches!(e, u::Effect::StopSession(_))));
    let events = u::UploadEvents::default();
    let life = tg_rpl::Lifetime::new();
    let observed = Rc::new(RefCell::new(Vec::new()));
    macro_rules! observe {
        ($producer:ident,$name:expr) => {{
            let out = observed.clone();
            events
                .$producer()
                .on_next(move |_| out.borrow_mut().push($name), &life);
        }};
    }
    observe!(photo_ready, "photo-ready");
    observe!(document_ready, "document-ready");
    observe!(secondary_file_ready, "secondary-ready");
    observe!(secure_ready, "secure-ready");
    observe!(photo_progress, "photo-progress");
    observe!(document_progress, "document-progress");
    observe!(secondary_file_progress, "secondary-progress");
    observe!(secure_progress, "secure-progress");
    observe!(photo_failed, "photo-failed");
    observe!(document_failed, "document-failed");
    observe!(secondary_file_failed, "secondary-failed");
    observe!(secure_failed, "secure-failed");
    observe!(non_premium_delays, "non-premium");
    for kind in [
        u::UploadKind::Photo,
        u::UploadKind::File,
        u::UploadKind::SecondaryFile,
        u::UploadKind::Secure,
    ] {
        events.apply(&u::Effect::Progress {
            item: message(1),
            id: 123,
            kind,
            offset: 1,
            size: 2,
        });
        events.apply(&u::Effect::Failed {
            item: message(1),
            id: 123,
            kind,
        });
        if kind == u::UploadKind::Secure {
            events.apply(&u::Effect::SecureReady {
                item: message(1),
                id: 123,
                parts: 1,
            });
        } else {
            events.apply(&u::Effect::Ready(u::UploadedMedia {
                id: 123,
                item: message(1),
                file: tl::mtp_input_file(123, 1, b"x".to_vec(), Vec::new()),
                thumb: None,
                kind,
                video_cover: None,
                attached_stickers: Vec::new(),
                force_file: false,
            }));
        }
    }
    events.apply(&u::Effect::NonPremiumDelay(message(1)));
    assert_eq!(observed.borrow().len(), 13);
}

#[test]
fn transport_location_and_web_reply_variants() {
    let self_id = UserId::new(1);
    use StorageFileType::*;
    for kind in [
        Legacy,
        Encrypted,
        Document,
        Secure,
        Takeout,
        Photo,
        PeerPhoto,
        StickerSetThumb,
        GroupCallStream,
    ] {
        let s = StorageFileLocation {
            dc_id: 4,
            kind,
            id: 55,
            access_hash: 99,
            volume_id: 77,
            local_id: 2,
            size_letter: b'c',
            file_reference: vec![3],
            ..Default::default()
        };
        let input = transport::storage_input(&s, self_id);
        match kind {
            Legacy => assert!(
                matches!(input,tl::InputFileLocation::InputFileLocation(p)if p.volume_id==77 && p.local_id==2)
            ),
            Encrypted | Secure => assert!(
                matches!(input,tl::InputFileLocation::InputSecureFileLocation(p)if p.id==55)
            ),
            Document => assert!(
                matches!(input,tl::InputFileLocation::InputDocumentFileLocation(p)if p.thumb_size==b"c")
            ),
            Photo => assert!(
                matches!(input,tl::InputFileLocation::InputPhotoFileLocation(p)if p.file_reference==[3])
            ),
            Takeout => assert!(matches!(
                input,
                tl::InputFileLocation::InputTakeoutFileLocation(_)
            )),
            PeerPhoto => assert!(
                matches!(input,tl::InputFileLocation::InputPeerPhotoFileLocation(p)if p.photo_id==77)
            ),
            StickerSetThumb => assert!(
                matches!(input,tl::InputFileLocation::InputStickerSetThumb(p)if p.thumb_version==2)
            ),
            GroupCallStream => assert!(
                matches!(input,tl::InputFileLocation::InputGroupCallStream(p)if p.video_channel==Some(35) && p.video_quality==Some(1))
            ),
        }
    }
    for peer in [
        tl::mtp_input_peer_self(),
        tl::mtp_input_peer_user(7, 8),
        tl::mtp_input_peer_chat(9),
        tl::mtp_input_peer_channel(10, 11),
        tl::mtp_input_peer_user_from_message(tl::mtp_input_peer_channel(10, 11), 12, 7),
        tl::mtp_input_peer_channel_from_message(tl::mtp_input_peer_user(7, 8), 12, 10),
    ] {
        let s = StorageFileLocation::peer_photo(4, self_id, &peer, 77, true);
        let tl::InputFileLocation::InputPeerPhotoFileLocation(p) =
            transport::storage_input(&s, self_id)
        else {
            panic!()
        };
        assert_eq!(tg_tl::serialize(&p.peer), tg_tl::serialize(&peer));
    }
    let (_s, mut m, t) = setup(0);
    m.enqueue(t, 0, 1);
    let id = ids(&mut m)[0];
    m.web_part_loaded(
        t,
        id,
        tl::mtp_upload_web_file(
            3,
            b"x/y".to_vec(),
            tl::mtp_storage_file_unknown(),
            0,
            b"abc".to_vec(),
        ),
        2,
    );
    assert!(m.tasks[&t].loader.file.finished());
    assert_eq!(m.tasks[&t].loader.file.bytes(), b"abc");
    let (_s, mut m, t) = setup(100);
    m.enqueue(t, 0, 1);
    let id = ids(&mut m)[0];
    m.web_part_loaded(
        t,
        id,
        tl::mtp_upload_web_file(
            3,
            b"x/y".to_vec(),
            tl::mtp_storage_file_unknown(),
            0,
            b"abc".to_vec(),
        ),
        2,
    );
    assert!(m.tasks[&t].loader.file.cancelled());
    m.pump_all(3);
}

#[test]
fn real_media_upload_state_applies_before_events() {
    use tg_core::data::{
        data_document::DocumentData, data_media_types::FileStatus, data_photo::PhotoData,
    };
    let scope = SessionScope::new();
    let mut document = DocumentData::new(&scope, 123);
    let mut photo = PhotoData::new(&scope, 123);
    let mut m = u::Uploader::default();
    let mut f = upload(10, u::UploadKind::File);
    f.preparing = Some(u::Preparation::Archive);
    m.upload(message(1), f, 1);
    for e in m.take_effects() {
        u::apply_document_upload_effect(&e, &mut document);
    }
    assert!(document.uploading_data.as_ref().unwrap().preparing);
    m.update_prepare_progress(message(1), 0.5);
    for e in m.take_effects() {
        u::apply_document_upload_effect(&e, &mut document);
    }
    assert_eq!(
        document.uploading_data.as_ref().unwrap().prepare_progress,
        0.5
    );
    m.finish_preparation(
        message(1),
        u::Preparation::Archive,
        Some(u::Content::Bytes(vec![3; 20])),
        2,
    );
    for e in upload_effects(&mut m, 2) {
        u::apply_document_upload_effect(&e, &mut document);
    }
    assert_eq!(document.size, 20);
    assert!(!document.uploading_data.as_ref().unwrap().preparing);
    m.cancel(message(1), 3);
    for e in upload_effects(&mut m, 3) {
        u::apply_document_upload_effect(&e, &mut document);
    }
    assert_eq!(document.status, FileStatus::UploadFailed);
    assert!(u::apply_photo_upload_effect(
        &u::Effect::BeginUpload {
            item: message(1),
            id: 123,
            kind: u::UploadKind::Photo,
            size: 30,
            preparing: false
        },
        &mut photo
    ));
    u::apply_photo_upload_effect(
        &u::Effect::Progress {
            item: message(1),
            id: 123,
            kind: u::UploadKind::Photo,
            size: 30,
            offset: 15,
        },
        &mut photo,
    );
    assert_eq!(photo.uploading_progress(), Some(0.5));
    assert!(!u::apply_photo_upload_effect(
        &u::Effect::Progress {
            item: message(1),
            id: 999,
            kind: u::UploadKind::Photo,
            size: 30,
            offset: 30
        },
        &mut photo
    ));
}
