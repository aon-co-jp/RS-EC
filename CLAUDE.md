# 開発方針＆開発環境ルール(RS-EC)

作業ドライブは`F:\runo`。この節は[`open-raid-z`](https://github.com/aon-co-jp/open-raid-z)の
`CLAUDE.md`を正本とし、各プロジェクトへコピーして同期する方針に準じる。
GitHubリポジトリ: [aon-co-jp/RS-EC](https://github.com/aon-co-jp/RS-EC)。
VPS上の作業パス: `/root/repository/RS-EC`(2026-08-28、VPSディレクトリ
構成整理によりリポジトリは`/root/repository/`配下へ移動。旧パス
`/root/RS-EC`はシンボリックリンクとして残しており、systemdサービス等
既存の参照はそのまま機能する)。

## このプロジェクトの役割

[EC-CUBE](https://www.ec-cube.net/)(PHP製)の、ハイスピード・
ハイセキュリティ・省メモリなRust+[poem](https://github.com/poem-web/poem)
(RPoem)版を目指す。`RS-Git`(Gitea相当)・`RJSON`(JSON処理)と同じ
`aon-co-jp`エコシステムの一員。

> ⚠️ **正直な開示**: 2026-07-21時点でコード未着手(このCLAUDE.mdのみの
> 状態)。実装が追いつくまでは「EC-CUBEの代替品」を名乗らず、
> 進捗をこのHANDOFFに正直に記録する。

## 着手時に踏襲すべき既存プロジェクトの設計方針

- **`RS-Git`**(git smart HTTP・OTPログイン・アクセス制御・容量ベースの
  自動判定)を先行実装として参照。「正直な開示」「段階的実装」
  「型チェックだけで完了と報告しない・実機検証必須」の3方針は共通。
- 決済・与信まわりは`aruaru-llm`の`credit`インテント(与信調査構想)や
  エコシステム内の他プロジェクトの決済関連設計を確認してから着手する
  こと(重複実装を避けるため)。

## EC-CUBEの主要機能(着手時の優先順位付けの参考)

- 商品・カテゴリ管理
- カート・注文・決済連携
- 会員管理
- 配送・在庫管理
- プラグイン機構(拡張性)
- 管理画面

## 方針決定事項(2026-07-21、ユーザー確認済み)

- **着手順番**: `RS-Chiketto`・`RS-Blog`・`RS-EC`は同時並行ではなく
  **1つずつ順番に、`RS-Git`と同じ深さまで作り込んでから次へ**進める。
  どれを最初にするかは次回セッション冒頭で決定。
- **データベース**: `aruaru-db`(ZFS互換・ACID互換のRust製DB)を採用、
  3プロジェクトで統一する。加えて**PostgreSQLとのDUAL DATABASE構成も
  可能にする**(ユーザー指示、2026-07-21追記)——`open-runo`/RPoemの
  「4層4重」DUAL DB思想と同じ方針、設定で切り替え可能にする。決済・
  注文データはACID・監査性が特に重要なため、DUAL DB構成が実際に効いて
  くる領域として優先的に設計する。
- **「分身の術」構成でDB層を共有する**(ユーザー指示、2026-07-21追記):
  `open-web-server`・`aruaru-llm`・RPoem/RCosmoと同じ設計思想により、
  `aruaru-db`/PostgreSQL接続は**1インスタンスを複数ドメインが共有**し、
  ドメイン追加のたびに個別インストールは不要とする。実装は`aruaru-llm`
  の`src/tenants.rs`(`TenantRegistry`)と同じパターン。**管理は
  `open-easy-web`側から行う**(`appserver_registration.rs`に
  `RS-EC`用の`AppServerKind`variantを追加する形)。
  **非同期・マルチCPU/マルチコア/マルチスレッド対応**: `#[tokio::main]`
  は既定の`multi_thread`フレーバー、CPU負荷の高い処理は`rayon`で
  全論理コアへ並列ディスパッチする。
- **決済機能**: **実決済連携(Stripe等)まで目指す**(モックのみでは
  終わらせない、ユーザー指示)。ただし本システムのセキュリティルール
  (財務系の実処理は必ずユーザー確認を挟む、支払い情報を平文で扱わない)
  に従い、実際の決済実行・カード情報の直接取り扱いはAIが単独で
  進めず、都度ユーザー確認を取ること。

## HANDOFF

- **2026-08-28 ログイン4択方式の横展開: `otp_qr`(QR確認2FA)+`login_mode`
  管理APIを追加(open-english/open-easy-web/RS-Blogと同じパターンだが
  複数アカウント制のためスコープを絞った)**: `F:\runo\PORTING.md`の
  「ログイン方式4択…横展開チェックポイント」節の対応、6リポジトリ中
  2番目。
  1. **「パスワード無し」に加え、QR単体(no-OTP)ログインも意図的に
     非実装(正直な設計判断)**: `RS-Blog`(単一固定管理者)や
     open-easy-web(TOTP登録済みアカウントのみqr可)とは異なり、RS-EC
     は`accounts.rs`による複数の登録メールアドレス(会員+管理者)を持ち、
     TOTP等の事前登録済み第二要素も無い。「メールアドレスさえ知って
     いればQRセッションを開始できてしまう」設計は注文・決済に関わる
     アカウントの完全性を損なうため、`login_mode`は`otp`(既定)・
     `otp_qr`(OTP検証成功後にQR確認を第二要素として要求する真の2FA)
     の2値のみとした。`auth::is_valid_login_mode`は`"qr"`/`"none"`を
     明示的に拒否する。
  2. **`src/auth.rs`**: `login_mode`(`Mutex<String>`、既定`"otp"`)・
     `qr_logins`(QRログインセッション、3分TTL・1回限り)を`AuthStore`
     へ追加。`start_qr_login`は常に「OTP検証成功後の第二要素」として
     のみ呼ばれる設計(モジュールdocに明記、QR単体でのログイン開始
     エンドポイントはmain.rs側にも存在しない)。
  3. **`src/main.rs`**: `GET/POST /api/auth/login-mode`(GETは未ログイン
     可、POSTは管理者セッション必須)・`GET /api/auth/qr-login/
     {whoami,status}`・`POST /api/auth/qr-login/{confirm,finish}`・
     `GET /qr-confirm.html`(open-english/RS-Blog版をRS-ECブランディング
     へ差し替え)。既存の`verify_otp`ハンドラを拡張し、`login_mode`が
     `otp_qr`の場合はセッションを即発行せず`{"qr_required": true,
     "qr_id": ...}`を返すようにした。**`POST /api/auth/qr-login/start`
     は存在しない**(上記1番の設計判断により、QRセッションは常に
     `verify_otp`経由でのみ開始される)。
  4. **実機検証(型チェック・ビルド成功だけで完了と報告しない方針の
     徹底)**: `cargo build`成功(警告0件)。`cargo test`
     **47件全green**(新規10件: `auth::tests`5件+`handler_tests`4件、
     既存37件から回帰なし)。`handler_tests`側は実HTTP相当の統合テスト
     (`otp_qr_mode_requires_qr_confirmation_as_second_factor_over_
     real_http`——OTP検証→qr_required確認→確認前finish拒否→confirm→
     status確認→finish→セッション発行→保護API到達→二重finish拒否まで
     一気通貫、`login_mode_defaults_to_otp_and_requires_admin_session_
     to_change`——未ログイン変更401・`"qr"`指定400・管理者ログイン後の
     変更成功、`otp_mode_verify_otp_still_issues_a_session_directly`
     ——既定`otp`モードでの既存挙動の非回帰確認)。さらに実バイナリを
     起動し、curl経由で`GET /`(200)・`GET /api/auth/login-mode`
     (`{"login_mode":"otp"}`)・未ログインでの`GET /api/products`
     (401、既存のprivateカタログ挙動)・未ログインでの
     `POST /api/auth/login-mode`(401)・`GET /qr-confirm.html`
     (RS-ECブランディング付きHTML)を実HTTPで確認した。
  5. **正直な開示・未実施**: この開発機にSMTPサーバーが用意できない
     ため、実際のメール送信を経由した`otp_qr`モードのフルE2E
     (実メール受信→OTP入力→QR確認→ログイン)はcurlでは検証していない
     (`cargo test`内の`TestClient`による実HTTP相当の統合テストで代替、
     既存のOTP関連検証と同じ制約)。実際のQRコード画像生成・スマホ
     実機でのカメラ撮影によるURL起動も未検証(RS-EC自体にまだ
     ブラウザUIが無く、JSON APIのみのため対象外——`RS-Blog`と同じ
     制約)。
  - 次にすべきこと: (1) 残り4リポジトリ(RS-Ops/open-gitea/
    open-redmine/rs-sync)への同様の横展開、(2) 実SMTP環境での
    `otp_qr`フルE2E検証、(3) RS-EC自体のブラウザUI実装時にQRコード
    画像のレンダリング(`qrcode`crate等)を追加する。

- **2026-07-21 プロジェクト新設(器のみ)**: GitHub空リポジトリ・
  VPS空フォルダ・ローカル作業フォルダを用意。次回、`RS-Git`と同じ構成
  (`Cargo.toml`+`poem`)でのブートストラップに着手する。
  - 次にすべきこと: (1) 3プロジェクトのうちどれから着手するか決定、
    (2) EC-CUBEの機能のうちMVP範囲の選定(商品一覧+カートのみ、等)、
    (3) 決済ゲートウェイ選定(Stripe等)・PCI DSS等の要件調査、
    (4) `aruaru-db`との接続方式の設計。

- **2026-07-21(続き) v0.1.0ブートストラップ完了: 商品カタログCRUD+OTP
  認証+アクセス制御(コミット`71acdb2`、`RS-Chiketto`/`RS-Blog`と同じ
  パターンを踏襲)**:
  1. `RS-Chiketto`の`src/auth.rs`/`src/mail.rs`をそのまま移植(OTP
     ログイン機構、環境変数名のみ`RSEC_*`に変更)。
  2. `src/access.rs`/`src/accounts.rs`も`RS-Chiketto`の設計思想を
     踏襲しつつ、**プロジェクト単位の粒度は使わず、商品カタログ全体を
     単一リソース(`AccessConfig`、ファイル1枚)として扱う簡略版**に
     した(EC-CUBEの商品カタログは通常公開されるものであり、
     `RS-Chiketto`のチケットのようなproject単位の細分化は今回のEC商品
     には不要と判断、詳細はコード内コメント参照)。`RSEC_ACCOUNTS_LOCKED`
     (既定`true`)・自己申請→管理者審査(`POST /api/accounts/request`
     → `POST /api/accounts/requests/:id/decide`)は`RS-Chiketto`と
     同じ形状。
  3. 商品カタログ(`Product`)のCRUD: `POST/GET /api/products`・
     `GET/PUT/DELETE /api/products/:id`。フィールドは
     `id/name/description/price_cents/stock/status(draft|on_sale|
     sold_out)/created_at/updated_at`。永続化はJSONファイル
     (`RSEC_DATA_DIR/products.json`)。**カート・注文・決済関連の
     ロジックは一切実装していない**(今回のタスク範囲外、README/
     main.rsのdocコメントに明記)。
  4. `install.sh`/`install.ps1`/`.github/workflows/release.yml`は
     `RS-Blog`版をそのままリネーム移植(`blog`→`ec`、`RSBLOG`→`RSEC`、
     ポートは`8102`)。
  5. **検証**: `cargo build`警告0件。`cargo test` **18件全green**
     (auth 6件+access 3件+accounts 2件+`poem::test::TestClient`による
     ハンドラ統合テスト7件〈未認証`GET /api/products`→401確認・商品
     作成/一覧・更新/削除ラウンドトリップ・自己申請→承認フロー2件・
     accounts_locked中の403確認〉)。実バイナリを起動しての`curl`
     スモークテストも実施: `GET /healthz`→`200`、未ログインでの
     `GET /api/products`→`401`(カタログ既定`Mode::Private`のため、
     `RS-Chiketto`のチケット一覧が200・空配列を返す設計とは異なる
     ことに注意)、`POST /api/accounts/request`(認証不要)→`201`、
     `POST /api/auth/request-otp`(未登録メール)→`403`、
     (管理者メール、SMTP未設定)→`503`、を確認。**正直な開示**:
     SMTPが無い環境のため実OTPメール送受信を伴うログイン成功パスの
     実HTTP確認は未実施(`TestClient`によるインメモリ統合テストで
     代替)。
  6. **VPSデプロイは今回未実施**: `ssh conoha`で確認したところ
     `/root/RS-EC`は空フォルダのみ存在、systemdサービス未登録
     (`systemctl list-unit-files | grep rs-ec`もヒット無し)。存在
     しないVPSインフラの構築は本タスクの範囲外と判断しスキップ、
     デプロイは次回以降の宿題として残す。
  - **次にすべきこと**: (1) VPSへの初回デプロイ(`/root/RS-EC`への
    バイナリ配置+systemdサービス登録、`runo.tokyo/ec`・ポート`8102`)、
    (2) 実SMTP環境でのOTPログイン実HTTP E2E、(3) 決済ゲートウェイ
    (Stripe等)選定・PCI DSS等の要件調査(実装は引き続き行わない、
    調査のみ)、(4) `aruaru-db`/PostgreSQL DUAL DB構成への移行
    (現状はJSONファイル永続化)、(5) カテゴリ管理・在庫の自動引き落とし
    等EC-CUBEの他機能の段階的追加。

- **2026-07-22 カート・注文・モック決済(`src/cart.rs`/`src/order.rs`)を
  追加、実HTTPで一気通貫の商品登録→カート追加→注文確定を確認**:
  1. **開始時点の正直な訂正**: 今回のタスク依頼では「2026-07-21時点で
     コード未着手」という前提が渡されたが、実際には本セッション開始時
     点で前回までの作業(商品カタログCRUD・カテゴリ・お気に入り・商品
     レビュー〈`src/reviews.rs`、コミット未実施のまま作業ツリーに存在〉)
     が既に完了していた。前提と実態の食い違いをここに明記した上で、
     未実装だった**カート・注文・決済**部分を今回のタスクとして実装した。
  2. **`src/cart.rs`**: ログイン中アカウント(メール)ごとの
     `product_id`→数量のカート。`favorites.rs`と同じJSONファイル
     永続化パターン(`RSEC_DATA_DIR/cart.json`)。カート自体は金額計算・
     注文確定を持たない(お気に入り同様「保存した商品IDのリスト」の
     延長にすぎない設計、詳細はコード内コメント参照)。
  3. **`src/order.rs`**: 注文はチェックアウト確定時点の商品名・単価
     ・数量のスナップショットを保持(以後の価格変更・商品削除の影響を
     受けない)。`process_mock_payment`は**実際の金銭のやり取りを一切
     行わないダミー関数**(カード情報・決済トークンは引数に取らない、
     そもそも扱わない設計)。合計金額が0円の場合のみ失敗させる以外は
     常に成功し、`MOCK-`で始まるフェイクの決済参照IDを返す。**実決済
     ゲートウェイ(Stripe等)連携は未着手**(CLAUDE.mdの方針決定事項では
     将来実決済連携まで目指す方針だが、今回のタスク範囲・安全ルール
     〈財務系の実処理は必ずユーザー確認を挟む〉に従い、モック実装の
     みに留めた)。
  4. **新規ルーティング**: `GET /api/cart`(自分のカート、商品スナップ
     ショット+小計+合計)・`POST /api/cart/items`(追加、既存なら数量
     加算)・`DELETE /api/cart/items/:product_id`・
     `POST /api/orders/checkout`(カート確定、在庫チェック→不足時400、
     成功時は在庫減算〈0になったら`sold_out`へ自動遷移〉・カート空化・
     `Order`永続化)・`GET /api/orders`(自分の一覧、管理者は全件)・
     `GET /api/orders/:id`(本人または管理者のみ、他人は403)。全て
     未ログインは401(`RS-Git`/既存ハンドラと同じ401/403の使い分け)。
  5. **前回作業(`src/reviews.rs`関連)のバグを発見・修正**: 作業開始
     時点で`cargo test`がコンパイルエラー(`main.rs`のテストモジュールが
     `reviews::Review`をuse し忘れ)およびロジックバグ(一般会員が全件
     レビュー一覧を見ようとした際、期待される`403`ではなく`401`を返す
     実装ミス)を含んでいたため、今回ついでに修正した(`use crate::
     reviews::Review;`追加、`list_reviews`のログイン済みだが権限不足
     ケースの分岐修正)。
  6. **検証**: `cargo build`警告0件。`cargo test` **38件全green**
     (既存32件+今回追加の`cart`モジュール3件・`order`モジュール3件の
     ユニットテスト、`handler_tests::cart_add_checkout_decrements_
     stock_and_creates_paid_order`〈商品登録→カート追加→在庫超過時
     400確認→カート修正→チェックアウト成功→在庫減算・カート空化・
     決済参照ID確認〉・`handler_tests::other_member_cannot_view_
     someone_elses_order`〈本人以外403・管理者は閲覧可〉の統合
     テスト2件を追加)。実バイナリを起動しての`curl`スモークテストも
     実施: `GET /`→200、未ログインでの`GET /api/cart`→401、
     `GET /api/products`→401、`POST /api/accounts/request`→201、
     `POST /api/cart/items`(未ログイン)→401、
     `POST /api/orders/checkout`(未ログイン)→401、を確認。
     **正直な開示**: SMTPが無い環境のため、ログインを要するカート/
     チェックアウトのフルフローを実バイナリ+実HTTPで確認することは
     今回もできなかった(前回セッションのOTPログイン確認時と同じ
     制約)。代わりに`poem::test::TestClient`(実際のPoemルーティング
     ・ハンドラを通す統合テスト)で全フローを確認済み。
  - **次にすべきこと(優先順位順)**: (1) 実SMTP環境の用意とOTPログイン
    〜カート〜チェックアウトの実バイナリ+実HTTPでのフルE2E確認、
    (2) 決済ゲートウェイ(Stripe等)の実連携(ユーザー確認を挟みながら
    段階的に、カード情報の直接取り扱いは行わない設計を維持)、
    (3) 注文キャンセル・返金(モック)・在庫復元ロジックの追加、
    (4) 配送・送料計算、(5) `aruaru-db`/PostgreSQL DUAL DB構成への
    移行(現状は`products.json`/`cart.json`/`orders.json`等、全て
    JSONファイル永続化)、(6) VPSへの初回デプロイ(前回から持ち越し)。


## 同時並行開発の対象プロジェクト(2026-07-21、ユーザー指示・拡張版)

`RS-Chiketto`・`RS-Blog`・`RS-EC`(この3プロジェクト自身、着手順は
「1つずつ順番に」の方針のまま)に加えて、以下の既存プロジェクトを
**同時に開発を進め、完成度を高めていく**:

- [open-raid-z](https://github.com/aon-co-jp/open-raid-z) — 開発ルールの
  正本。3プロジェクトの`CLAUDE.md`もここの記述と同期を取る。
- [aruaru-db](https://github.com/aon-co-jp/aruaru-db) — ZFS互換・ACID
  互換のRust製DB。3プロジェクトが採用する「分身の術」DB共有構成の実体。
- [open-cuda](https://github.com/aon-co-jp/open-cuda) — GPU抽象化・
  GEMM/Attention計算基盤(`opencuda-blas`/`opencuda-bert`)。
- [aruaru-llm](https://github.com/aon-co-jp/aruaru-llm) — 上記
  `open-cuda`を使った実装例(bag-of-words→文埋め込みベースの意図分類へ
  移行済み)。3プロジェクトが将来AI機能を持つ際の先行実装として参照。
- [open-web-server](https://github.com/aon-co-jp/open-web-server) —
  「分身の術」構成(1インスタンスを複数ドメインが共有)の基盤実装、
  Nginx/Apacheハイブリッド仕様のWebサーバー。
- [open-cosmo](https://github.com/aon-co-jp/open-cosmo) — 関連する
  Webサーバー/フロントエンド基盤(詳細は同リポジトリのCLAUDE.md参照)。
- [RPoem](https://github.com/aon-co-jp/RPoem) — アプリケーションサーバー
  層(旧poem-cosmo-tauri)。`open-raid-z`とVersionlessAPIによる
  バージョンレス運用、`aruaru-db`とのDUAL DATABASE構成の先行実装。

- Python製AIライブラリのRust移植ハイブリッド/トライブリッド版
  (マーケティング調査での1〜6位、vLLM/Transformers/NumPy/PyTorch互換/
  scikit-learn/Whisper相当の良いとこ取り)——**Rustを基本とし、必要なら
  `RPoem`(アプリケーションサーバー層)も併用する**(ユーザー指示、
  2026-07-21追記)。`open-cuda`ワークスペース内の`opencuda-blas`
  (NumPy相当)・`opencuda-bert`(Transformers推論パス相当、実装済み)が
  このトライブリッド化の実体。今後の`opencuda-llm`(vLLM相当、生成
  デコーダ追加時)を、必要であれば`RPoem`上のHTTPサービスとして
  提供することも視野に入れる。

**理由**: これらは3プロジェクトが実際に依存する基盤コンポーネント
(DB層・GPU計算基盤・「分身の術」共有構成・アプリケーションサーバー層)
であり、基盤側の完成を待ってから3プロジェクトに着手するのではなく、
実際に統合しながら並行して育て、エコシステム全体の完成度を高めていく
方針とする。

## 公開先・配布方針(2026-07-21、ユーザー確認済み、着手時に反映すること)

- **公開パス**: `runo.tokyo/ec`(`RS-Git`の`runo.tokyo/rgit`・
  `RS-Chiketto`の`runo.tokyo/chiketto`と同じパス方式、VPS上の
  ポートは`8102`)。
- **クロスプラットフォーム配布**: AlmaLinux・Ubuntu・Debian・Fedora・
  RHEL等の主要Linuxディストリ、Windows・Windows Server向けに、
  インストーラー付きのビルド済みバイナリをGitHub Releasesで配布する
  (ユーザー指示、`RS-Chiketto`の
  `.github/workflows/release.yml`・`install.sh`・`install.ps1`を
  雛形として踏襲すること)。

## HANDOFF追記(2026-07-31) インストーラーの電源プロファイル選択機能(未実装、エコシステム標準方針として記録)

`open-raid-z`のCLAUDE.md(全リポジトリ共通の設計思想セクション)に、
インストーラー(`install.sh`/`install.ps1`等)実行時に以下3つの電源
プロファイルを選択させる標準方針を追記した(ユーザー指示、2026-07-31):

1. **省電力(Power-saving)**: CPU使用率・ポーリング間隔を抑えた低負荷設定。
2. **省メモリ(Low-memory)**: メモリ確保量・キャッシュサイズを抑えた設定。
3. **常時電源接続(Always-on)**: 上記の抑制を行わないフル性能設定。
   **この場合のみ**ハードウェアアクセラレータ(NPU/GPU)のサポートを
   自動検出・自動有効化する(`open-cuda`の`GpuDevice`抽象化を利用)。

**正直な開示**: このリポジトリのインストーラーへの実装はまだ未着手。
実装時は`open-raid-z/CLAUDE.md`の該当節、および先行実装予定の
`open-redmine/CLAUDE.md`を参照し、`open-cuda`側のGPU/NPUベンダー検出
ロジックを再利用すること(車輪の再発明を避ける)。
- 次にすべきこと: このリポジトリの`install.sh`/`install.ps1`に上記3
  プロファイルの選択機能を追加する。

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
