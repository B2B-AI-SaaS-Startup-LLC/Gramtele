# Storage transfer native reference

Read-only original `0f2a31bb420de98f331990432265b131b8d836b5`.
This separate CMake target builds **outside** the source clone. It extracts the
complete unchanged bodies of `DownloadMtprotoTask::checkCdnFileHash` and
`Uploader::Entry::{setDocSize,setPartSize}`, with their actual constants.
The latter needs only a scalar Layout holder, not a fabricated Uploader/Session.
Hash checking uses actual lib_base `openssl_help.h`, flat_map/bytes and OpenSSL,
with real QByteArray/Qt6. Four requested source SHA256s are stored beside the capture.

Native dependencies: C++23 compiler, CMake/Ninja, Python3, Qt6 Core, OpenSSL,
Microsoft GSL and range-v3. They are reference-only, no new Rust dependencies.
An example using paths from the development environment:

```bash
cmake -S crates/tg-core/tests/reference-tools/storage-transfer \
  -B /tmp/ayugram-storage-transfer-native -G Ninja \
  -DTELEGRAM_ROOT=/home/yarik/Projects/AyuGramDesktop/Telegram \
  -DGSL_INCLUDE="$GSL_INCLUDE" -DRANGE_INCLUDE="$RANGE_INCLUDE" \
  -DCMAKE_PREFIX_PATH="$QT_PREFIX;$OPENSSL_PREFIX"
cmake --build /tmp/ayugram-storage-transfer-native
/tmp/ayugram-storage-transfer-native/native_storage_transfer > /tmp/storage-transfer-native.jsonl
cmp /tmp/storage-transfer-native.jsonl crates/tg-core/tests/fixtures/storage-transfer/native.jsonl
cmp /tmp/ayugram-storage-transfer-native/source-sha256.json crates/tg-core/tests/fixtures/storage-transfer/source-sha256.json
```

Capture: **76 rows**, repeated byte-identically.63 hash cases: seven lengths
0/1/15/16/1024/131071/131072 × three `limit`s0/1/131072 × absent/good/corrupt hash.
The Rust test compares both the digest bytes and the original decision.
13 part-layout cases range from0 to1TiB, including1/32MiB thresholds, 4000-part
fallbacks and the original `ushort` truncation before the limit check.

Not a native proof of either entire C++ state machine. They require real ApiWrap,
Main::Session/Data/media/History/Qt image and encoding infrastructure; their
bodies were not replaced. State transitions, virtual timers, real typed TL
Sender callbacks, default retries/migration, cancellation/reentrancy and CDN
flows are covered by own tests + reading. No user tdata, session or live server
was opened. No mocked server / fake model / dedicated event loop was introduced.
