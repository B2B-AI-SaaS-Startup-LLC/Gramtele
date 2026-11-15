"""API/body inventory for the requested 19 files. Header declarations and cpp
bodies are explicit separate rows (not unique-function counts). No hidden scope.
Run from repository root with the read-only Telegram/SourceFiles path.
"""
from pathlib import Path
import re,sys,collections
root=Path(sys.argv[1]);base=Path('crates/tg-core');dest=base/'docs/data-misc-map.md'
files=['data_auto_download','data_file_origin','data_audio_msg_id','data_authorization','data_abstract_structure','data_abstract_sparse_ids','data_boosts','data_bot_app','components/top_peers','business/data_business_common','data_emoji_statuses']
rows=[]
def snake(name):return re.sub(r'([a-z0-9])([A-Z])',r'\1_\2',re.sub(r'([A-Z]+)([A-Z][a-z])',r'\1_\2',name)).lower()
for stem in files:
 for ext in ['h','cpp']:
  path=root/'data'/f'{stem}.{ext}'
  if not path.exists():continue
  lines=path.read_text().splitlines();found=False
  for i,line in enumerate(lines):
   text=line.strip()
   if not text or text.startswith(('//','/*','*','#',':',',','return ','if ','if(','for ','while ','switch ','case ','using ','namespace ','Fn<','const auto ','auto &','auto result','auto i ','auto size','auto stream','auto buffer','auto clearing')):continue
   if ext=='cpp' and line[0].isspace() and not re.match(r'void push\(',text):continue
   if ext=='h' and text.startswith(('Expects(','Assert(','static_assert(','ranges::','std::','internal::','_p =')):continue
   if '(' not in text:continue
   # cpp signatures are column zero except real accumulator push methods.
   head=text.split('(')[0]
   if (('=' in head or '->' in head) and 'operator' not in head) or '.' in head or head.startswith('< '):continue
   match=re.search(r'(operator(?: bool|==|!=|=|->|<|\[\])|[\w:~]+)$',head)
   if not match:continue
   name=match[1]
   if name in ['Flag','Type','int','quint64','qint32','LOG','DEBUG_LOG','Ensures','sizeof','static_cast','const_cast','void']:continue
   signature=text
   if ')' not in text:
    signature=' '.join(x.strip() for x in lines[i:i+8]);signature=signature.split(')')[0]+')'
   # Header executable statements do not declare functions.
   if ext=='h' and name in ['createIfNull','insert','clear','push_back'] and text.startswith(('structures','impl','myData')):continue
   if name.startswith('_') or head.startswith(('delete ','structures','myData','begin','end')):continue
   found=True;rows.append((stem,ext,i+1,name,signature))
  if not found:rows.append((stem,ext,1,'value fields','value contract'))

