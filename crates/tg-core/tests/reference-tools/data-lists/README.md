# data-lists native reference

Read-only AyuGramDesktop `0f2a31bb420de98f331990432265b131b8d836b5`.
All build products are outside the clone, `/tmp/ayugram-data-lists-native`.
No user tdata/account/network, model stand-ins or replacement algorithm bodies.

The target compiles **unchanged full data_messages.cpp and data_sparse_ids.cpp**,
real headers, rpl, base/assertion/algorithm/integration, Qt6.11.2 Core, and full
original generated `scheme.cpp`. `extract.py` copies unchanged pure business
normalization/ToMTP functions and PremiumLimits constructor/getter bodies.
The generated original schema comes from the original generator, as in prior
reference-tools/data-peer and data-ids, never from the Rust implementation.

Explicit link seams: PremiumLimits::appConfigLimit and isPremium receive fixture
inputs; ForMessagesToMTP receives empty recipients. Their full owner-dependent
bodies are **not** compiled or proven. Opaque non-null Session pointer token is
never dereferenced, no fake Main::Session/UserData class is defined. LevelLimits,
maxBoostLevel, complete AppConfig JSON/rounding, and full business owner/Sender
state are own tests + source reading. The seam is not a native Session replay.

```sh
cmake -S crates/tg-core/tests/reference-tools/data-lists \
  -B /tmp/ayugram-data-lists-native -G Ninja \
  -DTELEGRAM_ROOT=/home/yarik/Projects/AyuGramDesktop/Telegram \
  -DSCHEME_INCLUDE=/tmp/ayugram-tl-reference/gen \
  -DQt6_DIR=/path/to/qtbase/lib/cmake/Qt6 \
  -DGSL_INCLUDE=/path/to/microsoft-gsl/include \
  -DRANGE_INCLUDE=/path/to/range-v3/include
cmake --build /tmp/ayugram-data-lists-native -j2
/tmp/ayugram-data-lists-native/native_lists > /tmp/lists-1.csv
/tmp/ayugram-data-lists-native/native_lists > /tmp/lists-2.csv
cmp /tmp/lists-1.csv /tmp/lists-2.csv
```

On this machine cmake/ninja/Python can require absolute Nix-store paths.
`fixtures/data-lists/native.csv`: 804 native rows, repeated identically, compared
exactly by `data_lists_native.rs` (no float tolerances):

| Group | Rows | Coverage |
|---|---:|---|
| premium | 216 | 54 unchanged getters, 4 premium/default/config-override modes |
| list | 504 | 0..6 messages, before/after0..2, positions outside/inside ranges |
| absent/less/bottom | 21 | absent removal decrement, cutoff and bottom invalidation |
| request/builder | 34 | initial/refill direction/order, missing counts/skips, removal/all/invalidation |
| merged | 18 | isolation in each half, unknown/zero/positive skipped counts, index/nearest |
| hours | 5 | normalized seconds→minutes and exact BusinessWorkHours bytes |
| schedule | 3 | Always/Outside/Custom, signed dates |
| away/greeting | 3 | exact ToMTP bytes, offline flag, shortcut/days, empty-recipient seam |

`source-sha256.json` records21 original files (all16 target files plus real
headers/common helpers/schema); the inventory/map generator verifies test names.

Full-owner probes: `probe_owners.py NINJA BUILD TELEGRAM_ROOT LOG [include_dirs...]`
tries actual premium/pts/business/shared/photos/stickers cpp with original
stdafx.h and real QtGui/OpenGL/openssl headers. All six stop at missing generated
`emoji.h` from real lib_ui/ui/emoji_config.h. No UI header or owner is substituted.
This is an explicit failed full-app probe, not evidence that pure functions fail.

Out of scope for native: History/scheduled/SavedMusic owners, full timer owner
Api::Updates, user-photo service and sticker loader/pixels. Their implemented
model contracts are tested in Rust. Sparse merged count-only key0 with no
migrated optional has C++ UB when accessing size/skips; native cases avoid it.
Rust diagnoses that invalid dereference with a precondition panic, no fabricated
slice/count. Arbitrary C++ signed overflow, invalidated iterators in reentrant
callbacks, pointer allocation order, UTF-16 lone surrogates and system local
UTC-offset/DST acquisition are not asserted.
