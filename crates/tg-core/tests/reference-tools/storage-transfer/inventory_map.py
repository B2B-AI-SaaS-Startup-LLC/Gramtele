"""Inventory declarations and bodies separately, including cpp-local Entry methods."""
from pathlib import Path
import re,sys
source=Path(sys.argv[1]);rows=[]
for stem in ['download_manager_mtproto','file_upload']:
 for ext in ['h','cpp']:
  local=False
  for line,text in enumerate((source/'storage'/f'{stem}.{ext}').read_text().splitlines(),1):
   if text.startswith('struct Uploader::Entry {'):local=True
   if text.startswith('};'):local=False
   s=text.strip()
   if not s or s.startswith(('/', '#', ':', ',', ')', '?', 'return ', 'if ', 'while','for ', 'Fn<','using ', 'const auto ', 'switch', 'case ', '_')) or '(' not in s:continue
   if ext=='cpp' and text[0].isspace() and not local:continue
   head=s.split('(')[0].strip()
   if '=' in head:continue
   m=re.search(r'([\w:~]+|operator<)$',head)
   if not m:continue
   name=m[1];member=name.split('::')[-1]
   if head==name and '::' not in name and member.lstrip('~') not in ['Entry','Uploader','DownloadManagerMtproto','DownloadMtprotoTask','DcSessionBalanceData','DcBalanceData','CdnFileHash']:continue
   rows.append((stem,ext,line,name,member))

Q='queue_lifo_reprioritize_generation_and_high_priority_starvation'
W='download_window_order_cancel_midway_and_idle_stop_preserves_count'
B='download_balance_growth_overload_timeout_removal_and_raw_restart_bug'
R='download_flood_wait_is_delegated_and_reference_expired_refreshes_same_offset'
C='download_cdn_redirect_reupload_hash_mismatch_and_token_fallback'
H='download_missing_hash_queues_real_origin_hash_request_then_finishes'
J='download_hash_callback_boundary_and_last_session_removal'
N='native_cdn_hash_and_unsigned_short_layout'
S='sender_adapters_register_real_rpc_ids_and_decode_replies_without_network'
V='upload_video_cover_real_photo_result_and_cancellation'
U='upload_small_file_md5_parts_and_terminal_ready'
T='upload_session_boundaries_pause_cancel_and_no_local_rpc_retry'
P='upload_prepare_parallel_queues_and_late_cancel_path_cleanup'
E='upload_metadata_events_and_failure_ready_order'
A='upload_fast_sessions_grow_slow_sessions_move_pending_and_normalize'
D={}
def add(names,rust,test,status='done',note=''):
 for n in names.split():D[n]=(rust,test,status,note)
