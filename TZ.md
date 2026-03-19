# Техническое задание: Harpoon

## 1. Общее описание проекта

**Harpoon** — это MITM/traffic-analysis proxy на Rust для Linux, предназначенный для:

* прозрачного и непрозрачного проксирования TCP и UDP трафика;
* редиректа трафика на другой IP/порт;
* перехвата трафика, идущего на уже занятый локальный порт, при наличии `nftables`;
* дублирования трафика;
* опционального TLS MITM для TCP;
* базовой фильтрации полезной нагрузки;
* экспорта трафика/событий во внешние анализаторы;
* опционального web-интерфейса;
* использования как:

  1. самостоятельного бинаря,
  2. библиотеки для встраивания в другие Rust-проекты.

Проект изначально разрабатывается под **Linux**. Поддержка `nftables`, `TPROXY`, `IP_TRANSPARENT` и related features считается Linux-specific.

---

## 2. Основные цели

1. Сделать компактную и расширяемую систему перехвата/редиректа трафика.
2. Жёстко разделить:

   * **библиотеку** с движком обработки трафика,
   * **бинарь** с UI, web, конфигами, daemon/control-plane.
3. Сделать минимальный MVP, который уже полезен без `nftables`.
4. При наличии `nftables` добавить advanced-возможности без дублирования логики.
5. Избежать тяжёлых зависимостей в библиотеке.
6. Заложить основу для дальнейшего API/SDK для других языков, но не реализовывать bindings в MVP.

---

## 3. Что должно быть в MVP

### 3.1. Обязательные возможности MVP

1. TCP proxy:

   * слушать локальный адрес/порт;
   * пересылать трафик на другой IP/порт;
   * дублировать трафик;
   * базово фильтровать полезную нагрузку;
   * опционально делать TLS terminate / re-encrypt для TCP.

2. UDP proxy:

   * слушать локальный UDP-порт;
   * пересылать датаграммы на другой IP/порт;
   * корректно проксировать ответы назад клиенту;
   * поддерживать session table для UDP;
   * дублировать UDP-трафик;
   * фильтровать UDP payload по простым правилам.

3. Control plane:

   * локальный Unix domain socket;
   * команды для запуска/остановки, списка правил, состояния, статистики;
   * CLI как клиент к демону.

4. Бинарь:

   * загрузка конфига из файла;
   * запуск в foreground;
   * опциональный daemon mode;
   * опциональный web-интерфейс через feature.

5. Разделение на библиотеку и приложение:

   * библиотека не должна зависеть от clap/web/config manager;
   * приложение может тянуть serde/toml/web/daemonization.

### 3.2. Что допускается отложить после MVP

1. Полноценный transparent UDP source-preserving proxy.
2. DTLS.
3. gRPC exporter.
4. Прямое управление `nftables` через netlink.
5. Bindings для Python/Go.
6. Полноценный rich web UI.

---

## 4. Что не входит в MVP

1. Поддержка Windows/macOS.
2. Полноценный HTTP-aware reverse proxy.
3. Парсинг/модификация HTTP/2, WebSocket, gRPC как L7-протоколов.
4. DTLS MITM.
5. FFI bindings для других языков.
6. Распределённый кластер Harpoon.
7. Высокоуровневый DSL-язык конфигов внутри библиотеки.

---

## 5. Архитектурные принципы

### 5.1. Разделение на library и binary

Проект должен быть реализован как Rust workspace с двумя основными crates:

1. **`harpoon-core`**

   * библиотека;
   * содержит движок проксирования и обработки трафика;
   * принимает уже готовый типизированный `Config`;
   * не знает ничего про CLI, web, toml/yaml/json, daemonization.

2. **`harpoon-app`** (имя бинаря может быть `harpoon`)

   * самостоятельный бинарь;
   * содержит:

     * CLI,
     * загрузку конфига,
     * daemon/control socket,
     * web UI,
     * интеграцию с `nftables`,
     * преобразование app-level config в `harpoon_core::Config`.

### 5.2. Главный принцип передачи конфига

Библиотека должна принимать **строго типизированный runtime config**, например:

