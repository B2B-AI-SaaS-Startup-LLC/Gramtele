# Storage prepare — Codex handoff

Base gramtele/main ef615ab (includes ba1c576 Xvfb -noreset and ef615ab cache
wait_compact fix). Own port/storage-prepare and worktree
/home/yarik/Projects/ayugram-rs-wt/storage-prepare. Original0f2a31bb remained read-only.
Claude tg-ui/codegen/runtime/mtproto/lang/countries/credits and foreign branches
were not changed. No new X server; inherited private X11 tests retain -noreset.

Four requested files2336LOC were fully read with relevant call sites and required
attach_prepare/SendFilesWay/MD5 dependencies. At this pin the actual names are
FilePrepareResult/FileLoadTask and Api::SendConfirmedFile. FileLoadResult and
SendMediaReady do not exist; no names/types were fabricated for them.

Implemented:

- Exact pure MIME/extension probe gates and Song→Video→Image/Lottie order;
  duration/thumbnail acceptance, path .mp4 MIME normalization, image metadata.
  Photo-side1280/2560, aspect ratio≤20, rounded320 thumbnail size,5MiB thumbnail
  upload gate/known MIME table, Mp4FileName,4bpp JPEG keep/progressive/reencode
  decision, sticker2MiB/dimensions and video conversion/1000MiB/force/GIF gates.
- Actual FilePrepareResult encoded/scalar fields, TL photo/document/attributes,
  filename/size/memory/path/voice/waveform/song/video/round/sticker/animation/photo
  logic, output photo sizes, fixed32KiB photo/thumb parts and lowercase MD5,
  exact append/no-clear behavior. Actual document part_layout reused unchanged.
  Typed Needs returns explicitly when real inspection/image/video work is absent;
  it does not produce an invented decoded result or pretend the file is ready.
- PreparedFile/PreparedList/SendFilesWay data and predicates, actual filesystem
  validation/error ordering/callback continuation, URL/locality facts, premium
  limits,10 inspected slots plus deferred files, album membership/group ranges/
  splitting/slowmode/GIF exceptions/caption/price/spoiler/HD options and bundles.
  Real UI/pixel/editor fields stay absent, rather than using empty fake images.
- TaskQueue is a persistent serial blocking worker on existing tg-runtime's pool
  (no new runtime/event loop), FIFO task ownership, actual process()/finish(),
  cancellation pending/running/finished tasks, worker stop/wait/restart, optional
  virtual idle deadline. Real MainQueue bridge, weak queue guard; finish executes
  on owner outside mutexes and permits reentrant add/cancel. No ObjectOnThread /
  ObjectAsync drop path or its known race used/changed. Timer bootstrap remains29.
- SendingAlbum item keys/random IDs/media/music batching/readiness/sample/take,
  removal and caption-transfer target; actual TL InputSingleMedia accepted after
  real owner normalizes text/entities. Caption normalization is not faked.
  // AyuGram reverseLocalPremiumEmoji is explicitly retained at that boundary.
- Real archive gather/estimate/write backend reused by FileLoadTask and
  write_archive_preparation; actual archive result feeds Uploader finish_preparation.
  FilePrepareResult→PreparedUpload carries actual fields and a strong preparation
  lease. Uploader records generated temp path in that real result, last lease
  deletes it after the terminal callback, as original Entry scope. Cancellation and successful transfer retain original content lifetime.

Coverage/accounting:
[map](../../crates/tg-core/docs/storage-prepare-map.md):262rows =151done /
111explicit partial /0pending. Header declarations and bodies counted separately,
all four storage files plus required attach/flags and MD5 slices; every row names
an existing test and every partial a future package.19 new tests, original pinned
files have no dedicated upstream tests. Four target ledger rows partial; two
attach_prepare files partial5; two pure SendFilesWay files done, value owner moved
from tg-app-ui5 to tg-core3 (no UI crate code edited). Core utils stay partial4.
Previous storage-transfer four constructor/runTranscode declaration/body rows now
remove preparation3/4 addresses: actual result/archive are here; pixel5/7 and owner29
remain. Previous map248/212/36 counts unchanged, not falsely marked fully done.

Native:
[recipe](../../crates/tg-core/tests/reference-tools/storage-prepare/README.md),
CMake outside clone,160rows repeated identical/10source hashes. Complete actual
original MD5 implementation, raw/hex helpers and exact setFileData/setThumbData
bodies; compare whole/stored MD5, part count/length/every-part digest and thumb
bytes through repeats/empty writes and padding/splitting boundaries. Per-part
MD5s are diagnostic comparison, Source has no separate per-part hash wire field.
Additional pure thumbnail/name/dimension/group/side cases. Document part_layout
uses already accepted original13 layout native cases from storage-transfer.
All other machines are own tests + source reading, not whole-class native proof.

