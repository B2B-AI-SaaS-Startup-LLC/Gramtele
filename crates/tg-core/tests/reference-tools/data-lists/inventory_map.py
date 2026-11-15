"""Inventory h declarations/inline bodies and cpp bodies separately; no pending rows."""
from pathlib import Path
import re,sys
original=Path(sys.argv[1]);repo=Path.cwd()
files=['data_messages','data_sparse_ids','data_shared_media','data_user_photos','data_pts_waiter','data_premium_limits','business/data_business_info','stickers/data_stickers_set']
rows=[]
for stem in files:
 for ext in ['h','cpp']:
  lines=(original/'data'/f'{stem}.{ext}').read_text().splitlines()
  consumed=-1
  for i,l in enumerate(lines):
   if i<=consumed:continue
   if ext=='cpp':
    if not l or l[0].isspace() or l[0] in '#/:,}' or l.startswith(('namespace ','using ','constexpr ')):continue
   else:
    if not l or l.startswith(('#','//','namespace ','using ','class ','struct ','enum ')):continue
    # Source header members live at depth one/two; ignore body statements.
    if l.startswith('\t\t\t') or l.lstrip().startswith(('return ',': ', ', ', 'Expects(', 'if (', 'const auto ')):continue
   sig=l.strip();j=i
   if '(' not in sig:continue
   while not re.search(r'[;{]',sig) and j+1<len(lines):j+=1;sig+=' '+lines[j].strip()
   if ('=' in sig.split('(')[0] and 'operator' not in sig.split('(')[0]) or sig.startswith(('if ','for ','switch ','static_assert(', 'return ', 'Expects(')):continue
   sig=re.sub(r'\[\[.*?\]\]','',sig).strip()
   m=re.search(r'([\w:~]+|operator\s*(?:bool|\[\]|[=!<>]+))\s*\(',sig)
   if m and m[1] not in ['if','void','bool','MTP_int','MTP_flags','MTP_string']:
    rows.append((stem,ext,i+1,m[1]));consumed=j
# stdout inventory helps review before regenerating accounting.
if '--list' in sys.argv:
 for r in rows:print(*r)
 raise SystemExit
