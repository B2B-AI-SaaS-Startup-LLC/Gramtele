"""Inventory h declarations/inline bodies and cpp bodies separately; no pending rows."""
from pathlib import Path
import re,sys
original=Path(sys.argv[1]);repo=Path.cwd()
files=['components/passkeys','components/recent_peers','data_reply_preview','data_streaming','data_search_controller','data_passkey_deserialize','../platform/platform_webauthn']
rows=[]
for stem in files:
 for ext in (['h'] if stem.startswith('../platform/') else ['h','cpp']):
  lines=(original/'data'/f'{stem}.{ext}').read_text().splitlines()
  consumed=-1
  for i,l in enumerate(lines):
   if i<=consumed:continue
   if ext=='cpp':
    if not l or l[0].isspace() or l[0] in '#/:,}' or l.startswith(('namespace ','using ','constexpr ')):continue
   else:
    if not l or l.startswith(('#','//','namespace ','using ','class ','struct ','enum ')):continue
    # Source header members live at depth one/two; ignore body statements.
    if l.startswith('\t\t\t') or l.lstrip().startswith(('//', '_controller.', '_timer.', 'return ',': ', ', ', 'Expects(', 'if (', 'const auto ')):continue
   sig=l.strip();j=i
   if '(' not in sig:continue
   while not re.search(r'[;{]',sig) and j+1<len(lines):j+=1;sig+=' '+lines[j].strip()
   if ('=' in sig.split('(')[0] and 'operator' not in sig.split('(')[0]) or sig.startswith(('if ','for ','switch ','static_assert(', 'return ', 'Expects(')):continue
   sig=re.sub(r'\[\[.*?\]\]','',sig).strip()
   m=re.search(r'([\w:~]+|operator\s*(?:bool|\[\]|[=!<>]+))\s*\(',sig)
   if m and m[1] not in ['if','void','bool','MTP_int','MTP_flags','MTP_string']:
    rows.append((stem,ext,i+1,m[1]));consumed=j


