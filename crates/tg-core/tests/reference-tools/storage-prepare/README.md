# Storage prepare native reference

Original read-only `0f2a31bb420de98f331990432265b131b8d836b5`.
Separate CMake build `/tmp/ayugram-storage-prepare-native`, never inside the clone.
Dependencies are reference-only C++23/CMake/Ninja/Python3/Qt6 Core/GSL/range-v3;
no Rust dependencies were added. No QtGui/codec/display/user session is needed.

`extract.py` copies the complete original HashMd5 class, internal helpers and
methods, raw/hex MD5 helpers from core/utils, and exact original bodies of
FilePrepareResult::setFileData/setThumbData. Their wrapper holds only the real
scalar/QByteArray/vector fields consumed by those functions, not a mock Session,
FileLoadTask or decoder. The native MD5 algorithm is the original implementation,
not a second handwritten algorithm or QCryptographicHash stand-in. Raw hashMd5 and
both hashMd5Hex entry points cross-check each other. Additional pure thumbnail,
filename/dimension/group type/side-limit functions are captured; only the qualified
PreparedFile::Type enum alias is rebound to a value enum for GroupTypeForFile.

```bash
cmake -S crates/tg-core/tests/reference-tools/storage-prepare \
  -B /tmp/ayugram-storage-prepare-native -G Ninja \
  -DTELEGRAM_ROOT=/home/yarik/Projects/AyuGramDesktop/Telegram \
  -DGSL_INCLUDE="$GSL_INCLUDE" -DRANGE_INCLUDE="$RANGE_INCLUDE" \
  -DCMAKE_PREFIX_PATH="$QT_PREFIX"
cmake --build /tmp/ayugram-storage-prepare-native
/tmp/ayugram-storage-prepare-native/native_storage_prepare > /tmp/storage-prepare-native.jsonl
cmp /tmp/storage-prepare-native.jsonl crates/tg-core/tests/fixtures/storage-prepare/native.jsonl
cmp /tmp/ayugram-storage-prepare-native/source-sha256.json crates/tg-core/tests/fixtures/storage-prepare/source-sha256.json
```

Capture160 rows repeated byte-identically, ten source hashes:

-56 cases:14 lengths0..100000 with MD5 padding boundaries55/56/63/64/65 and fixed
32KiB split boundaries; initial call, empty call, append abc, repeat original data.
Stored MD5, part count/length/every part MD5 and thumb bytes length/MD5 are checked
against Rust. Per-part hashes are diagnostic capture values, not invented protocol
fields: Source only stores a whole payload MD5.
-27 thumbnail upload decisions incl exactly5MiB and5MiB+1, known MIME/case folds.
-7 MP4 filename transformations,48 dimension cases,20 group-type cases,2 side limits.
Document splitting reuses file_upload::part_layout and the already accepted native
13 original Entry layout cases in storage-transfer fixtures; no duplicate splitter.

Own tests + reading cover remaining state, actual private filesystem/archive
writer, TaskQueue/real tg-runtime worker/MainQueue and uploader/temp-file leases.
Whole original classes require real Main::Session/History/Qt image/editor/ffmpeg;
none of those bodies was replaced for a native class claim. Pixel decoding,
scaling, encoding, progressive-JPEG rewriting and actual audio/video probing
remain explicit packages5/7 partials. No original standalone tests for the four
requested storage files exist in the pinned tree.
