"""Every requested declaration/body; required attach values and MD5 dependency slices."""
from pathlib import Path
import re,sys
root=Path(sys.argv[1]);rows=[]
files=['storage/'+s+'.'+e for s in ['localimageloader','storage_media_prepare'] for e in ['h','cpp']]+['ui/chat/attach/'+s+'.'+e for s in ['attach_prepare','attach_send_files_way'] for e in ['h','cpp']]
ctors={'Task','TaskQueue','TaskQueueWorker','SendingAlbum','Item','FileLoadTo','FilePrepareResult','FileLoadTask','PreparedFile','PreparedList','SendFilesWay'}
for file in files:
 ext=file.rsplit('.',1)[1]
 for num,line in enumerate((root/file).read_text().splitlines(),1):
  s=line.strip()
  if not s or '(' not in s or s.startswith(('/','#',':',',',')','return ','if ','while','for ','using ','const auto ','Fn<','switch','case ','_')):continue
  if ext=='cpp' and line[0].isspace():continue
  head=s.split('(')[0].strip()
  if '=' in head and 'operator=' not in head:continue
  m=re.search(r'(operator(?: bool|[<>=!]+)|[\w:~]+)$',head)
  if not m:continue
  name=m[1];member=name.split('::')[-1]
  if head==name and '::' not in name and member.lstrip('~') not in ctors:continue
  if member in ['Unexpected','Expects','Ensures','Assert']:continue
  rows.append((file,num,name,member))
N='native_original_hash_parts_and_pure_rules';Q='task_queue_fifo_cancel_in_process_finish_reentrancy_and_virtual_stop';L='file_load_work_runs_process_on_worker_and_finish_on_main';A='archive_preparation_real_writer_upload_bridge_and_temp_lease';F='filesystem_validation_errors_callbacks_and_deferred_eleven';D='drag_facts_local_forward_size_gif_and_editor_rules';G='group_ranges_all_kinds_slowmode_gifs_and_none_runs';P='prepared_list_options_and_reorder_drop_deferred_fields';I='process_image_photo_sticker_animation_and_missing_backend_boundaries';M='media_metadata_types_dimensions_and_preview_boundary';B='process_generic_voice_song_video_and_forced_file_attributes';S='sending_album_music_take_media_and_caption_transfer_contract';T='thumbnail_rounding_jpeg_threshold_and_document_layout';V='mime_probe_order_and_exact_mime_vs_extension';E='process_directory_empty_limits_and_pixel_requirements';W='send_way_serialization_and_compressed_sticker_dependencies'
local={};media={};attach={};way={}
def add(table,names,rust,test,status='done',note=''):
 for n in names.split('|'):table[n]=(rust,test,status,note)