add('enqueue remove resetGeneration empty nextTask','Queue::{enqueue,remove,reset_generation,next_task}; tasks.is_empty()',Q)
add('DcSessionBalanceData DcBalanceData','SessionBalance::default / DcBalance::default',B)
add('DownloadManagerMtproto','DownloadManagerMtproto::default + TransferNotices::{new,poll}', 'real_sender_flood_migrate_retry_and_nonpremium_notices','partial(26/29 ApiWrap/Main Session ownership)','real Instance subscriptions and virtual timers complete; application bootstrap owns/polls them')
add('~DownloadManagerMtproto','DownloadManagerMtproto::close',W,'partial(29 Session drop/effect dispatch)','explicit shutdown preserves due-only stop semantics; owner must apply effects before dropping')
add('api','transfer_transport::send_download + Sender argument',S,'partial(26 ApiWrap;29 Session binding)','real Sender is accepted explicitly; no fake ApiWrap/session accessor')
add('notifyTaskFinished taskFinished','Effect::TaskFinished + DownloadEvents::{task_finished,apply}','download_parent_loader_start_and_terminal_effect_order')
add('notifyNonPremiumDelay nonPremiumDelays','DownloadEvents::{non_premium_delays,apply} + non_premium_delayed','download_redirect_resends_window_first_hash_wins_and_nonpremium')
add('changeRequestedAmount','DownloadManagerMtproto::amount',W)
add('requestSucceeded chooseSessionIndex sessionTimedOut','DownloadManagerMtproto::{request_succeeded,choose_session_index,session_timed_out}',B)
add('checkSendNext checkSendNextAfterSuccess trySendNextPart','DownloadManagerMtproto::{pump,pump_all} / Effect::Continue',W)
add('killSessionsSchedule killSessionsCancel killSessions','DownloadManagerMtproto::{amount,advance_to,stop_dc,close}',W)
add('removeSession','DownloadManagerMtproto::remove_session',J)
add('DownloadMtprotoTask','DownloadMtprotoTask::new + DownloadManagerMtproto::insert',W)
add('~DownloadMtprotoTask','DownloadManagerMtproto::close_task','download_parent_loader_start_and_terminal_effect_order','partial(29 Session drop/effect dispatch)','explicit close cancels real loader and requests; owner applies effects before drop')
add('dcId fileOrigin objectId location','DownloadMtprotoTask::{dc,loader.origin,object_id,location}',R)
add('readyToRequest takeNextRequestOffset','MtpFileLoader::{ready_to_request,take_next_request_offset}',W)
add('loadPart','DownloadManagerMtproto::{pump,make_request}',W)
add('refreshFileReferenceFrom','DownloadManagerMtproto::refresh_file_reference_from',R)
add('haveSentRequests haveSentRequestForOffset','DownloadMtprotoTask::{have_sent_requests,have_sent_request_for_offset}',H)
add('cancelAllRequests cancelRequestForOffset cancelRequest','DownloadManagerMtproto::{cancel_all_requests,cancel_request_for_offset,cancel_request}',W)
add('addToQueue removeFromQueue','DownloadManagerMtproto::{enqueue,remove,drain_loader}','download_parent_loader_start_and_terminal_effect_order')
add('operator<','BTreeMap<i64, unchecked> offset ordering',J)
add('CdnFileHash','CdnHash + tl_hashes',N)
add('feedPart partLoaded','DownloadManagerMtproto::feed / MtpFileLoader::feed_part',H)
add('setWebFileSizeHook','MtpFileLoader::set_web_file_size_hook / web_part_loaded','transport_location_and_web_reply_variants')
add('cancelOnFail','DownloadManagerMtproto::fail',C)
add('makeRequest sendRequest placeSentRequest finishSentRequest','DownloadManagerMtproto::{make_request,send,finish} + transfer_transport::send_download',S)
add('normalPartLoaded','DownloadManagerMtproto::normal_part_loaded','download_redirect_resends_window_first_hash_wins_and_nonpremium')
add('webPartLoaded','DownloadManagerMtproto::web_part_loaded','transport_location_and_web_reply_variants')
add('cdnPartLoaded normalPartFailed partFailed cdnPartFailed switchToCDN changeCDNParams','DownloadManagerMtproto::{cdn_part_loaded,part_failed,change_cdn_params}',C)
add('reuploadDone','DownloadManagerMtproto::reupload_done',C)
add('requestMoreCdnFileHashes getCdnFileHashesDone','DownloadManagerMtproto::{request_more_hashes,hashes_done,continue_hashes}',J)
add('addCdnHashes','DownloadManagerMtproto::add_hashes','download_redirect_resends_window_first_hash_wins_and_nonpremium')
add('checkCdnFileHash','check_cdn_file_hash',N)
add('subscribeToNonPremiumLimit','TransferNotices::{new,poll} + DownloadManagerMtproto::non_premium_delayed','real_sender_flood_migrate_retry_and_nonpremium_notices')

F={}
def up(names,rust,test,status='done',note=''):
 for n in names.split():F[n]=(rust,test,status,note)
