//! Real typed Sender adapter. The caller posts replies to its MainRef/owner;
//! no callback holds a borrow of download/upload model state.
use super::{
    download_manager_mtproto::{self as download, DownloadRequest, TaskId},
    file_upload::{self as upload, UploadRequest},
};
use crate::data::{data_image_location::*, data_peer_id::*};
use std::sync::Arc;
use tg_mtproto::{mtproto_response::Error, sender::Sender};
use tg_scheme as tl;
fn input_peer(
    id: PeerId,
    access: u64,
    in_peer: PeerId,
    in_message: u32,
    self_id: UserId,
) -> tl::InputPeer {
    let base = if id.value == 0 {
        tl::mtp_input_peer_empty()
    } else if id == peer_from_user(self_id) {
        tl::mtp_input_peer_self()
    } else if peer_is_user(id) {
        tl::mtp_input_peer_user(peer_to_user(id).bare, access)
    } else if peer_is_chat(id) {
        tl::mtp_input_peer_chat(peer_to_chat(id).bare)
    } else if peer_is_channel(id) {
        tl::mtp_input_peer_channel(peer_to_channel(id).bare, access)
    } else {
        tl::mtp_input_peer_empty()
    };
    if in_message != 0 && peer_is_user(in_peer) {
        tl::mtp_input_peer_user_from_message(base, in_message as i32, peer_to_user(in_peer).bare)
    } else if in_message != 0 && peer_is_channel(in_peer) {
        tl::mtp_input_peer_channel_from_message(
            base,
            in_message as i32,
            peer_to_channel(in_peer).bare,
        )
    } else {
        base
    }
}
pub fn storage_input(s: &StorageFileLocation, self_id: UserId) -> tl::InputFileLocation {
    use StorageFileType::*;
    match s.kind {
        Legacy => tl::mtp_input_file_location(
            s.volume_id,
            s.local_id,
            s.access_hash,
            s.file_reference.clone(),
        ),
        Encrypted | Secure => tl::mtp_input_secure_file_location(s.id, s.access_hash),
        Document => tl::mtp_input_document_file_location(
            s.id,
            s.access_hash,
            s.file_reference.clone(),
            if s.size_letter == 0 {
                Vec::new()
            } else {
                vec![s.size_letter]
            },
        ),
        Photo => tl::mtp_input_photo_file_location(
            s.id,
            s.access_hash,
            s.file_reference.clone(),
            vec![s.size_letter],
        ),
        Takeout => tl::mtp_input_takeout_file_location(),
        PeerPhoto => tl::mtp_input_peer_photo_file_location(
            tl::tl::Flags::from_raw(u64::from(s.size_letter == b'c')),
            input_peer(
                PeerId::new(s.id),
                s.access_hash,
                s.in_message_peer_id,
                s.in_message_id,
                self_id,
            ),
            s.volume_id,
        ),
        StickerSetThumb => tl::mtp_input_sticker_set_thumb(
            tl::mtp_input_sticker_set_id(s.id, s.access_hash),
            s.local_id,
        ),
        GroupCallStream => tl::mtp_input_group_call_stream(
            tl::tl::Flags::from_raw(if s.size_letter != 0 { 1 } else { 0 }),
            tl::mtp_input_group_call(s.id, s.access_hash),
            s.volume_id,
            s.local_id,
            (s.size_letter != 0).then_some(i32::from(s.size_letter & 63)),
            (s.size_letter != 0).then_some(i32::from((s.size_letter >> 6) & 3)),
        ),
    }
}
#[derive(Clone)]
pub enum DownloadReply {
    File(tl::UploadFile),
    Web(tl::UploadWebFile),
    Cdn(tl::UploadCdnFile),
    Hashes(Vec<download::CdnHash>),
    Reupload(Vec<download::CdnHash>),
    Failed {
        error: Error,
        cdn: bool,
        reference: Option<Vec<u8>>,
    },
}
/// AudioAlbumThumb's InputDocument must come from the real Documents owner.
/// None means that required model input has not been supplied; no fake request.
pub fn send_download(
    sender: &Sender,
    effect: download::Effect,
    self_id: UserId,
    audio_document: Option<tl::InputDocument>,
    reply: Arc<dyn Fn(TaskId, i32, DownloadReply) + Send + Sync>,
) -> bool {
    let download::Effect::Send {
        task,
        id,
        dc,
        request,
    } = effect
    else {
        return false;
    };
    macro_rules! send {
        ($rpc:expr,$kind:path,$cdn:expr,$reference:expr) => {{
            let done = reply.clone();
            let fail = reply.clone();
            let reference = $reference;
            sender
                .request($rpc)
                .override_id(id)
                .to_dc(dc)
                .done(move |r| done(task, id, $kind(r)))
                .fail(move |e| {
                    fail(
                        task,
                        id,
                        DownloadReply::Failed {
                            error: e.clone(),
                            cdn: $cdn,
                            reference: reference.clone(),
                        },
                    )
                })
                .send();
        }};
    }
    match request {
        DownloadRequest::Cdn { token, offset } => send!(
            tl::UploadGetCdnFile::new(token, offset as u64, download::PART_SIZE),
            DownloadReply::Cdn,
            true,
            None::<Vec<u8>>
        ),
        DownloadRequest::Hashes { token, offset } => {
            let done = reply.clone();
            let fail = reply;
            sender
                .request(tl::UploadGetCdnFileHashes::new(token, offset as u64))
                .override_id(id)
                .to_dc(dc)
                .done(move |r| done(task, id, DownloadReply::Hashes(download::tl_hashes(&r))))
                .fail(move |e| {
                    fail(
                        task,
                        id,
                        DownloadReply::Failed {
                            error: e.clone(),
                            cdn: true,
                            reference: None,
                        },
                    )
                })
                .send();
        }
        DownloadRequest::Reupload {
            token,
            request_token,
        } => {
            let done = reply.clone();
            let fail = reply;
            sender
                .request(tl::UploadReuploadCdnFile::new(token, request_token))
                .override_id(id)
                .to_dc(dc)
                .done(move |r| done(task, id, DownloadReply::Reupload(download::tl_hashes(&r))))
                .fail(move |e| {
                    fail(
                        task,
                        id,
                        DownloadReply::Failed {
                            error: e.clone(),
                            cdn: true,
                            reference: None,
                        },
                    )
                })
                .send();
        }
        DownloadRequest::File { location, offset } => match location {
            DownloadLocation::Storage(s) => send!(
                tl::UploadGetFile::new(
                    tl::UploadGetFile::F_CDN_SUPPORTED,
                    storage_input(&s, self_id),
                    offset as u64,
                    download::PART_SIZE
                ),
                DownloadReply::File,
                false,
                Some(s.file_reference.clone())
            ),
            DownloadLocation::Web { url, access_hash } => send!(
                tl::UploadGetWebFile::new(
                    tl::mtp_input_web_file_location(url, access_hash),
                    offset as i32,
                    download::PART_SIZE
                ),
                DownloadReply::Web,
                false,
                None::<Vec<u8>>
            ),
            DownloadLocation::Geo(g) => send!(
                tl::UploadGetWebFile::new(
                    tl::mtp_input_web_file_geo_point_location(
                        tl::mtp_input_geo_point(tl::tl::Flags::from_raw(0), g.lat, g.lon, None),
                        g.access,
                        g.width,
                        g.height,
                        g.zoom,
                        g.scale
                    ),
                    offset as i32,
                    download::PART_SIZE
                ),
                DownloadReply::Web,
                false,
                None::<Vec<u8>>
            ),
            DownloadLocation::AudioAlbumThumb { .. } => {
                let Some(document) = audio_document else {
                    return false;
                };
                send!(
                    tl::UploadGetWebFile::new(
                        tl::mtp_input_web_file_audio_album_thumb_location(
                            tl::tl::Flags::from_raw(5),
                            Some(document),
                            None,
                            None
                        ),
                        offset as i32,
                        download::PART_SIZE
                    ),
                    DownloadReply::Web,
                    false,
                    None::<Vec<u8>>
                );
            }
            _ => return false,
        },
    }
    true
}
pub fn send_upload(
    sender: &Sender,
    effect: upload::Effect,
    reply: Arc<dyn Fn(i32, Result<bool, Error>) + Send + Sync>,
) -> bool {
    let upload::Effect::Send { id, dc, request } = effect else {
        return false;
    };
    macro_rules! send {
        ($rpc:expr) => {{
            let done = reply.clone();
            let fail = reply.clone();
            sender
                .request($rpc)
                .override_id(id)
                .to_dc(dc)
                .done(move |r| done(id, Ok(r)))
                .fail(move |e| fail(id, Err(e.clone())))
                .send();
        }};
    }
    match request {
        UploadRequest::Small(r) => send!(r),
        UploadRequest::Big(r) => send!(r),
    }
    true
}
pub fn apply_download_control(sender: &Sender, effect: &download::Effect) -> bool {
    match *effect {
        download::Effect::Cancel(id) => sender.cancel(id),
        download::Effect::KillSession(id) => sender.instance().kill_session(id),
        download::Effect::StopSession(id) => sender.instance().stop_session(id),
        _ => return false,
    }
    true
}
pub fn apply_upload_control(sender: &Sender, effect: &upload::Effect) -> bool {
    match *effect {
        upload::Effect::Cancel(id) => sender.cancel(id),
        upload::Effect::StopSession(id) => sender.instance().stop_session(id),
        _ => return false,
    }
    true
}