* `harpoon_core::Config`
* `harpoon_core::Engine::run(config)`

Все строки, парсинг адресов, CLI-флаги, TOML/YAML, web-формы должны обрабатываться только в приложении.

---

## 6. Рекомендуемая структура репозитория

```text
harpoon/
  Cargo.toml
  Cargo.lock
  README.md
  LICENSE
  docs/

  crates/
    harpoon-core/
      Cargo.toml
      src/
        lib.rs
        config.rs
        error.rs
        engine/
          mod.rs
          tcp.rs
          udp.rs
          pipeline.rs
        types/
          mod.rs
          endpoint.rs
          rule.rs
          filter.rs
          event.rs
          stats.rs
        tls/
          mod.rs
          passthrough.rs
          mitm.rs
        export/
          mod.rs
          sink.rs
          tcp_framed.rs
          uds.rs

    harpoon-app/
      Cargo.toml
      src/
        main.rs
        app.rs
        config/
          mod.rs
          schema.rs
          load.rs
        convert.rs
        control/
          mod.rs
          proto.rs
          server.rs
          client.rs
        daemon/
          mod.rs
          run.rs
          state.rs
        nft/
          mod.rs
          render.rs
          apply.rs
        ui/
          cli/
            mod.rs
            args.rs
            commands/
          web/
            mod.rs
            server.rs
            assets/
```

---

## 7. Платформа и ограничения

1. Целевая ОС: **Linux**.
2. Базовый режим без `nftables` должен работать на любой Linux-системе без advanced hooks.
3. Режим перехвата уже занятого порта, transparent proxy, TPROXY, TEE — только если в системе есть `nftables` и нужные kernel features.
4. Первая версия может быть IPv4-first.
5. IPv6 желательно заложить в типах сразу, но можно реализовать позже.

---

## 8. Основные сущности библиотеки

### 8.1. Config

Типизированная структура, передаваемая в `harpoon-core`.

Примерный состав:

* список правил;
* глобальные лимиты/таймауты;
* параметры буферов;
* параметры логирования/метрик;
* параметры экспортёров;
* параметры TLS-кэша.

### 8.2. Rule

Правило должно описывать:

* имя;
* протокол (`TCP` / `UDP`);
* входную точку (`listen endpoint` или `transparent interceptor`);
* действие редиректа;
* режим дублирования;
* фильтры;
* режим TLS;
* режим экспорта;
* таймауты/ограничения.

### 8.3. Endpoint

Типизированный адрес:

* IP + port + protocol family.

### 8.4. Event

Событие, которое может генерировать движок:

* входящий пакет/чанк;
* исходящий пакет/чанк;
* match filter;
* drop;
* создана UDP session;
* session timeout;
* TLS handshake success/fail;
* rule activated/deactivated;
* exporter error.

### 8.5. EngineHandle

Handle, возвращаемый после запуска движка:

* stop/shutdown;
* stats snapshot;
* optional event stream subscription.

---

## 9. Функциональные требования

---

## 10. TCP-проксирование

### 10.1. Базовые требования

Harpoon должен уметь:

1. слушать TCP сокет;
2. принимать входящее соединение;
3. устанавливать соединение с upstream;
4. проксировать трафик в обе стороны;
5. дублировать трафик;
6. применять фильтры к данным;
7. считать статистику.

### 10.2. Режимы TCP

1. **Passthrough**

   * просто проксирование без TLS MITM.

2. **TLS MITM**

   * terminate TLS от клиента;
   * при необходимости поднимать TLS к upstream;
   * давать доступ к plaintext трафику фильтрам/экспортёрам.

3. **Local redirect**

   * слушаем на одном порту и отправляем на другой локальный/удалённый адрес.

4. **Transparent intercept via nftables**

   * только при наличии `nftables`;
   * перехват соединений, идущих на другой локальный сервис/порт.

---

## 11. UDP-проксирование — ключевая часть проекта

UDP не является потоковым протоколом и не имеет соединений в классическом смысле, поэтому реализация должна строиться на модели **псевдо-сессий**.

### 11.1. Общий принцип

Harpoon должен:

