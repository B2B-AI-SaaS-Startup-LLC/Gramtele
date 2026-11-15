# Linux platform part1 native reference

Original: AyuGramDesktop `0f2a31bb420de98f331990432265b131b8d836b5`.
Build outside the read-only clone. The target compiles actual
`base_platform_info.cpp`, `base_linux_library.cpp`, algorithm/debug/assertion/
integration dependencies. `extract.py` copies the complete original chassis
function and StartReply enum verbatim into the build directory. No substitute
function bodies or GUI/session owners. KShell is the actual KDE library, not a
reimplementation in the witness.

Dependencies used here: GCC15.3, CMake4.1.6, Ninja1.13.2, Qt6.11.2 Core,
KF6CoreAddons6.26.0, MicrosoftGSL4.2.1, range-v3 0.12.0. Supply their CMake/include
paths from the local Nix store; no dependencies are vendored into the original.

```sh
cmake -S crates/tg-base/tests/reference-tools/platform-linux-1 \
  -B /tmp/ayugram-base-linux-1-native -G Ninja \
  -DTELEGRAM_ROOT=/home/yarik/Projects/AyuGramDesktop/Telegram \
  -DQt6_DIR="$qt_core_cmake_path" -DKF6CoreAddons_DIR="$kf_core_cmake_path" \
  -DGSL_INCLUDE="$gsl_include_path" -DRANGE_INCLUDE="$range_include_path"
cmake --build /tmp/ayugram-base-linux-1-native -j2
python3 crates/tg-base/tests/reference-tools/platform-linux-1/make_inputs.py \
  > /tmp/base-linux-inputs.jsonl
/tmp/ayugram-base-linux-1-native/native_linux_1 \
  < /tmp/base-linux-inputs.jsonl > /tmp/base-linux-capture.jsonl
cmp /tmp/base-linux-capture.jsonl \
  crates/tg-base/tests/fixtures/platform-linux-1/native.jsonl
nix develop -c cargo test -p tg-base --test platform_linux_1 native_pure_vectors
```

263 exact JSON rows, repeated capture identical. Device string limits include
UTF-16/surrogates/control/Unicode whitespace, HP normalization/first-long-word,
all chassis codes0..35; QVersion null/overflow/prefix/whitespace/plus/2.28 boundary;
KShell NoOptions split/quote/join including broken quotes, C/hex/octal escapes and
the second-hex-digit-zero quirk. Desktop environment list uses the original
Qt expression on explicit strings. Actual dlopen missing library/symbol and the
original enum values are included. This does **not** compile whole base_info_linux
or GIO-based DBus utilities; no claimed native proof of their stateful behavior.
No `/etc/os-release` parser exists in these pinned files.

The Rust test uses real isolated dbus-daemon processes with only its own service
directory. Non-portal fixtures implement Mutter, a11y.Status, PowerProfiles,
FileManager, feedbackd and Snap protocol endpoints. No mock portal is used.
No session/system connection constructors are invoked by tests; tests supply
private addresses. Source timers are checked with ManualClock, no sleep.

The GIO registration test reexecutes in a child with private XDG directories and
an existing test executable. Both unavailable-SONAME and loaded GLib2.88.3 paths
were exercised (local loaded run sets LD_LIBRARY_PATH to its lib directory).
Create/check/idempotent cleanup and g_filename_to_uri work with the real library;
`update-desktop-database` is absent locally and GIO emits a warning, but the
registration assertions pass. User MIME/application settings are untouched.

`examples/platform_linux_1_probe.rs` takes an **explicit private bus address**.
`probe_portal.py --portal /path/to/real/libexec/xdg-desktop-portal --gdbus /path/to/gdbus --probe /path/to/target/debug/examples/platform_linux_1_probe --output /tmp/linux-portal-probe`
reproduces the isolated run using a D-Bus owner readiness barrier, without sleep.
Run an actual xdg-desktop-portal1.20.4 on that bus with empty private XDG dirs,
no DISPLAY/WAYLAND_DISPLAY and an invalid private DBUS_SYSTEM_BUS_ADDRESS.
Observed: ReadOne returns portal.NotFound for both tested keys; Inhibit returns
exactly the predicted desktop_app123 request path; OpenDirectory accepts the Unix
FD; both requests emit actual Response(2,{}) because the Gtk backend and permission
store are unavailable on this isolated bus. This proves real frontend wire types,
paths and response decoding, **not** successful inhibition/directory opening or
SettingsChanged delivery. See [capture](../../fixtures/platform-linux-1/portal-probe.txt).
The desktop backend/live settings and granted-request lifecycle remain partial1;
Qt activation/focus/observer/subclass adapters remain5, XCB remains part2.

External implementation references used when replacing GIO machinery:
[GLib DBus provider](https://raw.githubusercontent.com/GNOME/glib/2.88.3/gio/gpowerprofilemonitordbus.c),
[GLib portal provider](https://raw.githubusercontent.com/GNOME/glib/2.88.3/gio/gpowerprofilemonitorportal.c),
[KShell](https://raw.githubusercontent.com/KDE/kcoreaddons/v6.26.0/src/lib/util/kshell_unix.cpp),
[QVersionNumber](https://raw.githubusercontent.com/qt/qtbase/v6.11.2/src/corelib/tools/qversionnumber.cpp).
Original toolkit legal notice: https://github.com/desktop-app/legal/blob/master/LEGAL;
KShell LGPL-2.0-or-later; Qt reference uses its installed LGPL/commercial library.
