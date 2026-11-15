# port/data-small — независимый срез моделей и сервисов

15.11.2026. База main7bc4737, отдельный worktree
`/home/yarik/Projects/ayugram-rs-wt/data-small`, ветка `port/data-small`.
Original read-only0f2a31bb420de98f331990432265b131b8d836b5. Все10 целевых файлов
прочитаны целиком с usage sites settings/intro/player/storage/search. Фактически
1908 строк, плюс нужные deserialize183 и platform WebAuthn header50.

## Статус каждого файла

| Original SourceFiles/ | h | cpp | Реализовано / остаток |
|---|---|---|---|
| data/components/passkeys | done (81) | done (210) | все поля/FromTL/list/register/delete/cache/timeouts/config/support facts, actual Sender requests/replies, Init/Finish login, real Instance auth-key lookup/DC switch; AppConfig/Session/WebAuthn support — required real owner29/33 |
| data/components/recent_peers | partial (58) | partial (183) | список/события/order,48-entry Qt frame/rollback, weak thread history32; existing PeerCodec actual Serialize::peer13/29, real Thread binding08/16/18, userpic cache5 |
| data/data_reply_preview | partial (50) | partial (146) | только чистая геометрия prepare; **весь класс** с настоящими Image/Prepare/thumbnail selection/cache/loaded/good/checked state —5, загрузка thumbnails3/4; фиктивного класса нет |
| data/data_streaming | partial (102) | partial (253) | weak Reader/Document sharing/pruning/forceRemote, alternate qualities metadata, keepalive5000/timer; concrete backend associated types/factories/Reader/Document/loaders/cache — storage/streaming7; History media qualities guard18–20 |
| data/data_search_controller | partial (221) | partial (604) | все TL filter/request/parse data-фазы, actual SparseIdsList/builders/merged viewers, cache/migrated/save/restore/dedup, delayed queries/timer; actual History insertion/facts/events/Histories scheduler18–20/26 — required key contracts |

Целевые10 ledger: **2 done /8 partial**. Нужные зависимости
`data/data_passkey_deserialize.{h,cpp}` **done**: все настоящие структуры и
QJson/base64/clientData функции, без добавления зависимостей. Значения настоящего
`platform/platform_webauthn.h` перенесены в tg-core platform module; header ledger
**partial** (tg-platform6), actual support/ceremonies backend33 ещё отсутствует.
Он должен использовать/re-export эти значения без второй несовместимой копии.

[Карта функций](../../crates/tg-core/docs/data-small-map.md): **164 строки,
136 done /28 partial /0 pending**. h declarations/inline и cpp definitions
считаются отдельно. Каждая done имеет тест, partial — будущий пакет. Генератор
проверяет имена тестов; комментарии/call statements не считаются функциями.

## Контракты и поведение

PasskeysOwner обязан читать настоящий AppConfig/support факт и отправлять
реальный Request через Sender bridge; default owner отсутствует. Cached
RegisterData скопирован до callback; borrows закрываются до fire/done/fail.
Session/API replies должны быть доставлены на owner thread с реальным lifetime
29/26. Callback map хранит конкретные registration/delete/list callbacks; drop
освобождает их, не подменяет исходного Session/API lifetime. No UI/network stub.
Free login функции работают с настоящими Sender/Instance/getKeysForWrite,
BareDcId(initialDc)/setMainDcId/toDC. ApiId/ApiHash — обязательные реальные inputs
от app credentials owner; тесты используют явно фиктивные значения без сети.
Платформенный caller, как в C++, проверяет result.success/error до обращения к
Passkeys; сам метод register их не фильтрует. Platform::RegisterKey/Login не
подменены функциями, возвращающими success/unsupported.