1. слушать UDP сокет;
2. принимать входящие датаграммы;
3. определять, к какой UDP session относится датаграмма;
4. создавать новую session при первом пакете;
5. пересылать датаграмму на upstream;
6. принимать ответы от upstream;
7. отправлять ответы обратно исходному клиенту;
8. удалять session по idle timeout.

### 11.2. Что считать UDP session

Ключ UDP session должен включать минимум:

* `rule_id`
* `client_src_ip`
* `client_src_port`
* `original_dst_ip`
* `original_dst_port`
* `ip_family`

Где:

* в обычном user-space режиме `original_dst` берётся из config rule;
* в transparent/TPROXY режиме `original_dst` должен получаться из kernel metadata.

Это нужно для корректного различения:

* нескольких клиентов;
* нескольких целевых адресов;
* разных transparent flows.

### 11.3. Структура UDP session

Каждая UDP session должна хранить:

* session key;
* время создания;
* время последней активности;
* сокет/контекст для общения с upstream;
* счётчики:

  * packets client->server,
  * packets server->client,
  * bytes client->server,
  * bytes server->client;
* optional metadata:

  * original dst,
  * intercept mode,
  * exporter state.

### 11.4. Таймауты UDP session

Должны быть настраиваемые idle timeouts:

* default: 30 секунд;
* возможность задать больше для “long-lived UDP”.

При истечении таймаута:

* session удаляется из таблицы;
* связанные ресурсы освобождаются;
* генерируется event о timeout cleanup.

### 11.5. Режимы UDP-проксирования

#### 11.5.1. Обычный user-space UDP relay

Без `nftables` Harpoon должен:

1. слушать указанный UDP-порт;
2. при приходе датаграммы от клиента:

   * найти или создать session;
   * отправить данные на configured upstream;
3. получать ответы от upstream;
4. отправлять их назад клиенту.

Это базовый и обязательный режим.

#### 11.5.2. UDP redirect to local/remote port

Harpoon должен уметь:

* принимать UDP на `listen`;
* отправлять на `target`;
* возвращать ответы клиенту.

Пример:

* слушаем `0.0.0.0:5353`
* форвардим на `10.0.0.5:53`

#### 11.5.3. UDP transparent intercept via nftables/TPROXY

При наличии `nftables` Harpoon должен поддерживать transparent interception UDP трафика, идущего на другой локальный/удалённый адрес.

Требования:

1. `harpoon-app` настраивает `nftables` rules для перехвата UDP.
2. `harpoon-core` получает датаграммы уже с transparent listener.
3. Для каждого пакета движок должен получать **original destination**.
4. Session key должен учитывать original destination.
5. Upstream для такого пакета определяется либо:

   * original destination,
   * либо явно заданным redirect target,
   * либо app-level routing policy.

### 11.6. Важное ограничение по UDP transparent mode

Для MVP достаточно следующей модели:

#### Поддержать:

* transparent intercept входящего UDP;
* получение original destination;
* проксирование на upstream;
* возврат ответа клиенту.

#### Не обязано входить в MVP:

* полноценное source-preserving transparent UDP proxy, где upstream видит реальный IP клиента.

То есть в MVP допустимо, что upstream в UDP-режиме видит **адрес Harpoon**, а не исходного клиента. Это резко упрощает реализацию и делает систему стабильнее.

Полноценный fully-transparent source-preserving UDP proxy считать advanced/future feature.

### 11.7. Обратный путь UDP

Ответы от upstream должны:

1. сопоставляться с конкретной UDP session;
2. отправляться обратно клиенту;
3. сохранять границы датаграмм;
4. не смешиваться между session.

### 11.8. Packet boundaries

Для UDP обязательно:

* сохранять границы датаграмм;
* не превращать UDP в stream;
* фильтры и экспортёры должны работать по датаграммам, а не по чанкам потока.

### 11.9. Большие датаграммы и MTU

Требования:

1. Поддерживать приём/передачу датаграмм вплоть до максимально допустимого размера UDP payload.
2. Пользовательский код не должен вручную собирать IP fragments.
3. Реализация может полагаться на kernel networking stack для фрагментации/реассемблинга.
4. Должен быть configurable upper bound:

   * если датаграмма больше разрешённого лимита, её можно отбросить и залогировать.

