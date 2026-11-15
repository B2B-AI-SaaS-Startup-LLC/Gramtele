# Storage serialization + file download — Codex handoff

Base gramtele/main90d53cd, own branch port/storage-files and worktree
/home/yarik/Projects/ayugram-rs-wt/storage-files. Original0f2a31bb stayed read-only.
Claude lang/countries/credits/tg-ui/codegen/runtime/mtproto and foreign branches
were not changed/integrated. file_upload, download_manager_mtproto,
localimageloader, storage_media_prepare were only consulted where needed, not ported.

Implemented phases:

- Qt_5_1 BE stream reuse, raw nullable UTF16/bytes wire DTOs, peer version tag/v3,
  document tag/v6, legacy app-version gates, generic legacy and modern image
  locations (all variants/type flags/in-message fields), phone prefix peeking,
  creator packing, old flag conversions and lastseen. StoredImage preserves the
  original encoded blob and nullness rather than guessing from a model Vec.
- Real Chat/Channel snapshots/application on Peers; loaded gate/version reset,
  flag replacement/conversion, photo location conversion, existing phased name
  mutations. User wire records are complete; no fake UserData was added.
- Real Documents application after validation: original attribute reconstruction,
  sticker-set context/special IDs/custom emoji gates, duration legacy conversion,
  thumbnails/inline data, unsigned32 size, remote location and ordered effects.
- FileLoader's local/cache/partial/byte/file state, actual file read/write/removal,
  filename/cloud/load-size rules, out-of-order skipped-byte accounting, finalization,
  cache/tag/location/postprocess/terminal notification order. Actual tg-cache
  put/get adapters and separate tg-rpl LoaderUpdates endpoint; borrow released
  before callbacks. Cancel preserves the old filename through the terminal event
  and then applies ClearFileNameAfterUpdate while the owner is alive.
- Actual factory selection/constructor overrides and owning in-memory loader.
  mtpFileLoader part offsets/readiness/unknown size/last-complete conditions,
  resume by whole128KiB parts, web size validation and exact queue/cancel effects.
- Web manager8-query/current+previous generation queue,200ms reset effect with
  virtual advance, reply identity/stale events, redirect budget5, scheme checks,
  status200/206/416, length/10MiB/4000MiB limits, streamed/memory progress, cancellation
  and completion ordering. Web virtual currentOffset is reflected in the base
  progress and Error.started state. OnlySize still uses GET as original.
- Concrete reqwest client/async HTTP on the caller runtime (no new loop),
  NoLessSafe automatic redirect policy, chunk callbacks and cancellation by
  dropping the real request future. No real network was used in tests.
- Folder gather/dedup/hidden/symlink handling, two-level send listing, ZIP estimate,
  real minizip/zlib FFI with the original compression settings/UTF8 names/ZIP64
  threshold/directory attributes/dates. Streaming progress/cancel/limit/cleanup,
  disappeared-file skipping and stale-file deletion worker with injected now.
  Unix temp files0600; Rust temp names use random32hex instead of Qt's6char mask.

Coverage/accounting:
[map](../../crates/tg-core/docs/storage-files-map.md):310 rows =227done/83explicit
partial/0pending, header declarations and bodies counted separately, cpp-local
classes and14 required image-location value functions included.14 requested files,
3491LOC; all14 remain partial at file level because their client/UI/platform/owner
adapters described below are not yet complete. This does not claim the whole
file lifecycle or all serialization model adapters finished.

Native reference:
[recipe](../../crates/tg-core/tests/reference-tools/storage-files/README.md),
separate CMake /tmp/ayugram-storage-files-native.235 JSON rows repeated identically,
17 source hashes checked.48 peer,138 document,18 raw location frames; compressed/
dedup names, size estimate, URL/registry/ABI and full original ZIP writer.
Current peer/document wire record write bytes equal native C++ stream expressions
(the entire current Document writer body with explicit scalar/blob projections);
legacy frames read and every truncated prefix rejected. The real original
readFromStreamHelper header phase runs against legacy frames too. The complete
original ZipToFile/WriteArchiveFile with real QFile/minizip produces exactly the
same full ZIP bytes as Rust's actual FFI callbacks. No native fake model or
Session method bodies. Whole peer/document class/Session effects and the remainder
of the C++ document reader are own tests+source reading, not native class proof.

Preserved original quirks:

- Document::sizeInStream underestimates current wire writes by4 bytes for ordinary
  documents and16 for stickers; retained as the reserve estimate, not corrected.
- Channel flags saved as quint32 lose upper bits (including AyuNoForwards63).
  Chat creator56-bit packing and saved chat version reset to0 are retained.
- Legacy thumbnail with valid generic/non-document-thumbnail location is refused;
  old custom emoji with contextual set info beforev5 is refused; missing sticker
  set info intentionally yields no sticker attribute for a non-empty set.
