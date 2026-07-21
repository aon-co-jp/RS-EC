# PORTING.md — RS-EC を他プロジェクトへお引越しする際のガイド

## 現状(2026-07-21)

コード未着手(`CLAUDE.md`のみ)。移植すべき実装はまだ存在しない。

## 同時並行開発の対象プロジェクト(ユーザー指示、`CLAUDE.md`と同内容・拡張版)

`RS-EC`は、以下のプロジェクトと**同時に開発を進め、完成度を高めていく**方針:

- [open-raid-z](https://github.com/aon-co-jp/open-raid-z) — 開発ルールの正本
- [aruaru-db](https://github.com/aon-co-jp/aruaru-db) — DB層(「分身の術」共有構成)
- [open-cuda](https://github.com/aon-co-jp/open-cuda) — GPU計算基盤
- [aruaru-llm](https://github.com/aon-co-jp/aruaru-llm) — open-cudaの実装例
- [open-web-server](https://github.com/aon-co-jp/open-web-server) — 「分身の術」基盤実装
- [open-cosmo](https://github.com/aon-co-jp/open-cosmo) — 関連Webサーバー/フロントエンド基盤
- [RPoem](https://github.com/aon-co-jp/RPoem) — アプリケーションサーバー層(旧poem-cosmo-tauri)
- Python製AIライブラリのRust移植ハイブリッド/トライブリッド版(vLLM/Transformers/NumPy/PyTorch互換/scikit-learn/Whisper相当) — Rustを基本とし必要なら`RPoem`も併用、`open-cuda`の`opencuda-blas`/`opencuda-bert`が実体

着手後、実際に他プロジェクトへ移植可能な形になった時点で、`RGit`や
`RJSON`の`PORTING.md`と同じ構成(依存の追加手順・共通パターンの
コピー手順・注意事項)に更新すること。
