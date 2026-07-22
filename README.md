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