### 11.10. Duplicate для UDP

Harpoon должен уметь дублировать UDP датаграммы:

* до upstream;
* после upstream;
* в exporter;
* в отдельный дублирующий endpoint.

Дублирование должно сохранять границы датаграмм.

### 11.11. UDP filters

Фильтры для UDP применяются к payload каждой датаграммы.
Должны поддерживаться действия:

* `pass`
* `drop`
* `tap-only`

Фильтр может применяться:

* до редиректа;
* после редиректа;
* к входящему и/или исходящему направлению.

### 11.12. UDP и TLS

В MVP:

* TLS относится только к TCP.
* **DTLS не реализовывать**.
* В конфиге/правилах явно указывать, что UDP+TLS MITM не поддерживается.
* DTLS вынести в future scope.

---

## 12. Redirect и intercept

### 12.1. Поддерживаемые действия

Harpoon должен поддерживать:

1. Redirect TCP/UDP порта на другой IP/порт.
2. Redirect TCP/UDP порта на свой IP/другой порт.
3. Intercept уже занятого локального TCP/UDP порта через `nftables` (advanced mode).
4. Traffic duplication.
5. Optional TPROXY mode.

### 12.2. Поведение без nftables

Если `nftables` недоступен:

* Harpoon должен работать как обычный userspace proxy;
* невозможно перехватывать трафик, идущий в уже занятый процесс;
* невозможно transparent redirect на уровне ядра;
* при попытке включить такие правила должен возвращаться понятный error.

### 12.3. Поведение с nftables

Если `nftables` доступен:

* приложение может создавать/обновлять свои tables/chains;
* всё steering трафика делается через `nftables`;
* `harpoon-core` не должен содержать код парсинга CLI для `nft`.

---

## 13. TLS MITM

### 13.1. Область поддержки

TLS MITM нужен только для TCP.

### 13.2. Режимы TLS

1. `passthrough`
2. `terminate`
3. `mitm` (terminate inbound + establish outbound TLS)

### 13.3. Требования

1. Поддержка CA certificate + private key.
2. Возможность динамически генерировать leaf certificates.
3. Возможность кэшировать выданные сертификаты.
4. Доступ к plaintext трафику внутри pipeline.
5. Обработка ошибок handshake.
6. Логирование SNI/ALPN, если доступно.

### 13.4. Необязательное для MVP

* глубокая HTTP/L7-aware модификация;
* HSM/PKCS#11;
* клиентская аутентификация mTLS.

---

## 14. Фильтрация трафика

### 14.1. Поддерживаемые фильтры

Минимум:

1. `substr`
2. `bsubstr`
3. `regex` — опционально через feature

### 14.2. Действия фильтров

1. `pass`
2. `drop`
3. `tap-only`

### 14.3. Направления

Фильтр может работать по направлениям:

* client -> server
* server -> client
* both

### 14.4. Требование по зависимостям

* `regex` не должен быть mandatory dependency для минимального билда;
* он должен включаться через feature.

---

## 15. Export / внешняя интеграция

### 15.1. Общая идея

Harpoon должен уметь отправлять события/данные во внешний анализатор.

### 15.2. В MVP поддержать

1. Unix domain socket exporter
2. framed TCP exporter

### 15.3. Почему не gRPC в MVP

gRPC не использовать для основного dataplane/export path в MVP, потому что:

* это тяжёлый dependency graph;
* лишний overhead для high-rate traffic shipping;
* усложняет бинарь.

### 15.4. gRPC

gRPC можно оставить как optional future feature:

* для control plane;
* или для metadata/events;
* но не как основной механизм перекачки сырого трафика в MVP.

### 15.5. Формат framed exporter

Рекомендуется простой framed-binary protocol:

* length prefix
* metadata
* payload

Поддержать versioning поля сообщения.

---

## 16. Control plane и daemon model

### 16.1. Общая модель

Бинарь должен поддерживать модель:

