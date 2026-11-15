# Параллельная работа: Claude и Codex

Репозиторий: https://github.com/B2B-AI-SaaS-Startup-LLC/GramTele.
Разделение согласовано пользователем 13.11.2026. Правила PLAN.md и PORTING.md
действуют для обеих веток; это собственный порт, без TDLib/grammers.

| Исполнитель | Ветка | Задача и файлы |
|---|---|---|
| Claude на другой машине | port/lang-countries от handoff/claude-lang-countries | Незавершённые lang/countries, credits_amount и необходимые им value types tg-ui, MainLocal/runtime, Sender и генератор языка |
| Codex: завершено | port/cache-file-format | tg-cache: storage_encryption, storage_encrypted_file и перенесённые тесты; native-паритет шифрования и файлового формата |
| Codex: принят пользователем, main 20695cb | port/data-ids | tg-core/src/data/data_peer_id и скалярная часть data_msg_id: типизированные ID, TL и legacy/modern serialization |
| Codex: завершено и проверено | port/cache-database от main 20695cb; worktree cache-database | Остаток tg-cache/lib_storage, 24 файла; native в обе стороны, 252 строки карты, 40 тестов; интеграция main после проверок |
| Codex: завершено и проверено | port/data-changes от main be0473f; worktree data-changes | SessionScope/typed keys, data_changes.h/.cpp; 136 строк карты, 25 tests, native O0/O2; handoff codex-data-changes |

Claude получает [отдельное задание](tasks/claude-lang-countries.md) и WIP-снимок
прежнего рабочего дерева. WIP не считается проверенным результатом и не вливается
в main до завершения. Исходное грязное дерево port/core-base сохраняется целиком.

Claude не меняет crates/tg-cache и файлы data-ids. Codex не меняет lang/countries,
credits_amount, tg-ui, tg-codegen, tg-runtime и tg-mtproto.
После просьбы пользователя продолжить блок после Claude добавлен срез data-ids.
Для него Codex добавляет только pub mod data в tg-core/src/lib.rs и переносит
существующую зависимость tg-scheme из dev в обычные зависимости tg-core.
Изменения экспорта/manifest при интеграции совмещаются с добавлениями Claude.
MessageHighlightId, FullReplyTo и SuggestOptions отложены до принятия его
TextWithEntities/CreditsAmount; их заглушек или копий в data-ids нет.
Тестовые native targets хранятся в соответствующем крейте, поэтому CMake-файлы
друг друга не затрагиваются. Не обновлять версии общих зависимостей.

Ветки пушатся в организационный репозиторий, без force push и без прямого пуша
Claude в main. Интеграцию в main выполняет Codex после проверки готового коммита.
Правки Cargo.lock — только необходимые своей задаче; при объединении проверять
сохранение обеих групп зависимостей, не откатывать весь lock из одной ветки.

Чтобы не конфликтовать в общих отчётах, Claude пишет
docs/handoffs/claude-lang-countries.md и свою карту функций; Codex —
docs/handoffs/codex-cache-file.md и карту tg-cache. Корневые PROGRESS.md и
ledger/files.csv обновляет Codex при интеграции по фактическим результатам.


Завершение cache-database: [handoff](handoffs/codex-cache-database.md),
[карта функций](../crates/tg-cache/docs/port-map.md).
Все три проверки workspace PASS, 678 passed / 0 failed / 3 прежних ignored.
Существующие файлы tg-runtime/tg-ui/tg-codegen/tg-mtproto и lang/countries
не менялись: Stats bridge локален tg-cache. Общий lock получил только две
существующие workspace-зависимости. Пушатся main и port/cache-database в gramtele,
без force push. Для дальнейшей работы не повторять завершённый cache-database;
ожидать проверенный коммит Claude для lang/countries и зависимых value types.

Срез data-changes: [handoff](handoffs/codex-data-changes.md),
[карта](../crates/tg-core/docs/data-changes-map.md). Код ограничен tg-core;
Claude files и Cargo.lock не меняются. Native compile не подменяет Main lifecycle:
живой Session/main guard остаётся own tests + чтение до пакета29. Изменяемые ID
media/topics/messages и pointer-order→key-order явно задокументированы как границы
будущего пакета моделей. Все три проверки PASS: 702 workspace tests + 1 дочерний процессный,
0 failed / 3 прежних ignored. Интеграция main + port/data-changes без force push.

Срез port/data-peer от main959488b, worktree data-peer:
[handoff](handoffs/codex-data-peer.md),
[карта](../crates/tg-core/docs/data-peer-map.md). Общая модель PeerData и Peers
component только в tg-core, без fake subtype records. Concrete User/Chat/Channel
и owned enum — 02/03; UI5/6, full owners29 и Unicode text backend5 остаются partial.
357 строк карты (300 done/57 partial), native 4247 строк, 31 новых tests.
Cargo manifests/lock и области Claude не менялись.