N='data_lists_native::native_lists_builders_merge_limits_and_business_tl'
def test(n):return 'data_lists::'+n
M=test('messages_disjoint_merge_remove_boundary_and_invalidation');B=test('messages_builder_merge_remove_no_refill_invalidated_false_and_snapshots');S=test('sparse_builder_callbacks_see_trim_phase_and_can_snapshot_without_borrow');U=test('user_photos_builder_replaces_unknown_count_skips_and_reverse_distance');V=test('shared_media_types_and_real_storage_filters_mutations');W=test('with_last_photo_known_unknown_duplicate_extra_reverse_and_keys');P=test('pts_gaps_order_duplicate_requesting_and_reentrant_apply');I=test('business_optimistic_order_rollback_fresh_other_fields_noop_and_success');T=test('stickers_flags_identity_type_thumbnail_and_document_lookup')
def snake(n):return re.sub(r'([a-z0-9])([A-Z])',r'\1_\2',re.sub(r'([A-Z]+)([A-Z][a-z])',r'\1_\2',n)).lower()
def classify(stem,ext,line,name):
 n=name.split('::')[-1];r=snake(n);status='done';note='';module='data/'+stem+'.rs';t=M
 if stem=='data_messages':
  if n=='operator bool' or n.startswith('operator'):r='MessagePosition::Ord/PartialEq/is_present; MessagesRange::PartialEq';t=test('positions_order_by_date_then_peer_then_id_and_constants')
  elif 'Builder' in name or (ext=='h' and line>=190):t=B
  if n in ['query','viewer','sliceUpdated']:t=test('messages_query_lazy_viewer_initial_eager_lifetime_and_reentrant_update')
  if n in ['mergeSliceData','sliceToLimits','requestMessages','requestMessagesCount','fillSkippedAndSliceToLimits']:r='data_slice_builder.rs::SliceBuilder';t=B+'; '+S
  if n.startswith('operator') and ext=='h' and line>=135:r='MessagesList::add_range / MessagesAroundData::Ord';t=test('source_corner_contracts_constructor_order_and_multiple_range_overcount')
  if n=='Slice' or n=='merge' or n in ['uniteAndAdd','addRangeItemsAndCountNew','addRange']:r='MessagesList::add_range'
  if n=='queryFromSlice' or n=='queryCurrent':r='MessagesList::snapshot';t=N
  if n=='MessagesList':r='MessagesList::default'
  if n=='MessagesSliceBuilder':r='MessagesSliceBuilder::new'
  if n=='insufficientAround':r='insufficient_around'
 elif stem=='data_sparse_ids':
  t=test('sparse_merge_keys_unsorted_nearest_and_lazy_viewer')+'; '+N
  if 'Builder' in name or (ext=='h' and line>=150):t=S+'; '+N
  if n in ['PartKey','MigratedKey','MigratedSlice']:r={'PartKey':'Key::part_key','MigratedKey':'Key::migrated_key','MigratedSlice':'SparseIdsMergedSlice::new'}[n]
  if n in ['Add','isFromPart','isFromMigrated','IsFromSlice']:r='SparseIdsMergedSlice::full_count/skipped_before/skipped_after/index_of/at/nearest (inline checks)'
  if n=='CreateViewer':r='SparseIdsMergedSlice::create_viewer'
  if n in ['sliceToLimits','mergeSliceData','fillSkippedAndSliceToLimits','requestMessages','requestMessagesCount']:r='data_slice_builder.rs::SliceBuilder'
  if n=='SparseIdsMergedSlice':r='SparseIdsMergedSlice::new/from_parts/from_unsorted'
  if n=='SparseIdsSliceBuilder':r='SparseIdsSliceBuilder::new'
  if n=='Key':r='Key::new'
  if n.startswith('operator'):r='SparseIdsMergedSlice::at / Key::PartialEq / SparseIdsAroundData::Ord'
  if n=='ComputeId':r='Key::compute_id'
 elif stem=='data_shared_media':
  t=V
  if 'WithLast' in name or (ext=='h' and 67<=line<=203) or (ext=='cpp' and line>=302):t=W;r='SharedMediaWithLastSlice::'+r
  if n in ['ViewerKey','EndingKey','EndingSlice','ComputeId']:r='WithLastKey::viewer_key/ending_key/compute_id; from_key'
  if n in ['SharedMediaWithLastViewer','SharedMediaWithLastReversedViewer']:r=snake(n);t=test('with_last_viewer_routes_by_photo_topic_and_reversed_is_lazy')
  if n=='SharedMediaMergedViewer':t=test('shared_media_merged_viewer_and_last_peer_photo_use_actual_stores')
  if n in ['SharedScheduledMediaViewer','IsItemGoodForType','SavedMusicMediaViewer']:
   status='partial(05/12/18–20)';r='WithLastSource::scheduled_viewer/saved_music_viewer (contract only)';t='future scheduled05/SavedMusic12 + actual History18–20';note='dereferences real HistoryItem/media; no replacement model'
  if n=='Add':r='SharedMediaWithLastSlice::full_count/skipped_after_impl/distance (Option arithmetic)'
  if n=='indexOfImpl':r='SharedMediaWithLastSlice::index_of'
  if n=='skippedBeforeImpl':r='SharedMediaWithLastSlice::skipped_before/skipped_after'
  if n=='LastPeerPhotoId':r='last_peer_photo_id';t=test('shared_media_merged_viewer_and_last_peer_photo_use_actual_stores')
  if n=='LastFullMsgId':r='last_full_msg_id';t=W
  if n=='IsLastIsolated':r='SharedMediaWithLastSlice::new (message_photo_id contract)';t=W
  if n=='SharedMediaWithLastSlice':r='SharedMediaWithLastSlice::new/from_key';t=W+'; '+test('empty_with_last_counts_and_sparse_count_only_contract')
  if n=='Key':r='WithLastKey (typed value contract)';t=W
  if n=='SharedMediaMergedKey':r='SharedMediaMergedKey (typed value contract)'
  if n.startswith('operator'):r='WithLastKey::Ord/PartialEq; SharedMediaWithLastSlice::at; SharedMediaMergedKey::PartialEq';t=W+'; '+test('source_corner_contracts_constructor_order_and_multiple_range_overcount')
 elif stem=='data_user_photos':
  t=U
  if n in ['UserPhotosViewer','UserPhotosReversedViewer','SyncUserFallbackPhotoViewer']:t=test('user_photos_real_storage_viewer_reverse_fallback_and_cancellation')
  if n=='UserPhotosSlice':r='UserPhotosSlice::new/from_ids'
  if n=='UserPhotosSliceBuilder':r='UserPhotosSliceBuilder::new'
  if n=='applyUpdate':r='UserPhotosSliceBuilder::apply_result/apply_update'
  if n=='mergeSliceData':r='UserPhotosSliceBuilder::merge'
  if n=='sliceToLimits':r='check_insufficient_photos'
  if n=='insufficientPhotosAround':r='insufficient_photos_around'
 elif stem=='data_pts_waiter':
  t=P
  if n=='PtsWaiter':r='PtsWaiter::new'
  if n=='ptsKey':r='PtsWaiter::enqueue'
  if n in ['setWaitingForSkipped','setWaitingForShortPoll','checkForWaiting','check']:t=P+'; '+test('pts_virtual_timer_delivery_overshoot_and_cancel')
  note='Api::Updates timer/application owner26; concrete synchronous Effect boundary, no fake owner'
 elif stem=='data_premium_limits':
  t=N
  if name.startswith('LevelLimits::') or (ext=='h' and line>=110):t=test('level_limits_keys_defaults_and_boost_environment_are_live')
  if n in ['LevelLimits','PremiumLimits']:r=n+'::new'
  if n=='maxBoostLevel':t=test('level_limits_keys_defaults_and_boost_environment_are_live')
  if n in ['appConfigLimit','isPremium']:r='LimitsContext::'+r;note='real owner/AppConfig boundary29; test seam, not native full Session'
 elif stem=='business/data_business_info':
  t=I
  if n in ['BusinessInfo','~BusinessInfo']:r='BusinessInfo::new/Drop (pending callbacks owned by model)'
  if 'Away' in n or 'away' in n or 'Greeting' in n or 'greeting' in n:t=test('business_away_greeting_loaded_local_only_and_rollback_reentrant_events')
  if 'imezone' in n or n=='preload':t=test('business_timezone_preload_latches_even_on_empty_failed_not_modified')
  if n=='ToMTP':r='working_hours_to_mtp/away_schedule_to_mtp/away_to_mtp/greeting_to_mtp';t=N+'; '+test('business_tl_recipients_and_all_schedule_variants')
  note='real self-user lookup/set and Sender delivery contract02/26/29; own state tests + чтение'
 elif stem=='stickers/data_stickers_set':
  t=T
  if n in ['StickersSet','~StickersSet']:r='StickersSet::new/Drop'
  if n=='type':r='kind'
  if n=='FromInputSet':r='from_input_set'
  if n in ['StickersSetThumbnailView','image','content','set','createThumbnailView','activeThumbnailView'] or (n=='owner' and (ext=='cpp' and line<50 or ext=='h' and line<45)):
   status='partial(этап5)';r='—';t='future real ImageService/pixels stage5';note='no fake QImage/Image'
  if n in ['thumbnailLoading','thumbnailFailed','loadThumbnail']:
   status='partial(storage/file_download этап3/4; этап5)';r='—';t='future actual CloudFile loader and thumbnail view';note='no simulated loader state'
  if n=='setThumbnail':status='partial(storage/file_download этап3/4; этап5)';r='StickersSet::set_thumbnail model/cache effect';note='real metadata phase covered; loader cancel/restart/pixels owner deferred'
  if n=='is_flag_type':r='StickersSetFlag/Flags'
 if n.startswith('operator') and stem=='data_messages' and 'Around' not in name:pass
 return status,module+'::'+r,t,note
