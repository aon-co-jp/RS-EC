# API 方針 (RS-EC)

aon-co-jp 全体の方針(正本: `aon-co-jp/runo` の `API-POLICY.md`)に従い、**REST は全廃**した。

- 自前の API は **Connect の単純呼び出し**(`POST /rs_ec.v1.<Service>/<Method>`、JSON)。サービス間は将来必要になれば gRPC / gRPC-web。
- JSON は RS-JSON(読み込みは上限つきの `from_str_secure`、書き出しは `rust_json::fast`)。
- 例外(規格が形を決めるもの): `GET /healthz`(ヘルスチェック)、`GET /`・`GET /qr-confirm.html`(画面の配信)。このリポジトリに他製品互換の API は無い。
- **新しい REST を足さない。** 追加は `src/main.rs` の `rpc_table!` に 1 行(Service / Method / 内部パス / GET 相当か / ハンドラ)。
