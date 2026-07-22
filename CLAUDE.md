# 開発方針＆開発環境ルール(RS-EC)

作業ドライブは`F:\runo`。この節は[`open-raid-z`](https://github.com/aon-co-jp/open-raid-z)の
`CLAUDE.md`を正本とし、各プロジェクトへコピーして同期する方針に準じる。
GitHubリポジトリ: [aon-co-jp/RS-EC](https://github.com/aon-co-jp/RS-EC)。
VPS上の作業パス: `/root/RS-EC`(空フォルダ作成済み、2026-07-21)。

## このプロジェクトの役割

[EC-CUBE](https://www.ec-cube.net/)(PHP製)の、ハイスピード・
ハイセキュリティ・省メモリなRust+[poem](https://github.com/poem-web/poem)
(RPoem)版を目指す。`RGit`(Gitea相当)・`RJSON`(JSON処理)と同じ
`aon-co-jp`エコシステムの一員。

> ⚠️ **正直な開示**: 2026-07-21時点でコード未着手(このCLAUDE.mdのみの
> 状態)。実装が追いつくまでは「EC-CUBEの代替品」を名乗らず、
> 進捗をこのHANDOFFに正直に記録する。

## 着手時に踏襲すべき既存プロジェクトの設計方針

- **`RGit`**(git smart HTTP・OTPログイン・アクセス制御・容量ベースの
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
  **1つずつ順番に、`RGit`と同じ深さまで作り込んでから次へ**進める。
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

- **2026-07-21 プロジェクト新設(器のみ)**: GitHub空リポジトリ・
  VPS空フォルダ・ローカル作業フォルダを用意。次回、`RGit`と同じ構成
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
     未ログインは401(`RGit`/既存ハンドラと同じ401/403の使い分け)。
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

- **公開パス**: `runo.tokyo/ec`(`RGit`の`runo.tokyo/rgit`・
  `RS-Chiketto`の`runo.tokyo/chiketto`と同じパス方式、VPS上の
  ポートは`8102`)。
- **クロスプラットフォーム配布**: AlmaLinux・Ubuntu・Debian・Fedora・
  RHEL等の主要Linuxディストリ、Windows・Windows Server向けに、
  インストーラー付きのビルド済みバイナリをGitHub Releasesで配布する
  (ユーザー指示、`RS-Chiketto`の
  `.github/workflows/release.yml`・`install.sh`・`install.ps1`を
  雛形として踏襲すること)。