По указанию пользователя параллельные Codex worktrees port/tdata-settings
(storage settings scheme/localstorage), port/base-linux (lib_base/platform/linux)
и docs-only stage4 audit зарезервированы за другими агентами. В data-peer их файлы
не меняются, ветки не интегрируются. Их интеграция в main — только отдельным
следующим заданием пользователя. Текущая интеграция касается только port/data-peer.

Пакет03 port/data-chat-channel от main1038c32, worktree data-chat-channel:
[handoff](handoffs/codex-data-chat-channel.md),
[карта](../crates/tg-core/docs/data-chat-channel-map.md). Concrete Chat/Channel,
sole Peers owner и released-borrow phases, rights/participants/TL scalar updates.
337 строк карты (287 done/50 partial), native481 строк,19 новых tests.
Полные UserData02/Session13/14/29/History18 и внешний UI/API apply не подменяются.
Native pure proof отделён от own tests+чтение полного состояния. Cargo manifests/lock,
Claude files и чужие ветки не менялись. Интегрируется только port/data-chat-channel.
Сообщённую пользователем гонку ObjectAsync::drop не чинить: tg-runtime у Claude.

Проверки data-chat-channel: fmt/clippy/test workspace PASS,769 успешных результатов,
0 failed/3 прежних ignored. Интеграция своей ветки после проверки,без force push.

Пакет04 port/data-media от main0c38855, worktree data-media:
[handoff](handoffs/codex-data-media.md), [карта](../crates/tg-core/docs/data-media-map.md).
Реализованы Photos/Documents owner/convert/indices и model/pure phases, byte
leases, TL, filename/MIME contract, cache queue adapter. Wire PhotoKey/DocumentKey
остаются lookup selectors; pointers представлены отдельными stable RecordKey,
без media generation. Это уточнение эскиза stage4-plan после чтения collision
бodies; reserved docs-only audit file не менялся. Lookup-key и mutable wire id
могут различаться ровно как C++. Source repeated conversion quirk сохранён.
307 строк188 done/119 partial/0 pending; native1717,33 targeted tests/clippy PASS.
Финальные fmt/clippy/test workspace PASS после merge уже принятого main750204e:
809 successful (808+1 child),0 failed,3 existing ignored. Интеграция своей ветки
в main обычным fast-forward, push main+port/data-media без force. Не интегрировать private port/localstorage:
он у Claude; уже принятый main750204e сохранить. Области Claude и чужие файлы
не менялись. Известная гонка ObjectAsync::drop остаётся владельцу tg-runtime.

Предварительная полная проверка на базе0c38855: fmt --check PASS, clippy --workspace --all-targets -D warnings PASS, test --workspace PASS (802 успешных результата,0 failed,3 прежних ignored). После совмещения с уже принятым main750204e проверки повторяются.

После точного переноса Windows post-dialog lowercasing и расширения native до1717 строк все три проверки workspace повторены: fmt/clippy/test PASS,809 successful,0 failed,3 прежних ignored. Лог /tmp/data-media-final-checks.log.

Срез06/07 port/data-misc от main4265492, отдельный worktree data-misc:
[handoff](handoffs/codex-data-misc.md),
[карта](../crates/tg-core/docs/data-misc-map.md). Только tg-core data modules,
свои tests/reference tools/accounting.19 files3627LOC,15done/4partial;
443 map rows412done/31partial/0pending (h declarations/cpp bodies отдельно),
34 новых tests,1779 native rows/16 source hashes. AutoDownload effect из принятых
settings применяется к Full без fake SessionSettings; concrete Sender bridge и
Peers/Changes automatic clear проверены. Full peer persistence/chats list13/29,
UI emoji groups5 и UniqueGift/name16/01 остаются partial. Не трогать Claude
lang/countries/credits/tg-ui/codegen/runtime/mtproto и storage/localstorage, а также
чужие worktrees. Интегрируется только своя ветка, push main+ветки в gramtele без force.

Финальные три проверки data-misc workspace PASS:843successful,0failed,3 старых
ignored;34 новых tests. Известная runtime race локально не воспроизвелась.


Срез port/data-lists от main9575f65, отдельный worktree data-lists:
[handoff](handoffs/codex-data-lists.md),
[карта](../crates/tg-core/docs/data-lists-map.md).16files4043LOC,12done/4partial,
511map rows483done/28futurepartial/0pending. Actual storage reuse, no fake owners,
25new tests,804native rows/21source hashes. History18–20 + scheduled05/SavedMusic12,
loader3/4/pixels/EmojiPtr5 и настоящие API26/owner02/29 bindings остаются по карте.
Claude lang/countries/credits/tg-ui/codegen/runtime/mtproto/localstorage и чужие
worktrees/ветки не менялись и не интегрируются. Новых зависимостей нет.
Интеграция только port/data-lists после проверок, push main+ветки в gramtele
без force. Известную runtime thread-drop race не исправлять в этом пакете.

Финальные fmt/clippy/test workspace data-lists PASS:868successful(867+1child),
0failed/3existingignored,25 новых tests. Native804 exact rows twice,21hashes,
maprefs/diffchecks PASS. Full workspace tests пройдены один раз после завершения
пакета; финальный lint cache проверяет последние source/key изменения.
