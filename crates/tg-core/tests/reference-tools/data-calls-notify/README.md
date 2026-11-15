# data-calls-notify native reference

Read-only AyuGramDesktop `0f2a31bb420de98f331990432265b131b8d836b5`.
Build `/tmp/ayugram-data-calls-notify-native`, outside the source clone. No account,
network, tdata, substituted Session/Peer/Calls model or UI headers.

The target uses real **raw_countries_bounds.cpp** and **segment_tree.cpp**,
original chart header, unchanged chart index/getDayString functions and the
exact numeric fragments of measure (before daysLookup.clear + final
oneDayPercentage assignment). The font/lang/local-time labels loop is excluded,
not replaced by fake font metrics. extract.py copies the **entire unchanged
StatisticalChartFromJSON body** from statistics/statistics_data_deserialize.cpp
(the requested data_statistics_chart.cpp contains no parser), with actual Qt
QJsonDocument/QColor and the actual Ui::kCreditsCurrency header literal. It also
copies unchanged SerializeComplexPartsMap and actual kPartSize from streaming.
Real lib_base assertion/algorithm/integration/logging, Qt6.11.2 and GCC15.3.

CMake QtGui discovery requested unavailable OpenGL metadata in this environment;
the target finds Qt Core normally and links the **real** Qt6Gui shared library
and headers directly. It does not substitute QColor or parse colors in the driver.

```sh
set -e
cmake -S crates/tg-core/tests/reference-tools/data-calls-notify \
  -B /tmp/ayugram-data-calls-notify-native -G Ninja \
  -DTELEGRAM_ROOT=/home/yarik/Projects/AyuGramDesktop/Telegram \
  -DQt6_DIR=/path/to/qtbase/lib/cmake/Qt6 \
  -DPython3_EXECUTABLE=/path/to/python3 \
  -DGSL_INCLUDE=/path/to/microsoft-gsl/include \
  -DRANGE_INCLUDE=/path/to/range-v3/include
cmake --build /tmp/ayugram-data-calls-notify-native -j2
python3 crates/tg-core/tests/reference-tools/data-calls-notify/inputs.py /tmp/calls-inputs.jsonl
/tmp/ayugram-data-calls-notify-native/native_calls_notify < /tmp/calls-inputs.jsonl > /tmp/calls-1.csv
/tmp/ayugram-data-calls-notify-native/native_calls_notify < /tmp/calls-inputs.jsonl > /tmp/calls-2.csv
cmp /tmp/calls-1.csv /tmp/calls-2.csv
cmp /tmp/calls-1.csv crates/tg-core/tests/fixtures/data-calls-notify/native.csv
```

This machine needs absolute Nix-store cmake/ninja/Python/CXX paths (recorded in
CMakeCache.txt outside the repository). Regenerate all fixture files only after
a successful build; never continue to a stale binary after a compile failure.

400 rows, two identical captures; Rust compares every row:

| Group | Rows | Coverage |
|---|---:|---|
| country | 173 | every key and all4 original double coordinates |
| index | 42 | empty/single/multiple points, edge/interior/outside positions |
| tree | 120 | 0/1/29/30/31/60 items, clipped/empty/outside/full ranges, negative values |
| json | 30 | invalid/root shapes, column ordering, null/types, rounding, repeated x, BOM, flags/zoom/currency |
| line | 31 | names/IDs/colorKey, hidden, exact y values/extrema |
| parts | 4 | empty/multiple maps, offsets, embedded NUL/FF, exact native framing |

Finite floats compared bit-for-bit; two NaNs compare as NaN, not their unspecified
payload/sign. Source hashes:17 files, all10 requested files +4 statistics deps +
actual Ui currency/streaming helper headers/source. GetDayString is compiled but
its rendered-input lookup is own tests; colors themselves and labels/font metrics
are **not** compared by the native CSV. Rust preserves raw color input/colorKey;
QColor/colorDark and real labels remain5.

Full-owner probes `probe_owners.py NINJA BUILD TELEGRAM_ROOT LOG [include_dirs...]`
try all4 real group/notify/media/chart translation units with original stdafx.h,
CRL_USE_QT, real QtGui/Widgets/OpenGL/openssl/lib_ui/lib_tl/lib_storage/generated
original scheme headers. All4 stop at absent generated **emoji.h** from actual
lib_ui/ui/emoji_config.h. No UI type/body is stubbed. Failure log on this machine:
`/tmp/calls-notify-native-owners.log`.

Participant order has **no standalone sort function** in the source model:
applyParticipantsSlice appends, updates in place and erases. A std::vector replay
would only test a replacement algorithm. Full GroupCall/Notify service/preload
state therefore remains **own tests + full source reading**, including real
Peers/Documents/Photos stores, virtual TimerHost/ManualClock and concrete Sender
schema bridges; no full C++ owner/native-state proof is claimed.

Excluded input domains: NaN index searches (source loop can hang), reversed tree
ranges with30+ elements (source may recurse outside heap), C++ signed overflow,
QString lone surrogates and arbitrary allocator/unordered pointer order. They
are not silently included in native parity claims. countries fields retain the
source Lat/Lon naming even though values are lon/lat, including RU minLat=-18.
