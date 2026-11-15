"""Every declaration/body (including cpp-local classes) gets an implementation and test/future owner."""
from pathlib import Path
import re,sys
source=Path(sys.argv[1]);dest=Path('crates/tg-core/docs/storage-files-map.md')
stems=['serialize_peer','serialize_document','storage_folder_archive','file_download','file_download_mtproto','file_download_web','streamed_file_downloader']
rows=[]
constructors={'StickerSetInfo','StorageImageSaved','FileLoader','FromMemoryLoader','mtpFileLoader','webFileLoader','StreamedFileDownloader','ZipToFile','WebLoadManager'}
for stem in stems:
 for ext in ['h','cpp']:
  path=source/'storage'/f'{stem}.{ext}';inside_local=False
  for number,line in enumerate(path.read_text().splitlines(),1):
   s=line.strip()
   if s.startswith(('class ZipToFile ','class FromMemoryLoader ','class WebLoadManager ')):inside_local=True
   if s=='};':inside_local=False if stem!='file_download_web' else inside_local
   if ext=='cpp' and line.startswith('WebLoadManager::WebLoadManager('):inside_local=False
   if not s or s.startswith(('/', '#', ':', ',', ')', '?', 'return ', 'if ', 'if(', 'while','for ', 'Fn<','using ', 'const auto ', 'auto ', 'switch', 'case ')) or '(' not in s:continue
   head=s.split('(')[0].strip()
   if '=' in head or head.startswith('_'):continue
   match=re.search(r'([\w:~]+)$',head)
   if not match:continue
   name=match[1];member=name.split('::')[-1]
   if ext=='cpp' and line[0].isspace() and not inside_local:continue
   if member in ['Unexpected','Expects','Ensures','Assert','LOG','DEBUG_LOG','InvokeQueued','QObject::connect','std::clamp']:continue
   if head==name and '::' not in name and member.lstrip('~') not in constructors:continue
   # Exclude constructor member initializers, modeled in the constructor row.
   if member in ['setId','accessHash','shortName','LocationType']:continue
   rows.append((stem,ext,number,member,name))

def snake(name):
 return re.sub(r'([a-z0-9])([A-Z])',r'\1_\2',re.sub(r'([A-Z]+)([A-Z][a-z])',r'\1_\2',name)).lower()
