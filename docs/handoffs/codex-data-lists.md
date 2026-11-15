# port/data-lists — списки и сервисные модели

15.11.2026. База main9575f65, отдельный worktree
`/home/yarik/Projects/ayugram-rs-wt/data-lists`, ветка `port/data-lists`.
Original read-only0f2a31bb420de98f331990432265b131b8d836b5.16 файлов/4043 строки.

## Статус каждого файла

| Original в SourceFiles/data/ | h | cpp | Перенесено / остаток |
|---|---|---|---|
| data_messages | done (243) | done (531) | MessagePosition/order/constants, ranges/results, весь MessagesList и builder, query/viewer, invalidation, merging/skips/refill |
| data_sparse_ids | done (203) | done (428) | Typed sparse/unsorted slices, merged halves/isolation, nearest/index/distance, keys, lazy CreateViewer, весь builder |
| data_shared_media | partial (217) | partial (581) | overview/search, real-storage viewer/merged, WithLast snapshots+viewer routing/reverse; IsItemGoodForType/ScheduledMediaViewer —05/18–20, SavedMusicMediaViewer —12/18–20 |
| data_user_photos | done (55) | done (257) | slices/key/reverse/distance, replace builder, counts/skips, real storage viewer/reversed/fallback |
| data_pts_waiter | done (104) | done (187) | all overloads/state/gap ordering/queue/updateAndApply; exact timer/apply calls as required Effect contract, actual Api::Updates owner26 |
| data_premium_limits | done (129) | done (339) | all PremiumLimits/LevelLimits getters; actual owner AppConfig/premium/test-mode contract, no cached/default owner |
| business/data_business_info | done (64) | done (318) | TL conversions, optimistic mutations/field rollbacks, away/greeting/events/loaded, timezones preload/latch/value, closest-timezone algorithm with real clock inputs; concrete Sender adapter |
| stickers/data_stickers_set | partial (138) | partial (249) | all scalar fields/flags/document packs/dates, inputs/type/identity, thumbnail metadata/cache effect and document selection; EmojiPtr map/image views5, CloudFile loader3/4+5 deferred |

Ledger: **12 done/4 partial**. Scope of completion is this data layer; full
Main::Session/UserData/API/History owners are not replaced. No fake History,
UserData, Session, QImage or loader classes. No todo!/unimplemented! in new code.

## Ownership and API boundaries

SharedMedia/UserPhotos take the **existing** storage types; no duplicate lists.
MessagesList is a separate source algorithm (it differs from SparseIdsList).
Callback requests retain actual peer/topic/monoforum/type/id/direction values.
Subscriptions are held in tg-rpl Lifetimes and cancelled on drop. The application
must retain model lifetime guards and route API replies to the owner thread29.
The future storage/API adapter must preserve the original queued facade delivery;
synchronously mutating the same RefCell storage during its mutation callback is
not a supported substitute for that queue. Models release their own borrows before
fire, request callbacks and cross-record lookups.

WithLastSource exposes the actual merged/scheduled/saved-music/message-photo/peer-
photo calls. Normal/photo/05-scheduled/12-music routing and combine order are
implemented and tested. Bodies that dereference HistoryItem remain18–20; no dummy
scheduled history is constructed. Missing History→media→photo lookup must return
0 as in C++, not an invented PhotoData. SharedMediaWithLastSlice uses real Photos
stable record handles from04, so photo ID conversion remains visible in indexOf.
FullMsgId remains a source value selector, not a fabricated HistoryItem record.
WithLastKey ordering follows source field/variant order; photo handles replace
pointer identity. Arbitrary C++ allocator pointer ordering is not reproduced.

PtsWaiter itself contains no timer in C++; Api::Updates owns its aggregated Timer.
The port sends exactly StartTimerFor/ApplyUpdateNoPtsCheck/ApplyUpdatesNoPtsCheck
calls, synchronously in original order, with ChannelKey instead of ChannelData*.
Actual getDifference/getChannelDifference/global-vs-channel deadline aggregation
is package26. Tests use real tg-base Timer/TimerHost+ManualClock, no sleep, and
check1000ms/1ms/cancel, queue flush order, stale/requesting/shortpoll and callback
reentry. This does not claim a completed Api::Updates timer owner.

BusinessOwner is required self-user details/inputUser/document lookup/set API02/29.
Sending precedes local mutation; errors restore only the changed field using fresh
owner details. Away/greeting rollbacks restore Option and loaded state, and fire
before fail callback. Concrete tg_mtproto::Sender bridge uses actual schema methods,
request IDs/done/fail; tests feed real Instance callbacks without opening sockets.
Owner dispatch/lifetime/network delivery remains26/29. FindClosestTimezoneId takes
local UTC offset and corrected-server-time shift from the real platform/clock
owner; its circular-distance/tie algorithm is ported. DST/local-clock acquisition
is not simulated. AppConfig typed integer getter (including its parsing/rounding)
belongs to real owner29; every limit call reads that required boundary live.

