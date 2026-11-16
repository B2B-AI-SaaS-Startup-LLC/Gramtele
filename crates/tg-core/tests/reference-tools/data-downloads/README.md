# Download list native reference

Read-only AyuGramDesktop `0f2a31bb420de98f331990432265b131b8d836b5`.
Standalone CMake target `/tmp/ayugram-data-downloads-native`, no original-tree
build/output, user session, tdata, display or network involved.

The genuine data_download_manager.h supplies DownloadType/DownloadedId and other
value definitions; genuine MsgId/FullMsgId/GlobalMsgId bodies are extracted from
the original header, and PeerId/basic aliases come from original headers.
HistoryItem/DocumentData/PhotoData are opaque forward declarations, never given
substitute bodies. The class itself is not instantiated.

`extract.py` projects the original serializator's pure QByteArray/QDataStream
statements by removing its sessionData lookup and replacing `data.downloaded`
with the supplied vector. The reader body is unchanged after the initial
`session->account().local().downloadsSerialized()` lookup; input bytes replace
that actual owner lookup. This is a reference for the format and rejection rules,
not a claimed native build of the complete Session/History graph. Own tests cover
the real scope guard, mutation/effect phases and virtual timers separately.

Reference-only dependencies: C++23, CMake/Ninja/Python3, Qt6 Core, GSL/range-v3,
original generated scheme headers from `/tmp/ayugram-tl-reference/gen` (generation
recipe in ../data-ids). No new Rust dependency or lock change.

```sh
cmake -S crates/tg-core/tests/reference-tools/data-downloads \
  -B /tmp/ayugram-data-downloads-native -G Ninja \
  -DTELEGRAM_ROOT=/home/yarik/Projects/AyuGramDesktop/Telegram \
  -DSCHEME_INCLUDE=/tmp/ayugram-tl-reference/gen \
  -DGSL_INCLUDE="$GSL_INCLUDE" -DRANGE_INCLUDE="$RANGE_INCLUDE" \
  -DCMAKE_PREFIX_PATH="$QT_PREFIX"
cmake --build /tmp/ayugram-data-downloads-native
/tmp/ayugram-data-downloads-native/native_downloads > /tmp/data-downloads-native.jsonl
cmp /tmp/data-downloads-native.jsonl crates/tg-core/tests/fixtures/data-downloads/native.jsonl
cmp /tmp/ayugram-data-downloads-native/source-sha256.json \
  crates/tg-core/tests/fixtures/data-downloads/source-sha256.json
AYUGRAM_DOWNLOADS_REFERENCE_OUTPUT=/tmp/data-downloads-rust.hex \
  nix develop -c cargo test -p tg-core --test data_downloads codec_full_rejection_trailing_raw_qstring_and_inverse_export
/tmp/ayugram-data-downloads-native/native_downloads --read \
  < /tmp/data-downloads-rust.hex > /tmp/data-downloads-rust-in-original.jsonl
cmp /tmp/data-downloads-rust-in-original.jsonl \
  crates/tg-core/tests/fixtures/data-downloads/rust-in-original.jsonl
```

Capture:164 repeat-identical JSON rows (8 signed TimeId conversions +156 codec
samples),6 source hashes. Inputs cover both types and invalid enum, count bounds,
null/empty/Unicode/unpaired UTF16 paths, size0/1/4000MiB/one-over/UINT_MAX/negative/
2^32+1, signed dates/messages, raw peer/hash bits, every truncation of a valid
record, trailing bytes, multi-record success and invalid last-row full rejection.
Native canonical bytes are original read→original write; Rust must reproduce them.

Inverse:4 Rust-created frames are accepted/read/written back by the actual original
with identical bytes, including unpaired UTF16 and quint32 size truncation. The
inverse fixture is also asserted against current Rust writes in regular tests.
Qt stream version is Qt_5_1, default BE, no invented version/tag or sizeof copies.

Not native-verified: full DownloadManager model ownership/generated HistoryItems,
filesystem callback scheduling/Qt platform edge cases, real Session integration,
image/MIME/launch backends, local timezone/DST, SendActionPainter statuses/phases,
and internal text-entity parsing. These have explicit package addresses in the
function map and own tests for the available state/decision boundaries.
