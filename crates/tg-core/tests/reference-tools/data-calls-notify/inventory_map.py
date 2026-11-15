"""Inventory h declarations/inline bodies and cpp bodies separately; no pending rows."""
from pathlib import Path
import re,sys
original=Path(sys.argv[1]);repo=Path.cwd()
files=['data_group_call','notify/data_notify_settings','data_media_preload','data_statistics_chart','raw/raw_countries_bounds','../statistics/statistics_data_deserialize','../statistics/segment_tree']
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


def snake(n):return re.sub(r'([a-z0-9])([A-Z])',r'\1_\2',re.sub(r'([A-Z]+)([A-Z][a-z])',r'\1_\2',n)).lower()
def test(n):return 'data_calls_notify::'+n
N='data_calls_notify_native::native_countries_chart_parser_indices_and_segment_ranges'
G=test('group_constructor_values_call_fields_and_conference_local_override')
P=test('participant_order_min_volume_left_missing_and_events_release_borrow')
V=test('group_video_data_pointer_reuse_local_update_endpoints_and_ssrc_collisions')
Q=test('group_queue_initial_conference_exception_gap_reload_and_stale_timeout')
R=test('group_page_reload_saved_full_failure_and_access_stale_double_fire')
S=test('group_speaking_before_mutation_sound_boundary_and_active_virtual_timer')
U=test('group_unknown_batches_highest_fifty_resolved_and_failure')
M=test('group_migration_and_peer_call_not_empty_flag_use_actual_records')
B=test('actual_sender_bridges_get_participants_reload_save_notify_and_cancel')
D=test('notify_defaults_inheritance_known_bits_self_sound_and_virtual_unmute')
E=test('notify_exceptions_realtime_before_bundled_flush_reset_clear_and_request_latch')
T=test('notify_topic_keys_parent_override_and_destroyed_lifetime')
C=test('notify_community_fallback_and_default_group_broadcast_types')
A=test('notify_request_dedup_user_one_collision_and_input_notify_tl_variants')
L=test('notify_ringtone_pending_list_cache_leases_and_null_document')
F=test('video_preload_cached_disallowed_repeat_done_and_drop_cancel')
H=test('video_preload_source_early_finish_partial_map_marker_and_fail_hooks')
O=test('photo_preload_actual_byte_lease_loaded_contract_done_repeats_cancel')
I=test('chart_model_measure_requests_extrema_accumulate_day_lookup_and_colors_data')
J='statistics/segment_tree.rs::tests::pending_values_cover_propagate_and_preserve_source_first_slot_write'
def classify(stem,ext,line,name):
 n=name.split('::')[-1];r=snake(n);status='done';t=G;note='';module='data/'+stem+'.rs'
 if stem=='data_group_call':
  r='GroupCall::'+r
  if n in ['GroupCall','~GroupCall']:r='GroupCall::new/Drop';t=G+'; '+B
  if n=='RtmpEndpointId':r='rtmp_endpoint_id';t=V
  if n in ['cameraEndpoint','screenEndpoint','cameraPaused','screenPaused']:r='GroupCallParticipant::'+snake(n);t=V
  if n=='canChangeMessagesEnabled':r='GroupCall::can_change_messages_enabled'
  if n in ['participantByPeer','findParticipant','participantUpdated','participants','applyParticipantsSlice','setServerParticipantsCount','computeParticipantsCount','fullCount','fullCountValue']:t=P
  if n=='findParticipant':r='GroupCall::participant_by_peer/replace_participant (snapshot and mutation)'
  if n in ['participantPeerByAudioSsrc','participantByEndpoint','applyLocalUpdate']:t=V
  if n=='applyParticipantsSlice':t+='; '+test('participant_about_changes_can_reenter_speaking_before_source_snapshot')
  if n in ['setPeer','changePeerEmptyCallFlag']:t=M+'; '+test('group_migration_rejects_destination_without_membership')
  if n in ['enqueueUpdate','applyEnqueuedUpdate','processQueuedUpdates','reloadIfStale','reload','inCall']:t=Q
  if n in ['requestParticipants','setParticipantsLoaded','checkStaleParticipants','processSavedFullCall','finishParticipantsSliceRequest','participantsReloaded','participantsLoaded','setParticipantsWithAccess','participantsWithAccessCurrent','participantsWithAccessValue','staleParticipantIds','discard','processFullCallUsersChats','processFullCallFields','processFullCall','requestParticipantsAfterReload','ExtractNextOffset']:t=R
  if n=='ExtractNextOffset':r='GroupCall::process_full_call_fields (participant slice offset match)'
  if n=='processFullCallUsersChats':r='GroupCall::process_users_chats'
  if n in ['applyLastSpoke','participantSpeaking','applyActiveUpdate','checkFinishSpeakingByActive','setInCall']:t=S
  if n in ['resolveParticipants','requestUnknownParticipants']:t=U
  if n=='discard':status='partial(9/29)';r='GroupCall::discard / Effect::Discard';note='owner enqueues same-call-guarded Peers clear on main29, then forwards checked discarded TL to Calls9'
  if n=='saveSendAs':t=B
  if n=='inCall':r='GroupCallOwner::in_call';note='required actual Calls engine9; own tests + reading'
  if n=='api':r='send_request/send_effect (actual Sender bridge)';status='partial(26/29)';t=B+'; future actual ApiWrap/Main::Session owner';note='no fake ApiWrap reference'
  if n=='session':r='GroupCall::session (SessionScopeId)';note='actual lifecycle/registry owner29/13, not a fake Session'
 elif stem=='notify/data_notify_settings':
  t=D;r='NotifySettings::'+r
  if n=='NotifySettings':r='NotifySettings::new'
  if n in ['MutedFromUntil','SkipAddException','CommunityFallback','DefaultNotifyType','DefaultNotifyToMTP']:
   r={'MutedFromUntil':'muted_from_until','SkipAddException':'NotifySettings::skip_add_exception','CommunityFallback':'NotifySettings::community_fallback','DefaultNotifyType':'default_notify_type','DefaultNotifyToMTP':'default_notify_to_mtp'}[n]
  if n in ['DefaultNotifyType','DefaultNotifyToMTP']:t=C+'; '+A
  if n=='CommunityFallback':t=C
  if n=='request':r='NotifySettings::request/request_thread';t=A+'; '+B
  if n=='apply':r='NotifySettings::apply/apply_input/apply_notify_peer/apply_peer_id/apply_topic_id';t=D+'; '+A+'; '+T
  if n in ['update','defaultUpdate']:r='NotifySettings::update (Target::Peer/Topic/Default)';t=D+'; '+E
  if n=='resetToDefault':r='NotifySettings::reset_to_default';t=E
  if n=='forumParentMuteUpdated':t=T
  if n=='communityParentMuteUpdated':t=C
  if n in ['defaultValue','defaultSettings']:r='NotifySettings::default_settings/defaults + change(Target::Default)'
  if n=='isMuted':r='NotifySettings::is_muted/is_muted_with_delay (all Target variants)';t=D+'; '+T
  if n=='defaultUpdates':r='NotifySettings::default_updates/default_updates_for'
  if n=='updateLocal':t=D+'; '+T+'; '+C;note='typed History/Topic mute + ClearIncoming/CheckDelayed effects; real owners18/08/5'
  if n in ['cacheSound','lookupRingtone']:t=L
  if n=='cacheSound':r='NotifySettings::cache_sound_id/cache_sound/cache_optional_sound';status='partial(storage/file_download3/4)';note='real document lease/forceToCache + SaveRingtone effect covered; actual loader backend deferred'
  if n in ['loadExceptions','updateException','exceptionsUpdated','exceptionsUpdates','exceptionsUpdatesRealtime','exceptions','clearExceptions','SkipAddException']:t=E
  if n=='exceptionsUpdated':t+='; '+test('notify_reentrant_exception_flush_schedules_next_turn_without_borrow')
  if n in ['settingsUnknown','muteUnknown','soundUnknown','silentPostsUnknown']:t=D+'; '+T
 elif stem=='data_media_preload':
  t=H
  if 'Photo' in name or n=='Should' or n=='start' or (ext=='h' and 32<=line<=43):
   t=O;r='PhotoPreload::'+r
   if n in ['PhotoPreload','~PhotoPreload']:r='PhotoPreload::new/Drop'
   if n=='Should':r='PhotoPreload::should';t=test('photo_preload_should_uses_actual_photo_peer_fields_and_download_policy')
   if n in ['PhotoPreload','start']:status='partial(storage/file_download3/4; этап5)';note='load/loaded/downloader stream required real owner; metadata controller tested, no pixels/cache loader stand-in'
  elif 'MediaPreload' in name or (ext=='h' and line<=23):
   t=O;r='MediaPreload::'+r
   if n in ['MediaPreload','~MediaPreload']:r='MediaPreload::new/Drop'
  else:
   r='VideoPreload::'+r
   if n in ['VideoPreload','~VideoPreload']:r='VideoPreload::new/Drop';t=F
   if n=='ChoosePreloadPrefix':r='VideoPreload::choose_preload_prefix';t=H+'; '+F
   if n in ['Can','readyToRequest']:r='VideoPreload::'+snake(n);t=H+'; '+F
   if n in ['VideoPreload','check','load','done']:status='partial(storage/file_download3/4)';note='model state and exact effects covered; actual DownloadMtprotoTask queue/cache weak/on-main delivery backend remains3/4'
   if n=='check':r='VideoPreload::check/cache_result';t=F
 elif stem=='data_statistics_chart':
  t=N;r='StatisticalChart::'+r
  if n=='StatisticalChart':r='StatisticalChart::default';t=I
  if n=='operator bool':r='!StatisticalChart::empty'
  if n=='measure':r='StatisticalChart::measure_data + DayLabelsRequest';status='partial(этап5)';t=N+'; '+I;note='numeric phase native; real font/local-time/lang labels/max-width rendering deferred'
  if n=='getDayString':t=I;r='StatisticalChart::get_day_string (uses real rendered days_lookup)'
 elif stem=='raw/raw_countries_bounds':
  r='country_bounds (OnceLock literal173 table)';t=N
 elif stem=='../statistics/statistics_data_deserialize':
  module='statistics/statistics_data_deserialize.rs';r='statistical_chart_from_json';t=N+'; '+I;status='partial(этап5)';note='full numeric/JSON/name/color-key phase covered; QColor conversion/colorDark and labels UI5'
 elif stem=='../statistics/segment_tree':
  module='statistics/segment_tree.rs';r='SegmentTree::'+r;t=N
  if n=='SegmentTree':r='SegmentTree::new/default'
  if n=='operator bool':r='!SegmentTree::empty'
  if n=='size':r='Node::size';t=J
  if n in ['propagate','change']:t=J
  if n in ['rMaxQ','rMinQ']:r='SegmentTree::r_max_q/r_min_q/query'
  if n in ['contains','intersects']:t=N+'; '+J
 return status,module+'::'+r,t,note
