# port/data-misc — срез 06/07

15.11.2026. База main4265492, собственный worktree
`/home/yarik/Projects/ayugram-rs-wt/data-misc`. Original read-only
`0f2a31bb420de98f331990432265b131b8d836b5`. 19 файлов, 3627 строк.

## Реализация и границы файлов

| Оригинал (data/) | Статус | Реализация / остаток |
|---|---|---|
| data_auto_download.h (149), .cpp (492) | done | Full/Set/Single, Source/Type/Override, defaults/disabled, peer overrides, все Should/ShouldAutoPlay, WithDisabledAutoPlay, v1/v2/v3 Qt stream; в pinned коде нет Preload |
| data_file_origin.h (250), .cpp (381) | done | Все 14 origin variants, comparator equivalence, document/photo location IDs, все 19 GetFileReferences overloads и исходный рекурсивный TL traversal; собственного Origin persistence format в оригинале нет |
| data_audio_msg_id.h (50), .cpp (103) | done | Type, cached speed eligibility, identity/order, nonzero external IDs, ForVideo; document pointer заменён стабильным DocumentRecordKey |
| data_authorization.h (20) | done | UnreviewedAuth fields/defaults |
| data_abstract_structure.h (70), .cpp (38) | done | Safe explicit application registry, lazy slots, const/mutable closure access, clear/recreate/Drop; нет raw aliases/virtual-base stand-in |
| data_abstract_sparse_ids.h (83) | done | Full/skipped counts, index/distance, order-preserving Vec/VecDeque snapshots, reverse, FlatSet nearest; deque нужен настоящему UserPhotosSlice |
| data_boosts.h (73) | done | Все data structures/fields; null QDateTime → Option<SystemTime> как в tg-base unixtime |
| data_bot_app.h (27), .cpp (13) | done | Все fields, immutable owner scope, stable photo/document record references; Session BotApps owner остаётся пакету13/29 |
| components/top_peers.h (64), .cpp (363) | partial | Полная model/rating/request/reply phase и framing. Actual Serialize::peer codec и chats-list subscription/lifecycle —13/29 |
| business/data_business_common.h (302), .cpp (398) | done | Recipients/chats, inputUser service, permissions TL, schedules, details, интервалы; intro использует настоящий Documents::process_document и проверяет sticker |
| data_emoji_statuses.h (179), .cpp (572) | partial | Non-UI lists/hashes/requests, parsing/interning, set/cancel, periodic/delayed/clear timers, real Peers/Changes binding; Ui::EmojiGroup/requestGroups/value — stage5; fromUniqueGift + localized name —16/01 |

Итого ledger: **15 done, 4 partial**. Больше ничего из пакетов06/07 не объявляется
переведённым. Native pure proof отдельно от own model tests.

## Контракты владельцев

`Full::apply_settings_effect` применяет именно существующий
`SessionEffect::AutoDownloadBytesLimit` из storage_settings_scheme: исходный вызов
устанавливает ноль. Тест проводит данные через настоящий read_setting → effect →
реальный Full. SessionSettings не подменяется; AddFromSerialized остаётся29.
Файлы settings/localstorage Claude не менялись.

TopPeers хранит PeerKey и рейтинг, не fake PeerData/UserData. Facts о боте и
supportsGuestChat обязательны от настоящего owner; сам canIncrementPeer и
ограничение user tag перенесены. ProcessUsers/Chats/read/write suggestions и
LoadAfterChats — синхронные typed owner effects. LoadAfterChats должен связаться
с оригинальными двумя main-queue turns/событием списка чатов в13/29. TopPeers
serialize/applyLocal пишут настоящее framing/rating/version/disabled/count;
PeerCodec обязан реализовать реальный storage/serialize_peer, production default
provider отсутствует. Test-only codec проверяет только framing/rollback, это не
новый peer wire format.

Запросы — конкретные методы tg-scheme; `send_request` использует настоящий
`tg_mtproto::Sender` с override ID/done/fail, без фиктивного сервера. Ответы должны
доставляться owner на поток модели с его lifetime guard. Проверены реальные
Instance/Sender callbacks без start/сокетов; full native request/session replay
не заявлен. Binding к общему Main::Session —29.

EmojiStatuses использует настоящие Timer/TimerHost/DelayedCallTimer. Есть
часовой refresh, отложенный recent, шестичасовой cap clearing, merge после
reentrant callbacks. `bind_peer_effects` + `apply_peer_status_effect` соединяют
реальные Peers::update/PeerData::set_emoji_status/Changes/automatic clear. Lifetime
подписки хранит owner. UI groups и UniqueGift не имитируются; реальный collectible
payload интернируется по ID с неизменяемым Rc, как shared_ptr исходника.

Все state borrows заканчиваются до fire, owner effect и user/document lookup.
Audio equality не включает cached Type/speed, как C++; ссылка на документ —
стабильная record identity, сохраняющаяся при convert из пакета04. FullMsgId
остаётся контекстом воспроизведения, не fake HistoryItem.

## Native и проверки

[Карта](../../crates/tg-core/docs/data-misc-map.md): **443 строки,
412 done /31 partial /0 pending**. Счёт отдельно включает API declarations в h
и cpp bodies (это не 443 уникальные функции); inline/value contracts и все
accumulator overloads присутствуют. Все done rows имеют реальные test references;
каждый partial содержит будущий пакет. Генератор карты проверяет имена тестов.