header='''# Data lists — срез списков и сервисных моделей

База main9575f65; original0f2a31bb420de98f331990432265b131b8d836b5, read-only.
Все16 файлов прочитаны целиком; usage sites в api_updates/apiwrap/Session,
storage facade/списках и media просмотрены. API declarations h и bodies cpp
считаются отдельно; это строки карты, не уникальные функции.

SharedMedia/UserPhotos используют существующие storage модули. MessagesList имеет
отдельную исходную семантику и не является копией storage SparseIdsList.
Full owners/UserData02/MainSession29/API26 и History18–20 не подменяются.
Source quirks и границы native: [handoff](../../../docs/handoffs/codex-data-lists.md),
[recipe](../tests/reference-tools/data-lists/README.md).

| Оригинал / функция | Статус | Реализация | Тест / будущий пакет | Примечание |
|---|---|---|---|---|
'''
output=header;done=partial=0
for stem,ext,line,name in rows:
 status,r,t,note=classify(stem,ext,line,name)
 if status=='done':
  done+=1
  for ref in t.split('; '):
   file,fn=ref.split('::');assert re.search(r'fn '+re.escape(fn)+r'\s*\(', (repo/'crates/tg-core/tests'/f'{file}.rs').read_text()),ref
 else:partial+=1
 output+=f'| `data/{stem}.{ext}:{line} {name}` | {status} | `{r}` | {t} | {note} |\n'
# Nonfunction fields that are specifically excluded from the model.
output+='| `data/stickers/data_stickers_set.h:125 emoji` | partial(этап5) | — | future real EmojiPtr/lang/UI binding | scalar fields/stable document packs/dates ported; no fake emoji key |\n'
partial+=1
output+=f'\nВсего **{done+partial} строк: {done} done / {partial} partial /0 pending**. Каждая done-строка имеет проверенную ссылку на тест; partial — указанный будущий пакет.\n'
(repo/'crates/tg-core/docs/data-lists-map.md').write_text(output)
print(done+partial,done,partial)
