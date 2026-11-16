# Data downloads handoff

Branch `port/data-downloads`, own worktree `ayugram-rs-wt/data-downloads`, base
`7dbd935`. Read-only original AyuGramDesktop
`0f2a31bb420de98f331990432265b131b8d836b5`.14 requested files4263LOC, all file-level
partial for explicitly absent owners/UI. [Function map](../../crates/tg-core/docs/data-downloads-map.md)
**378 rows =205done+173explicitpartial+0pending**. Header declarations and body
entries are counted separately, with actual fields/defaults/projections/dependencies.

| Requested originals | Core behavior present | Remaining actual binding |
|---|---|---|
| data_download_manager.h/.cpp (1660LOC) | scoped queues, progress, external cancellation, visibility,5s clear, reverse resolve batches100/channel grouping, exact list codec/weak serializator, all-row detach/delete, UTF16-path dedup, real filesystem export whitelist/delete helper, direct FileLoader bridge |16/18–20 real History/nonhistory message factory, IV/webpage membership;29 Session/Account/local-storage subscriptions, reply processing, current wire-ID/sessionUniqueId descriptors and async worker completion;5 download bar/confirmation |
| data_document_resolver.h/.cpp (416) | extension/MIME/IP warning decision, verified override, extensionless OpenWith, real64MiB gate, image source precedence, media/theme/launch/save routes, AyuGram plugin gate |5/6 QImageReader/access guard/plugin metadata+box/Markdown/Window;7 player/TTL/image codecs;33 actual MIME/platform launch;18/29 owner |
| data_file_click_handler.h/.cpp (438) | real document/photo/peer keys/context, save modes and deferred decision, saveFromData/new-file rules, cancellation/upload callbacks, OpenWith fallback, photo save gate |5 actual ClickHandler hierarchy/property/wrapped handlers/tooltip;18/29 save/load/tracking delivery;33 save dialog/file launch |
| data_send_action.h/.cpp (234) | history/root-key registration, self exclusion, madeAction-before-start, dedup/start time, animation callback retention and update/speaking streams, clear |16/18 thread/history;5/7 actual SendActionPainter, weak painter keepalive, Ui::Animations driver, typing/status deadlines and animation phases;29 owner |
| data_search_calendar.h/.cpp (489) | actual typed request, newest/deferred months, offsets/pagination/repeated-offset stop, authoritative period maxima, first eligible media/day, coverage cache, failure cache, callback reentry |18–20 actual History media lookup;29 processUsers/Chats/Messages/typed reply delivery;33 real local timezone/DST Calendar adapter;5 DynamicImage thumbnails |
| data_ai_compose_tones.h/.cpp (493) | metadata/flags/input TL, refresh hashes/full-over-incremental coalescing, hourly TimerHost, create/update/save/remove/resolve, recent ordering, typed Sender, full RPC error propagation |01/question1 internal AiComposeToneExample/ParseTextWithEntities (raw original TL preserved);29 Session lifetime/users/callback delivery |
| data_community.h/.cpp (533) | real Peers link IDs, visibility, membership sets, date maximum,20 newest histories, ordered model effects/signals, old-list collapse planning and one cached flag flip |18 real History/Dialogs::MainList/row lifecycle;29 subscriptions/owner;01/question1/lang/5 marked preview/Text::String cache |

No new fake Session/History/Painter/QImage/ClickHandler body, internal
TextWithEntities model, CreditsAmount clone or no-op trait implementation.
Calendar is a mandatory service boundary like existing birthday code, with no
production UTC default. Tests use actual libc UTC conversion, without changing
TZ or touching the user's session. UI thumbnails are explicit MonthDay descriptors,
not a fabricated DynamicImage. SendAction timing is owned by the original
history/view/history_view_send_action painter, outside these data files; deadlines
and animation constants are not guessed in the manager.

The queue uses real MessageKey generations/SessionScopeId and stable document/
photo record keys. Source History getters enter as explicit facts/keys; admin-log
cancellation is never guessed from a fake item. FileLoader is the existing storage
implementation; document progress index membership retains the original duplicate
behavior. Generated/nonhistory HistoryItems are requested as effects requiring
real future model keys; none are invented. Full restore resolution requires the
real History owner before GenerateEntry/Regenerate can be completed.

Community mutations return ordered effects before RPL publication. Collapse is
two phases: plan removals while cached collapsed still names the old list, release
borrow/apply actual old-list removals, then flip once and return reindex/existence
and repaint effects. Linked changes are distinct from refreshed notifications.