def classify(stem,ext,line,name,signature):
 module='data/'+stem+'.rs';status='done';note='';member=name.split('::')[-1];rust=snake(member)
 if stem=='data_auto_download':
  test='data_misc::auto_defaults_unsigned_limits_overrides_and_disable'
  if member in ['serialize','setFromSerialized','enums_view','Defaults','set','setOrDefault','single','shouldDownload','bytesLimit']:test='data_misc_native::native_auto_serialization_truncation_versions_and_limits'
  if member in ['Should','ShouldAutoPlay','SourceFromPeer','AutoPlayTypeFromDocument','ForceAllowLimit','ForceAllowed']:test='data_misc::auto_decisions_stickers_streaming_gifv_override_and_photo; data_misc::auto_photo_video_boundary_and_failed_deserialize_is_transactional';rust='should_document/should_photo/should_auto_play_document/should_auto_play_photo/source_from_peer'
  if member=='WithDisabledAutoPlay':test='data_misc::auto_defaults_unsigned_limits_overrides_and_disable'
  if member in ['Defaults','enums_view','setOrDefault','set','single']:rust='Full/Set/Single data indexing + default_limit + SOURCES/TYPES'
 elif stem=='data_file_origin':
  test='data_misc_native::native_file_reference_accumulator_all_variants';rust='FileReferenceSource::collect_references / get_file_references'
  if ext=='h' and member not in ['GetFileReferences']:
   test='data_misc::origins_all_constructors_keep_cpp_variant_discriminant_order; data_misc::origins_preserve_variant_order_identity_fields_and_utf16';rust='FileOrigin variants / From / Ord; FileLocationId; UpdatedFileReferences'
  if name=='push':note=signature.split('{')[0]
 elif stem=='data_audio_msg_id':
  test='data_misc::audio_identity_classification_and_cached_fields; data_misc_native::native_audio_default_external_and_origin_ordering';rust='AudioMsgId::'+{'AudioMsgId':'new/default','type':'kind','setTypeFromAudio':'new','operator bool':'is_present','operator<':'cmp','operator==':'eq','operator!=':'ne'}.get(member,snake(member))
 elif stem=='data_authorization' or stem=='data_boosts' or stem=='data_bot_app':
  test='data_misc::authorization_boosts_and_bot_app_preserve_data_contracts';rust='UnreviewedAuth' if stem=='data_authorization' else 'Boost* data structs' if stem=='data_boosts' else 'BotAppData::new/owner'
 elif stem=='data_abstract_structure':
  test='data_misc::global_registry_clears_handles_once_and_can_recreate';rust='GlobalStructures / GlobalStructurePointer / Slot<T>::clear';note='safe slots; address ordering uses live slot addresses; app owner supplies registry'
 elif stem=='data_abstract_sparse_ids':
  test='data_misc::sparse_vectors_and_ordered_nearest_have_source_semantics; data_misc::sparse_wrapped_deque_preserves_photos_order';rust='AbstractSparseIds<Vec/VecDeque/FlatSet>';note='nearest restricted to FlatSet; reverse on ordered containers is not exposed'
 elif stem=='components/top_peers':
  rust='TopPeers::'+snake(member)
  test='data_misc_services::top_requests_reply_order_throttle_and_local_skip'
  if member in ['RatingDelta','SerializeRating','DeserializeRating']:rust=snake(member);test='data_misc_native::native_top_rating_rounding_and_decay'
  elif member in ['TypeToCategory','TypeToGetFlags','TypeToCategoryConstructor']:rust='type_to_category/type_to_get_flags/category_matches';test='data_misc_services::top_hash_only_first_64_and_type_flags; data_misc_services::real_sender_adapters_deliver_typed_replies_without_network'
  elif member in ['CanIncrementPeer','increment','remove','toggleDisabled']:test='data_misc_services::top_increment_eligibility_disabled_and_source_reset_latch; data_misc_services::top_ratings_ties_persistence_and_reentrant_updates'
  elif member=='countHash':test='data_misc_native::native_business_rights_and_top_peer_hash; data_misc_services::top_hash_only_first_64_and_type_flags'
  elif member in ['list','disabled','updates','updated']:test='data_misc_services::top_ratings_ties_persistence_and_reentrant_updates'
  elif member in ['serialize','applyLocal']:status='partial(storage/serialize_peer, packages 13/29)';test='data_misc_services::top_ratings_ties_persistence_and_reentrant_updates';note='real framing/rating/state rollback + required PeerCodec; full peer codec/format not invented'
  elif member in ['loadAfterChats','TopPeers','~TopPeers']:status='partial(Session/chats-list, packages 13/29)';rust='TopPeers::new/load_after_chats/Drop';note='constructor emits LoadAfterChats; owner binds original two main-queue turns and lifetime'
  elif member=='request':rust='TopPeers::request/handle_reply + send_request';test='data_misc_services::top_requests_reply_order_throttle_and_local_skip; data_misc_services::real_sender_adapters_deliver_typed_replies_without_network'
 elif stem=='business/data_business_common':
  test='data_misc::business_intervals_keep_source_suffix_loss_and_presence_rules';rust='business value structs / interval methods'
  if member in ['SortAndMerge','MoveTailToFront','normalized','ExtractDayIntervals','IsFullOpen','RemoveDayIntervals','ReplaceDayIntervals','shifted','united','intersected']:rust=snake(member);test='data_misc_native::native_business_normalization_extract_remove_replace'
  elif member in ['RecipientsFlags','ForMessagesToMTP','ForBotsToMTP','MakeValid','empty']:rust='recipients_flags/for_messages_to_mtp/for_bots_to_mtp/BusinessRecipients::make_valid';test='data_misc::business_recipients_flags_user_contract_and_permissions'
  elif member=='FromMTP':rust='recipients_from_mtp/bot_recipients_from_mtp/permissions_from_mtp/details_from_mtp/away_from_mtp/greeting_from_mtp';test='data_misc::business_recipients_flags_user_contract_and_permissions; data_misc::business_tl_details_use_real_media_store_and_schedule_variants; data_misc_native::native_business_rights_and_top_peer_hash'
  elif member=='ToMTP':rust='permissions_to_mtp';test='data_misc_native::native_business_rights_and_top_peer_hash'
 elif stem=='data_emoji_statuses':
  rust='EmojiStatuses::'+snake(member);test='data_misc_services::emoji_constructor_hash_failures_lists_and_periodic_refresh'
  if ext=='h' and line<40 and member=='operator bool':rust='EmojiStatusCollectible::is_present';test='data_misc_services::emoji_collectible_first_payload_keeps_shared_identity'
  elif 'Groups' in member or member=='requestGroups':status='partial(UI EmojiGroup, stage 5)';rust='—';test='future stage 5 UI group tests';note='Ui::EmojiGroup/GroupsFromTL/requestGroups/value methods; no fake Ui group'
  elif member=='fromUniqueGift':status='partial(UniqueGift package 16; localized name 01)';rust='intern_collectible (shared identity only)';test='data_misc_services::emoji_collectible_first_payload_keeps_shared_identity';note='no substitute UniqueGift or translated name'
  elif member in ['EmojiStatuses','~EmojiStatuses','owner','session']:rust='EmojiStatuses::new/owner/session/Drop';note='real TimerHost, no fake Main::Session; complete application ownership remains package29'
  elif member in ['parse','ParseEmojiStatusCollectible','collectibleInfo']:test='data_misc_services::emoji_collectible_tl_color_and_until_parse; data_misc_services::emoji_collectible_first_payload_keeps_shared_identity';rust='parse/parse_list/intern_collectible/collectible_info'
  elif member in ['set']:test='data_misc_services::emoji_set_cancels_before_mutation_and_preserves_local_on_failure; data_misc_services::emoji_binds_real_peers_changes_and_clear_effects; data_misc_services::real_sender_adapters_deliver_typed_replies_without_network';rust='set/set_self/status_to_mtp/send_request/apply_peer_status_effect'
  elif member in ['registerAutomaticClear','automaticClearAt','processClearing','processClearingIn']:test='data_misc_services::emoji_automatic_clear_cap_remove_and_virtual_expiration; data_misc_services::emoji_clearing_reentrancy_merges_new_registration';rust='register_automatic_clear/automatic_clear_at/process_clearing/process_clearing_in'
  elif member in ['refreshRecentDelayed','requestRecent','refreshRecent','updateRecent','recentUpdates']:test='data_misc_services::emoji_recent_delayed_requests_coalesce_without_sleep';rust='refresh_recent/refresh_recent_delayed/request/handle_reply/recent_updates'
  elif member in ['requestColored','requestChannelColored','updateColored','updateChannelColored','requestCollectibles','updateCollectibles','refreshCollectibles','refreshColored','refreshChannelColored','collectiblesUpdates']:test='data_misc_services::emoji_colored_processes_all_documents_then_collectibles';rust='request/handle_reply/refresh_colored/refresh_collectibles/collectibles_updates'
  elif member.startswith(('request','update')):rust='request/handle_reply/updates'
 return status,module+'::'+rust,test,note

