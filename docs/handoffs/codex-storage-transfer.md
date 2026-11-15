# Storage transfer — Codex handoff

Base gramtele/main141b422, own branch port/storage-transfer and worktree
/home/yarik/Projects/ayugram-rs-wt/storage-transfer. Original0f2a31bb stayed read-only.
Claude lang/countries/credits/tg-ui/codegen/runtime/mtproto and foreign worktrees
were not changed/integrated. localimageloader/storage_media_prepare remain excluded.

Four requested files,2837LOC, fully read with callers and actual Sender/Instance,
MTP loader/base loader, TL location/reference and original preparation boundaries.
All four ledger rows remain **partial** for concrete application/worker ownership
listed below; no fake ApiWrap/Session/History/FilePrepareResult was added.

Implemented:

- DC priority queues, exact LIFO ties/reprioritization/200ms generation, positive
  highest-priority blocking, least-loaded session choice,128KiB request accounting,
  adaptive4..16part windows/1..8sessions, success/overload/timeouts/normalization,
  request cancellation/reassignment,15s+5ms idle stop and retained session count.
- DownloadMtprotoTask owns the actual existing MtpFileLoader: start/local-cache
  completion/partial resume/queue/cancel hooks feed the manager; normal/web/CDN
  responses feed real FileLoader memory/files/cache effects. FILE_REFERENCE_* uses
  real UpdatedFileReferences document/photo keys, retries the same offset with a
  new request ID, refuses unchanged reference. Stale callbacks after close ignored.
- CDN redirect resends all outstanding requests, origin-DC hashes/reupload,
  32-byte key/16-byte IV validation, real AES-CTR at offset>>4/BE low32 counter,
  SHA256 whole-part verification, unchecked-part ordering/late hashes, first hash
  wins, reupload needed and TOKEN_INVALID fallback, no-progress hash reply failure.
  Removing the last session remaps unchecked parts and reschedules hash queries.
- Uploader actual byte/path reads,32/64/128/256/512KiB parts,>30MiB big RPCs,
  small-file MD5/TL file+thumb, eight-session balancing,512KiB pacing/200ms,
  fast/slow8s removal/pending resends, pause/unpause/cancel/failure/deferred send,
 15s stopping, two preparation queues, late-cancel temp-path cleanup. Worker is
  explicitly external. TL attached stickers/force_file/filename and cover metadata
  preserved. Video-cover ID supplied by real owner; actual MessagesUploadMedia
  Sender adapter returns InputPhoto to the waiting uploaded document.
- Real typed Sender adapters for upload/getFile/getWebFile/getCdnFile/hash/reupload
  and cover methods; actual request IDs/shifted routes, cancellation/stop/kill.
  No fake RPCs or new event loop. Instance default flood/backoff/FILE_MIGRATE retained
  rather than duplicating transport retries. TransferNotices polls actual Instance
  restart/nonpremium subscriptions; virtual clocks passed explicitly.
- Separate real tg-rpl UploadEvents/DownloadEvents and explicit continuation effects.
  Apply media mutation, release store/model borrow, then publish RPL; continuation
  effects resume only after callbacks. Ready/failure/unchecked-hash feeds also yield
  at callbacks so a subscriber may cancel/destroy/pause before further work.
- Concrete apply_document_upload_effect/apply_photo_upload_effect mutate genuine
  PhotoData/DocumentData uploading_data/size/progress/failure state. They are used
  within the existing stores' edit closures, without inventing a Session.

Dispatch contract (package29 must integrate, not rewrite the machines):

1. Call a transition with virtual/current monotonic now on the owner queue; take
   its effects; release mutable borrow.
2. For each effect in order apply actual store mutations/network/worker control,
   then the relevant RPL notification. Callbacks may reenter the owner.
3. apply_continuation(effect,current_now) resumes the C++ frame. Drain nested
   effects depth first before the next outer effect (tests demonstrate this).
4. next_deadline arms the owner runtime Timer; has_deferred_work posts advance_to
   for the next owner turn. No sleep, extra event loop, or background clock here.
5. Explicit close_task/close/clear must dispatch cancellation/stop effects before
   actual owner destruction. Dropping the state itself cannot execute an unowned
   Sender. Audio album thumb needs real Documents InputDocument; missing input
   returns false rather than constructing a guessed request.

Coverage/accounting:
[map](../../crates/tg-core/docs/storage-transfer-map.md): **248rows =212done /
36explicit partial /0pending**, including246 declarations/bodies of the four
requested files and two required image-location TL functions. Every row has a test.
26 new integration tests; all original four files have no dedicated upstream tests.
The ten previous constructor/destructor rows in storage-files-map now name only
Session29: DownloadMtprotoTask/manager queues/CDN/Sender3/4 boundary is closed.
That previous map remains310/227done83partial because concrete lifetime ownership
has not been falsely marked done.