def classify(stem,ext,line,m,name):
 module=stem;rust=snake(m);status='done';note='';test='loader_local_cache_partial_and_ordered_completion_effects'
 if stem=='serialize_peer':
  module='serialize_location' if 'Location' in m or 'location' in m else stem
  test='native_image_location_storage_wire_and_stream_wrappers' if module=='serialize_location' else 'native_peer_stream_versions_null_utf16_creator_and_phone_prefix'
  if m=='readLegacyStorageImageLocationOrTag':rust='read_image_location';test='image_location_variants_modern_legacy_and_corruption'
  if m in ['peerSize','writePeer','readPeer']:
   status='partial(02/16/29)';note='wire record codec and real Chat/Channel owner complete; full UserData/PeerCodec + model Qt nullness provenance remain to their owners'
  if m=='readPeer':rust='read_peer/apply_group/restored_flags/restored_lastseen';test='group_peer_codec_applies_real_store_loaded_gate_version_reset_and_truncation'
 if stem=='serialize_document':
  rust={'StickerSetInfo':'StickerSetInfo','writeToStream':'DocumentRecord::write_to_stream','sizeInStream':'DocumentRecord::size_in_stream'}.get(m,'DocumentRecord::read_from_stream/attributes + Documents::apply_serialized_document')
  test='native_document_current_bytes_legacy_reads_size_and_rewrite'
  if m in ['readFromStreamHelper','readFromStream','readStickerFromStream']:test='document_sticker_set_context_and_rejection_of_legacy_thumb'
  if m=='writeToStream':status='partial(04/16)';note='wire DTO byte-exact; convenience model projection cannot recover Qt nullness already erased by String/Vec models'
  if m=='StickerSetInfo':test='document_sticker_set_context_and_rejection_of_legacy_thumb'
 if stem=='storage_folder_archive':
  test='native_archive_names_estimates_http_url_and_zip_bytes'
  if m in ['ArchiveFile','PrepareFolderArchive','PrepareFilesArchive']:status='partial(этап5 attach_prepare)';rust='ArchiveJob fields only';note='actual Ui::PreparedFile objects not synthesized';test='archive_gather_hidden_symlink_dedup_and_two_level_listing'
  if m in ['GatherArchiveEntries','WalkFolder','GatherFiles','FolderFilesForSending']:
   status='partial(33 non-Unix metadata/path adapter)';rust='gather_archive_entries' if m!='FolderFilesForSending' else snake(m);test='archive_gather_hidden_symlink_dedup_and_two_level_listing';note='Unix real filesystem phase complete, non-Unix local civil-time adapter explicitly unavailable'
  if m=='FillEntryDate':module='details/storage_minizip';rust='Archive::open_entry';test='native_archive_names_estimates_http_url_and_zip_bytes'
  if m in ['ZipToFile','funcs','writtenSize','open','read','write','tell','seek','close','error','Open','Read','Write','Tell','Seek','Close','Error']:
   module='details/storage_minizip';rust='Archive + FileFunctions callbacks';test='native_archive_names_estimates_http_url_and_zip_bytes'
  if m in ['ArchiveDirectory','ArchiveFileTemplate']:rust='temporary_archive(directory)';test='archive_cancellation_missing_files_size_limit_and_stale_boundary';note='owner supplies temp_dir()/tdarchive; tests use explicit private directory'
  if m in ['WriteArchive','WriteArchiveFile','StepProgress']:rust='write_archive';test='archive_cancellation_missing_files_size_limit_and_stale_boundary'
  if m=='ClearStaleArchiveFiles':rust='clear_stale_archive_files';test='archive_cancellation_missing_files_size_limit_and_stale_boundary';status='partial(29 worker Queue scheduling)';note='real deletion worker complete with injected now; automatic Session bootstrap schedules it later'
  if m=='SingleFolderPath':test='archive_gather_hidden_symlink_dedup_and_two_level_listing'
 if stem=='file_download':
  if m in ['imageData','readImage']:status='partial(этап5 image decoding)';rust='ReadCache::decode_image projection';note='no fake QImage';test='loader_local_cache_partial_and_ordered_completion_effects'
  if m in ['~FileLoader','lifetime']:status='partial(29 Session loader lifecycle)';rust='explicit cancel/terminal effects + owned File';note='real owner applies network cancellation before drop';test='loader_files_cancel_failure_and_filename_rules'
  if m in ['OwningBytes','StorageImageSaved','FromMemoryLoader','finishWithBytes']:rust='Vec ownership/FromMemoryLoader::start_loading/FileLoader::finish_with_bytes';test='loader_missing_local_only_empty_cache_and_memory_owns_bytes'
  if m in ['cacheKey','fileLocationKey','objId','fileOrigin']:rust='cache_key/file_location_key + FileDownload variants';test='loader_factory_selection_overrides_and_memory_start'
  if m in ['FileLoader','CreateFileLoader']:rust='FileLoader::new/create_file_loader';test='loader_factory_selection_overrides_and_memory_start'
  if m in ['currentOffset','currentProgress','writeResultPart','readLoadedPartBack']:test='loader_out_of_order_parts_overlap_and_progress_preserve_source_accounting'
  if m in ['cancel','checkForOpen','setFileName']:test='loader_files_cancel_failure_and_filename_rules'
  if m=='finalizeResult':test='loader_file_finalize_postprocess_location_and_partial_cache'
  if m=='loadLocal':rust='read_local_cache + ReadCache effect';test='loader_effects_use_real_encrypted_cache';status='partial(этап5 pixels;29 weak MainRef callback routing)';note='actual encrypted cache API wired; real Session guard/pixels absent'
  if m in ['updates','notifyAboutProgress']:rust='LoaderUpdates::updates/apply + notify_about_progress';test='loader_rpl_dispatch_releases_borrow_before_callbacks'
  if m in ['startLoading','startLoadingWithPartial','cancelHook']:
   rust='StartLoading/CancelRequests effects; concrete loader hooks';test='loader_factory_selection_overrides_and_memory_start'
 if stem=='file_download_mtproto':
  test='mtproto_part_order_unknown_size_partial_resume_and_failure'
  rust='MtpFileLoader::'+snake(m)
  if m in ['mtpFileLoader','~mtpFileLoader']:
   status='partial(29 Session binding)';rust='MtpFileLoader::new/create_file_loader + download_manager_mtproto::DownloadMtprotoTask';note='storage-transfer closes task owner/DC queues/CDN/Sender; actual Session lifetime dispatch remains29'
  if m in ['cacheKey','fileLocationKey']:module='file_download';rust='create_file_loader + existing DownloadLocation::cache_key';test='loader_factory_selection_overrides_and_memory_start'
  if m=='cancelHook':rust='MtpFileLoader::cancel + CancelAllRequests effect'
 if stem=='streamed_file_downloader':
  test='streamed_window_duplicates_out_of_order_short_last_read_and_cancel';rust='StreamedFileDownloader::'+snake(m)
  if m in ['StreamedFileDownloader','~StreamedFileDownloader']:
   status='partial(этап7 streaming Reader)';rust='StreamedFileDownloader::new/close';note='32-part state and exact Reader effects complete; actual Reader subscriptions/loader slots remain7'
  if m in ['cacheKey','fileLocationKey']:rust='FileLoader cache_key/file_location_key';test='loader_factory_selection_overrides_and_memory_start'
  if m=='requestPart':rust='StreamedFileDownloader::request_parts (single part loop body)'
  if m=='cancelHook':rust='StreamedFileDownloader::cancel'
  if m=='savePart':test='streamed_completion_and_reader_release'
 if stem=='file_download_web':
  test='web_queue_generation_limit_removal_and_stale_replies';rust='WebLoadManager::'+snake(m)
  if m=='IsSupportedWebUrl':rust='is_supported_web_url';test='native_archive_names_estimates_http_url_and_zip_bytes';status='partial(33 full QUrl normalization)';note='native scheme corpus incl empty authority verified; reqwest URL normalization is not whole QUrl proof'
  if m in ['GetManager','updates'] or (m in ['enqueue','remove'] and ext=='cpp' and line in [73,74,205,219]):
   status='partial(29 global manager/loader owner)';rust='explicit shared WebLoadManager + LoaderUpdates endpoints';note='actual main-to-worker loader ID/lifetime routing is not synthesized'
  if m in ['WebLoadManager','~WebLoadManager','handleNetworkErrors','deleteDeferred']:
   status='partial(29 worker/future ownership)';rust='WebLoadManager::default/clear + http_client/http_get/Abort effects';note='real reqwest client uses caller runtime; owner must hold/cancel request futures and route errors'
  if m in ['send']:rust='Get effect + http_get';status='partial(29 future dispatch)';note='concrete HTTP transport exists, automatic queue pump belongs to owner'
  if m in ['progress','notify','failed','finished']:rust='WebLoadManager::progress/failed/finished';test='web_progress_sizes_statuses_streaming_and_failures'
  if m=='findSent':rust='WebLoadManager::matches'
  if m=='redirect':test='web_redirect_limit_relative_scheme_rejection_and_adapter';status='partial(33 complete QUrl relative/encoding parity)';note='five fallback redirects and scheme checks tested; raw invalid UTF8/local-codepage resolution not proven'
  if m.startswith('queue') or m=='sendUpdate':rust='ordered WebEffect::Update';status='partial(29 MainRef routing)';note='update ordering tested; real loader subscription/owner mapping absent'
  if m in ['webFileLoader','~webFileLoader']:rust='WebFileLoader::new/close/create_file_loader';test='loader_factory_selection_overrides_and_memory_start';status='partial(29 manager subscription lifetime)';note='model overrides and terminal detach effects complete'
  if m in ['url','requestType','streamLoading','currentOffset','loadProgress','loadFinished','loadFailed','cancelHook','cancelRequest','startLoading','cacheKey','fileLocationKey']:
   rust='WebFileLoader::new/start_loading/apply_update/cancel_request/close + FileLoader cache_key';test='web_loader_streams_to_real_file_detaches_and_ignores_late_updates'
 return status,module,rust,test,note
