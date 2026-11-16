# Codex data-user handoff

Worktree `/home/yarik/Projects/ayugram-rs-wt/data-user`, branch `port/data-user`,
base `a236ae1`. Read-only original `0f2a31bb420de98f331990432265b131b8d836b5`.
Only this branch is integrated. Claude's tg-ui/codegen/runtime/mtproto,
lang/countries and credits_amount are untouched. ba1c576/ef615ab retained.
No real tdata, user bus, DISPLAY, live server or new event loop is involved.

Requested12files4754LOC:10partial/2done. `data_groups.h/.cpp` have complete keyed
album logic with explicit view effects; User/values/filters/folder/unread files
retain the specific owners below. Additional dialogs_common.h scalar unread /
badges values live in tg-core, while its ledger crate/stage ownership stays
unchanged and the rest of the header remains its original stage6 (tg-app). Dependencies/manifests/lock
unchanged. The historical peer/group maps now point at the concrete UserData.

Implemented:

- Concrete UserData replaces UserBase in the sole Peers store. Scoped keys,
  actual self identity via new_with_self, live read/edit access and real PeerFields
  projection. Flags/contact/calls/lastseen/name/usernames/photos/birthday/premium,
  business details, counts, stars, personal channel, linked community, bot manager,
  gift restrictions, bot scalar/media-key metadata and TL apply phases. Existing
  PeerData retains emoji/photo/location state. No extra fake peer/history models.
- Peers mutation/Flags/Changes/owner-effect phases release every store borrow
  before synchronous fire. Names accept the real language formatter/search index,
  including existing Ayu Zalgo filtering and first/last-name empty handling.
  Bot media callbacks use actual stable PhotoRecordKey/DocumentRecordKey.
- Online phrase descriptors (no lang rendering), status/timeouts, pure write /
  pin/call/premium/forward/reaction/rating predicates and real tg-rpl streams over
  the existing concrete Peer/Chat/Channel stores. Topic closed/right streams have
  a narrow adapter requiring the actual future ForumTopic producer.
- ChatFilter flags/type selection, include/exclude/pins/community/unread/muted /
  archive precedence, equality excluding id/pins, TL wire-title conversion,
  ordering/limits/hideAll/suggestions, actual typed Sender requests, link model,
  tag/list/link/suggested streams, exceptions and watched more-chats lists.
  TimerHost supplies virtual time; no sleep. Received dispatches each filter's
  color phase before the next insertion, with no outstanding model borrow.
- Archive dates/pins/top-eight scan, story counters/transitions, real UnreadState
  arithmetic + archive subtraction/setting streams/badges, grouped-message
  ordering/reordering/limit/unregister/edit selection by generation-bearing keys.

Explicit remaining work (not silently marked done):

- 01/question1: internal TextWithEntities, force-static title entities, note,
  bot-verification description, language full-name/online/archive rendering.
  `wire_title` is only the already generated TL field. There is no new internal
  text/entity type and no local-to-wire entity conversion claim.
- 01 Claude CreditsAmount and05/13/14/23/25/29 owner: StarRefProgram/credits status loads.
  BotInfo contains no counterfeit CreditsAmount or forum object.
- 08/18/5: actual Forum lifecycle/icons/preload/notification UI. UserForumOwner
  requires genuine owned Taken data; it has no stand-in implementation. The
  simple set_user_flags refuses a bot Forum transition until that owner exists.
- 13/14/29 and18–20/23–28: complete ApplyUserUpdate orchestration (photos/notify /
  wallpaper/themes/business/stories/saved music/credits), messageWithPeer lookup,
  trusted payment and real Session effects. Standalone scalar/gift phases are
  named explicitly and do not pretend to be the complete update method.
- 18/5: Dialogs::MainList/Entry ownership, membership/tag-height/pinning delivery,
  full Folder subscriptions/cloud dialog/pinned update application and painting.
  RulesChanged/ApplyPinned/Folder/User/Group effects are verified boundaries,
  not proof of reentrant behavior inside an unported MainList.
- Internal peer-value FullFlags binding needs actual13/14 producers; locale/local
  calendar formatting is an owner boundary. Pure descriptors/timeout arithmetic
  are tested with explicitly supplied local-calendar facts.