header='''# Data misc — срез пакетов 06/07

База main `4265492`, pinned original `0f2a31bb420de98f331990432265b131b8d836b5`.
Все 19 файлов прочитаны целиком; также прочитаны вызовы в apiwrap/Session,
настройках, sparse snapshots и исходные базовые timer/stream/Sender contracts.

Ниже отдельно учитываются API-декларации из h и тела из cpp: это **строки карты,
не число уникальных функций**. Header-only тела, accumulator push overloads и
value-only headers включены. Каждая строка содержит реализацию/явный partial и
тест/будущий пакет. Тесты находятся в `tests/{data_misc,data_misc_services,
data_misc_native}.rs`; ссылки вида `file::test` именуют настоящие test functions.

- Full/Set/Single, decisions, peer overrides, v1/v2/v3 и legacy settings effect
  реализованы. В pinned auto_download **нет Preload**. `FileOrigin` не имеет
  собственного сериализатора/постоянного формата: перенесены варианты, order и
  все 19 overloads `GetFileReferences`, включая исходный recursive TL traversal.
- Required owner services (InputUser lookup, peer codec, effects/reply routing)
  не имеют пустой production implementation. TopPeers peer persistence требует
  настоящий `storage/serialize_peer`; записан framing, а не выдуманный peer wire.
- Emoji timers работают в настоящем TimerHost с виртуальным clock в тестах;
  все borrow закрываются до fire/effect. Есть binding к реальному Peers/Changes.
  Группы Ui::EmojiGroup — stage5, UniqueGift/name —16/01.
- Native: [рецепт и границы](../tests/reference-tools/data-misc/README.md).
  Полные Session/model state и attrs/Should(DocumentData) — **own tests + чтение**;
  воспроизведения полного C++ Session/UI/peer codec здесь нет.

| Оригинал / функция | Статус | Реализация | Тест / будущий пакет | Примечание |
|---|---|---|---|---|
'''
text=header;counts=collections.Counter()
for stem,ext,line,name,signature in rows:
 status,rust,test,note=classify(stem,ext,line,name,signature);counts[status.split('(')[0]]+=1
 # Escape pipes used by C++ operators/signatures to keep markdown stable.
 name=name.replace('|','\\|');note=note.replace('|','\\|')
 text+=f'| `data/{stem}.{ext}:{line} {name}` | {status} | `{rust}` | {test} | {note} |\n'
