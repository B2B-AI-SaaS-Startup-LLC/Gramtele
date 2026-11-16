# Data user native reference

Read-only AyuGramDesktop `0f2a31bb420de98f331990432265b131b8d836b5`.
Separate CMake target `/tmp/ayugram-data-user-native`. No user session, tdata,
DISPLAY, live requests or fake Session/History/Forum/server are involved.

`extract.py` copies original predicate bodies, the complete LastseenFromMTP body,
actual Chat/Channel sending lambdas, online timeout arithmetic, UnreadState
arithmetic and the genuine generated C++ TL constructors/writers. Only scalar
projections replace `flags()`, a Change.value and the local-calendar boundary:
seconds until local tomorrow are provided explicitly. This is **not** a native
build of whole UserData/ChatFilters/Folder/Groups: those bodies require real
Main::Session/Data::Session/History/ApiWrap/Forum/credits/UI/text entities. They
are verified by own tests + reading, with explicit remaining packages in the map.

The TL rows prove the generated original binary layout for filter inputs and
LastseenFromMTP inputs. They do not execute ChatFilter::tl or internal text entity
conversion: those depend on unresolved question1. Rust normalizes filter flags,
pinned/include/exclude lists and supplied wire titles against these real native
bytes; malformed/truncated inputs and unresolved peers are tested separately.
The C++ include/exclude sets use pointer order; Rust uses stable scoped key order.
The specified native samples use the same order; arbitrary allocation-order
byte equivalence is not claimed. Pinned order remains exact.

Reference-only requirements: C++23/CMake/Ninja/Python3/Qt6 Core/GSL/range-v3,
and genuine generated C++ scheme from the pinned original (existing
`/tmp/ayugram-tl-reference/gen`; generation recipe in data-ids reference tools).
No new Rust dependency or lock entry is needed.

```sh
cmake -S crates/tg-core/tests/reference-tools/data-user \
  -B /tmp/ayugram-data-user-native -G Ninja \
  -DTELEGRAM_ROOT=/home/yarik/Projects/AyuGramDesktop/Telegram \
  -DSCHEME_INCLUDE=/tmp/ayugram-tl-reference/gen \
  -DGSL_INCLUDE="$GSL_INCLUDE" -DRANGE_INCLUDE="$RANGE_INCLUDE" \
  -DCMAKE_PREFIX_PATH="$QT_PREFIX"
cmake --build /tmp/ayugram-data-user-native
/tmp/ayugram-data-user-native/native_user > /tmp/data-user-native.jsonl
cmp /tmp/data-user-native.jsonl crates/tg-core/tests/fixtures/data-user/native.jsonl
cmp /tmp/ayugram-data-user-native/source-sha256.json \
  crates/tg-core/tests/fixtures/data-user/source-sha256.json
```

Capture: **183** repeat-identical JSON rows:6 user-flag vectors (19 predicates),
108 channel sending cases,24 chat cases,24 timeout cases,12 status conversions,
8 filter TL samples,1 unread arithmetic case. Twenty hashes record the12 requested
original files, five required scalar headers, original API schema and the two
actual generated C++ inputs. Fixtures are regenerated from this target, not
produced by the Rust implementation.

Native confirms the original channel predicate's logical `&&` produces raw bit0
when default restrictions are present and boosts do not unrestrict. It does not
apply the default restriction mask. This behavior is deliberately retained.
