# port/data-calls-notify — GroupCall, уведомления, preload и графики

15.11.2026. База main2f58312, отдельный worktree
`/home/yarik/Projects/ayugram-rs-wt/data-calls-notify`, своя ветка
`port/data-calls-notify`. Original read-only0f2a31bb420de98f331990432265b131b8d836b5.
Все10 целевых файлов/3170 строк прочитаны целиком вместе с usage sites.

## Статус файлов

| Original SourceFiles/ | h | cpp | Data-фаза / остаток |
|---|---|---|---|
| data/data_group_call | partial (328) | partial (1159) | все поля/флаги/TL, вектор участников/SSRC/video-data/min-volume/mute, count, speaking/access, версии/очередь, reload/page/unknown, реальные Peers и GroupCallKey; actual registry/Calls engine9/13/29, ApiWrap26 — effects/contracts |
| data/notify/data_notify_settings | partial (174) | partial (744) | DefaultNotify helpers/service, defaults/inheritance, known flags, exceptions, request/dedup/Sender bridge, unmute Timer, ringtone leases; real Topics08/History18, loader3/4, Window5/API26/Session29 — boundaries |
| data/data_media_preload | partial (83) | partial (209) | photo metadata controller/leases/Should, video prefix/parts/request state/serialization/cancel, exact queue/cache effects; actual DownloadMtprotoTask/loader/cache delivery3/4 and pixels5 deferred |
| data/data_statistics_chart | partial (91) | partial (166) | all chart/line/graph scalar fields, numeric measure/indices/lookup, SegmentTree; QColor/colorDark, localized/local-time labels/font metrics5 |
| data/raw/raw_countries_bounds | done (23) | done (193) | literal173 entries, exact native double values and key lookup |

Target ledger: **2 done /8 partial**. Additional necessary dependencies:
statistics/segment_tree.{h,cpp} **done** (71+151), now tg-core/stage4;
statistics/statistics_data_deserialize.{h,cpp} **partial5** (21+183), now tg-core4.
This actual parser is outside requested data_statistics_chart.cpp. Pure used
Calls VideoParams helpers, ApiWrap notify request dedup subset and streaming
SerializeComplexPartsMap are accounted separately; full Calls/API/Reader remain
partial9/26/3–4. No ownership of those entire subsystems is claimed.

[Function map](../../crates/tg-core/docs/data-calls-notify-map.md): **376 rows,
343 done /33 partial /0 pending**. Header declarations and cpp definitions count
separately. Each done has a test reference verified by inventory_map.py; partial
has a named future package. Nested PendingVal bool, chart/graph fields and backend
bodies have explicit rows. No todo!/unimplemented! or fake production model.

## Ownership, effects and order

GroupCall uses SessionScopeId/GroupCallKey/PeerKey + existing Peers, including
actual Chat/Channel migration checks and call-not-empty flags. The model does not
sort participants: append, update in place, erase preserves source order. Registry,
self-user, current call engine facts, processUsersChats, discard and forwarding
are narrow **required** owner contracts. Discard must enqueue a same-call-key-guarded
clearGroupCall mutation on main29, then forward the discarded update to Calls9;
the retained peer/key/data effect carries both phases without a fake registry. Real User/Session/Calls/History records
are not replaced. Concrete Sender adapters send actual schema requests/cancel and
handle actual request IDs/done/fail; tests invoke real Instance callbacks without
network. The application must own model lifetime and deliver replies on its
owner/main queue; no synchronously completed fake backend is shipped.

All model borrows end before fire/effect or cross-record access. Important source
reentry: setAbout fires Changes synchronously; source copies the participant
snapshot only AFTER setAbout. A regression test reenters applyLastSpoke through
real Changes and preserves that in-place speaking mutation. Reentrant erasing of
a participant from that C++ callback invalidates its iterator; such source UB is
not advertised as safe native parity.

NotifySettings takes real Peers/Documents. Thread targets are PeerKey/TopicKey;
Topic settings/lifetime/forum enumeration, History mute lookup/mutation and
Window notifications remain required backend08/18/5, not dummy History objects.
Default/peer/topic mutations, fallback precedence, known/unknown bits, delays,
notification effects and their order are implemented. Actual ApiWrap delayed
update transport/coalescing is26; exact UpdateDelayed target is retained. The
service request dedup portion follows actual apiwrap.cpp; it is not a fabricated
server. Ringtones retain actual DocumentMedia leases/forceToCache and exact
SaveRingtone effect (FileOrigin::Ringtones/empty destination for future loader).

Timers use real tg-base Timer/TimerHost with runtime MainQueue and ManualClock;
no sleep or wall-clock test delays. Exception realtime fire follows setting the
scheduled bit, main-queue flush clears all scheduled bits before fire so a
reentrant update schedules the next turn. Model drop destroys lifetimes/timers;
owner callback guards/queue routing remain explicit requirements.

PhotoPreload loads before subscribing, checks actual media lease loaded through
required image service, cancels on drop. VideoPreload snapshots original location
and full size, reevaluates Can/prefix on load, tracks requested parts and emits
exact cache/queue/cancel calls. Complex parts map framing is explicit LE. Actual
DownloadMtprotoTask scheduler/network/cache weak callback→on-main delivery stays
3/4; decoding/loaded pixels5. No simulated network/loader state substitutes it.

