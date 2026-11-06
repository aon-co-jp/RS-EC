# RS-EC

**開発開始日: 2026-07-21**(このリポジトリのGitHub作成日)

[EC-CUBE](https://www.ec-cube.net/)のRust＋[RPoem](https://github.com/aon-co-jp/RPoem)版です。新規開発開始。
実際の決済連携(Stripe等)を目指します。

## 現状(v0.1.0)

> ⚠️ **正直な開示**: v0.1.0時点では商品カタログ(Product)のCRUD、
> カテゴリ管理、お気に入り(wishlist)、OTPログイン(管理者+登録
> アカウント、自己申請→管理者審査)のみを実装している。EC-CUBEが持つ
> 以下の機能は**まだ一切無い**:
>
> - **カート・注文・決済連携(実決済は一切未実装。Stripe等のゲートウェイ
>   呼び出し・カード情報の取り扱いは一切行っていない)**。お気に入り
>   機能はあるが、これは「保存した商品IDのリスト」にすぎず、数量・
>   合計金額・注文作成・決済のいずれも持たないため**カートではない**。
> - 会員管理(ポイント等。お気に入りのみ実装済み)
> - 配送・在庫管理(在庫数フィールドはあるが自動引き落とし等は無し)
> - プラグイン機構・管理画面(APIのみ、UIは無し)
> - `aruaru-db`/PostgreSQL DUAL DB構成(現状はJSONファイル永続化のみ)

実装済みのAPI:

- `GET /healthz` — ヘルスチェック
- `POST /api/auth/request-otp` / `POST /api/auth/verify-otp` / `POST /api/auth/logout` — OTPメールログイン(管理者+登録アカウント)
- `POST /api/accounts` / `GET /api/accounts` — 登録済みメールアドレスの追加・一覧(管理者のみ)
- `POST /api/accounts/request` — アクセス許可の自己申請(認証不要)
- `GET /api/accounts/requests` / `POST /api/accounts/requests/:id/decide` — 申請一覧・審査(管理者のみ)
- `POST /api/products` / `GET /api/products` — 商品の作成・一覧(カタログへの閲覧/編集権限が必要、`?category=<id>`で絞り込み可)
- `GET /api/products/:id` / `PUT /api/products/:id` / `DELETE /api/products/:id` — 商品の取得・更新・削除(所属カテゴリの変更含む)
- `GET /api/categories` / `POST /api/categories` — カテゴリ一覧(公開)・新規作成(管理者のみ)
- `DELETE /api/categories/:id` — カテゴリ削除(管理者のみ)
- `GET /api/favorites` / `POST /api/favorites` — 自分のお気に入り商品一覧取得・追加(要ログイン、**カートではない**)
- `DELETE /api/favorites/:product_id` — お気に入りから削除(要ログイン)

商品は`draft`(下書き)/`on_sale`(販売中)/`sold_out`(売り切れ)の3ステータス。
永続化はJSONファイル(`RSEC_DATA_DIR/products.json`・`categories.json`・
`favorites.json`)。詳細な設計方針・今後の予定は`CLAUDE.md`の
HANDOFFセクションを参照。

## インストール(ビルド済みバイナリ、インストーラー付き)

タグ付きリリース(`vX.Y.Z`)ごとに、GitHub Actions
(`.github/workflows/release.yml`)がLinux・Windows向けバイナリを
自動ビルドし、[GitHub Releases](https://github.com/aon-co-jp/RS-EC/releases)へ添付する。

### Linux(AlmaLinux・Ubuntu・Debian・Fedora・RHEL等、systemdを使う主要ディストリ共通)

静的リンクされたmuslバイナリのため、ディストリ固有のライブラリ依存は無い。

```bash
curl -fsSL https://github.com/aon-co-jp/RS-EC/releases/latest/download/rs-ec-linux-x86_64.tar.gz | tar xz
sudo ./install.sh
sudo systemctl edit rs-ec   # RSEC_ADMIN_EMAIL等を設定
sudo systemctl enable --now rs-ec
```

### Windows / Windows Server

管理者権限のPowerShellで:

```powershell
Invoke-WebRequest -Uri "https://github.com/aon-co-jp/RS-EC/releases/latest/download/rs-ec-windows-x86_64.zip" -OutFile rs-ec.zip
Expand-Archive rs-ec.zip -DestinationPath rs-ec
cd rs-ec
.\install.ps1
```

## ソースからビルド

```bash
cargo build --release
```

## 環境変数

| 変数名 | 説明 | デフォルト |
| --- | --- | --- |
| `RSEC_DATA_DIR` | JSONデータの保存先ディレクトリ | `./data` |
| `RSEC_PORT` | リッスンポート | `8102` |
| `RSEC_ADMIN_EMAIL` | 管理者ログイン用メールアドレス | `admin@example.com` |
| `RSEC_SMTP_HOST` | SMTPホスト | (未設定なら`request-otp`は503) |
| `RSEC_SMTP_PORT` | SMTPポート | `587` |
| `RSEC_SMTP_USERNAME` | SMTPユーザー名 | — |
| `RSEC_SMTP_PASSWORD` | SMTPパスワード | — |
| `RSEC_SMTP_FROM` | 送信元メールアドレス | — |
| `RSEC_ACCOUNTS_LOCKED` | `true`(既定)で管理者以外のアカウント登録・承認を拒否 | `true` |

## テスト

```bash
cargo test
```

v0.1.0時点で22件(OTP認証6件+アクセス制御3件+アカウント管理2件+
カテゴリ永続化1件+お気に入り永続化1件+商品CRUD/アクセス制御/カテゴリ
絞り込み/お気に入りを含むハンドラ統合テスト9件)。

## ライセンス

Apache-2.0

詳細は`CLAUDE.md`(設計思想＆開発方針＆開発環境ルール)・`PORTING.md`(お引越しポーター)を参照。

## API 方針: REST 全廃・gRPC / Connect へ (2026-11-04)

### 日本語
- **REST は全廃する。新しい API は REST で作らない。** ブラウザや `curl` から呼ぶ API は **Connect の単純呼び出し**(`POST /<パッケージ>.<Service>/<Method>`、`content-type: application/json`、本文は JSON)、サービス間は **gRPC / gRPC-web**(RPoem の管理API `open_runo.admin.v1.Admin` と同じ流儀)。ストリーミングは、必要になってから。
- **既存の REST は、触るときに順に移す。** 移したら旧 REST の経路は消す。移行が終わるまで残るが、**新規には追加しない**。
- **JSON は RS-JSON に統一**する(読み込みは `from_str_secure` / `parse_secure` でサイズ・深さ・重複キーの上限つき、書き出しは `rust_json::fast`)。ブラウザの WASM に `prost` は入れない(サイズが増えるため)。
- **例外(自分たちの API ではなく、相手の規格が決めているもの)**: ACME、OAuth/OIDC のコールバック、第三者(GitHub など)からの Webhook、ヘルスチェック、静的ファイル配信、OTLP など。これらは規格どおりの HTTP を受ける。
- 参考実装: `RPoem`(`open_runo.admin.v1.Admin`)、`open-easy-web`(`server/src/connect.rs`、`proto/open_easy_web/admin/v1/admin.proto`)。
- ※ このリポジトリのコードが既に方針どおりかどうかは、各リポジトリの現状による。この節は方針の記録であり、コードの変更を意味しない。

### English
- **REST is being abolished. New APIs are not built as REST.** APIs called from browsers or `curl` use **Connect unary calls** (`POST /<package>.<Service>/<Method>`, `content-type: application/json`, JSON body); service-to-service calls use **gRPC / gRPC-web** (the same style as RPoem's admin API `open_runo.admin.v1.Admin`). Streaming only when it is actually needed.
- **Existing REST endpoints are migrated one by one, when they are touched.** The old REST route is removed once migrated. Until migration is finished they remain, but **no new ones are added**.
- **JSON is unified on RS-JSON**: reading with `from_str_secure` / `parse_secure` (size, depth and duplicate-key limits), writing with `rust_json::fast`. `prost` is not put into browser WASM (it would grow the binary).
- **Exceptions (shapes dictated by someone else's standard, not our own API)**: ACME, OAuth/OIDC callbacks, webhooks from third parties (e.g. GitHub), health checks, static file serving, OTLP, and so on. These accept plain HTTP as the standard requires.
- Reference implementations: `RPoem` (`open_runo.admin.v1.Admin`) and `open-easy-web` (`server/src/connect.rs`, `proto/open_easy_web/admin/v1/admin.proto`).
- Note: whether the code in a given repository already follows this policy depends on that repository. This section records the policy; it does not mean the code has been changed.

### Deutsch (KI-Übersetzung, redaktionell überarbeitet; noch nicht von Muttersprachlern geprüft)
- **REST wird vollständig abgeschafft; neue APIs entstehen nicht mehr als REST.** APIs, die aus dem Browser oder per `curl` aufgerufen werden, verwenden **unäre Connect-Aufrufe** (`POST /<Paket>.<Service>/<Methode>`, `content-type: application/json`, JSON-Body). Zwischen Diensten kommen **gRPC bzw. gRPC-web** zum Einsatz, im selben Stil wie die Admin-API von RPoem (`open_runo.admin.v1.Admin`). Streaming wird erst ergänzt, wenn es tatsächlich gebraucht wird.
- **Bestehende REST-Endpunkte werden schrittweise umgestellt, und zwar immer dann, wenn man ohnehin an ihnen arbeitet.** Sobald ein Endpunkt umgestellt ist, wird die alte REST-Route entfernt. Bis zum Abschluss der Migration bleiben die übrigen bestehen, es werden aber **keine neuen mehr hinzugefügt**.
- **JSON wird einheitlich über RS-JSON verarbeitet**: Gelesen wird mit `from_str_secure` bzw. `parse_secure` (mit Obergrenzen für Größe, Verschachtelungstiefe und doppelte Schlüssel), geschrieben mit `rust_json::fast`. `prost` wird nicht in das Browser-WASM aufgenommen, da es die Datei vergrößern würde.
- **Ausnahmen (Schnittstellen, deren Form ein fremder Standard vorgibt und die nicht zu unserer eigenen API gehören)**: ACME, OAuth/OIDC-Callbacks, Webhooks von Drittanbietern (z. B. GitHub), Health-Checks, die Auslieferung statischer Dateien, OTLP und Ähnliches. Diese nehmen, wie vom jeweiligen Standard verlangt, gewöhnliches HTTP entgegen.
- Referenzimplementierungen: `RPoem` (`open_runo.admin.v1.Admin`) und `open-easy-web` (`server/src/connect.rs`, `proto/open_easy_web/admin/v1/admin.proto`).
- Hinweis: Ob der Code eines einzelnen Repositorys diese Richtlinie bereits erfüllt, hängt vom jeweiligen Repository ab. Dieser Abschnitt dokumentiert lediglich die Richtlinie; er besagt nicht, dass der Code schon angepasst wurde.

### Русский (перевод выполнен с помощью ИИ и отредактирован; носителями языка пока не проверен)
- **От REST мы полностью отказываемся; новые API больше не создаются в стиле REST.** API, к которым обращаются из браузера или через `curl`, используют **унарные вызовы Connect** (`POST /<пакет>.<Service>/<Method>`, `content-type: application/json`, тело запроса — JSON). Для взаимодействия между сервисами применяются **gRPC и gRPC-web** — в том же стиле, что и административный API RPoem (`open_runo.admin.v1.Admin`). Потоковая передача данных добавляется лишь тогда, когда в ней действительно возникает необходимость.
- **Существующие REST-эндпоинты переводятся поэтапно — по мере того, как с ними приходится работать.** После перевода старый REST-маршрут удаляется. Пока миграция не завершена, оставшиеся эндпоинты сохраняются, но **новые не добавляются**.
- **Работа с JSON унифицирована на основе RS-JSON**: чтение — через `from_str_secure` или `parse_secure` (с ограничениями на размер, глубину вложенности и повторяющиеся ключи), запись — через `rust_json::fast`. `prost` в браузерный WASM не включается, поскольку это увеличило бы размер файла.
- **Исключения (интерфейсы, форму которых определяет чужой стандарт, а не наш собственный API)**: ACME, обратные вызовы OAuth/OIDC, вебхуки сторонних сервисов (например, GitHub), проверки работоспособности, раздача статических файлов, OTLP и тому подобное. Такие интерфейсы принимают обычный HTTP, как того требует соответствующий стандарт.
- Эталонные реализации: `RPoem` (`open_runo.admin.v1.Admin`) и `open-easy-web` (`server/src/connect.rs`, `proto/open_easy_web/admin/v1/admin.proto`).
- Примечание: соответствует ли код конкретного репозитория этой политике, зависит от самого репозитория. Данный раздел лишь фиксирует политику и не означает, что код уже изменён.

## API 方針の補足: 用語の整理と使い分け (2026-11-04)

### 日本語
**1つに統一するものではなく、別の層の話です。** 「REST の代わり」になるのは Connect・gRPC・GraphQL だけで、WASM は画面を作る技術です。

| 用語 | 何か | REST の代わりか |
|---|---|---|
| REST | URL と HTTP メソッドで操作を表す API の作り方 | 全廃する側 |
| **Connect** | `POST /サービス名/メソッド名` に JSON を送る形式。`curl` でもブラウザの `fetch` でも呼べる | はい(**採用**) |
| **gRPC** | 同じ「サービス/メソッド」の考え方を、バイナリ(protobuf)と HTTP/2 で行う形式。速く、ストリーミングも得意 | はい(**サービス同士に採用**) |
| gRPC-web | gRPC のブラウザ向けの変形版 | はい(必要なら) |
| GraphQL | 欲しい項目を問い合わせる形式 | 一部だけ(**問い合わせ(読み取り)専用**) |
| WASM | Rust をブラウザの中で動かす仕組み | いいえ(画面を作る技術) |

- **ブラウザの画面や `curl` から呼ぶ API は Connect**(POST、JSON を RS-JSON で読み書き)。ブラウザの WASM は素の gRPC を話せず、Connect なら `fetch` のままで、`prost` を足さずに済む(画面のサイズも増えない)。
- **サービス同士の通信(サーバー間、重い処理、ストリーミング)は gRPC。** 多くは Connect で足りるので、ストリーミングが必要になった所だけ gRPC に上げる。
- **GraphQL は、まとめて問い合わせたいときの読み取り専用。** 書き込み(操作)には使わない。
- **WASM は画面を作る技術として、そのまま使う。** 画面から呼ぶ API が Connect になっただけ。
- 外部の規格が形を決めるもの(ACME、OAuth のコールバック、第三者の Webhook、ヘルスチェック、静的ファイル、OTLP)は、規格どおりの HTTP のまま。
- **迷ったら Connect。**

### English
**These are not alternatives to unify; they sit at different layers.** Only Connect, gRPC and GraphQL can replace REST; WASM is a way to build the UI.

| Term | What it is | Replaces REST? |
|---|---|---|
| REST | API style that expresses operations through URLs and HTTP methods | The thing being abolished |
| **Connect** | `POST /Service/Method` with a JSON body; callable from `curl` and from a browser `fetch` | Yes (**adopted**) |
| **gRPC** | The same service/method idea over binary (protobuf) and HTTP/2; fast, good at streaming | Yes (**for service-to-service**) |
| gRPC-web | A browser-friendly variant of gRPC | Yes (if needed) |
| GraphQL | A query language: ask for exactly the fields you want | Partly (**read-only queries**) |
| WASM | Runs Rust inside the browser | No (a UI technology) |

- **APIs called from browser screens or `curl` use Connect** (POST, JSON read and written with RS-JSON). Browser WASM cannot speak plain gRPC; with Connect it stays a `fetch`, and `prost` is not needed (the binary does not grow).
- **Service-to-service calls (server to server, heavy work, streaming) use gRPC.** Connect is enough in most places; move a call to gRPC only where streaming is actually needed.
- **GraphQL is read-only**, for queries that gather several pieces of data. It is not used for writes (operations).
- **WASM stays as the way to build the UI.** The API it calls simply became Connect.
- Interfaces whose shape an outside standard dictates (ACME, OAuth callbacks, third-party webhooks, health checks, static files, OTLP) stay plain HTTP.
- **When in doubt, use Connect.**

### Deutsch (KI-Übersetzung, sprachlich überarbeitet; noch nicht von Muttersprachlern geprüft)
**Es geht nicht darum, eine einzige Technik auszuwählen – die Begriffe liegen auf unterschiedlichen Ebenen.** Als Ersatz für REST kommen nur Connect, gRPC und GraphQL in Frage; WASM dagegen ist eine Technik, mit der die Benutzeroberfläche gebaut wird.

| Begriff | Was ist das? | Ersatz für REST? |
|---|---|---|
| REST | Ein API-Stil, bei dem Operationen über URLs und HTTP-Methoden ausgedrückt werden | wird abgeschafft |
| **Connect** | `POST /Dienst/Methode` mit JSON im Body; aufrufbar mit `curl` ebenso wie mit `fetch` im Browser | ja (**gewählt**) |
| **gRPC** | Dasselbe Prinzip aus Dienst und Methode, aber über binäres protobuf und HTTP/2; schnell und gut für Streaming geeignet | ja (**zwischen Diensten**) |
| gRPC-web | Eine für den Browser angepasste Variante von gRPC | ja (bei Bedarf) |
| GraphQL | Abfragesprache, mit der man genau die benötigten Felder anfordert | teilweise (**nur für lesende Abfragen**) |
| WASM | Führt Rust-Code direkt im Browser aus | nein (Technik für die Oberfläche) |

- **APIs, die von Browseroberflächen oder per `curl` aufgerufen werden, verwenden Connect** (POST; JSON wird mit RS-JSON gelesen und geschrieben). WASM im Browser kann kein reines gRPC sprechen. Mit Connect bleibt es bei einem gewöhnlichen `fetch`, und `prost` ist nicht nötig – die WASM-Datei wird dadurch nicht größer.
- **Für die Kommunikation zwischen Diensten (Server zu Server, rechenintensive Aufgaben, Streaming) wird gRPC eingesetzt.** In den meisten Fällen reicht Connect aus; auf gRPC wird nur dort umgestellt, wo Streaming tatsächlich gebraucht wird.
- **GraphQL dient ausschließlich lesenden Abfragen**, wenn mehrere Informationen gebündelt abgefragt werden sollen. Für Schreibzugriffe (Operationen) wird es nicht verwendet.
- **WASM bleibt die Technik für die Oberfläche.** Geändert hat sich lediglich, dass die aufgerufene API nun auf Connect beruht.
- Schnittstellen, deren Form durch einen externen Standard festgelegt ist (ACME, OAuth-Callbacks, Webhooks von Drittanbietern, Health-Checks, statische Dateien, OTLP), bleiben gewöhnliches HTTP.
- **Im Zweifel: Connect.**

### Русский (перевод выполнен с помощью ИИ и литературно отредактирован; носителями языка пока не проверен)
**Это не варианты, из которых нужно выбрать один, а технологии разных уровней.** Заменить REST могут только Connect, gRPC и GraphQL; WASM же — это способ создания пользовательского интерфейса.

| Термин | Что это | Заменяет REST? |
|---|---|---|
| REST | Стиль построения API, при котором операции выражаются через URL и HTTP-методы | отменяется |
| **Connect** | `POST /Сервис/Метод` с телом в формате JSON; вызывается как из `curl`, так и из браузерного `fetch` | да (**выбран**) |
| **gRPC** | Тот же принцип «сервис — метод», но поверх двоичного формата protobuf и HTTP/2; работает быстро и хорошо подходит для потоковой передачи | да (**для связи между сервисами**) |
| gRPC-web | Адаптированный для браузера вариант gRPC | да (при необходимости) |
| GraphQL | Язык запросов: клиент запрашивает ровно те поля, которые ему нужны | частично (**только запросы на чтение**) |
| WASM | Позволяет выполнять код на Rust прямо в браузере | нет (технология для интерфейса) |

- **API, которые вызываются из браузерных интерфейсов или через `curl`, строятся на Connect** (POST; JSON читается и записывается через RS-JSON). Браузерный WASM не умеет работать с «чистым» gRPC. С Connect достаточно обычного `fetch`, а `prost` не требуется — размер WASM-файла не увеличивается.
- **Для обмена между сервисами (сервер — сервер, ресурсоёмкие задачи, потоковая передача) применяется gRPC.** В большинстве случаев достаточно Connect; на gRPC переходят только там, где потоковая передача действительно необходима.
- **GraphQL используется только для чтения**, когда нужно получить сразу несколько связанных данных. Для записи (операций) он не применяется.
- **WASM по-прежнему остаётся технологией для интерфейса.** Изменилось лишь то, что вызываемый им API теперь построен на Connect.
- Интерфейсы, форма которых задана внешним стандартом (ACME, обратные вызовы OAuth, вебхуки сторонних сервисов, проверки работоспособности, статические файлы, OTLP), остаются обычным HTTP.
- **Если есть сомнения — выбирайте Connect.**

## REST 全廃・Connect 移行 (2026-11-06〜07)

**状況**: 自前の API の操作 36 個(旧ルート 28 本)をすべて Connect に移し、旧 REST(`/api/...`)の経路は**削除した**(旧パスは 404)。ユーザー決定(2026-11-04)の「REST 全廃・gRPC / Connect へ」に従う。方針の正本は `aon-co-jp/runo` の `API-POLICY.md`。

- **呼び方**: `POST /rs_ec.v1.<Service>/<Method>`、`content-type: application/json`、本文は JSON オブジェクト。認証は従来どおり `Authorization: Bearer <セッション>`。
  パス引数・クエリだった値は、JSON の欄に入れる(例: `POST /rs_ec.v1.Products/Get ほか` に `{"id": 3}`)。
- **サービス**: Auth(RequestOtp, VerifyOtp, Logout, GetLoginMode, SetLoginMode, QrLogin*)・Accounts・Products・Categories・Favorites・Reviews・Cart・Orders
- **読み込みは RS-JSON**(`rust_json::from_str_secure`: 本文 64 KiB・深さ16・重複キー拒否)、書き出しは `rust_json::fast`。
- **エラー**: `{"code": "...", "message": "..."}`(HTTP: 400 不正入力・重複キー / 401 未認証 / 403 / 404 / 415 / 503 など)。成功は常に 200(文字列だけの応答は `{"message": "..."}`)。
- **実装**: `src/connect.rs` が入口。JSON の欄を内部ルーター(公開されない)のパス引数・クエリ・本文へ変換して既存ハンドラを呼ぶ。`build_routes` の `rpc_table!` が唯一の表。
- **残したもの(例外)**: `GET /`(案内ページ)・`GET /healthz`(ヘルスチェック)・`GET /qr-confirm.html`(QR 確認ページ、画面)。
- **試験**: `cargo test` 48 件が通る(旧 REST が 404、認証なし 401、不正 JSON・重複キー・非オブジェクト 400、JSON 以外の content-type 415、パス引数欠落 400、GET 405 を含む)。
- **呼び出し元**: 同リポジトリの画面(`/` の案内、`/qr-confirm.html` の JS)は Connect に更新済み。他リポジトリからの呼び出しは確認できなかった(文書内の言及のみ)。
- **注意**: 旧 REST をそのまま叩いている外部のスクリプトがあれば壊れる(互換は残していない)。
- **本番**: `rs-ec.service`(ポート 8102)への反映状況は下の PORTING.md を参照。