text+='\nИтого: **'+str(len(rows))+' строк**, '+str(counts['done'])+' done / '+str(counts['partial'])+' partial / **0 pending**.\n'
text+='''
## Дополнительные точки соединения

| Реализация | Проверка |
|---|---|
| Full::apply_settings_effect(SessionEffect::AutoDownloadBytesLimit) | data_misc::auto_legacy_effect_applies_to_real_full (настоящий read_setting → Full) |
| EmojiStatuses::bind_peer_effects + apply_peer_status_effect | data_misc_services::emoji_binds_real_peers_changes_and_clear_effects |
| concrete TL → real Sender adapters, main-thread reply boundary | data_misc_services::real_sender_adapters_deliver_typed_replies_without_network |
| Business details TL → actual Documents::process_document | data_misc::business_tl_details_use_real_media_store_and_schedule_variants |

Source-compatible quirks: auto partial stream acceptance; stream types always
excluded; forced Channel File still has default zero limit; top Reset request
latches reload; channel-default uses account.getDefaultEmojiStatuses; collectible
first payload wins; callback registrations merge without overwriting newer ones,
but processClearing's original minWait can cancel a reentrant new timer; removing
a business day can lose its suffix. Details and limits in the handoff.
'''
dest.write_text(text)
# Check every referenced own test actually exists; future partial tests explicit.
tests='\n'.join(p.read_text() for p in (base/'tests').glob('data_misc*.rs'))
for test in re.findall(r'data_misc(?:_services|_native)?::(\w+)',text):assert re.search(r'fn '+test+r'\(',tests),test
print(len(rows),dict(counts))
