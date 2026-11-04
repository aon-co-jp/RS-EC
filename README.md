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
