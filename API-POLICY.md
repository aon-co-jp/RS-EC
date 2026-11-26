# API 方針 (RS-EC)

aon-co-jp 全体の方針(正本: `aon-co-jp/runo` の `API-POLICY.md`)に従い、**REST は全廃**した。

- 自前の API は **Connect の単純呼び出し**(`POST /rs_ec.<Service>/<Method>`、JSON)。サービス間は将来必要になれば gRPC / gRPC-web。
- JSON は RS-JSON(読み込みは上限つきの `from_str_secure`、書き出しは `rust_json::fast`)。
- 例外(規格が形を決めるもの): `GET /healthz`(ヘルスチェック)、`GET /`・`GET /qr-confirm.html`(画面の配信)。このリポジトリに他製品互換の API は無い。
- **新しい REST を足さない。** 追加は `src/main.rs` の `rpc_table!` に 1 行(Service / Method / 内部パス / GET 相当か / ハンドラ)。


## 2026-11-26 `.v1` 即削除・版なし名へ統一 / `.v1` removed immediately, versionless names only

### 日本語
- ユーザー指示(2026-11-26)により、自前APIの版つき名(`rs_ec.v1.…`)は**猶予なしで削除**し、版なし名(`rs_ec.<Service>/<Method>`)だけにした。旧名は404を返す(試験 `v1_names_are_gone_versionless_works` で確認)。画面・試験・文書・メール文面の呼び出しも版なし名へ書き換え済み。
- 第三者の規格名(`grpc.health.v1` 等)は対象外で触っていない。
- 本番(VPS)への反映・実測は、このコミットの後の記録(下の「本番反映」行)を参照。反映前は旧名が本番で動いている。
- **再開用メッセージ(別アカウントでそのまま貼り付け)**: 前の続きです。`RS-EC` の README/CLAUDE/PORTING の「2026-11-26 `.v1` 即削除」を読み、(1)`vps-build.sh`(Ubuntu-24.04)でビルドし `check-glibc.sh` を通す、(2)VPSの該当サービスを `*.before-v1del-20261126` にバックアップして入れ替える、(3)新名200/401・旧名404・`/healthz` 200・エラーログ0を実測、(4)結果をこの節に追記してpush。VPSのcloneは `git pull` せず fetch+reset。関連リポジトリ: RS-EC・RS-Blog・RS-Sync(RS-Opsは必要なら)・`runo`。秘密は出さない、VPSでビルドしない、新しいRESTは作らない。

### English
- Per the user's instruction (2026-11-26), the versioned names of our own API (`rs_ec.v1.…`) were **deleted immediately with no grace period**; only the versionless names (`rs_ec.<Service>/<Method>`) remain. Old names return 404 (test `v1_names_are_gone_versionless_works`). Callers in the UI, tests, docs and mail text were rewritten to the versionless names.
- Third-party standard names (`grpc.health.v1` etc.) are out of scope and untouched.
- For the production swap and measurements see the later "production" note below; before that, production still serves the old names.
- **Resume message (paste as is on another account)**: Continuing from before. Read "2026-11-26 `.v1` removed immediately" in the README/CLAUDE/PORTING of `RS-EC`, then (1) build with `vps-build.sh` (Ubuntu-24.04) and pass `check-glibc.sh`, (2) back up the VPS service as `*.before-v1del-20261126` and swap it, (3) measure new names 200/401, old names 404, `/healthz` 200, error log 0, (4) append the result to this section and push. Never `git pull` VPS clones (fetch+reset). Related repos: RS-EC, RS-Blog, RS-Sync (RS-Ops if needed), `runo`. Never print secrets, never build on the VPS, no new REST.

### 本番反映(2026-11-26) / Production (2026-11-26)
- **日本語**: `vps-build.sh`(Ubuntu-24.04、glibc検査OK)でビルドし、`*.before-v1del-20261126` にバックアップして入れ替え・再起動。実測: 新名 401(認証なし)、旧名(`.v1`) 404、`/healthz` 200、エラーログ0。ロールバックはバックアップを元の名前へ戻して再起動。上の「反映前は旧名が本番で動いている」は反映済みのため無効。
- **English**: Built with `vps-build.sh` (Ubuntu-24.04, glibc check OK), backed up as `*.before-v1del-20261126`, swapped and restarted. Measured: new names 401 without auth, old (`.v1`) names 404, `/healthz` 200, error log 0. Roll back by restoring the backup under the original name and restarting. The "production still serves the old names" remark above is obsolete now.