## Native and differences

[Map](../../crates/tg-core/docs/data-lists-map.md): **511 rows,
483 done/28 partial/0 pending**. Declarations in h and bodies in cpp are counted
separately, not as511 distinct functions. Every done row points to a real test,
every partial has a future package. The generator checks all done test references.

[Native recipe](../../crates/tg-core/tests/reference-tools/data-lists/README.md),
`/tmp/ayugram-data-lists-native`. Real unchanged messages/sparse cpp, real original
TL generator/output, lib_base/rpl/Qt6.11.2/GCC15.3. **804 rows**, two identical
captures; Rust compares all exactly.54 PremiumLimits getters ×4 modes; list504 +
21 boundary states; sparse requests/builders34 + merged18; business11 TL payloads.
21 source SHA256 values include all16 requested original files and actual deps.

Native seams are explicit: PremiumLimits::appConfigLimit/isPremium fixture inputs,
ForMessagesToMTP empty recipients. No owner/model body is substituted and then
reported as source proof. Native does **not** prove full AppConfig JSON/rounding,
LevelLimits/maxBoostLevel owner calls, arbitrary business recipients, full Session
state, sticker loader/pixels or user-photo service. These are own tests + reading.
Full source probes for premium/pts/business/shared/photos/stickers with original
stdafx.h and real Qt/OpenGL/openssl stop at absent generated `emoji.h` from
lib_ui/ui/emoji_config.h; no generated UI asset/type is stubbed.

Preserved source quirks:

- similarChannelsCurrent uses channelsDefault/Premium, unlike the adjacent
  similarChannelsDefault/Premium getters (recommendation limits).
- MessagesList::removeOne decrements total even for absent IDs, and joining
  multiple ranges in addNew subtracts only the first slice's old size (overcount).
  removeLessThan removes an entire range when its till equals cutoff; no event.
- Sparse merged unsorted indexOf ignores peer, nearest falls back to middle;
  sorted nearest is lower_bound, not minimum absolute distance. Isolated migrated
  slices can still return a part index outside visible size: preserve source.
- UserPhotos empty result can reset count to unknown while retaining IDs; provided
  skippedBefore is ignored by the source merge body. Source does end trim first.
- Pts init/requesting clear queues but preserve wait flags/skipped-key counter;
  nonnegative StartTimerFor fires before its waiting flag is set.
- Away/greeting enabled with shortcut0 update local state without a request.
  Timezone request ID is never reset, including notModified/failure/empty result.
- WithLast retains lastPeerPhotoId even for non-chat-photo types, so indexOf(Photo)
  may alias the last message when extra-photo skip is0.
- StickersSet::identifier omits shortName; model mtpInput with id/hash0 uses a
  shortName input, but InputStickerSet(identifier()) can produce empty.

Defined Rust preconditions in place of source UB: merged key0 without migrated
slice can dereference disengaged optional in C++; Rust panics on those accessors,
no fallback is invented. Empty timezone list similarly requires nonempty input.
Sparse signed-overflow inputs, source iterator invalidation when callbacks clear
queued updates during iteration, raw-pointer allocator order and QString lone
surrogates are not native-proven. Thumbnail big-file key on an unsupported empty
StorageFileLocation kind has the source fatal/precondition behavior.

## Verification and integration

25 new tests (24 data_lists +1 native), all targeted and workspace PASS. Final
fmt/clippy use the completed source/key comparator and peer-photo lookup.
No new dependencies, Cargo manifests/lock unchanged. Claude scope (lang/countries,
credits_amount/tg-ui/codegen/runtime/mtproto/localstorage) and foreign worktrees
untouched. Known runtime ObjectAsync::drop race belongs to Claude, not modified.

Final checks through nix develop, all PASS:

- `cargo fmt --check`: exit0.
- `cargo clippy --workspace --all-targets -- -D warnings`: exit0.
- `cargo test --workspace`: exit0,868 successful results (867+1 child),0failed,
  3existingignored.25 new tests; tg-core223 tests. Known runtime race not observed.

Logs `/tmp/data-lists-workspace-checks.log`, final cached lint validation after
last source changes `/tmp/data-lists-final-lints.log` (no second full test run).
Source hashes, repeated native capture, map references, git diff --check PASS.
Accounting completed before commit/report. Integrate only port/data-lists into
main by fast-forward; atomic push main+branch to gramtele without force. CI result
is checked separately after push; local PASS alone is not labelled CI success.
