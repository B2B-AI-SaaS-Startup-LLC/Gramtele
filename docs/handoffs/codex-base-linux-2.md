# Linux platform part2 — Codex handoff

Base/main f926e45. Same own worktree `ayugram-rs-wt/base-linux`, branch
`port/base-linux`, fetched/reset to gramtele/main as explicitly requested.
Original `0f2a31bb420de98f331990432265b131b8d836b5` stayed read-only.
No Claude/runtime/foreign worktree changes or foreign branch integrations.

Implemented:
- libxcb.so.1 optional dlopen/NODELETE and lazy symbol cache; exact used C ABI,
  validated owned connections, replies/errors/events, shared connection and RAII
  object-before-connection destruction. No link-time/header dependency on XCB.
- WM atoms/chunked properties, supporting window/name, selection owner, extension
  probes, event-mask leases and typed event polling; real timestamp protocol,
  map/restack/EWMH ClientMessage activation. Actual screen-saver query/reset and
  cached headless X11/Xwayland probes close the earlier stage1/part2 contracts.
- _XSETTINGS_SETTINGS endian/padding/int/string/color/signed serial parser,
  retained valid prefixes on truncation, unknown-value updates, real X11 selection
  ownership/property retrieval/change callbacks and weak singleton/subscriptions.
- Actual RECORD and keysyms optional libraries, live key stream, reply-boundary
  filtering, column/keypad selection,305 literal key pairs/295 unique mappings,
  mouse/modifier names, singleton platform Available/Allowed/Start/Stop contracts.
- Actual MPRIS root/player/ObjectManager export on zbus, original XML, methods,
  properties, notification dedup/coalescing, reentrant Position callbacks,
  loop/shuffle/volume/seeking behavior and metadata. MainQueue dispatch releases
  model locks before RPL; async-channel avoids blocking the D-Bus executor.

[Part2 map](../../crates/tg-base/docs/platform-linux-2-map.md):265 rows,
216done/49explicitpartial/0pending. Declarations/bodies/constructor/templates,
conversion and schema rows count separately.11 requested originals3536LOC:
4done/7partial (Qt boundaries and explicit allocation defer); actual mpris.xml and
required common contracts also accounted.22 original source hashes. Part1 map
updated to209rows146done63partial, closing the6 previous XCB stage1 rows; remaining
Qt entrypoints are explicitly5. Ledger last_input.h/.cpp nowdone.

Verification:
- Nine new test functions plus two isolated child results. Actual Xvfb21.1.24
  with displayfd readiness, explicit private display strings, no user DISPLAY
  mutation. Actual XTEST/RECORD keypress+release,2050 WM atoms over chunks,
  timestamp/mapstate/client message fields, mask Add/Remove/Replace/reversion,
  settings selection/updates/serial/callback leases, destructor order and X server
  termination/error dispatch all tested. Error event is delivered once.
- Real dbus-daemon session with no user bus: MPRIS methods, Get/Set, actual
  PropertiesChanged, ObjectManager, name release and unexport. Position callback
  can call setPosition without recursion/deadlock; seek ignores track ID and uses
  duration ratio, including zero-duration NaN/infinity. Loop/volume dedup,
  shuffle suppression, metadata types/base64/C-string truncation are covered.
- Native separate CMake target outside clone:391 exact JSON cases,295 numeric
  mappings, repeated capture identical. Actual extracted parser/update methods,
  real QByteArray/QVariant/QColor, original media conversion bodies/RECORD ABI;
  mask pure branch expressions. No substituted GUI/session methods.
  [Recipe/limits](../../crates/tg-base/tests/reference-tools/platform-linux-2/README.md).
- ELF NEEDED contains libc/libm/libgcc/loader only; no libxcb, RECORD, keysyms,
  Qt or GIO link-time dependency. Soname absence tested on Nix without LD path;
  system distributions where libxcb cannot be hidden check invalid-display failure.

Source quirks preserved: UTF-16 length used with UTF-8 atom bytes; first root/S0,
Add mask skips when any bit overlaps; Remove skips old==mask too. Signed serial
starts-1, global serial ignored, invalid QColor16-bit input yields invalidRGBA0,
unknown setting type yields invalid value. Left/wheel events drop the rest of
that RECORD reply; event send-bit is not stripped in RECORD; keyboard state is
ignored for the source column selection. Mouse descriptor arithmetic wraps.
MPRIS fixed track path, ignored incoming track ID, strict >1second Seeked threshold,
programmatic loop command notification/shuffle suppression, zero duration division,
and Linux no-op stop/rate/display methods remain as original.