RecentPeers использует SessionScope/PeerKey и **существующий** TopPeers::PeerCodec.
Это контракт настоящего Serialize::writePeer/readPeer; он обязан вернуть None
при любой ошибке чтения (Qt Reader не имеет sticky status). Тестовый codec
с одним ID служит только проверке frame/order/rollback и не объявлен форматом
пира. readSearchSuggestions вызывается в исходных местах, writeDelayed — после
изменения и fire. applyLocal публикует прочитанный префикс по мере чтения; при
ошибке очищает всё без события, как в источнике. Заголовок/count BE Qt_5_1,
сериализуются первые48, список в памяти не обрезается.

ThreadKey имеет только исходные History/Topic/Sublist варианты и существующие
PeerKey/TopicKey/SublistKey. Обязательный thread_lifetime возвращает weak token
настоящего **экземпляра** Thread; key existence недостаточно. Новый Topic с тем
же ключом должен получить новый token. Модель не аллоцирует поддельные Thread
records/tokens; guard даёт будущий настоящий владелец08/16/18. Порядок opens,
лимит32, prune expired/rotate/remove сохранены. Ui::PeerUserpicView полностью5.

Streaming<B> управляет Weak/Rc настоящих backend B::Reader/B::Document, не
определяет их заглушки. create_reader обязан создать настоящий loader/cache
reader7, set_other_qualities — вызвать реальный media document. Вектор alternate
qualities фильтрует выбранный ключ/height0 и делает source uint32 casts; реальные
resolveQualities(HistoryItem*) и size/resolveVideoQuality — required model facts.
Получение существующего Document обновляет alternate список только непустым
результатом, пустой не стирает прежний. Фото не принимает original/context в API.
Разные document/photo registries при prune сохраняются; expired entries чистятся
только при успешном создании, loader failure старую weak запись не меняет.
Scope проверяется; timer отменяется перед освобождением retained documents,
backend остаётся жив во время их деструкторов. Expired values дропаются без
RefCell borrow. Полная real-backend lifetime/session интеграция остаётся7/29.

SearchOwner содержит только фактические исходные вызовы: inputPeer,
processUsers/Chats/Topics/ptsReceived, addNewMessage(Existing)→MessageKey,
position/shared-media/forward/topic/sublist facts, live-key removal/history clear
streams, Histories send/cancel/finish. Fake HistoryItem/History/Session отсутствуют.
SearchController использует **существующие** storage SparseIdsList и data builders;
нет второго алгоритма sparse merge. source kCount представлен Option::None.
Query Ord сохраняет порядок полей, kCount после13 типов и QString UTF16 order.
Уничтожение cache entry/restore отменяет pending requests. Собственные borrows
закрываются перед callback/owner effects/подписками. Requests используют реальный
logical Histories ID, а API half — конкретный Sender bridge; scheduling/
finish/cancel backend остаётся18–20/26, не симулируется немедленным ответом.

Viewers должны закончиться до уничтожения Data/cache entry и save/restore, как
требует исходный pointer lifetime. Rust stale requestMore даёт precondition panic,
не предоставляет фиктивный живой cache record; C++ имеет dangling-pointer UB.
Сохранённый SparseIdsList переносит состояние/stream через Rc lease; moved-from
slot получает реальный пустой list. Это не гарантия для C++ invalidated viewers.
Delayed controller требует setQueryFast/инициализацию до currentQuery/setQuery;
C++ иначе разыменовывает cache.end(), Rust диагностирует precondition.

## Native и отличия

[Native recipe](../../crates/tg-core/tests/reference-tools/data-small/README.md),
сборка вне клона `/tmp/ayugram-data-small-native`. **135 строк**, две идентичные
съёмки,16 SHA256:4 FromTL optional flags,9 credentials с exact TL и compact
clientData JSON,14 DC handles,14 filters,18 offset cases,50 geometry,26 JSON.
Весь original passkey_deserialize.cpp неизменён; оригинальные чистые функции/
фрагменты остальных файлов, настоящие Qt Core/headers/schema/Storage enum.
Geometry parameterizes только actual UI size/DPR inputs; не доказывает Image.
Search offsets проверяют фрагмент/limits, **не** полный PeerData request body.
Остальные TL input fields покрыты own tests + чтение.