Native [recipe](../../crates/tg-core/tests/reference-tools/data-downloads/README.md):
standalone `/tmp/ayugram-data-downloads-native/native_downloads`,164 repeated exact
rows/6source hashes. Genuine DownloadedId/MsgId/PeerId declarations and projected
original QDataStream writer/reader statements are compiled, with no substituted
Session/History body. Original→Rust verifies all codec canonical bytes/fields and
TimeId narrowing. Rust→original:4 frames accepted/read/written back byte-identically,
including unpaired UTF16 and quint32 truncation; inverse fixtures asserted in Rust.
Whole model graph/real UI/media/timezone are not native-built: own tests + reading.

Twenty-three new integration tests cover the codec/corruption/whole-list rejection,
virtual-time clear/reload, progress/visibility/external cancellation, callback
reentry (including newly added work during stop), exact first-item external lookup,
multiple files/message/path dedup, deferred months/period IDs/pagination/failure
cache, flags/recent-tone ordering and actual Sender/Instance memory-server TL
success/error replies for tones/calendar/messages. File deletion tests use only
fresh synthetic temporary directories; actual tdata is never opened.

Preserved source quirks/explicit representation boundaries:

- No invented version/tag: count then u64 object ID/i32 type/i64 started/u32 size/
  u64 peer/i64 message/u64 accessHash/QString, BE Qt_5_1. Size truncates to32bits
  on write. Reader rejects entire list for one bad row/type/empty path/zero or
  >4000MiB size, bounds count1..99999, ignores trailing bytes. Raw nullable UTF16
  is preserved; display/filesystem conversion of malformed surrogate paths and
  Qt-specific filesystem edge cases are not native-verified (33).
- DateFromDownloadDate returns32-bit TimeId after division; zero-time first start
  is1ms, backward clock changes reset the counter. Resolve scans backwards in
  batches100, groups nonchannels under self, treats size changes as deletion,
  preserves sticky loadedResolveDone and delegates actual generation to18.
- Delete removes every matching downloaded row, deduplicates physical paths by
  UTF16 code-unit order, and writes once/session. deleteAll retains the source's
  double postponed write. Stop/clear/untrack use live reverse loops, including
  reentrant additions, not a fixed snapshot. External lookup checks the first
  matching item before testing its cancellation marker.
- Removing one document progress entry clears that document's set membership
  even if another download of the same document remains. This source behavior
  is retained and tested, not silently repaired.
- Calendar periods provide maxMsgId independently of thumbnail origin; cache is
  not sorted, failed months become loaded, repeated offsets stop pagination.
  Older deferred months can be filled by a newer response. Local-calendar/DST
  application and real message preprocessing remain explicit33/18/29.
- Verified peers bypass launch warnings; extensionless files always use OpenWith;
  actual MIME and image-readability facts are never replaced by extension guesses.
  Plugin metadata/pixels/launch/dialogs remain actual backend work.
- Tone update Some(false) sets display_author presence; full pending refresh wins,
  recent custom ordering survives refresh, default tones are not promoted, delete
  failures are silent as in C++. Examples are retained only as existing wire TL,
  not an internal entities stand-in. RPC error code/kind/description stay intact.
- Cross-session/history set iteration and equal-date ties use stable scoped key
  order rather than C++ pointer-address order. UTF16 path order remains exact.

New dependencies: none; manifests/lock and protected Claude areas (tg-ui/codegen/
runtime/mtproto/lang/countries/credits) unchanged. Prior ba1c576 private Xvfb-noreset
and ef615ab compactor check fixes are inherited. CI monitoring/retries remain with
the user; the known runtime thread-drop race is not altered.

Final checks: nix develop: cargo fmt --check PASS; cargo clippy --workspace --all-targets
-- -D warnings PASS; cargo test --workspace PASS:1093 successful (1089 regular
+4 inherited isolated child results),0failed,3existingignored.23 new tests.
One completed full final run: /tmp/data-downloads-final-workspace-checks.log.
An earlier workspace attempt was stopped during clippy before tests for deletion
fidelity fixes; the final run includes all subsequent queue/callback/collapse and
full RPC-error changes. Final focused clippy/23tests also PASS. Native164 repeat
rows/6source hashes +4 inverse original-reader byte-exact frames; map378/205done/
173explicitpartial links/test refs, ledger14/4263, scope/dependency/diff audits PASS.
Actual private XCB and known runtime thread-drop test passed locally; protected
crates unchanged. Dependencies/lock unchanged, ba1c576/ef615ab retained. Accounting
completed before commit/report. CI monitoring/retries are left to the user.
Accounting precedes commit/report. Integrate only this branch via main fast-forward,
then atomic push main+port/data-downloads to gramtele, without force.