def snake(n):return re.sub(r'([a-z0-9])([A-Z])',r'\1_\2',re.sub(r'([A-Z]+)([A-Z][a-z])',r'\1_\2',n)).lower()
def t(n):return 'data_small::'+n
N='data_small_native::original_qt_passkey_json_credentials_handle_filters_geometry_and_offsets'
P=t('passkeys_registration_cache_boundary_concurrent_requests_invalid_json_and_fail')
L=t('passkeys_list_eager_call_lazy_stream_timeout_failure_registration_order_and_delete')
D=t('passkeys_register_failure_drops_cache_delete_failure_and_live_config')
A=t('actual_sender_passkeys_and_search_replies_no_network')
F=t('passkey_login_real_sender_parse_invalid_handle_dc_switch_done_and_fail')
R=t('recent_peers_unbounded_bump_first_noop_absent_remove_clear_and_reentry')
C=t('recent_peers_frame_limit_corruption_duplicate_order_and_no_update_on_read')
W=t('recent_open_weak_identity_no_revive_cap_reorder_remove_and_sublist')
S=t('streaming_shares_weak_readers_force_remote_and_loader_failure')
K=t('streaming_keepalive_extends_deadline_no_early_rearm_and_releases_on_drop')
Q=t('streaming_quality_metadata_updates_only_nonempty_and_shares_reader')
E=t('search_parsing_source_filter_expands_range_before_after_and_not_modified')
G=t('search_global_filtered_offset_channel_pts_and_not_modified_precondition')
V=t('search_cache_viewer_real_storage_removal_filters_and_request_failure_latch')
B=t('search_save_restore_cancel_empty_cache_and_query_order')
M=t('search_migrated_requests_saved_lists_and_cancellation_on_cache_clear')
I=t('delayed_search_virtual_debounce_cached_fast_path_keeps_old_timer_and_cancel')
J=t('search_request_defaults_topic_mono_global_and_empty_query')
def classify(stem,ext,line,name):
 n=name.split('::')[-1];r=snake(n);status='done';test=N;note='';module='data/'+stem+'.rs'
 if stem=='components/passkeys':
  test=L;r='Passkeys::'+r
  if n in ['Passkeys','~Passkeys']:r='Passkeys::new/Drop'
  if n=='FromTL':r='from_tl';test=N
  if n=='initRegistration':test=P
  if n=='registerPasskey':test=L+'; '+D+'; '+N;r='Passkeys::register_passkey/handle_reply + registration_credential'
  if n=='deletePasskey':test=L+'; '+D
  if n in ['canRegister','possible']:test=L+'; '+D;note='required real AppConfig29/WebAuthn support33 facts; no default backend'
  if n=='loadList':r='Passkeys::load_list/handle_reply';test=L
  if n in ['InitPasskeyLogin','FinishPasskeyLogin']:r=snake(n);test=F+'; '+N;note='real Sender/Instance/DC/auth keys; returns actual request ID for explicit caller lifetime/cancel'
 elif stem=='components/recent_peers':
  test=R;r='RecentPeers::'+r
  if n in ['RecentPeers','~RecentPeers']:r='RecentPeers::new/Drop'
  if n in ['serialize','applyLocal']:test=C;status='partial(13/29)';note='full frame/order/rollback covered; real existing PeerCodec adapter Serialize::peer remains13/29'
  if n in ['collectChatOpenHistory','chatOpenPush','chatOpenRemove']:test=W;note='real instance lifetime token via required owner08/16/18; no invented Thread record'
  if n=='chatOpenKeepUserpics':r='—';status='partial(этап5)';test='future actual Ui::PeerUserpicView cache5';note='no pretend userpic type'
 elif stem=='data_reply_preview':
  r='preview_geometry (prepare arithmetic fragment only)';status='partial(этап5; loader3/4)';test=N+'; future real Image/Prepare/loaded cache5 + thumbnails3/4'
  note='entire ReplyPreview image selection/state/loaded requires actual Image; no fake always-unloaded class'
 elif stem=='data_streaming':
  test=S;r='Streaming::'+r
  if n=='Streaming':
   if ext=='h' and line==28:r='Rust move-only value (no Clone/Copy)';test=K
   else:r='Streaming::new';test=K
  if n=='operator=':r='Rust move-only value (no Clone/Copy)';test=K
  if n=='~Streaming':r='Streaming::Drop';test=K
  if n=='PruneDestroyedAndSet':r='prune_destroyed_and_set';test=S
  if n=='sharedDocument':r='Streaming::shared_document/shared_quality_document/document_impl';test=S+'; '+Q
  if n in ['keepAlive','clearKeptAlive']:test=K
  if n=='LookupOtherQualities':
   if ext=='cpp' and line==44:status='partial(18–20)';r='Streaming::document_impl metadata loop + StreamingBackend::resolve_qualities/quality_data';test=Q;note='exclude chosen key/height0/casts ported; real History media guard belongs18–20'
   else:r='Streaming::document_impl Photo branch (no original/context possible)';test=S
  if n=='sharedReader':note='full sharing/force/prune policy covered; real factory backend stage7 separately partial'
 elif stem=='data_search_controller':
  test=B;r='SearchController::'+r
  if n in ['PrepareSearchFilter','PrepareGlobalMediaRequest','PrepareSearchRequest','PrepareHistoryRequest']:r=snake(n);test=N+'; '+J
  if n in ['ParseSearchResult','ParseHistoryResult']:r=snake(n);test=E;note='History item insertion/facts use real MessageKey owner18–20, no item model stand-in'
  if n=='ParseGlobalMediaResult':r=snake(n);test=G
  if n=='operator<=>':r='Query::Ord/PartialOrd (field order, kCount last, UTF16 QString order)';test=B
  if n=='SearchController':r='SearchController::new'
  if n in ['CacheEntry']:r='SearchController::entry';test=M
  if n=='Data':r='Data::new';test=M
  if n=='idsSlice':test=V+'; '+M
  if n=='simpleIdsSlice':test=V+'; '+M
  if n=='requestMore':status='partial(18–20/26)';r='SearchController::request_more/handle_reply + send_search_request';test=V+'; '+M+'; '+A;note='request dedup/failure latch/order covered; actual Histories scheduling/finish/cancel owner18–20/26'
  if n in ['saveState','restoreState']:test=B+'; '+M
  delayed='DelayedSearchController' in name or (ext=='h' and line>=175) or (ext=='cpp' and line>=574)
  if delayed:
   r='DelayedSearchController::'+snake(n);test=I
   if n=='DelayedSearchController':r='DelayedSearchController::new'
   if n=='setQuery':r='DelayedSearchController::set_query/set_query_delayed'
 elif stem=='data_passkey_deserialize':
  r=snake(n);test=N
 elif stem=='../platform/platform_webauthn':
  module='platform/platform_webauthn.rs';r='result value types only';status='partial(33)';test=F+'; future actual platform WebAuthn support/RegisterKey/Login33';note='no support probing or key ceremony stubs'
 return status,module+'::'+r,test,note
