# data-small native reference

Read-only source `0f2a31bb420de98f331990432265b131b8d836b5`, standalone CMake
`/tmp/ayugram-data-small-native`. No account, tdata, WebAuthn ceremony or network.
Real Qt6.11.2 Core, GCC15.3, lib_base/lib_tl/rpl, original schema generated in
`/tmp/ayugram-tl-reference/gen` by the original generator (see data-ids recipe).

Compiles **unchanged whole data_passkey_deserialize.cpp**, actual original
headers/platform WebAuthn result values, and extracted unchanged pure FromTL,
credential construction and userHandle validation fragments from passkeys.cpp.
The userHandle wrapper calls fixture done/fail, without an Instance/Session body.
Search filter is unchanged original PrepareSearchFilter with real Storage enum
and schema. Request offset fragments/limit constants are extracted from actual
PrepareSearchRequest/PrepareHistoryRequest, without substituting PeerData methods.
Geometry uses the actual arithmetic/QSize fragment from ReplyPreview::prepare,
parameterizing only style size and integer DPR; null/Image/Prepare/cache/font
behavior is excluded. No fake PeerData, Session, Reader, Document or Image class.
DocumentId/PhotoId prefix aliases are the exact data_types.h scalar aliases;
forward class declarations stay opaque and never implement owner methods.

```sh
set -e
cmake -S crates/tg-core/tests/reference-tools/data-small \
  -B /tmp/ayugram-data-small-native -G Ninja \
  -DTELEGRAM_ROOT=/home/yarik/Projects/AyuGramDesktop/Telegram \
  -DSCHEME_INCLUDE=/tmp/ayugram-tl-reference/gen \
  -DQt6_DIR=/path/to/qtbase/lib/cmake/Qt6 \
  -DPython3_EXECUTABLE=/path/to/python3 \
  -DGSL_INCLUDE=/path/to/microsoft-gsl/include \
  -DRANGE_INCLUDE=/path/to/range-v3/include
cmake --build /tmp/ayugram-data-small-native -j2
python3 crates/tg-core/tests/reference-tools/data-small/inputs.py /tmp/small-inputs.jsonl
/tmp/ayugram-data-small-native/native_small < /tmp/small-inputs.jsonl > /tmp/small-1.jsonl
/tmp/ayugram-data-small-native/native_small < /tmp/small-inputs.jsonl > /tmp/small-2.jsonl
cmp /tmp/small-1.jsonl /tmp/small-2.jsonl
cmp /tmp/small-1.jsonl crates/tg-core/tests/fixtures/data-small/native.jsonl
```

On this machine cmake/ninja/CXX/Python need absolute Nix-store paths, recorded in
external CMakeCache.txt. Build must succeed before capture; do not run stale
binary after a compile failure. Fixture hashes cover16 original files, including
all10 targets,2 deserialize files, actual platform/storage/type headers and schema.

**135 rows**, repeated identically, every row compared by data_small_native.rs:

| Group | Rows | Proof |
|---|---:|---|
| entry | 4 | all optional flag combinations/default fields in FromTL |
| credential | 9 | binary IDs length0..8, URL padding difference register/login, exact TL wire bytes + exact compact Qt clientData JSON |
| dc | 14 | valid/empty second field, bounds/overflow/sign/space/multiple colons/Unicode; U3000 rejected by Qt toInt |
| filter | 14 | all SharedMediaType constructors including kCount fallback |
| offset | 18 | three directions,0/negative/limit/large IDs; search+history limits/add-offset/clamp |
| geometry | 50 | zero/negative/portrait/landscape dimensions, DPR1/2 |
| json | 26 | invalid/root shapes, defaults/fractional/out-of-range ints, standard vs URL base64, junk/padding/nonstring, BOM |

User/DC credential text is compared on valid UTF8 plus tested Unicode whitespace;
arbitrary QString lone surrogates/invalid-UTF8 grouping, huge JSON numbers and
signed-overflow geometry/offset inputs outside source-valid domain are not proved.
Tests compare values of parsed JSON structures, not QJsonObject's in-memory layout.
ClientData JSON strings and credential TL are compared byte-for-byte.

No native proof is claimed for full Passkeys service/Session/backend state,
RecentPeers peer codec bytes or weak Thread integration, streaming reader/loader/
media lifetime, History message insertion/filter facts/scheduling/events or UI
preview/loaded state. Implemented policies are own tests + complete source
reading; actual backend bodies have named future packages in data-small-map.md.
