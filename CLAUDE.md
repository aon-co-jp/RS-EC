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
