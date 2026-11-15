# Storage-files native witness

Read-only AyuGramDesktop `0f2a31bb420de98f331990432265b131b8d836b5`.
Artifacts outside clone: `/tmp/ayugram-storage-files-native/native_storage_files`.
235 JSON rows; two independent runs compare identically.17 source hashes.

What actually runs:

- Exact original QDataStream expressions from writePeer's user/chat/channel
  branches and the complete Document::writeToStream body, over explicit scalar
  wire projections (thumbnail serialization supplied as its real encoded blob). Their model access expressions are replaced by parameters in
  extract.py; no fake PeerData/DocumentData class or Session method body is linked.
- The original readFromStreamHelper header phase verbatim (before attributes),
  with real Qt stream, across app-version gates 9061/1003013, tagged0..6 and
  pre-tag records. Attribute/model application is own tests+source reading.
- Exact original StorageFileLocation stream expression, including fields/padding,
  modern/in-message flags; raw-field input includes a deliberately invalid
  Legacy+in-message combination, which Rust rejects. It is not claimed to be
  a constructible valid original location.
- Original complete ZipToFile callbacks/WriteArchiveFile/StepProgress/FillEntryDate
  and naming/estimate pure functions with real Qt QFile/QTemporaryFile/minizip.
  Rust's real minizip callback writer produces identical complete ZIP bytes.
- Real QUrl supported-scheme corpus and QMimeDatabase registry projection for the
  model test. QUrl empty authority (https://) is accepted before network failure.

Fixture counts:48 peer frames,138 document frames,18 raw storage-location frames,
9 compressed-name,8 dedup-name,1 estimate,10 URL,1 ZIP,1 ABI,1 MIME registry.
Qt6.11.2, GCC15.3 x86_64, minizip/zlib1.3.2, actual lib_base assertion/integration.

```sh
cmake -S crates/tg-core/tests/reference-tools/storage-files \
  -B /tmp/ayugram-storage-files-native -G Ninja \
  -DTELEGRAM_ROOT=/home/yarik/Projects/AyuGramDesktop/Telegram \
  -DQt6_DIR="$qt_cmake_path" -DGSL_INCLUDE="$gsl_include_path" \
  -DRANGE_INCLUDE="$range_include_path" -DMINIZIP_LIBRARY="$minizip_library" \
  -DCMAKE_CXX_FLAGS="-I$minizip_include -I$zlib_include"
cmake --build /tmp/ayugram-storage-files-native -j2
mkdir -p /tmp/storage-files-native-private
TMPDIR=/tmp/storage-files-native-private \
  /tmp/ayugram-storage-files-native/native_storage_files > /tmp/storage-files-native.jsonl
cmp /tmp/storage-files-native.jsonl \
  crates/tg-core/tests/fixtures/storage-files/native.jsonl
```

Rust reads every frame and rejects every truncated prefix of peer/document frames;
current peer/document write bytes equal the C++ capture. The decoded wire records
retain null/non-null-empty QByteArray/QString and arbitrary UTF16. Roundtrips
through the wire DTO do not assume String/Vec model projections retain Qt nullness.
Existing String/Vec models erase that distinction: snapshot convenience methods
are explicit partial04/16 until owners retain/provide the actual nullable state.
Full UserData02/Session29 callbacks, whole C++ classes and live HTTP/MTProto/CDN
are not native-proven. No user tdata/session/display or actual network was used.
Document::sizeInStream's existing underestimate is preserved (4 bytes non-sticker,
16 sticker for current format); it is a reserve estimate rather than a wire size.