impl download::DownloadManagerMtproto {
    pub fn handle_reply(&mut self, task: TaskId, id: i32, reply: DownloadReply, now: i64) {
        match reply {
            DownloadReply::File(r) => self.normal_part_loaded(task, id, r, now),
            DownloadReply::Web(r) => self.web_part_loaded(task, id, r, now),
            DownloadReply::Cdn(r) => self.cdn_part_loaded(task, id, r, now),
            DownloadReply::Hashes(r) => self.hashes_done(task, id, r, now),
            DownloadReply::Reupload(r) => self.reupload_done(task, id, r, now),
            DownloadReply::Failed {
                error,
                cdn,
                reference,
            } => {
                self.part_failed(task, id, &error, cdn, reference.as_deref(), now);
            }
        }
    }
}
pub type CoverCallback = dyn Fn(
        crate::data::data_msg_id::FullMsgId,
        crate::data::data_msg_id::FullMsgId,
        Result<tl::MessageMedia, Error>,
    ) + Send
    + Sync;
pub fn send_cover(
    sender: &Sender,
    video: crate::data::data_msg_id::FullMsgId,
    cover: upload::UploadedMedia,
    peer: tl::InputPeer,
    reply: Arc<CoverCallback>,
) {
    let id = cover.item;
    let done = reply.clone();
    sender
        .request(tl::MessagesUploadMedia::new(
            tl::tl::Flags::from_raw(0),
            None,
            peer,
            tl::mtp_input_media_uploaded_photo(
                tl::tl::Flags::from_raw(0),
                cover.file,
                None,
                None,
                None,
            ),
        ))
        .done(move |r| done(video, id, Ok(r)))
        .fail(move |e| reply(video, id, Err(e.clone())))
        .send();
}

/// Subscribe once with the owner's lifetime. Poll on that same owner queue;
/// Instance handles network timers, the transfer machines receive virtual now.
pub struct TransferNotices {
    restarts: tg_mtproto::mtp_instance::NoticeStream<i32>,
    non_premium: tg_mtproto::mtp_instance::NoticeStream<i32>,
}
impl TransferNotices {
    pub fn new(instance: &tg_mtproto::mtp_instance::Instance) -> Self {
        Self {
            restarts: instance.restarts_by_timeout(),
            non_premium: instance.non_premium_delayed_requests(),
        }
    }
    pub fn poll(
        &mut self,
        downloads: &mut download::DownloadManagerMtproto,
        uploads: &mut upload::Uploader,
        now: i64,
    ) {
        while let Ok(dc) = self.restarts.try_recv() {
            downloads.restart_by_timeout(dc, now);
        }
        while let Ok(id) = self.non_premium.try_recv() {
            downloads.non_premium_delayed(id);
            uploads.non_premium_delayed(id);
        }
    }
}