add(local,'PhotoSideLimit','photo_side_limit',T)
add(local,'Task|~Task|id','Task / TaskId / FileLoadWork',L)
add(local,'TaskQueue|addTask|addTasks|cancelTask|onTaskProcessed|stop|~TaskQueue','TaskQueue::{new,add_task,add_tasks,cancel_task,on_task_processed,stop}',Q)
add(local,'taskAdded|taskProcessed|wakeThread|TaskQueueWorker|onTaskAdded','TaskQueue::bind_main_queue + run_worker',Q,'partial(5 Qt nested event processing;29 timer bootstrap)','actual persistent tg-runtime worker/MainQueue and cancellation complete; QCoreApplication::processEvents is not synthesized')
add(local,'Item|SendingAlbum|preparedMusicBatching|preparedMusicReady|preparedMusicSample|takePreparedMusic|removeTask','SendingAlbum / AlbumItem + music methods',S)
add(local,'fillMedia|refreshMediaCaption|removeItem|FindAlbumItem|PrepareAlbumItemMedia','SendingAlbum::{fill_media,refresh_media_caption,remove_item}',S,'partial(01/18–20/26 caption/History)','real message keys and InputSingleMedia accepted; original trim + AyuGram reverseLocalPremiumEmoji/EntitiesToMTP and caption transfer need real History/lang')
add(local,'FileLoadTo','FileLoadArgs/descriptor owner boundary',B,'partial(26/29 SendOptions;18 FullReplyTo)','no fake options/caption/session transport envelope')
add(local,'FilePrepareResult|MakePreparedFile|~FilePrepareResult','FilePrepareResult::{new,drop} + Arc',A,'partial(5 decoded pixel slots;01/18 caption metadata)','encoded/scalar/TL fields real; absent QImage/TextWithTags fields not faked')
add(local,'setFileData|setThumbData','FilePrepareResult::{set_file_data,set_thumb_data}',N)
add(local,'FileLoadTask|~FileLoadTask','FileLoadTask::new + FileLoadArgs',B,'partial(29 weak Session/DC/filename policy;5/7 editor jobs)','caller supplies actual IDs/time/MIME/name/codec facts; no dummy Session/encoder')
add(local,'fileid|peekResult','FileLoadTask::{fileid,peek_result}',B)
add(local,'ReadMediaInformation|readMediaInformation|CheckForSong|CheckForVideo|CheckForImage|FillImageInformation','probe_plan / accept_song_facts / accept_video_facts / fill_image_information',V,'partial(5 image/Lottie decode;7 audio/video codecs;33 MIME sniff)','complete pure gates/normalization/metadata boundary; no actual decoder exists in workspace')
add(local,'CheckMimeOrExtensions|IsVideoFile','check_mime_or_extensions / is_video_file',V)
add(local,'process','FileLoadTask::process + FileLoadWork',I,'partial(5 pixels/theme/editor;7 video encoding;33 MIME/name services)','actual files/voice/attributes/photo encoded outputs/archive/uploader bridge complete; real codecs/jobs required explicitly via Needs')
add(local,'finish|removeFromAlbum','FileLoadTask::finish + SendingAlbum::{finish_music_item,remove_task}',S,'partial(29 real weak Session;26 SendConfirmedFile;5 errors)','size/session/music decisions complete; actual UI/API side effects remain owners')
add(local,'PrepareFileThumbnail|FinalizeFileThumbnail|PreparedFileThumbnail','thumbnail_dimensions / EncodedThumbnail / process',T,'partial(5 scaling/JPEG/WebP pixels)','metadata, quality95 and upload threshold complete; no QImage stand-in')
add(local,'FileThumbnailUploadRequired|Mp4FileName','file_thumbnail_upload_required / mp4_file_name',N)
add(local,'ComputePhotoJpegBytes','photo_jpeg_plan',T,'partial(5 progressive JPEG conversion/encoding)','exact 4bpp decision retained; actual JPEG writer/manipulation unavailable')
add(local,'ExtractStickersFromScene','FilePrepareResult::attached_stickers / PreparedUpload',I,'partial(5 Editor Scene;04 document resolution)','actual scene iteration/DocumentData::mtpInput is not replaced by fake scene objects')
add(media,'ValidPhotoForAlbum|ValidVideoForAlbum|PrepareShownDimensions','valid_photo_for_album / valid_video_for_album / prepare_shown_dimensions',M)
add(media,'PrepareDetailsInParallel','PreparedFile::prepare_details + actual TaskQueue worker',L,'partial(5 image inspection;7 codecs;29 main-thread settings snapshot)','no fake PrepareDetails completion without real inspection')
add(media,'ValidatePhotoEditorMediaDragData|ValidateEditMediaDragData|ComputeMimeDataState','validate_photo_editor_media_drag_data / validate_edit_media_drag_data / compute_mime_data_state',D,'partial(5 QMimeData;33 real URL/MIME services)','all deterministic branches accept actual drag/registry/readability facts')
add(media,'ReadPhotoEditorMedia|PhotoEditorMedia|video|operator bool','photo_editor_read_allowed + MediaFacts',D,'partial(5 actual image;7 frame read)','read64MiB gates complete; no fake pixel-bearing PhotoEditorMedia')
add(media,'PrepareMediaList','prepare_media_list / prepare_media_urls',F,'partial(5/7 actual parallel inspection;33 URL display/path adapters)','real filesystem/error/callback/10+deferred validation complete; no invented too-many-files rejection')
add(media,'PreparedFileFromFilesDialog','prepare_media_list validation boundary',F,'partial(5 file dialog/QImage;01 translated errors)','actual dialog/remote-image/phrase callbacks not invented')
add(media,'PrepareMediaFromImage','PreparedFile data + PreparedFile::prepare_details',M,'partial(5 actual QImage/decoder)','source non-null image precondition retained as future boundary')
add(media,'PrepareDetails|ComputeVideoDetails|ApplyVideoDetails|UpdateVideoDetails|UpdateImageDetails','PreparedFile::prepare_details + prepare_shown_dimensions',M,'partial(5 pixel/editor/blur/style)','pure metadata/rules complete; real modified dimensions/preview need UI/editor')
add(media,'ApplyModifications','PreparedFile/MediaFacts + FileLoadTask::process requirements',I,'partial(5 Editor scene;7 compose/encode)','no fabricated modifications or animated-job execution')
add(attach,'PreparedFile|~PreparedFile|operator=','PreparedFile::new + Rust move/drop',M,'partial(5 pixel/Editor/caption slots)','real scalar fields complete; missing UI ownership not faked')
add(attach,'PreparedList|Reordered|mergeToEnd','PreparedList::{error,reordered,merge_to_end}',P)
add(attach,'GroupTypeForFile|GroupRanges|size|DivideByGroups','group_type_for_file / group_ranges / divide_by_groups',G)
add(attach,'CanBeInAlbumType|canBeInAlbumType|albumType|MaxAlbumItems','can_be_in_album_type / PreparedFile::album_type / MAX_ALBUM_ITEMS',G)
add(attach,'isSticker|isVideoFile|isGifv|canUseHighQualityPhoto|sendsVideoAsGif|canEditVideo|videoQuality','PreparedFile predicates',M)
add(attach,'hasAnimatedEditScene','PreparedFile::has_animated_edit_scene',M,'partial(5 actual Scene snapshot)','pure snapshot predicate complete; live scene remains Editor owner')
add(attach,'canAddCaption|canMoveCaption|canChangePrice|canBeSentInSlowmode|canBeSentInSlowmodeWith|hasGroupOption|hasSendImagesAsPhotosOption|canHaveEditorHintLabel|hasSticker|hasSpoilerMenu|hasSendLargePhotosOption|InsertTextOnImageCancel','PreparedList predicates / insert_text_on_image_cancel',P)
add(attach,'PrepareFilesBundle|MakeSingleFileBundle','prepare_files_bundle / make_single_file_bundle',G)
add(attach,'ValidateThumbDimensions','validate_thumb_dimensions',N)
add(attach,'MediaBadgeImage|HighQualityBadgeImage|AnimatedBadgeImage|PaintMediaBadge|PrepareSongCoverForThumbnail|BlurredPreviewFromPixmap|PaintHighQualityBadge|PaintAnimatedBadge|PaintMediaTtlBadge|PaintVideoQualityBadge','pure geometry boundary only',T,'partial(5 actual painter/pixmap/style)','no UI rendering implementation or UI test claimed')
add(way,'is_flag_type|groupFiles|sendImagesAsPhotos|sendLargePhotos|setGroupFiles|setSendImagesAsPhotos|setSendLargePhotos|setHasCompressedStickers|hasCompressedStickers|serialize|FromSerialized|operator<|operator>|operator<=|operator>=|operator==|operator!=','SendFilesWay flags/serialization/Rust comparisons',W)
lines=['# Storage prepare function map','','Read-only original `0f2a31bb420de98f331990432265b131b8d836b5`.',
'Four requested files2336LOC; required headless attach_prepare/SendFilesWay and original MD5 slices are included. No tg-ui/tg-runtime/tg-mtproto code changed.',
'Header declarations and bodies counted separately. Partial tests prove implemented data boundaries only; pixel/editor/codec/UI/Session behavior remains explicitly unverified.',
'Native160 exact repeated rows: full original HashMd5 + fixed32KiB setFileData/setThumbData and pure rules. Existing storage-transfer native13 layout cases reused for document part_layout.', '',
'| Original | Function | Status/future package | Rust | Test/evidence |','|---|---|---|---|---|']
counts={'done':0,'partial':0}
for file,num,name,m in rows:
 if 'localimageloader' in file:table=local;module='localimageloader'
 elif 'storage_media_prepare' in file:table=media;module='storage_media_prepare'
 elif 'send_files_way' in file:table=way;module='storage_media_prepare'
 else:table=attach;module='storage_media_prepare'
 assert m in table,(file,num,name)
 rust,test,status,note=table[m];counts['partial' if status.startswith('partial') else 'done']+=1
 lines.append(f'| `{file}:{num}` | `{name}` | {status} | [{rust}](../src/storage/{module}.rs)'+(f' — {note}' if note else '')+f' | [{test}](../tests/storage_prepare.rs) |')
