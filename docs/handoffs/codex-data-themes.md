# Data themes handoff

Branch `port/data-themes`, own worktree `ayugram-rs-wt/data-themes`, base
`06df1c5`; original read-only AyuGramDesktop
`0f2a31bb420de98f331990432265b131b8d836b5`. User authorizes integration/push
of this branch only. All22 requested originals3756LOC are accounted for:
10done/12partial. No new dependency, manifest/lock change or edit in Claude's
lang/countries/credits_amount/tg-ui/tg-codegen/tg-runtime/tg-mtproto scope.
Existing ba1c576 private Xvfb `-noreset` and ef615ab compactor check fixes remain.

The port contains current/legacy wallpaper persistence, URL/slug/flags/gradient/
pattern/TL rules, CloudTheme values and CloudThemes hashes/pagination/update/
install/reload phases with actual TimerHost and typed Sender requests. Model
mutation releases RefCell borrows before publishing effects or RPL events.
Document/Photo/Peer references use the existing stable scoped keys; gift IDs
read the current Telegram document ID through the real store, including after
conversion. Gift values/counter merge/typed request construction, statistics,
subscriptions, game/report/colors/rotation and recent component logic are present.
No fake Session/History/Window/pixel decoder or internal TextWithEntities/Credits
clone is introduced. Location registry implements the actual weak-lookup body
as a generic registry; concrete SendAction/LocationPicker bindings remain absent.

| Original files | Ledger status | Remaining actual dependency |
|---|---|---|
| data_wall_paper.h / .cpp | partial / partial |5 thumbnail/pixel identity/local-image methods;3/4/7 document loader;29 Session binding |
| data_cloud_themes.h / .cpp | partial / partial |5/6 Window/preview/emoji-index binding;3/4/7 loading document;24 gift parser;29 subscriptions/Session/resolve owner |
| data_star_gift.h / .cpp | partial / partial |01/question1 message entities/lang rendering;24 CreditsAmount and Api::FromTL, full slice producer/owner;5 ColorCollectible/style |
| data_statistics.h | done | complete value fields/predicates, existing StatisticalGraph |
| data_statistics_lists.h | done | complete values/validity/equality/order |
| data_subscriptions.h | done | values/validity; until is optional SystemTime instant, timezone/display remains29/5 |
| data_premium_subscription_option.h | done | all original string/month fields |
| data_game.h / .cpp | done / done | owner SessionScopeId, real Photo/DocumentRecordKeys |
| data_report.h | done | equality deliberately ignores optionText/ids/stories |
| data_peer_colors.h | done | sRGB RGBA palette/bg/story fields; rendering5 outside value header |
| data_media_rotation.h / .cpp | done / done | stable media identity maps + exact set/get rules |
| components/recent_inline_bots.h / .cpp | partial / partial |29 Session +3/4 Local storage read/write effect delivery |
| components/recent_shared_media_gifts.h / .cpp | partial / partial |24 actual saved-gift parser;29 reply/users/chats delivery;01/5 error/toast rendering |
| components/location_pickers.h / .cpp | partial / partial |23 actual Api::SendAction;5 Ui::LocationPicker |

[Function map](../../crates/tg-core/docs/data-themes-map.md):410 rows =296done +
114explicitpartial +0pending. Declarations/definitions have separate rows;
value rows include fields/defaults/enums/flags/derived equality. Every row names
a real implementation or an explicit absent future body and a boundary test.
Twenty new integration tests cover fixtures/TL, virtual-time reload, callback
reentry (including a cold request at time0 waiting for its own response),
cache threshold/pin/reorder/failure latch, weak cleanup, document re-ID,
request flags/counter duplicate behavior and actual Sender callbacks without network.

Native evidence: [recipe](../../crates/tg-core/tests/reference-tools/data-themes/README.md).
Standalone `/tmp/ayugram-data-themes-native/native_themes` uses the genuine
WallPaper class, unchanged extracted pure C++ bodies, real QColor/QDataStream
Qt_5_1 and generated TL.588 captured rows repeated identically,13 source hashes.
Original→Rust:current270/read140/legacy54/truncated72 +slug/URL/factory/trailing
cases, TL settings, saved IDs/rarity/badges/attribute IDs. Rust→original:7 distinct
Rust-built papers accepted and written back byte-identically by real C++.
The inverse samples are checked in and asserted against current Rust writes.
Whole CloudThemes/Session/UI and gift parsing were not honestly assembled:
they require the actual Window/ApiWrap/Session/credits/text entities graph.
Their evidence is own tests + reading, with no substituted C++ model bodies.

Preserved source quirks and explicit differences:

- Wallpaper current reader ignores the version word, accepts trailing bytes,
  rejects persisted0xffffffff colors/counts outside0..4/intensity outside±100;
  blur is true only for raw1. Legacy owner has zero/one/two optional32-bit words.
  Null/empty and even unpaired UTF16 slug units round-trip through the raw codec.
- Wallpaper persistence does not carry emoji/document/thumbnail, like C++.
  Rotation is still serialized when >=3 colors make gradientRotation return0.
  withParamsFrom does not copy rotation. Nonpattern document key preserves
  the original `?&intensity=` parameter oddity. Present optional MTP -1 is white,
  while persisted sentinel -1 means invalid/absent under its separate branch.
- Testing theme links write `out_accent` without `=`; dark parsing ignores the
  requested `dark_` prefix; first6hex digits are consumed even with trailing text;
  zero intensity is not applied from testing links. These defects are retained.
- Gift cache request has no fail callback in the original: failure retains its
  in-flight/pending state. Pin limit1 retains one old ID plus the new ID because
  the stop check follows insertion. Ordered cache replacement does not repair
  stale `pinned` flags. Duplicate attribute counters keep the first value.
- Shared gift equality uses Rc pointer identity as shared_ptr did. SavedStarGiftId
  ordering uses scoped key order rather than C++ pointer addresses. MediaRotation
  keeps stable identity through re-ID; i64 intermediate avoids C++ overflow at
  extreme negative integers. Negative multiples remove entries before normalize;
  the original helper's invalid360 result is not exposed by set().
- RGBA is an8-bit sRGB value, not a replacement for extended QColor color spaces.
  Subscription dates preserve validity/instant, not QDateTime timezone metadata;
  locale/timezone rendering is explicitly outside this headless value contract.
- Backend application, full gift slice producers/cancellation and parser fields
  requiring CreditsAmount/entities remain partial; typed request builders and
  pure counter merge are complete. No fake callbacks claim those bindings exist.

Final checks: nix develop: cargo fmt --check PASS; cargo clippy --workspace --all-targets
-- -D warnings PASS; cargo test --workspace PASS:1070 successful (1066 regular
+4 inherited isolated child results),0failed,3existingignored.20 new tests.
One completed full final run: /tmp/data-themes-final-workspace-checks.log.
Earlier attempts were interrupted at test compilation for extra factory coverage
and during tests for the reentrant first-callback fix; no failed tests were hidden.
Final run includes both changes. Native588 repeated identical/13source hashes +7
inverse Rust→actual C++ byte-identical samples, map410/296done114partial references,
ledger22/3756 and scope/dependency/diff audits PASS. Actual private XCB and known
runtime thread-drop race passed locally; protected crates untouched. Dependencies/
lock unchanged. Accounting completed before commit/integration/report. CI is left
to the user as requested.
Accounting is finalized before commit/report. Integrate only this branch via
fast-forward main and atomic push main+port/data-themes to gramtele, without force.
CI monitoring/retries belong to the user; do not fix the known runtime race here.