Preserved Source quirks:

- More than10 files are deferred, not rejected with a nonexistent TooManyFiles error.
- setFileData(empty) resets only partssize; old fileparts/filemd5 survive. Nonempty
  setFileData/setThumbData append parts; empty thumb call changes nothing.
- Reordered/mergeToEnd/DivideByGroups intentionally discard deferred files and/or
  override metadata exactly as Source functions do. Consecutive None group items
  can share one None range; that is not a Telegram album. Empty spoiler predicate true.
- GroupTypeForFile can send videos as File when uncompressed; PreparedFile::albumType
  still returns PhotoVideo for video. These distinct functions are not conflated.
- Source MTP_document uses flags=0 even though a thumb vector is passed to the
  constructor; the actual C++ generator's conditional getter hides it and write
  omits it. Rust uses flags0/None, keeping thumbnail dimensions/encoded bytes
  separately. Regression checks TL flags/wire; this is own test + generator reading.
- Local isAnimation is copied only in existing-file branch, not memory image branch.
- Source4bpp JPEG threshold means bytes≤width*height/2, format comparison is exact
  lowercase jpeg. Transcode source size is strict<1000MiB; sticker size strict<2MiB.
- Source PreparedFileFromFilesDialog calls PrepareMediaFromImage with null QImage
  for remote content, although the latter Expects(non-null image). That Qt-path
  inconsistency is recorded, not silently fixed or claimed implemented.

Explicit remaining partial / unverified:

-5 actual QImage/QPixmap storage, image/Lottie read, opaque/scale/blur/preview,
  JPEG/WebP/PNG writer and progressive rewrite, theme preview, geometry/paint/
  attached-sticker scene traversal, PhotoEditorMedia/file-dialog/drag UI. Pure
  metadata uses real unmodified geometry facts; modified preview sizes need Editor.
-7 actual audio/video inspection/frames/transcode/animation jobs and editor VideoSource.
  FileLoadTask emits Needs on absent required codec output; no backend/default fake.
  EncodedPhoto/EncodedThumbnail are actual backend byte/size inputs, not encoders.
-33 actual MIME registry/sniff and Qt URL/path/local-codepage/default timestamp-name
  services; actual path/MIME/generated name supplied by owner. Non-UTF8 path parity
  not proved by lossy display strings. Filesystem validation itself uses real files.
-01/18–20 captions, FullReplyTo/History caption trim + Ayu reverseLocalPremiumEmoji /
  EntitiesToMTP, actual send options;26/29 real weak Main Session/DC/context,
  SendConfirmedFile/error UI, album registry and owner worker/timer scheduling.
  FinishOutcome is an actual decision/effect boundary, not an ApiWrap substitute.
- Shared post-encode size/bytes/path are real mutex-backed fields with getters;
  Uploader finish_preparation updates them at the original callback boundary.
  Archive entries are consumed once, and generated temp path is retained until
  the last prepared lease drops. Actual document cache/location effects remain29.
- Qt processEvents nested dispatch in original worker is not implemented without
  actual Qt event loop5. Rust processes jobs serially on a real persistent worker;
  owner MainQueue dispatch is real. No whole Qt/codec/Session lifecycle proof claimed.
- Safe IDs replace task pointers; hostile dimension arithmetic uses i64 to avoid
  C++ signed-overflow UB; invalid duplicate reorder indices are refused rather
  than reproducing moved-from element access. No normal-input wire changes intended.

No new dependencies/manifests/lock/unsafe; existing MD5/media/rpl/runtime/scheme and
actual archive writer reused. Pixel/codec tests are explicitly absent, not mocked.

nix develop: cargo fmt --check PASS; cargo clippy --workspace --all-targets
-- -D warnings PASS; cargo test --workspace PASS:1013 successful (1009 regular +4
inherited isolated child results),0failed,3existingignored.19 new tests. One
completed final workspace run: /tmp/storage-prepare-final-workspace-checks.log.
Earlier attempts were interrupted for actual TL flags/shared-state/callback-lock
corrections; some had reached cache tests, no failed test results were hidden.
Final run includes all fixes and the reentrant archive progress regression.
Native160 repeated identical/10source hashes, map262/151done111partial refs and
ledger/scope/diff audits PASS. Dependencies/lock unchanged; ba1c576/ef615ab preserved.
Private XCB test and known runtime race both passed; no tg-runtime edits.
Accounting finalized before integration/report; CI monitoring is left to the user.