Native reference: `tests/reference-tools/data-user/README.md`; separate CMake
`/tmp/ayugram-data-user-native`,183repeat-identical rows/20source hashes. The
original predicate/status/lambda/unread bodies and real generated C++ TL writers
confirm6flag vectors/108channel/24chat/24timeout/12status/8filter/1unread cases.
Full UserData/ChatFilters/Folder/Groups with Session cannot honestly be linked
without real unported History/Forum/ApiWrap/credits/text/UI bodies. They are
**own tests + reading**. Native TL samples prove their stated order and generated
format, not ChatFilter::tl's internal entity conversion or arbitrary allocations.

Preserved quirks and explicit differences:

- Channel send value ORs explicit restrictions with `(default && !boosts)`, i.e.
  bool/raw bit0, not the default mask. Native confirms it.
- ContactStatus is separate from Contact flags; Self is sticky; negative bot
  version keeps botInfo. Full-profile conditional masks retain old HasStars /
  HasRequirePremium knowledge bits when the corresponding optional data clears.
- UserData::allowsForwarding always true (AyuGram); the RPL forwarding value
  nevertheless follows the two no-forwards flags.
- Filter equality ignores id and pinned order; withChatlist clears StaticTitle;
  Chatlist TL writer omits has_my_invites; order update can emit changed twice.
- Suggested failure stores now+one hour, so the next request is due three hours
  later. Exceptions collect into the outer vector before checking100; a single
  filter can exceed100 and remains queued at the overfill break.
- Album register caps10, refresh insertion does not; duplicate identities are
  removed together. Folder story counts clamp only above65535; negatives wrap.
- Include/exclude sets use scoped key order rather than C++ pointer-address
  order. Native sample bytes agree for their specified ordering; arbitrary
  pointer-allocation byte equality is not asserted. Pinned order stays exact.
- Arithmetic uses explicit wrapping for inherited fixed-width fields, i64 for
  large online differences; real local calendar/locale behavior is not simulated.

Final accounting: [function map](../../crates/tg-core/docs/data-user-map.md)
**542 rows =380done+162explicitpartial+0pending**, including the11 actual
UnreadState/BadgesState/debug/UI dependency entries and flag-discovery glue.
All map paths/test names/future addresses,20source hashes and requested ledger
rows have been audited. No TODO/unimplemented/stub body in the new modules.

| Original pair | Status | Actual remaining owner |
|---|---|---|
| data_user.h/.cpp | partial | 01/question1/CreditsAmount,05 credits,08 Forum,13/14/29 Session,18/23–28 full update delivery |
| data_peer_values.h/.cpp | partial | 01 locale/rendering,08/16 Topic/Thread,13/14 full flags,5 userpic |
| data_chat_filters.h/.cpp | partial | 01/question1 title,13/14/18 real MainList/History,5/6 rows |
| data_folder.h/.cpp | partial | 18 real Entry/MainList/History,01/question1 +5/6 rendering |
| data_unread_value.h/.cpp | partial | 18 actual list and29 settings producer binding |
| data_groups.h/.cpp | done | keyed model/effects complete; real parent message/view delivery remains18–20 |

37newtests, including the source bot-version callback phase: cleared commands
are observable with the OLD description/version/inited before the remaining
mutation. The callback reenters Peers safely. The nullable Forum take contract
also preserves a real owned guard across Flags callbacks without a fake Forum.

Final checks (one completed full workspace run):

```text
nix develop --command cargo fmt --check                         PASS
nix develop --command cargo clippy --workspace --all-targets -- -D warnings PASS
nix develop --command cargo test --workspace                     PASS
1050 successful results (1046regular+4inherited child),0failed,3existingignored
```

Actual combined log: `/tmp/data-user-final-workspace-checks.log`.
Focused final37/37 and tg-core/all-targets clippy also PASS before the full run.
An earlier workspace attempt was stopped during TEST COMPILATION, before running
any tests, to correct the bot-version callback phase found by source review;
log `/tmp/data-user-interrupted-workspace-checks.log`. Earlier focused failures
(reference bare/boxed TL capture and an initially Forbidden channel fixture)
were corrected, not hidden. Final tests include both corrections.
Native target rebuilt and repeated:183exact rows/20hashes PASS. Whole Session
state remains own tests+reading, with the packages above; it is not native-proven.

Both ba1c576 and ef615ab remain ancestors; actual private XCB and known runtime
thread-drop tests passed. Dependencies/manifests/lock and Claude areas unchanged.
Integrate only port/data-user via main fast-forward, push main+branch atomically
to gramtele without force. CI monitoring/restarts remain with the user.