Original errors / explicit differences and limits:
- Original int milliseconds*1000 has C++ signed overflow before int64 conversion.
  GCC15.3/x86_64 capture wraps; Rust wraps explicitly. This is not proof for other
  original C++ compilers/architectures.
- Negative settings string length can bypass the unsigned C++ bound expression and
  read beyond the message via QByteArray(ptr,-1); excluded from native execution,
  Rust rejects it and retains the valid prefix. Empty WM_CHECK avoids the original
  unchecked first-element dereference by returning None. Malformed native reply
  sizes/missing symbols are refused rather than invoking undefined memory/function
  behavior. Normal valid formats preserve original results.
- C++ raw pointers/unique_ptr become validated types/Rc leases. X11 callbacks are
  dispatched without RefCell borrows held, with deterministic subscription lifetime;
  invalidating C++ callbacks while their vectors move is not native-proven.
- MPRIS export/unexport runs in an ordered tg-runtime Queue; original bus-name
  ownership was asynchronous too. Notification batches use the existing MainQueue,
  not GLib idle priorities. Exact GUI-loop/proxy scheduling and ordering of bus
  replies versus idle notifications are own tests+reading, not C++ native proof.
- Source has RECORD only; there is no XInput path to port in this revision.

Remaining partial with concrete destination:
- Stage5: actual Qt borrowed-connection selection/native filter/socket notifier,
  automatic owner-loop wiring, nested Qt dispatcher while awaiting timestamp,
  native focus/WId/IsX11 entrypoints; actual XCB wire operations are complete.
- Stage5: QKeySequence NativeText/localized names (numeric code/keypad directive
  is available), QImage JPEG encoding at quality87 (encoded-byte metadata phase
  complete), actual QGui/KSandbox/bootstrap facts for MPRIS.
- Stage1/allocator-tracer-hooks: enabled allocation tracer registration/buffering/
  loggers. SetMallocLogger and eight related functions have no definitions in
  this pinned tree; enabled integration needs process-wide allocator interposition.
  The task explicitly permits this defer. No fake enabled tracer or no-op hook
  installation API is exposed. The module and every function are addressed in map.
  No whole-process malloc replacement was installed in tests.
- Generic shortcut recording/chord aggregation belongs to its separate common
  source package, outside the requested Linux files; platform contracts are ready.

Dependencies: async-channel2.5, async-trait0.1, base640.22 are direct Linux deps
reusing existing lock packages (no new package/version changes). Dev shell adds
xorg-server/libxcb/libxcb-keysyms and runtime library search paths for tests. CI
explicitly installs xvfb/libxcb1/libxcb-record0/libxcb-keysyms1. These are test/
optional runtime libraries; the Rust binary does not link XCB.

Final checks: nix develop: cargo fmt --check PASS; cargo clippy --workspace --all-targets
-- -D warnings PASS; cargo test --workspace PASS:937 successful (933 regular +4
isolated child results),0failed,3existingignored. One full final workspace run:
/tmp/base-linux-2-workspace-checks.log. Nine new tests +2child; final focused
clippy/tests also passed. Native391 JSON cases/295 key pairs repeated identically;
22 source hashes/map refs/package-version audit/diff --check PASS. Known runtime
race passed locally; runtime untouched. Accounting updated before commit/report.
CI monitoring/restarts are left to the user as explicitly requested.

Requested file statuses:

| Original | Status |
|---|---|
| `base_linux_xcb_library.h` | done |
| `base_linux_xcb_library.cpp` | done |
| `base_linux_xcb_utilities.h` | partial |
| `base_linux_xcb_utilities.cpp` | partial |
| `base_linux_xsettings.h` | done |
| `base_linux_xsettings.cpp` | done |
| `base_global_shortcuts_linux.h` | partial |
| `base_global_shortcuts_linux.cpp` | partial |
| `base_linux_allocation_tracer.h` | partial |
| `base_linux_allocation_tracer.cpp` | partial |
| `base_system_media_controls_linux.cpp` | partial |

Only this branch is integrated into main and pushed to gramtele without force
after checks. User monitors CI/restarts known runtime race; no runtime fix.
Questions for user: none.