Нет native full-state proof для Session/Passkeys service, RecentPeers peer codec
или Thread владельцев, streaming loaders/documents/retention интеграции, History
insertion/filter/scheduling/events, ReplyPreview images/loaded/Prepare. По заданию
это own tests + полное чтение data-фаз; backend/UI тела отмечены partial.
Никакие Session/PeerData/History/Image тела в эталоне не подменены.

Сохранённые особенности исходника:

- initRegistration не дедуплицирует concurrent requests; кеш валиден строго
  age<timeout. List refresh строго age>5000, последний timestamp выставляется
  при отправке и сохраняется при fail. requestList запускает запрос при вызове,
  возвращаемый producer ленив; недавний fail всё равно даёт initial event.
- register append допускает дубликаты; fail очищает pending registration.
  delete сбрасывает known/timestamp, запускает loadList **до** done, старый список
  сохраняется до ответа. canRegister сохраняет C++ size_t/int conversion даже
  при отрицательном config max. possible/support читаются живыми фактами.
- registration credential ID URL-base64 с '=', login без '='; id/raw_id равны.
  userHandle допускает пустую вторую часть, требует ровно2 части и DC1..9999.
  Native показал: U3000 whitespace отвергается QString::toInt. Rust использует
  ASCII numeric whitespace, вместо более широкого trim().
- bump не ограничивает RecentPeers48 в памяти; remove отсутствующего всё равно
  пишет delayed; clear пустого всё равно fire/write. applyLocal не dedup и не
  fire; remove убирает только первое совпадение. chatOpenPush не читает local.
- streaming forceRemote заменяет живой local reader, но не существующий media
  Document; loader failure сохраняет прежний reader. keepAlive продлевает till,
  не перезапускает активный timer; раннее пробуждение вычисляет новый min wait.
- Search range расширяется по **всем** added items, даже не попавшим в media
  filter; пустой filtered result при ненулевом around открывает соответствующие
  границы до0/MAX. Global offset обновляется и для отфильтрованных forward items.
- Search request fail вызывает finish, но не удаляет request key/lifetime, что
  блокирует retry до уничтожения cache Data. Success удаляет lifetime/cancel
  **до** parse/addSlice/finish. Empty query очищает весь cache, затем создаёт entry.
- setQueryFast/cached-query path не отменяют старый debounce timer: он может
  переключить query обратно на старый pending. Только равный текущему query
  отменяет timer. Restore сам не fire currentQueryChanges.
- ParseGlobalMediaResult(messagesNotModified) разыменовывает null messages в C++;
  Rust даёт явный precondition panic, без выдуманного пустого результата.

Rust adaptation: login free functions возвращают настоящий Sender RequestID /
Option для явной отмены/guard, исходные C++ функции void; запросы/порядок прежние.
Strong/weak ownership и typed-key ordering заменяют raw pointers. Одновременный
порядок destructor вызовов в keptAlive по C++ allocator pointer order не обещан.
C++ signed overflow, count>INT_MAX/огромные аллокации в повреждённом RecentPeers,
invalidated iterators/viewers и QString lone surrogates вне доказанного домена.
Source-valid geometry/MsgId arithmetic проверены; overflow не «исправлен» новым
форматом. Deserialized malformed peer records зависят от настоящего PeerCodec.

## Проверки и интеграция

19 новых тестов:18 own services/policies +1 native135 comparison. Виртуальные
ManualClock/TimerHost, без sleep. Реальные Sender/Instance callbacks, без сокетов.
Новых dependencies/manifest/lock изменений нет (serde_json уже обычная зависимость).
Claude scope, все чужие worktrees/ветки и исходный клон не менялись.
Known runtime ObjectAsync::drop race не исправляется, владелец Claude.

FINAL_DATA_SMALL_CHECK_PENDING