Chart numeric measure returns DayLabelsRequest (start/step/count); UI5 supplies
real localized/local-time labels and measures font width. daysLookup is cleared
on measure, numeric extrema are not reset. color_spec stores the captured source
color input plus colorKey; QColor conversion/theme colorDark remains5. There is
no fake QColor/font/label formatting. StatisticalGraph carries chart/token/error.

## Native verification and differences

[Reproducible recipe](../../crates/tg-core/tests/reference-tools/data-calls-notify/README.md).
Build `/tmp/ayugram-data-calls-notify-native`, real Qt6.11.2/GCC15.3/lib_base.
**400 rows**, two identical captures, all compared by Rust:173 countries,120
SegmentTree ranges,42 index cases,30 JSON cases/31 lines,4 streaming maps.17
source hashes verified. Actual country/segment cpp unchanged; exact numeric
chart fragments and entire original JSON parser body; actual streaming helper.
No algorithm body is substituted and reported as proof. Floats exact bits except
NaN payload/sign (both NaN accepted). UTF8 BOM accepted by Qt and ported.

All4 full source probes (GroupCall/Notify/MediaPreload/Chart) with real stdafx.h,
CRL_USE_QT and Qt/UI/TL/storage/OpenGL/openssl headers stop at missing generated
`emoji.h` in lib_ui/ui/emoji_config.h. No fake UI/owner headers. Native therefore
does **not** prove full participant state/order, notify service/Session state,
preload scheduler/cache, renderer colors/font/local-time. These are own tests +
full reading of applicable source bodies; the future backend itself is partial.
GroupCall has no pure standalone sorting helper; replacing applyParticipantsSlice
with a std::vector harness would not be a native proof, so none is claimed.

Preserved original behavior (including surprising cases):

- left decrements server count even if absent; just_joined increments even if
  already present. Min updates preserve old volume/muted-by-me rules; local
  constructed participant update preserves old video pointer. Additional audio
  SSRC reads screen only. Reused empty video half may retain endpoint/pause.
- SSRC first-emplace wins, but removing a colliding later peer erases that key.
  participantSpeaking fires before state mutation and can fire without change.
  Active timers use6000ms and in-call clamp1000ms; sound threshold is1500ms.
- Conference stale access can fire twice (internal variable subscription plus
  explicit check). Initial conference version2 permits equal queued participant
  update. Gap reload3000ms, stale reload strict >16000ms; reload first drains
  queued events without version filtering. Unknown batches take highest50 SSRCs;
  unconditional erase in reply may discard a newly queued update for same key.
- Unknown mute returns true with changesIn0; Saved Messages sound ignores own
  override while soundUnknown still reads it. Defaults are known by optional
  presence, including false/zero. Millisecond wake + integer Unix seconds can
  keep loaded History muted up to999ms past deadline after an earlier wake.
- Notify exception request IDs never reset (including failure). API request
  dedup defaults use real User1/Chat1/Channel1 keys, colliding with those peers;
  response is applied before erasing the pending request key. Ringtone pending
  IDs retain duplicates; list subscription lives until next update.
- MediaPreload callDone is repeatable. Source feedPart can finish when inflight
  empties **before requesting final part**, serializing an empty final slot;
  readyToRequest then still returns true. A trailing0 marker is added if encoded
  map length equals original full size. All preserved and tested.
- SegmentTree <30 max scan starts0; full-tree negative range may stay negative,
  but partial query combines an outside0. Pending change writes only first
  original-array slot. Chart extrema accumulate across measure; duplicate x
  retains NaN/Inf; measure runs after each JSON column, preserving column order.
- Countries Lat/Lon fields actually contain lon/lat, including RU minLat=-18.0.

Defined differences/limits: allocator/raw-pointer/unordered-container iteration
becomes deterministic typed-key order for muted sets/speaking deadlines/exceptions;
it is not claimed to reproduce arbitrary C++ allocation order. Nonfinite chart
index searches retain source comparisons and can hang, so native avoids them.
Reversed tree ranges for>=30 items/source signed overflow/invalid iterator
reentry/QString lone surrogates are outside proved input domain. Rust segment
sum arithmetic wraps explicitly, C++ signed overflow is UB. Invalid indexing or
missing required model records diagnose a precondition rather than invent data.

## Checks and integration

25 new tests:23 own integration +1 native +1 SegmentTree unit. New dependency:
**serde_json =1**, per PLAN QJson replacement; already present in Cargo.lock,
only tg-core dependency edge added, no version changes. Native-only CMake/Qt are
not production dependencies. Claude files and all foreign worktrees untouched;
known runtime ObjectAsync::drop race remains Claude-owned and is not fixed here.

Final single workspace run, all PASS:

- `nix develop --command cargo fmt --check`: exit0.
- `nix develop --command cargo clippy --workspace --all-targets -- -D warnings`: exit0.
- `nix develop --command cargo test --workspace`: exit0, **893 successful results**
  (892+1 child),0failed/3existingignored. tg-core248 including4 compile-fail docs.

Executed together using PORTING.md command; log
`/tmp/data-calls-notify-workspace-checks.log`.25 new tests. Full workspace tests
run once after code completion; no follow-up full run. Known runtime race test
passed, code untouched. Narrow checks only during implementation. Source hashes,
repeated native captures, map/test references and staged diff checks PASS.
Accounting completed before commit/report. Integration is only this own branch
by fast-forward into main; atomic push main+port/data-calls-notify to gramtele,
without force. CI is observed separately after push; local PASS is not a CI claim.