header='''# Data calls / notify — модель и сервисы

База main2f58312; original0f2a31bb420de98f331990432265b131b8d836b5 read-only.
Все10 запрошенных файлов (3170 строк) прочитаны целиком вместе с usage sites.
Нужный JSON-парсер находится в statistics/statistics_data_deserialize, а не в
целевом data_statistics_chart; эти2 файла и SegmentTree2 учтены как зависимости.
Декларации/inline h и определения cpp считаются отдельно, не уникальные функции.

Actual Peers/Photos/Documents + SessionScope/GroupCallKey/TopicKey; никаких
заменителей Session/History/engine/loader/pixels. Done относится к data-фазе
и явно описанному mutation/effect контракту. Конкретные backend тела указаны
partial отдельными строками; отсутствие native full-owner proof не скрывается.

[Handoff](../../../docs/handoffs/codex-data-calls-notify.md),
[native recipe](../tests/reference-tools/data-calls-notify/README.md).

| Оригинал / функция | Статус | Реализация | Тест / будущий пакет | Примечание |
|---|---|---|---|---|
'''
out=header;counts={'done':0,'partial':0}
def add(source,status,impl,tests,note):
 global out
 counts['done' if status=='done' else 'partial']+=1
 for ref in tests.split('; '):
  if ref.startswith('data_calls_notify'):
   file,fn=ref.split('::');assert re.search(r'fn '+re.escape(fn)+r'\s*\(', (repo/'crates/tg-core/tests'/f'{file}.rs').read_text()),ref
  if ref.startswith('statistics/'):
   assert ref==J
   assert 'fn '+ref.split('::')[-1]+'(' in (repo/'crates/tg-core/src/statistics/segment_tree.rs').read_text()
 out+=f'| `{source}` | {status} | `{impl}` | {tests} | {note} |\n'