out='''# Data small — Passkeys, RecentPeers, streaming/search и reply preview

База main7bc4737; original0f2a31bb420de98f331990432265b131b8d836b5 read-only.
Все10 целевых файлов1908LOC прочитаны целиком с usage sites settings/intro/player/
storage/search. Passkey Deserialize2 и platform WebAuthn result header — нужные
зависимости. h declarations/inline и cpp definitions считаются отдельно.

Done означает реализованную data-фазу с тестом. Backend bodies, зависящие от
настоящих History/Thread/Session/loader/Image, имеют отдельные partial строки.
Нет fake History/Session/Reader/Document/Image реализаций. Required backend
associated types должны быть настоящими streaming7 types. PeerCodec взят из
существующего TopPeers; собственный peer persistence format не изобретён.

[Handoff](../../../docs/handoffs/codex-data-small.md),
[native recipe](../tests/reference-tools/data-small/README.md).

| Оригинал / функция | Статус | Реализация | Тест / будущий пакет | Примечание |
|---|---|---|---|---|
'''
counts={'done':0,'partial':0}
def add(source,status,impl,test,note):
 global out
 counts['done' if status=='done' else 'partial']+=1
 for ref in test.split('; '):
  if ref.startswith('data_small'):
   file,fn=ref.split('::');assert re.search(r'fn '+re.escape(fn)+r'\s*\(', (repo/'crates/tg-core/tests'/f'{file}.rs').read_text()),ref
 out+=f'| `{source}` | {status} | `{impl}` | {test} | {note} |\n'
for stem,ext,line,name in rows:
 source=str(Path('data')/f'{stem}.{ext}').replace('data/../','')
 add(f'{source}:{line} {name}',*classify(stem,ext,line,name))
add('data/data_reply_preview.cpp:prepare geometry fragment','done','data/data_reply_preview.rs::preview_geometry',N,'real QSize arithmetic; style size/DPR supplied as inputs; rest of prepare remains5')
add('data/components/passkeys.h PasskeyEntry fields','done','data/components/passkeys.rs::PasskeyEntry/from_tl',N,'all fields/default optional flags')
add('data/data_passkey_deserialize.h structs/default timeout','done','data/data_passkey_deserialize.rs::RelyingParty/User/CredentialParameter/RegisterData/Credential/LoginData',N,'all scalar/byte fields, timeout60000')
add('platform/platform_webauthn.h Error/RegisterResult/LoginResult','done','platform/platform_webauthn.rs value types',F+'; '+N,'values only; actual platform ceremonies33 partial above')
add('data/components/recent_peers.cpp weak Thread backend','partial(08/16/18)','RecentPeersOwner::thread_lifetime',W+'; future real Topic/SavedSublist/History08/16/18','owner token bound to instance, not key existence; expired key cannot revive')
add('data/data_streaming.cpp createStreamingLoader/Reader/Document/cache','partial(этап7 storage/streaming)','StreamingBackend associated types/factories',S+'; '+Q+'; future actual storage/streaming7 backend','sharing controller done; no simulated production Reader/Document/loader/cache')
add('data/data_search_controller.cpp addNewMessage/HistoryItem/Histories events owner','partial(18–20)','SearchOwner required MessageKey facts/events/insertion',E+'; '+V+'; future actual History18–20','key still live on removed event; genuine shared-media/topic/sublist/forward facts required')
add('data/components/passkeys.cpp Session/AppConfig/WebAuthn support owner','partial(29/33)','PasskeysOwner required facts/send',L+'; '+F+'; future actual Session29/platform33 binding','real Sender bridge exists; callbacks guarded/delivered on owner thread29')
out+=f"\nВсего **{sum(counts.values())} строк: {counts['done']} done / {counts['partial']} partial /0 pending**. Все test refs проверены генератором; partial имеет адрес будущего пакета.\n"
(repo/'crates/tg-core/docs/data-small-map.md').write_text(out)
print('functions',len(rows),'rows',sum(counts.values()),counts)