- Overwriting duplicate parts overcounts currentOffset by decrementing skipped
  bytes again, as Source. Whole-part partial resume and short final streamed reads
  retained. Leftover holes are zero-filled in Rust rather than Qt uninitialized data.
- Web constructor ignores its first `to` argument; OnlySize is not a HEAD request.
  Explicit redirect fallback permits http(s) targets; automatic HTTP redirects
  forbid HTTPS downgrade. Duplicate already-sent ids can still emit an orphan GET,
  matching original emplace-after-send ordering; owner29 must own all futures.
- ZIP limit checked after payload chunks only: directories/final central directory
  may take final size over limit. Missing source file skipped; partial temp archives
  removed on failure/cancel; strict lastModified<now-24h boundary retained.

Explicit remaining partial / unproven:

- 02/29 real UserData fields/application, phone display formatting5 and concrete
  PeerCodec for self/top/recent peers; no invented user/session implementation.
- 04/16 Qt nullness provenance in materialized String/Vec models. Wire DTO roundtrip
  is exact including null vs empty and lone UTF16; snapshot_group/from_document
  cannot recover nullness already erased by existing model strings/bytes. Use the
  preserved wire record for exact reserialization; convenience snapshots normalize
  nullness and are NOT claimed byte-identical to a C++ model snapshot in all cases.
- 3/4 download_manager_mtproto actual DownloadMtprotoTask, session balancing,
  priorities, request IDs/retries, FILE_REFERENCE_EXPIRED/reference refresh and CDN
  redirect/hash/decrypt/reupload logic. These bodies are in the explicitly excluded
  file, not in mtpFileLoader. Their scenarios are NOT implemented or claimed tested
  here; no fake upload.getFile/CDN requests were manufactured.
- 29 actual loader manager/Session lifetimes, global weak WebLoadManager, owner
  queue/MainRef delivery and future pump/abort/error routing, platform postprocess/
  local FileLocation effect application. The real owner must drain task/reader/
  file effects in source order, process ClearFileNameAfterUpdate after RPL, guard
  callbacks by lifetime and apply cancellation before dropping a loader.
- Stage7 concrete streaming Reader callbacks/subscriptions/slots; stage5 pixels/
  image cache decode and PreparedFile UI objects. The32-part state and real Reader
  call effects exist; no fake Reader/network server was installed.
- 33 complete QUrl/path/local-codepage parity (reqwest normalization differs from
  Qt outside the native corpus) and non-Unix local civil-time metadata provider.
  Non-Unix gathering explicitly refuses unavailable dates instead of fabricating
  timestamps. Unix/native current-zone paths are tested without TZ/session changes.
- Whole original Qt/GLib worker scheduling, HTTP progress timing/proxy behavior,
  live network and large actual4GiB ZIP64 payloads were not exercised. ZIP64 ABI/
  threshold is carried to the same library; no4GiB test file was created.
- Corrupt parsing returns None before mutation; original readPeer could mark or
  partially mutate a peer before observing a later stream error. Invalid UTF16
  remains in wire records; applying it to String models refuses instead of replacement.
  Legacy positive duration*1000 uses explicit32-bit wrapping where C++ overflow
  is undefined; extreme overflow inputs are not native-proven on other compilers.

Dependencies: libc0.2 and reqwest0.12 reuse already locked workspace packages;
no package versions added/upgraded. minizip is the original foreign library,
added to Nix shell and Ubuntu CI libminizip-dev; no newly chosen ZIP crate.
tg-core forbid(unsafe_code) changed to deny with narrow allow only for audited
minizip/localtime FFI, SAFETY comments and native ABI test. Other code remains denied.

Checks: nix develop: cargo fmt --check PASS; cargo clippy --workspace --all-targets
-- -D warnings PASS; cargo test --workspace PASS:968 successful (964 regular +4
isolated child results),0failed,3existingignored. One completed full final run:
/tmp/storage-files-final-workspace-checks.log. Earlier test compilation interrupted
before any tests to fix QFile open modes; final checks include the fix.31newtests
(29integration+2ABI/open-mode units). Native235 repeated identical including whole
projected current Document writer and full original ZIP writer;17sourcehashes,
map310/227done83partial refs and unchanged lock package versions audited.
Known runtime race passed; tg-runtime untouched. CI left to user.
31 new tests (29 integration +2 native ABI/file-open-mode units); targeted tests/clippy PASS before
final doc/accounting freeze. Initial workspace test build was interrupted before
any tests ran to fix QFile WriteOnly→ReadWrite behavior; one complete final
workspace run follows that fix. The descriptor-mode unit test checks real I/O.
PROGRESS/ledger/map/parallel/handoff updated before commit/report. Integrate only
this branch, push main+port/storage-files atomically without force. CI is left to
the user; known tg-runtime thread-drop race is not modified.
