# Linux platform part1 — Codex handoff

Base/main6cba7aa, own worktree `ayugram-rs-wt/base-linux`, branch
`port/base-linux`. Original `0f2a31bb420de98f331990432265b131b8d836b5`, read-only.
Reset of the old empty branch was explicitly authorized. No foreign integration;
Claude directories and all part2 implementations remain untouched.

Implemented: zbus blocking D-Bus methods/errors/raw StartReply, MainQueue signal
readiness with owner-guarded cancellation, service/property/portal watchers and
variant parsing; device/chassis/libc/update/locale transformations, optional
POSIX libraries/Wayland probe; Mutter idle fallback with permanent failure latch;
feedbackd/Snap/FileManager protocols; URL shell normalization and real GIO MIME
registration; battery standard-provider state, a11y state and observers contract;
power inhibition protocol and 10s timer/idempotence; folder FD protocol/fallback;
app-launch environment and null/empty activation-token contracts.

[Function map](../../crates/tg-base/docs/platform-linux-1-map.md):
209 rows,140 done,69 explicit partial,0 pending. Header declarations, bodies,
inline overloads and include-only/descriptor rows count separately. 34 source
hashes, 28 target Linux files (2427 LOC), plus six common files.
No new fake QWindow/QAccessible types; no portal stub or private event loop.
No original unit-test files for this slice were found;13 new behavioral tests
plus their isolated GIO child. Two existing filesystem tests retained.

Native CMake outside clone:263 exact rows, repeated capture identical. Actual
common-info/library cpp, extracted complete chassis/enum, real Qt/KShell. State,
DBus and GIO ownership coverage is own tests + reading; no full Linux owner native
linkage is claimed. Whole Qt/XCB/GIO-generated-proxy source cannot be built as a
pure-function target without unrelated window stack; its pure subsets suffice.
The requested os-release parser is absent in the pinned source (DMI/uname/XDG/libc
are used). Native recipe and coverage limits are in tests/reference-tools.

Real private xdg-desktop-portal1.20.4: ReadOne NotFound, matching predicted Inhibit
request path, Unix FD OpenDirectory accepted, two actual Response(2,{}) signals.
Gtk backend/permission store absent on this bus; successful grants and live
SettingsChanged were not verified. User session and system bus were not used.
GIO actual create/check/delete and filename-to-URI pass with loaded GLib2.88.3;
absent-library branch also passes. Private XDG child only. Missing optional
update-desktop-database causes a warning without failing the exercised operations.

Remaining boundaries (explicit partial, not represented as successful defaults):
- Stage5: native QWindow/focus/portal identifiers, QWayland token and hidden-window
  retry/nested Qt wait, actual GAppLaunchContext subclass callback, QAccessible
  activation observer, QLocale/QGui facts, QDesktopServices fallback/global startup.
- Stage1 Linux part2: actual XCB screen-saver query/reset, WM/X11/Xwayland probing,
  map/restack/send window activation. Payload and timer are implemented now.
- Stage1 live portal validation: SettingsChanged valid/type/filter events and
  granted-request close/late-response lifecycle. Frontend failure responses work.
- Battery: provider selection is an explicit required startup fact; standard
  DBus/portal/dummy drivers use zbus. No GIO monitor is constructed internally
  (that would perform D-Bus outside zbus). Third-party GIO extensions, exact
  asynchronous initialization timing/provider selection and full startup binding
  remain stage1/5; current explicit constructor reads already-owned service
  properties synchronously. Unsupported facts are not silently substituted.

Preserved original quirks: StartReply names0/1 vs D-Bus raw1/2 (real daemon2
verified), Mutter first error latches process-wide, unique-name path replaces only
first dot, repeated blockers do not update descriptions, source no-op Start/Finish,
RemoveQuarantine and filename identity, symlink-directory deletion failure,
flush skips descriptor0, HP first word can exceed32 UTF-16 units, KShell escapes.
BundledResourcesPath still Unexpected (port's existing panic policy).

Explicit differences/limits: GIO is optional dlopen rather than linked; absence
returns failure/unsupported. GIO stays resident because registered GTypes retain
code pointers; unsafe is restricted to that audited FFI module with SAFETY notes.
Library failures return None without original log messages. FolderRequest has an
explicit owning lifetime; dropping it cancels owner callbacks, and its FD closes
with RAII (the original open()/FD-list branch does not explicitly close the raw
FD). Actual Qt entrypoint binding remains5. Errors use zbus Error rather than
GError/std::exception; async finishes retain the result until the finish thunk.
Closed-connection async failure stays inline, normal completion goes to MainQueue;
signal match installation is synchronous; callbacks remain queued. Exact GIO
proxy-creation/activation timing is not native-proven. Signed idle-time overflow
and display/hardware behavior are not claimed verified.

Dependencies: direct Linux/all-other-unix zbus=5.19.0 and futures-core0.3;
51 newly resolved lock packages, no existing package version changes. Dev shell
adds dbus-daemon (dbus). No custom event loop: zbus internal I/O executor wakes
tg-runtime MainQueue. Desktop session startup provides real adapters/connections.
No lib_base screen_reader_state/single_instance substitute was necessary.

Final checks: nix develop: cargo fmt --check PASS; cargo clippy --workspace --all-targets
-- -D warnings PASS; cargo test --workspace PASS:926 successful (924 regular +2
isolated child results),0failed,3existingignored. One full final workspace run,
/tmp/base-linux-workspace-checks.log. Final tg-base clippy/all-targets, fmt check,
13 focused tests (+GIO child) and loaded-GIO child confirmation also PASS after
final source/native adjustments. Final native263 exact rows twice,34 sourcehashes,
all map file/test references and git diff --check PASS. Runtime race did not
reproduce; tg-runtime unchanged. Accounting updated before commit/report.

File statuses (every requested Linux original):

| Original | Status |
|---|---|
| `base_battery_saving_linux.cpp` | partial |
| `base_battery_saving_linux.h` | partial |
| `base_file_utilities_linux.cpp` | partial |
| `base_file_utilities_linux.h` | partial |
| `base_haptic_linux.cpp` | done |
| `base_haptic_linux.h` | done |
| `base_info_linux.cpp` | partial |
| `base_info_linux.h` | done |
| `base_last_input_linux.cpp` | partial |
| `base_last_input_linux.h` | partial |
| `base_linux_app_launch_context.cpp` | partial |
| `base_linux_app_launch_context.h` | partial |
| `base_linux_dbus_utilities.cpp` | done |
| `base_linux_dbus_utilities.h` | done |
| `base_linux_library.cpp` | done |
| `base_linux_library.h` | done |
| `base_linux_xdg_activation_token.cpp` | partial |
| `base_linux_xdg_activation_token.h` | partial |
| `base_linux_xdp_utilities.cpp` | partial |
| `base_linux_xdp_utilities.h` | partial |
| `base_power_save_blocker_linux.cpp` | partial |
| `base_power_save_blocker_linux.h` | partial |
| `base_process_linux.cpp` | partial |
| `base_process_linux.h` | partial |
| `base_screen_reader_state_linux.cpp` | partial |
| `base_screen_reader_state_linux.h` | partial |
| `base_url_scheme_linux.cpp` | done |
| `base_url_scheme_linux.h` | done |

Integration: only this branch into main, push main+port/base-linux to gramtele
without force after passing required checks. Questions for user: none; outstanding
integration/runtime/desktop validations are explicitly recorded above.