* daemon process;
* локальный control socket;
* CLI-клиент к control socket;
* optional web UI поверх того же control API.

### 16.2. Требования

1. `harpoon run` — запуск движка.
2. `harpoon stop` — остановка.
3. `harpoon status` — состояние.
4. `harpoon rules list/add/remove` — работа с правилами.
5. `harpoon stats` — статистика.
6. `harpoon events` — подписка на события.

### 16.3. Foreground/daemon

Поддержать:

1. foreground mode — обязателен;
2. daemon/background mode — желательно;
3. systemd-friendly mode — желательно.

Не нужно делать демонизацию единственным режимом запуска.

---

## 17. Web UI

### 17.1. Общий принцип

Web UI должен быть только в приложении, не в библиотеке.

### 17.2. Требования

1. Поддержка через cargo feature `web`.
2. Web UI должен работать поверх того же control plane/state, что и CLI.
3. Минимум:

   * список правил;
   * создание/удаление правила;
   * просмотр статистики;
   * просмотр событий/логов;
   * базовая форма настройки redirect/duplicate/filter/TLS.

### 17.3. Не тянуть web в core

`harpoon-core` не должен зависеть от HTTP server, HTML, websocket, templating и т.п.

---

## 18. Работа с конфигом

### 18.1. Разделение конфигов

Нужно два уровня конфигурации:

1. **AppConfig**

   * человеко-читаемый;
   * загружается из файла/CLI/web;
   * может содержать строки, удобные формы, shorthand.

2. **CoreConfig**

   * строгий;
   * типизированный;
   * передаётся в `harpoon-core::run()`.

### 18.2. Правило конвертации

Конвертация `AppConfig -> CoreConfig` должна жить в `harpoon-app`, а не в библиотеке.

### 18.3. Библиотека не должна знать про:

* clap;
* serde/toml/yaml/json как обязательные зависимости;
* web forms;
* environment variables;
* daemon flags.

---

## 19. Зависимости и ограничения по размеру

### 19.1. Общие требования

Нужно минимизировать размер и dependency graph, особенно в `harpoon-core`.

### 19.2. В `harpoon-core` допустимы

* базовые сетевые зависимости;
* `thiserror`;
* `log`;
* TLS crates только через feature;
* `regex` только через feature.

### 19.3. В `harpoon-core` нежелательны

* clap;
* axum/warp/actix;
* config manager;
* daemonization helpers;
* netlink crates на первом этапе.

### 19.4. В `harpoon-app` допустимы

* serde/toml/yaml;
* лёгкий CLI parser;
* web framework через feature;
* subprocess-based `nft`.

---

## 20. Интеграция с nftables

### 20.1. MVP-подход

В первой версии не нужно реализовывать прямое netlink-управление nftables.

Достаточно:

* генерировать `nft` ruleset как текст;
* применять его через subprocess `nft -f`.

### 20.2. Что должен делать app layer

1. Создавать свою table/chain.
2. Не ломать чужие правила.
3. Поддерживать install/update/remove.
4. Уметь clean rollback при ошибке.

### 20.3. Поддерживаемые режимы

1. REDIRECT
2. DNAT
3. TPROXY
4. TEE — если будет нужен и доступен

---

## 21. Ошибки и observability

### 21.1. Требования к ошибкам

Ошибки должны быть:

* типизированы;
* понятны;
* разделены по уровням:

  * config errors,
  * runtime I/O,
  * TLS,
  * nft apply,
  * exporter,
  * session handling.

### 21.2. Статистика

Нужно считать:

* bytes/packets per rule;
* active TCP connections;
* active UDP sessions;
* dropped packets;
* filter matches;
* TLS handshake success/fail;
* exporter queue/backpressure stats.

### 21.3. Логи

Поддержать уровни:

* error
* warn
* info
* debug
* trace

---

## 22. Требования к производительности

1. Не копировать данные без необходимости.
2. Для UDP сохранять datagram boundaries.
3. Не использовать gRPC как default dataplane exporter.
4. Избегать чрезмерного выделения памяти на каждый пакет/чанк.
5. Для UDP session table использовать эффективную hash map + periodic cleanup.
6. Предусмотреть backpressure или controlled dropping для exporters.