for n,name in [(167,'HashMd5::HashMd5'),(172,'HashMd5::feed'),(202,'HashMd5::result'),(207,'HashMd5::init'),(217,'HashMd5::finalize'),(232,'HashMd5::transform'),(310,'hashMd5'),(317,'hashMd5Hex')]:
 lines.append(f'| `core/utils.cpp:{n}` | `{name}` | done | [md5::Md5::{{update,digest,finalize}}; md5_hex](../src/storage/localimageloader.rs) | [{N}](../tests/storage_prepare.rs) |');counts['done']+=1
for n,name in [(73,'HashMd5'),(74,'HashMd5::feed'),(75,'HashMd5::result'),(79,'HashMd5::init'),(80,'HashMd5::finalize'),(81,'HashMd5::transform'),(105,'hashMd5 declaration'),(106,'hashMd5 inline'),(112,'hashMd5Hex declaration'),(113,'hashMd5Hex inline'),(116,'hashMd5Hex array')]:
 lines.append(f'| `core/utils.h:{n}` | `{name}` | done | [md5::Md5; md5_hex](../src/storage/localimageloader.rs) | [{N}](../tests/storage_prepare.rs) |');counts['done']+=1
lines.insert(6,f'Rows: **{sum(counts.values())}** = **{counts["done"]} done**, **{counts["partial"]} explicit partial**, **0 pending**.\n')
Path('crates/tg-core/docs/storage-prepare-map.md').write_text('\n'.join(lines)+'\n');print(sum(counts.values()),counts)