up('ThumbnailFormat','thumbnail_format',E)
up('Entry','Entry::new',U)
up('setDocSize setPartSize','part_layout',N)
up('Uploader','Uploader::default + UploadEvents + TransferNotices',E,'partial(18/26/29 owner subscriptions)','ready/progress streams real; send/edit message and History subscriptions remain their owners')
up('~Uploader','Uploader::clear',T,'partial(29 lifetime shutdown)','explicit clear cancels requests/stops sessions; owner applies effects before drop')
up('session','Sender argument + FullMsgId effects',S,'partial(29 Main Session)','no synthetic Session; real Photos/Documents upload-state adapters exist')
up('currentUploadId','Uploader::current_upload_id',E)
up('upload','Uploader::upload + PreparedUpload/BeginUpload',U,'partial(3/4 localimageloader;04/05/29 media projection)','queue/parts complete; real FilePrepareResult/image/cache/model mutations need owners; no fake result or model')
up('pause unpause cancel cancelAll cancelRequests cancelAllRequests clear stopSessions','Uploader::{pause,unpause,cancel,cancel_all,cancel_requests,cancel_all_requests,clear,stop_sessions}',T)
up('photoReady documentReady secondaryFileReady secureReady photoProgress documentProgress secureProgress secondaryFileProgress photoFailed documentFailed secondaryFileFailed secureFailed nonPremiumDelays','UploadEvents producers/apply',E)
up('maybeSend','Uploader::{maybe_send,send_loop,apply_continuation}',T)
up('startTranscode maybeStartTranscode','Uploader::{upload,maybe_start_preparation}',P)
up('runTranscode','StartPreparation/CancelPreparation + finish_preparation',P,'partial(3/4 archive worker;5/7 media encode;29 worker scheduling)','real archive backend already exists; actual FilePrepareResult/job/VideoSource wiring not invented')
up('updatePrepareProgress','Uploader::update_prepare_progress + apply_document_upload_effect','real_media_upload_state_applies_before_events')
up('finishTranscode','Uploader::finish_preparation',P,'partial(04/29 document cache/location application)','actual bytes/path sizing, queues and real DocumentData upload state complete; cache/location application remains owner')
up('canAddDcIndex chooseDcIndexForNextRequest removeDcIndex','Uploader::{can_add_dc_index,choose_dc_index,remove_dc_index}',A)
up('chooseEntryForNextRequest','Uploader::choose_entry',P)
up('sendPart sendPendingPart sendDocPart sendPreparedRequest','Uploader::{send_part,send_request} + transfer_transport::send_upload',U)
up('sendSlicedPart','Uploader::send_part','upload_sliced_resend_preserves_original_missing_part_number')
up('readDocPart','Entry::read_doc_part','upload_big_threshold_capacity_and_bad_input')
up('maybeFinishFront finishFront','Uploader::{continue_finish,finish_front}',E,'partial(26 SendOptions/edit routing)','real TL file/thumb/cover/stickers/force_file preserved; caller retains actual send options/edit metadata under FullMsgId')
up('partLoaded finishRequest','Uploader::{part_loaded,finish_request,apply_continuation}','upload_progress_callback_cancels_before_completion_and_next_send')
up('partFailed notifyFailed failed','Uploader::{part_failed,failed,continue_failure} + apply_document_upload_effect',T)
up('uploadVideoCover','Uploader::upload_video_cover + NeedCoverId',V,'partial(29 local message ID allocation)','owner supplies genuine nextLocalMessageId and peer; no fake local ID generator')
up('uploadCoverAsPhoto','transfer_transport::send_cover + Uploader::cover_uploaded',V)
up('processPhotoProgress processPhotoFailed processDocumentProgress processDocumentFailed sendProgressUpdate','UploadEvents / ordered Progress/Failed effects',E,'partial(18–20 HistoryItem;26 SendProgressManager;5 UI repaint)','real sendAction/HistoryItem/repaint are not available; no fabricated thread/item methods')

lines=['# Storage transfer function map','','Original read-only `0f2a31bb420de98f331990432265b131b8d836b5`; 4 files / 2837 LOC.',
'Header declarations and cpp bodies (including Entry methods) counted separately. Every row names a test; partial tests cover the implemented boundary only.',
'Native:76 repeated JSON rows for exact original hash/layout bodies. Machines/Sender integration: own tests + source reading; no live Telegram server or fake Session.',
'Owner dispatch: release mutable borrow, apply notification/control, then drain nested continuation effects depth first with current virtual time. Deferred work uses has_deferred_work/advance_to; Session bootstrap29 owns runtime timers.','',
'| Original | Function | Status/future package | Rust | Test/evidence |','|---|---|---|---|---|']
counts={'done':0,'partial':0}
for stem,ext,line,name,m in rows:
 table=D if stem=='download_manager_mtproto' else F
 assert m in table,(stem,line,name)
 rust,test,status,note=table[m]
 # Manager/Queue overloads of enqueue/remove and task removeSession have distinct implementations.
 if stem=='download_manager_mtproto' and m in ['enqueue','remove'] and ((ext=='cpp' and line>=138) or (ext=='h' and line<65)):
  rust='DownloadManagerMtproto::'+m;test=W
 if stem=='download_manager_mtproto' and m=='removeSession' and ((ext=='cpp' and line==106) or (ext=='h' and line<100)):rust='DownloadManagerMtproto::remove_session (queued task traversal)'
 if stem=='file_upload' and m=='nonPremiumDelays':test=E
 if stem=='download_manager_mtproto' and m=='resetGeneration' and ((ext=='h' and line==112) or (ext=='cpp' and line==155)):
  rust='DownloadManagerMtproto::advance_to';test=W
 module='transfer_transport' if rust.startswith(('transfer_transport::', 'TransferNotices')) else stem
 counts['partial' if status.startswith('partial') else 'done']+=1
 lines.append(f'| `storage/{stem}.{ext}:{line}` | `{name}` | {status} | [{rust}](../src/storage/{module}.rs)'+(f' — {note}' if note else '')+f' | [{test}](../tests/storage_transfer.rs) |')
for line,name,rust in [(40,'GenerateInputPeer','input_peer'),(215,'StorageFileLocation::tl','storage_input')]:
 lines.append(f'| `ui/image/image_location.cpp:{line}` | `{name}` | done | [{rust}](../src/storage/transfer_transport.rs) | [transport_location_and_web_reply_variants](../tests/storage_transfer.rs) |')
 counts['done']+=1
lines.insert(6,f'Rows: **{len(rows)+2}** = **{counts["done"]} done**, **{counts["partial"]} explicit partial**, **0 pending**.\n')
Path('crates/tg-core/docs/storage-transfer-map.md').write_text('\n'.join(lines)+'\n')
print(len(rows)+2,counts)