---

## 23. Безопасность

1. Если включён TLS MITM, приватный CA ключ должен храниться безопасно.
2. Доступ к control socket должен быть ограничен локальным пользователем/группой.
3. Правила, связанные с `nftables`, должны применяться аккуратно, без глобального повреждения системы.
4. При ошибке нельзя оставлять систему в полубитом состоянии с частично применёнными правилами.
5. Для web UI желательно предусмотреть bind only to localhost по умолчанию.

---

## 24. Тестирование

### 24.1. Unit tests

Обязательны для:

* парсинга app config;
* конвертации `AppConfig -> CoreConfig`;
* фильтров;
* session key logic;
* timeout cleanup.

### 24.2. Integration tests

Нужны для:

* TCP redirect;
* UDP relay;
* UDP sessions;
* duplicate;
* TLS MITM basic path;
* `nftables` integration в isolated environment.

### 24.3. Рекомендация

Для integration tests использовать:

* network namespaces;
* локальные echo services;
* UDP echo service;
* isolated `nftables` test rules.

---

## 25. Требования к API библиотеки

### 25.1. Базовый API

Библиотека должна предоставлять простой API вида:

* `Engine::run(config) -> Result<EngineHandle>`
  или
* `run(config) -> Result<EngineHandle>`

### 25.2. Требования к API

1. API должен принимать только `CoreConfig`.
2. Библиотека не должна сама читать файлы конфигов.
3. Библиотека не должна сама парсить CLI.
4. Библиотека должна быть пригодна для embed в другой Rust-проект.

---

## 26. Рекомендуемый порядок реализации

### Этап 1

1. `harpoon-core`

   * типы;
   * Config;
   * TCP forward;
   * UDP relay с session table;
   * basic filters;
   * basic exporter abstraction.

2. `harpoon-app`

   * AppConfig;
   * conversion layer;
   * CLI;
   * foreground run.

### Этап 2

1. daemon model;
2. control socket;
3. stats/events;
4. local config reload.

### Этап 3

1. `nftables` integration;
2. redirect/DNAT;
3. transparent TCP/UDP intercept foundation.

### Этап 4

1. TLS MITM;
2. exporter improvements;
3. web UI.

### Этап 5

1. performance tuning;
2. optional gRPC;
3. optional language bindings;
4. optional advanced transparent UDP source-preserving mode.

---

## 27. Ключевые решения, которые нужно соблюдать

1. **Библиотека и приложение должны быть жёстко разделены.**
2. **`harpoon-core` не должен тянуть CLI/web/config-parser зависимости.**
3. **UDP должен быть реализован через session table, а не как поток.**
4. **Для UDP must-have:**

   * session key,
   * idle timeouts,
   * correct reverse path,
   * datagram boundaries,
   * transparent intercept support с original destination metadata.
5. **В MVP достаточно intercept-only UDP transparent mode без полного source-preserving.**
6. **TLS только для TCP в MVP.**
7. **gRPC не использовать как основной канал переноса трафика в MVP.**
8. **`nftables` использовать как optional accelerator/traffic steering layer, а не как обязательное условие работы Harpoon.**

---

## 28. Итоговый ожидаемый результат

Должен получиться проект, в котором:

* без `nftables` Harpoon работает как полноценный TCP/UDP userspace proxy с фильтрацией, дублированием и базовым экспортом;
* с `nftables` Harpoon получает дополнительные возможности:

  * transparent intercept,
  * redirect уже занятого порта,
  * TPROXY-based capture,
  * advanced steering;
* библиотека может быть встроена в другой Rust-анализатор;
* бинарь предоставляет CLI, daemon mode, control socket и optional web UI;
* архитектура позволяет потом добавить bindings для Python/Go без переделки ядра.

---

## 29. Правила коммитов

Каждый подпункт должен быть закоммичен. Формат коммита: "UPD: ..., Feat: ..., Impl: ..., Fix: ...". 
Все комментарии в программе и коммиты должны быть лаконичными и на английском языке.
Переизбыток комментариев - это проблема.
