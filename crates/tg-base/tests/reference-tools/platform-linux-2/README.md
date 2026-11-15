# Linux part2 native reference

Original `0f2a31bb420de98f331990432265b131b8d836b5`, read-only. Build artifacts live
outside the clone in `/tmp/ayugram-base-linux-2-native`. No replacement Qt/session
bodies or process-wide allocator hooks are linked.

`extract.py` copies the complete original PropertyValue update implementation,
rounding/populateSettings bodies, key initializer, four media conversion helpers
and RECORD C declarations. A pure parser holder supplies only the actual data
members (opaque null connection, real base::flat_map/QByteArray/QVariant/QColor).
The same parser/update bodies run on all fixture inputs; null connection is never
dereferenced. Masks evaluate the original pure branch expressions on explicit old
mask/mode inputs. The witness does not run the whole Qt/GIO owners.

Used: GCC15.3, Qt6.11.2 Core/Gui, XKB1.13.1 headers, range-v3 0.12.0, GSL4.2.1,
CMake4.1.6 and Ninja1.13.2. QtGui is linked as the real shared library for QColor;
no display or OpenGL context is created (CMake's GUI/WrapOpenGL dependency finder
is unnecessary for this pure target).

```sh
cmake -S crates/tg-base/tests/reference-tools/platform-linux-2 \
  -B /tmp/ayugram-base-linux-2-native -G Ninja \
  -DTELEGRAM_ROOT=/home/yarik/Projects/AyuGramDesktop/Telegram \
  -DQt6_DIR="$qt_cmake_path" -DXKB_INCLUDE="$xkb_include_path" \
  -DGSL_INCLUDE="$gsl_include_path" -DRANGE_INCLUDE="$range_include_path"
cmake --build /tmp/ayugram-base-linux-2-native -j2
python3 crates/tg-base/tests/reference-tools/platform-linux-2/make_inputs.py \
  > /tmp/linux2-inputs.jsonl
/tmp/ayugram-base-linux-2-native/native_linux_2 < /tmp/linux2-inputs.jsonl \
  > /tmp/linux2-capture.jsonl
cmp /tmp/linux2-capture.jsonl crates/tg-base/tests/fixtures/platform-linux-2/native.jsonl
nix develop -c cargo test -p tg-base --test platform_linux_2 native_xsettings
```

391 JSON cases: little/big endian, all types/paddings, every truncation of a mixed
packet, embedded NUL/arbitrary-byte names/strings, signed per-property serial and
replacement, ignored global serial, unknown types, QColor invalid values; all295
unique numeric key table entries (305 source pairs/aliases); event-mask Add/Remove/
Replace corner cases; media status/command mappings; i32 milliseconds scaling,
mouse descriptor overflow and actual ABI sizes. The Rust numeric key table was
translated from the literal XKB/Qt header constants; the witness independently
compiles the original initializer. Both paths are checked across every pair.

Preserved: Add skips all changes when any old bit overlaps the requested mask;
Remove also skips when old==mask. QColor takes original16-bit values directly as
0..255 ints; out-of-range input yields invalid QColor with all channels0. Serial
is signed and starts at-1. Unknown type updates to an invalid QVariant. A malformed
suffix retains already applied entries. Negative string length is excluded from
native inputs: QByteArray(ptr,-1) can read beyond packet boundaries. Rust returns
NegativeLength and keeps the valid prefix. Empty WM_CHECK property is also safely
None instead of dereferencing absent original data.

The source `int * 1000` overflows before conversion to int64. GCC15.3/x86_64 capture
wraps; Rust explicitly wraps. C++ signed overflow is undefined, so the witness does
not prove other original compiler/architecture behavior. Mouse descriptors use
unsigned wrapping exactly.

State/transport: own tests plus source reading. Tests start actual Xvfb with
`-displayfd`, explicit private display arguments, XTEST-generated key input and a
real RECORD stream. They install an actual selection owner/settings property,
exercise2050 WM atoms across chunks, timestamps, map/restack/ClientMessage, mask
revert/leases, resource destructor order, screen-saver query/reset and server
termination. No user DISPLAY is changed. Soname absence is exercised in a child
without LD_LIBRARY_PATH on Nix; on systems with an unavoidable system libxcb the
same child checks invalid-display failure instead.

MPRIS is a real zbus server on an isolated dbus-daemon. Tests use actual methods,
Properties.Get/Set, PropertiesChanged, ObjectManager and unexport, including
position callback recursion, duplicate volume/loop updates, ignored track ID,
zero-duration NaN/infinity and C-string metadata truncation. The pinned zbus
Interface trait is implemented directly to retain the original XML and support
notification dedup/coalescing without macro-generated duplicate signals.

Qt5 remains: borrowed QGui connection/native event/socket dispatcher, nested Qt
wait, NativeText/localized key names, QImage JPEG87 encoding and application facts.
There is no XInput path in this pinned source. Allocation tracing remains explicitly
partial1/allocator-tracer-hooks: SetMallocLogger and eight companion registrations
have no definitions in this clone and require process-wide allocation interception;
the task excludes that test mode. No fake enabled tracer is exposed.

Original legal notices: https://github.com/desktop-app/legal/blob/master/LEGAL .
XCB ABI declarations are X11-licensed as noted in the original header.
