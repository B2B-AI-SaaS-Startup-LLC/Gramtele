# Wallpaper and gift native reference

Original: read-only AyuGramDesktop `0f2a31bb420de98f331990432265b131b8d836b5`.
The standalone CMake target builds outside that clone. `extract.py` copies
unchanged pure method bodies from data_wall_paper.cpp and uses the genuine
WallPaper header/class/default member values, actual color conversion bodies,
Qt QDataStream Qt_5_1, genuine MsgId/SavedStarGiftId/rarity/attribute definitions
and generated C++ TL constructors/writers. The constructor initializer is copied
literally (`_id(id)`). Scalar aliases/opaque forward declarations come from the
original headers; no Session/PeerData/DocumentData body is fabricated. The opaque
PeerData marker in saved-ID samples is never dereferenced.

Session/media/network/pixel/UI functions are not linked or exercised. CloudThemes,
recent components, game/statistics/rotation and gift service ownership use own tests
+ reading, not a claimed complete native Session build. Internal gift message
entities, CreditsAmount and language/UI rendering remain explicit partials.

Reference-only build inputs: C++23/CMake/Ninja/Python3/Qt6 Core and Gui (QColor,
no QApplication/display), GSL/range-v3, genuine generated C++ scheme from the
original schema (existing `/tmp/ayugram-tl-reference/gen`, generation recipe in
../data-ids). QtGui is linked directly because this headless Nix shell lacks
QtGui's OpenGL CMake discovery inputs; no graphics server is used. No new Rust
package/dependency or lock entry is needed.

```sh
cmake -S crates/tg-core/tests/reference-tools/data-themes \
  -B /tmp/ayugram-data-themes-native -G Ninja \
  -DTELEGRAM_ROOT=/home/yarik/Projects/AyuGramDesktop/Telegram \
  -DSCHEME_INCLUDE=/tmp/ayugram-tl-reference/gen \
  -DGSL_INCLUDE="$GSL_INCLUDE" -DRANGE_INCLUDE="$RANGE_INCLUDE" \
  -DCMAKE_PREFIX_PATH="$QT_PREFIX"
cmake --build /tmp/ayugram-data-themes-native
/tmp/ayugram-data-themes-native/native_themes > /tmp/data-themes-native.jsonl
cmp /tmp/data-themes-native.jsonl crates/tg-core/tests/fixtures/data-themes/native.jsonl
cmp /tmp/ayugram-data-themes-native/source-sha256.json \
  crates/tg-core/tests/fixtures/data-themes/source-sha256.json
AYUGRAM_THEMES_REFERENCE_OUTPUT=/tmp/data-themes-rust.hex \
  nix develop -c cargo test -p tg-core --test data_themes export_rust_wallpapers_for_original_reader
/tmp/ayugram-data-themes-native/native_themes --read \
  < /tmp/data-themes-rust.hex > /tmp/data-themes-rust-in-original.jsonl
cmp /tmp/data-themes-rust-in-original.jsonl \
  crates/tg-core/tests/fixtures/data-themes/rust-in-original.jsonl
```

Capture:588 repeat-identical JSON rows,13 source hashes (11 pinned originals +2
actual generated C++ inputs). Original→Rust verifies current/legacy parsing,
canonical bytes, flags/IDs/nullness/rotation/intensity/key and boxed TL settings;
colors/slug/URL params, unsupported versions, rejected counts/sentinel colors,
optional legacy owner words, truncation/trailing bytes are included. Gift rows
verify rarity values/special badges (alpha38), SavedStarGiftId and boxed TL IDs
including negative/narrowed backdrop IDs. Native bytes come only from C++.

Rust→original:7 distinct Rust-created papers are accepted by original
FromSerialized and serialize back identically. This includes default/theme/
custom/emoji/legacy-ID/two-color/rotation variants. Emoji itself, document and
thumbnail are intentionally absent in the persistence format in both languages.
This is a sample-based interoperability proof, not every possible input sequence.

Preserved source quirks: FromSerialized accepts any version word; legacy owner
can have zero/one/two32-bit words; blur means exactly1 in current format; more
than2 colors report gradientRotation0 while raw rotation is serialized. A present
optional MTP color -1 becomes white, whereas raw persisted0xffffffff is rejected.