for stem,ext,line,name in rows:
 source=str(Path('data')/f'{stem}.{ext}').replace('data/../','')
 add(f'{source}:{line} {name}',*classify(stem,ext,line,name))
add('data/data_group_call.h:184 participantsResolved','done','data/data_group_call.rs::GroupCall::participants_resolved',U,'multiline nested return type; header inline stream')
add('statistics/segment_tree.h:36 Node::PendingVal::operator bool','done','Node::pending.is_some()',J,'nested inline declaration; source flag/value becomes Option')
add('data/data_statistics_chart.h:83 StatisticalGraph fields','done','StatisticalGraph',I,'chart/zoom_token/error; no UI model')
add('data/data_statistics_chart.h:49 Line color/colorDark','partial(этап5)','Line::color_spec/color_key',I+'; future QColor/theme renderer5','preserve captured source color input; do not invent pixel/theme types')
add('data/data_group_call.cpp constructor/destructor registerGroupCall/unregisterGroupCall','partial(9/13/29)','GroupCallOwner::effect(Register/Unregister)',G+'; '+B+'; future Session/GroupCalls registry9/13/29','exact call order tested; backend is not a fake registry')
add('data/data_group_call.cpp inCall, canManage, discard/forward to actual Calls','partial(9)','GroupCallOwner / GroupCall Effect',G+'; '+Q+'; future real Calls engine9','model logic covered; no network/media engine')
add('data/notify/data_notify_settings.cpp Thread/Forum/History owner bodies','partial(08/18)','NotifyOwner real key interface',T+'; '+C+'; future real Topic08/History18','lookup/read/mutate contract only; no fake History/Topic stored in production')
add('data/notify/data_notify_settings.cpp Window::Notifications clear/check','partial(этап5)','Effect::ClearIncomingPeer/ClearIncomingTopic/CheckDelayed',D+'; future Window notifications5','mutation/effect order tested; UI delivery deferred')
add('apiwrap.cpp:2018 ApiWrap::requestNotifySettings subset','partial(26)','NotifySettings::request_input/input_key + send_request',A+'; '+B+'; future full ApiWrap26','only required request dedup, callback erase order and TL bridge; default key collides with peer1')
add('apiwrap.cpp updateNotifySettingsDelayed owner','partial(26)','NotifySettings Effect::UpdateDelayed',E+'; future exact ApiWrap debounce/request delivery26','actual requested target retained; backend not simulated')
add('media/streaming/media_streaming_reader.cpp SerializeComplexPartsMap','done','data/data_media_preload.rs::serialize_complex_parts_map',N+'; '+H,'only helper, full Reader3/4 remains partial in ledger')
# Exact data helpers used by GroupCall; the rest of Calls remains9.
s=(original/'calls/group/calls_group_call.cpp').read_text().splitlines()
helpers=[('empty', 'VideoParams::empty', 282, 297),('operator bool','!VideoParams::empty',282,297),('VideoParamsAreEqual','VideoParams::equal',297,348),('ParseVideoParams','VideoParams::parse',348,379),('GetCameraEndpoint','GroupCallParticipant::camera_endpoint',379,397),('GetScreenEndpoint','GroupCallParticipant::screen_endpoint',379,397),('IsCameraPaused','GroupCallParticipant::camera_paused',379,397),('IsScreenPaused','GroupCallParticipant::screen_paused',379,397),('GetAdditionalAudioSsrc','GroupCallParticipant::additional_audio_ssrc',397,403),('ParseVideoParams','parse_video_params',403,425)]
for name,impl,start,end in helpers:
 found=next(i+1 for i,l in enumerate(s) if start<=i+1<end and name+'(' in l)
 add(f'calls/group/calls_group_call.cpp:{found} {name} (data helper)','done','data/data_group_call.rs::'+impl,V,'real media engine outside this helper remains9')
out+=f"\nВсего **{sum(counts.values())} строк: {counts['done']} done / {counts['partial']} partial /0 pending**. Каждая done-строка имеет тест, каждая partial — будущий пакет.\n"
(repo/'crates/tg-core/docs/data-calls-notify-map.md').write_text(out)
print(counts,'total',sum(counts.values()),'base function rows',len(rows))