[Native recipe](../../crates/tg-core/tests/reference-tools/data-misc/README.md),
`/tmp/ayugram-data-misc-native/native_misc`, GCC15.3 x86_64/O1 Qt6.11.2 Core,
полный original generated scheme.cpp и реальные lib_base/TL bodies. Источник
синтетического input corpus — pinned api.tl, независимо от Rust serializer.
**1779 rows**: refs192, limits882, auto558, business84, rating28, rights21,
hash12, audio1, origin1. Каждый output сравнен Rust тестами; два native запуска
побайтно одинаковы. 16 source SHA256 записаны и проверены; источники не изменялись.
Integer/stream/reference outputs точные, exp/rating float с явным малым tolerance.

Настоящие целые owner-dependent cpp также пробовались через syntax-only без
подмен классов. Начальные узкие prefix probes обнаружили PCH-зависимости Qt
forever/rtl/rpl::type_erased/QDir/QAccessible. Повтор с настоящим stdafx.h и Qt
include directories, затем с настоящими OpenGL headers записан отдельно в
`/tmp/data-misc-owner-gl-probes.log`: все пять теперь требуют настоящий generated
`emoji.h` из ui/emoji_config.h. UI assets/build не подменялись. Это диагностика сборочного окружения,
не native подтверждение классов и не утверждение, что будущая Qt/offscreen сборка
невозможна. Full Should(Document/Peer), Audio classification, service/session
state — **own tests + чтение**; native покрывает Full::shouldDownload и pure
Audio methods без создания fake DocumentData.

34 новых tests: 14 data_misc, 14 services, 6 native. Время виртуальное, без sleep;
новых ignored нет. Все ранее принятые files/helpers остаются совместимы.
Новых Rust зависимостей, manifest/lock/version changes нет. Не открывались реальные
tdata/аккаунты, Telegram sockets не запускались.

## Исходные особенности и непроверенное

- AudioMsgId compares DocumentData::duration() (milliseconds) with
  `1 * TimeId(60)`. Despite the source comment "1 minute", actual cutoff is
  **60ms**, preserved with59/60ms tests; no60000ms correction.
- Full::setFromSerialized не проверяет stream.status после цикла лимитов:
  неполные записи могут приниматься с нулями на хвосте. Нельзя добавлять strict
  EOF/error check: это изменит совместимость. Тесты native проходят все обрезания.
- Voice/Music streamed types всегда запрещены для autodownload. ForceAllow берёт
  default при нулевом лимите; Channel File default также нулевой, поэтому остаётся
  запрещённым. Sticker обходится до ForceDeny, gifv после него.
- TopPeers remove пишет Reset request ID в общий `_requestId`, без callback,
  снимающего его: reload блокируется. Не исправлено. Равные рейтинги всплывают
  вверх (`>=`); изменения только рейтинга пишут storage без updates.fire.
- Channel-default emoji list использует GetDefaultEmojiStatuses, несмотря на
  отдельный метод GetChannelDefaultEmojiStatuses в схеме. First collectible
  payload сохраняется; последующие payload с тем же ID не обновляют Rc.
- processClearing берёт map, испускает peer mutations и emplace-мержит остаток.
  Новая reentrant registration не перезаписывается; при minWait=0 исходный код
  всё равно отменяет её timer. Это сохранено, отдельный тест.
- RemoveDayIntervals сначала присваивает `list[i]=first`, затем ищет after на
  уже обрезанном значении: хвост spanning interval теряется. Сохранено/native.
- FileOrigin StickerSet/Wallpaper/Theme ordering игнорирует access hash и часть
  payload. Rust Eq означает эквивалентность ключа по этому comparator; C++ в этих
  типах определял только `<`. WebPage URL сравнивается по UTF-16.
- Cross-language pointer order не эквивалентен scoped record/key order (Audio,
  одинаковые clearing deadlines). GlobalStructures сравнивает реальные адреса
  своих safe slots, но не обещает тот же порядок адресов, что C++ globals.
  Destructor-time raw aliases/reentrant deletion не являются доказанным native
  контрактом safe registry; доступ к данным требует closure borrow.
- C++ signed-overflow/NaN-cast UB вне проверенного домена, QString lone surrogates,
  полный peer codec/app lifetime, действующий remote Sender/session и GUI не
  доказаны этим эталоном. Они не заменены заглушками.

Области Claude (lang/countries, credits_amount, tg-ui/codegen/runtime/mtproto),
его storage/localstorage и чужие worktrees/ветки не менялись и не интегрировались.
Принятый main750204e уже содержится в базе4265492. Известную runtime Drop-гонку
не исправлял: владелец tg-runtime — Claude.

## Итоговая локальная проверка

```sh
nix develop --command bash -euc 'cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace'
```

Все три команды **PASS**, exit0. Workspace: **843 успешных результата**
(842 основных +1 существующий child-process),0failed/3 прежних ignored;
34 новых tests включены. Лог `/tmp/data-misc-verified-workspace-checks.log`.
Прогон выполнен после уточнения registry Drop и тестов порога59/60мс.
Предыдущий повтор выявил ошибочную единицу в новом тесте (59секунд вместо59мс);
она исправлена по настоящему DocumentData::duration, код порога60 не менялся.
Локально известная runtime race не воспроизвелась.
16 hashes/native repeat/map references/git diff --check также PASS.
Слияние своей ветки в main — fast-forward; push обеих refs в gramtele без force.