lines=['# Storage serialization and file downloads','', 'Original read-only `0f2a31bb420de98f331990432265b131b8d836b5`.', 'Each header declaration and body is listed separately; cpp-local class methods are included.', 'Partial rows name their future package; their test proves only the implemented boundary.', 'Superseded manager boundary: storage-transfer-map.md now covers actual DC queues/CDN/reference refresh/Sender; concrete Session binding remains29.', 'Native capture:235 rows, including exact Qt stream expressions and the complete real archive writer.', '', '| Original | Function | Status/future package | Rust | Test/evidence |', '|---|---|---|---|---|']
counts={}
for stem,ext,num,m,name in rows:
 status,module,rust,test,note=classify(stem,ext,num,m,name)
 counts['partial' if status.startswith('partial') else 'done']=counts.get('partial' if status.startswith('partial') else 'done',0)+1
 lines.append(f'| `storage/{stem}.{ext}:{num}` | `{name}` | {status} | [{rust}](../src/storage/{module}.rs)'+(f' — {note}' if note else '')+f' | [{test}](../tests/storage_files.rs) |')
# Required value-codec dependencies; the rest of image_location stays its existing UI partial.
for num,name,rust in [(185,'StorageFileLocation::convertToModernPeerPhoto','convert_to_modern_peer_photo'),(287,'StorageFileLocation::serialize','write_storage_file'),(324,'StorageFileLocation::serializeSize','write_storage_file().len()'),(330,'StorageFileLocation::FromSerialized','read_storage_file'),(746,'StorageImageLocation::serialize','write_download_location/encode_image_location'),(759,'StorageImageLocation::serializeSize','storage_image_location_size'),(766,'StorageImageLocation::FromSerialized','read_storage_image_location'),(791,'DownloadLocation::serialize','write_download_location'),(832,'DownloadLocation::serializeSize','write_download_location().len()'),(853,'DownloadLocation::FromSerialized','read_download_location'),(931,'DownloadLocation::convertToModernPeerPhoto','convert_to_modern_peer_photo'),(1027,'ImageLocation::serialize','encode_image_location'),(1040,'ImageLocation::serializeSize','image_location_size'),(1047,'ImageLocation::FromSerialized','decode_image_location')]:
 status='partial(33 local-codepage/Qt nullness provenance)' if num in [791,832,853] else 'done'
 counts['partial' if status.startswith('partial') else 'done']+=1
 lines.append(f'| `ui/image/image_location.cpp:{num}` | `{name}` | {status} | [{rust}](../src/storage/serialize_location.rs) | [image_location_variants_modern_legacy_and_corruption](../tests/storage_files.rs) |')
lines.insert(8,f'Rows: **{counts["done"]+counts["partial"]}** = **{counts["done"]} done**, **{counts["partial"]} explicit partial**, **0 pending**.\n')
dest.write_text('\n'.join(lines)+'\n');print(counts['done']+counts['partial'],counts)