Native:
[recipe](../../crates/tg-core/tests/reference-tools/storage-transfer/README.md),
separate /tmp/ayugram-storage-transfer-native. **76rows repeated byte-identically**,
4 source SHA256s:63 cases execute the complete actual original checkCdnFileHash
body against real lib_base/OpenSSL/Qt bytes;13 actual Entry part-layout body cases
include ushort truncation through1TiB. Rust compares exact SHA256 bytes, decisions,
part sizes/counts. Entire original state classes cannot honestly link without real
ApiWrap/Main::Session/Data/History/images/encoding. No method bodies were replaced:
state/queue/virtual timers and actual Sender callback integration are **own tests +
reading**, not native whole-class proof. No network/user tdata/session touched.

Preserved original quirks:

- download_manager_mtproto.cpp:135 passes raw GetDcIdShift (16+session) to a
  0..7 index check: restart notices are ignored. Direct slow-request timeout path
  still removes sessions. Source link/comment and test retain this behavior;
  this quirk was read/tested, not claimed native state-machine verification.
- file_upload.cpp:843 omits Request.part for sliced parts: initial RPC uses the
  right part, but a resend after removal uses0. Preserved and regression-tested.
- docPartsCount truncates to ushort before<=4000; final512KiB fallback ignores
  failure. Native-tested.>30MiB big threshold is strict; sliced capacity rejects
  exactly1MiB while document capacity permits equality.
- CdnFileHash.limit is stored but ignored, duplicate first hash wins and hashes
  survive changed CDN params. Source rejects empty/nonmatching-needed hash replies.
- Source uploader has no own RPC retry: a delivered final partFailed fails the
  whole file. Sender/Instance absorbs default flood/migration before that callback.
- Idle stop retains download-session vector length; destructor only stops already
  due idle DCs, not all active sessions. Explicit shutdown follows that distinction.

Explicit partial / not verified:

- 26/29 ApiWrap/Main Session bootstrap, automatic Timer/notice dispatch, reference
  refresh via actual ApiWrap, weak owner/drop guards. All real network/control
  adapters and post-refresh transition exist, not fabricated refresh responses.
- 18–20/26 HistoryItem send-action/progress/repaint/failure bindings and actual
  send/edit API subscriptions; owner retains real SendOptions/edit metadata keyed
  by FullMsgId. No substitute options/history type.04/05/29 actual FilePrepareResult
  model/TL/image/cache/local file projection and thumbnails/save-path policy;
  real upload-state mutations are already implemented separately.
- 3/4 localimageloader/archive job wiring and5/7 actual encode/transcode workers,
  fallback layer filtering/cancellation/progress throttle/UI limit/toast, prepared
  file temp-path ownership; real WriteArchive already exists from storage-files.
  PreparationFinished changes real model size/upload state; actual setLocation /
  setDataAndCache is still its owner effect application. No fake job execution.
- Actual MessagesUploadMedia peer/local message ID resolution comes from owner29;
  Sender transport and cover machine are complete. Future streaming loaders7
  need their real task adapter, current DownloadMtprotoTask uses real MtpFileLoader.
- No real Telegram network/CDN integration test, whole-class C++ equivalence or
  Session lifecycle proof claimed. Safe stale-callback no-op replaces unsafe
  lifetime/assert failures. Event scheduling is explicit via documented continuations.

Dependencies: **none added/changed**, manifests/Cargo.lock unchanged; existing
AES/SHA256/MD5, tg-rpl/runtime/mtproto/scheme are reused. No unsafe or TODO stubs.

nix develop: cargo fmt --check PASS; cargo clippy --workspace --all-targets
-- -D warnings PASS; cargo test --workspace PASS:994 successful (990 regular +4
isolated child results),0failed,3existingignored.26 new integration tests. One
completed final workspace run: /tmp/storage-transfer-final-workspace-checks.log.
An earlier attempt stopped during clippy before any tests for the final Source
Queue generation invariant (-1); final checks include that assertion/test update.
Native76 repeated identical/4source hashes, map248/212done36partial test refs and
ledger4partial, unchanged dependencies/lock, diff --check PASS. Known runtime race
passed locally; tg-runtime untouched. Accounting updated before commit/report.
CI monitoring/restarts are left to the user as requested.

Superseding preparation boundary (storage-prepare): see
[codex-storage-prepare](codex-storage-prepare.md). Actual encoded/scalar
FilePrepareResult and strong PreparedUpload lease now exist, archive entries are
gathered and actual archive worker can run; uploader owns temp path cleanup through
that lease. Missing preparation3/4 is removed from four map rows; actual image/
encode/editor5/7 and MainRef worker dispatch29 remain explicitly partial.
