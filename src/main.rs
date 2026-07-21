//! # RS-EC (v0.1.0)
//!
//! [EC-CUBE](https://www.ec-cube.net/)(PHP製)の、ハイスピード・
//! ハイセキュリティ・省メモリなRust+[poem](https://github.com/poem-web/poem)版を目指す。
//!
//! ## 正直な開示(最重要、`RGit`/`RS-Chiketto`/`RS-Blog`と同じ流儀)
//!
//! **v0.1.0時点では、商品カタログ(Product)のCRUDのみ実装している。**
//! EC-CUBEが持つ以下の機能は**まだ一切無い**:
//!
//! - カート・注文・**決済連携(実決済は将来対応、今回は一切実装しない)**
//! - 会員管理(ポイント・お気に入り等、ログイン自体はOTP認証のみ実装)
//! - 配送・在庫管理(在庫数フィールドはあるが自動引き落とし等は無し)
//! - プラグイン機構・管理画面(APIのみ、UIは無し)
//!
//! 認証は`RGit`/`RS-Chiketto`で先行実装したOTPログイン(固定管理者+
//! 登録アカウント、自己申請→管理者審査)をそのまま移植して使用。
//! 商品カタログへのアクセス制御は、`RS-Chiketto`のプロジェクト単位の
//! 粒度ではなく、カタログ全体を単一リソースとして扱う`access.rs`
//! (`Mode::Private`/`Public`)を採用(EC-CUBEの商品カタログは通常
//! 公開されるものであり、プロジェクト単位の細分化は不要と判断、
//! `CLAUDE.md`参照)。ストレージは現時点でJSONファイル永続化
//! (`aruaru-db`/PostgreSQL DUAL DB構成への移行は未着手)。

mod access;
mod accounts;
mod auth;
mod mail;

use std::path::PathBuf;
use std::sync::Arc;

use poem::listener::TcpListener;
use poem::middleware::Tracing;
use poem::web::Data;
use poem::{get, handler, post, web::Path as PathExtractor, EndpointExt, Request, Response, Result as PoemResult, Route, Server};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
struct AppState {
    data_root: PathBuf,
    auth: Arc<auth::AuthStore>,
    admin_email: String,
    smtp: Option<mail::SmtpConfig>,
    /// `RSEC_ACCOUNTS_LOCKED`(既定`true`)。`RGit`/`RS-Chiketto`と同じ
    /// 方針で、ロック中は管理者以外のアカウント登録・申請承認を拒否する。
    accounts_locked: bool,
}

fn require_admin_session(req: &Request, state: &AppState) -> PoemResult<()> {
    let header = req.header(poem::http::header::AUTHORIZATION).unwrap_or("");
    let token = header.strip_prefix("Bearer ").unwrap_or("");
    match state.auth.session_email(token) {
        Some(email) if email == state.admin_email => Ok(()),
        _ => Err(poem::Error::from_string("admin login required", poem::http::StatusCode::UNAUTHORIZED)),
    }
}

/// リクエストの`Authorization: Bearer`ヘッダからログイン中のメール
/// アドレスを取得する(未ログインなら`None`、管理者・一般アカウント
/// いずれも区別しない)。
fn session_email(req: &Request, state: &AppState) -> Option<String> {
    let header = req.header(poem::http::header::AUTHORIZATION).unwrap_or("");
    let token = header.strip_prefix("Bearer ").unwrap_or("");
    state.auth.session_email(token)
}

/// カタログ全体に対して`need`の操作が許可されているかを判定する
/// (`access.rs`の`is_allowed`を利用)。管理者は常に許可。未ログインは
/// `401`、ログイン済みだが権限不足は`403`(`RGit`と同じ401/403の使い分け)。
async fn check_catalog_access(req: &Request, state: &AppState, need: access::Need) -> PoemResult<()> {
    let email = session_email(req, state);
    if let Some(email) = &email {
        if *email == state.admin_email {
            return Ok(());
        }
    }
    let config = access::load(&state.data_root).await;
    if access::is_allowed(&config, need, email.as_deref()) {
        return Ok(());
    }
    if email.is_none() {
        Err(poem::Error::from_string("login required", poem::http::StatusCode::UNAUTHORIZED))
    } else {
        Err(poem::Error::from_string("insufficient permission", poem::http::StatusCode::FORBIDDEN))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProductStatus {
    Draft,
    OnSale,
    SoldOut,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Product {
    id: u64,
    name: String,
    description: String,
    price_cents: u64,
    stock: u64,
    status: ProductStatus,
    created_at: u64,
    updated_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ProductStore {
    next_id: u64,
    products: Vec<Product>,
}

fn products_path(data_root: &std::path::Path) -> PathBuf {
    data_root.join("products.json")
}

async fn load_products(data_root: &std::path::Path) -> ProductStore {
    match tokio::fs::read(products_path(data_root)).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => ProductStore::default(),
    }
}

async fn save_products(data_root: &std::path::Path, store: &ProductStore) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(store).expect("ProductStore serialization is infallible");
    tokio::fs::write(products_path(data_root), bytes).await
}

fn now_unix() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Deserialize)]
struct CreateProductRequest {
    name: String,
    #[serde(default)]
    description: String,
    price_cents: u64,
    #[serde(default)]
    stock: u64,
    #[serde(default = "default_status")]
    status: ProductStatus,
}

fn default_status() -> ProductStatus {
    ProductStatus::Draft
}

/// `POST /api/products` — 商品を新規作成する。カタログへの`Need::Edit`
/// 権限が必要(管理者は常に許可、`access.rs`参照)。決済・在庫の自動
/// 引き落としロジックは無し(v0.1.0の範囲外)。
#[handler]
async fn create_product(req: &Request, state: Data<&AppState>, body: poem::web::Json<CreateProductRequest>) -> PoemResult<Response> {
    check_catalog_access(req, &state, access::Need::Edit).await?;
    if body.name.trim().is_empty() {
        return Ok(Response::builder().status(poem::http::StatusCode::BAD_REQUEST).body("name must not be empty"));
    }
    let mut store = load_products(&state.data_root).await;
    let id = store.next_id;
    store.next_id += 1;
    let now = now_unix();
    let product = Product {
        id,
        name: body.name.clone(),
        description: body.description.clone(),
        price_cents: body.price_cents,
        stock: body.stock,
        status: body.status,
        created_at: now,
        updated_at: now,
    };
    store.products.push(product.clone());
    save_products(&state.data_root, &store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Response::builder()
        .status(poem::http::StatusCode::CREATED)
        .content_type("application/json")
        .body(serde_json::to_vec(&product).unwrap_or_default()))
}

/// `GET /api/products` — 商品一覧。カタログへの`Need::View`権限がある
/// 場合のみ結果を返す(管理者は常に許可、未ログインはカタログが
/// `Mode::Public`かつ`allow_view`なら閲覧可、`RGit`と同じprivate既定)。
#[handler]
async fn list_products(req: &Request, state: Data<&AppState>) -> PoemResult<Response> {
    check_catalog_access(req, &state, access::Need::View).await?;
    let store = load_products(&state.data_root).await;
    Ok(Response::builder()
        .status(poem::http::StatusCode::OK)
        .content_type("application/json")
        .body(serde_json::to_vec(&store.products).unwrap_or_default()))
}

#[handler]
async fn get_product(req: &Request, PathExtractor(id): PathExtractor<u64>, state: Data<&AppState>) -> PoemResult<Response> {
    check_catalog_access(req, &state, access::Need::View).await?;
    let store = load_products(&state.data_root).await;
    match store.products.iter().find(|p| p.id == id) {
        Some(product) => {
            Ok(Response::builder().status(poem::http::StatusCode::OK).content_type("application/json").body(serde_json::to_vec(product).unwrap_or_default()))
        }
        None => Ok(Response::builder().status(poem::http::StatusCode::NOT_FOUND).body("product not found")),
    }
}

#[derive(Deserialize)]
struct UpdateProductRequest {
    name: Option<String>,
    description: Option<String>,
    price_cents: Option<u64>,
    stock: Option<u64>,
    status: Option<ProductStatus>,
}

/// `PUT /api/products/:id` — 商品の各フィールドを更新する(カタログへの
/// `Need::Edit`権限が必要、指定したフィールドのみ更新)。
#[handler]
async fn update_product(
    req: &Request,
    PathExtractor(id): PathExtractor<u64>,
    state: Data<&AppState>,
    body: poem::web::Json<UpdateProductRequest>,
) -> PoemResult<Response> {
    check_catalog_access(req, &state, access::Need::Edit).await?;
    let mut store = load_products(&state.data_root).await;
    let Some(product) = store.products.iter_mut().find(|p| p.id == id) else {
        return Ok(Response::builder().status(poem::http::StatusCode::NOT_FOUND).body("product not found"));
    };
    if let Some(name) = &body.name {
        product.name = name.clone();
    }
    if let Some(description) = &body.description {
        product.description = description.clone();
    }
    if let Some(price_cents) = body.price_cents {
        product.price_cents = price_cents;
    }
    if let Some(stock) = body.stock {
        product.stock = stock;
    }
    if let Some(status) = body.status {
        product.status = status;
    }
    product.updated_at = now_unix();
    let updated = product.clone();
    save_products(&state.data_root, &store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Response::builder().status(poem::http::StatusCode::OK).content_type("application/json").body(serde_json::to_vec(&updated).unwrap_or_default()))
}

/// `DELETE /api/products/:id` — 商品を削除する(カタログへの`Need::Edit`
/// 権限が必要)。
#[handler]
async fn delete_product(req: &Request, PathExtractor(id): PathExtractor<u64>, state: Data<&AppState>) -> PoemResult<Response> {
    check_catalog_access(req, &state, access::Need::Edit).await?;
    let mut store = load_products(&state.data_root).await;
    let before = store.products.len();
    store.products.retain(|p| p.id != id);
    if store.products.len() == before {
        return Ok(Response::builder().status(poem::http::StatusCode::NOT_FOUND).body("product not found"));
    }
    save_products(&state.data_root, &store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Response::builder().status(poem::http::StatusCode::OK).body("deleted"))
}

#[handler]
async fn healthz() -> &'static str {
    "ok"
}

#[handler]
async fn request_otp(state: Data<&AppState>, body: poem::web::Json<serde_json::Value>) -> PoemResult<Response> {
    let email = body.get("email").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if email != state.admin_email {
        let registered = accounts::load(&state.data_root).await;
        if !registered.emails.contains(&email) {
            return Ok(Response::builder().status(poem::http::StatusCode::FORBIDDEN).body("email not registered"));
        }
    }
    let Some(smtp) = state.smtp.clone() else {
        return Ok(Response::builder().status(poem::http::StatusCode::SERVICE_UNAVAILABLE).body("SMTP not configured"));
    };
    let auth::RequestOtpOutcome::Issued(code) = state.auth.request_otp(&email);
    match mail::send_otp(smtp, email, code).await {
        Ok(()) => Ok(Response::builder().status(poem::http::StatusCode::OK).body("otp sent")),
        Err(e) => {
            tracing::warn!("failed to send OTP mail: {e}");
            Ok(Response::builder().status(poem::http::StatusCode::BAD_GATEWAY).body("failed to send mail"))
        }
    }
}

#[derive(Deserialize)]
struct VerifyOtpRequest {
    email: String,
    code: String,
}

#[handler]
async fn verify_otp(state: Data<&AppState>, body: poem::web::Json<VerifyOtpRequest>) -> PoemResult<Response> {
    match state.auth.consume_otp(&body.email, &body.code) {
        Ok(()) => {
            let token = state.auth.create_session(&body.email);
            Ok(Response::builder()
                .status(poem::http::StatusCode::OK)
                .content_type("application/json")
                .body(serde_json::to_vec(&serde_json::json!({ "token": token })).unwrap_or_default()))
        }
        Err(e) => Ok(Response::builder().status(poem::http::StatusCode::FORBIDDEN).body(e.message())),
    }
}

/// `POST /api/auth/logout` — セッショントークンを失効させる。
#[handler]
async fn logout(req: &Request, state: Data<&AppState>) -> PoemResult<Response> {
    let header = req.header(poem::http::header::AUTHORIZATION).unwrap_or("");
    if let Some(token) = header.strip_prefix("Bearer ") {
        state.auth.logout(token);
    }
    Ok(Response::builder().status(poem::http::StatusCode::OK).body("logged out"))
}

#[derive(Deserialize)]
struct AddAccountRequest {
    email: String,
}

/// `POST /api/accounts` — ログイン可能なメールアドレスを1件登録する
/// (管理者のみ)。`accounts_locked`中は管理者メール以外を拒否する。
#[handler]
async fn add_account(req: &Request, state: Data<&AppState>, body: poem::web::Json<AddAccountRequest>) -> PoemResult<Response> {
    require_admin_session(req, &state)?;
    let email = body.email.trim().to_string();
    if !email.contains('@') {
        return Ok(Response::builder().status(poem::http::StatusCode::BAD_REQUEST).body("invalid email"));
    }
    if state.accounts_locked && email != state.admin_email {
        return Ok(Response::builder()
            .status(poem::http::StatusCode::FORBIDDEN)
            .body("account registration is currently restricted to the administrator email only"));
    }
    let mut store = accounts::load(&state.data_root).await;
    store.emails.insert(email);
    accounts::save(&state.data_root, &store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Response::builder().status(poem::http::StatusCode::CREATED).body("ok"))
}

/// `GET /api/accounts` — 登録済みメールアドレス一覧(管理者のみ)。
#[handler]
async fn list_accounts(req: &Request, state: Data<&AppState>) -> PoemResult<Response> {
    require_admin_session(req, &state)?;
    let store = accounts::load(&state.data_root).await;
    let mut emails: Vec<&String> = store.emails.iter().collect();
    emails.sort();
    Ok(Response::builder().status(poem::http::StatusCode::OK).content_type("application/json").body(serde_json::to_vec(&emails).unwrap_or_default()))
}

#[derive(Deserialize)]
struct AccessRequestPayload {
    email: String,
    #[serde(default)]
    message: Option<String>,
}

/// `POST /api/accounts/request` — **認証不要、誰でも申請可能**。
/// ログイン許可を求める申請を保留リストへ追加する
/// (管理者が[`decide_access_request`]で許可するまでは無効)。
#[handler]
async fn request_access(state: Data<&AppState>, body: poem::web::Json<AccessRequestPayload>) -> PoemResult<Response> {
    let email = body.email.trim().to_string();
    if !email.contains('@') {
        return Ok(Response::builder().status(poem::http::StatusCode::BAD_REQUEST).body("invalid email"));
    }
    let mut store = accounts::load(&state.data_root).await;
    let id = accounts::generate_request_id();
    store.pending_requests.push(accounts::AccessRequest { id, email: email.clone(), message: body.message.clone() });
    accounts::save(&state.data_root, &store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    if let Some(smtp) = state.smtp.clone() {
        if let Err(e) = mail::send_access_request_notice(smtp, state.admin_email.clone(), email, body.message.clone()).await {
            tracing::warn!("failed to notify admin of access request: {e}");
        }
    }
    Ok(Response::builder().status(poem::http::StatusCode::CREATED).body("request submitted"))
}

/// `GET /api/accounts/requests` — 保留中の申請一覧(管理者のみ)。
#[handler]
async fn list_access_requests(req: &Request, state: Data<&AppState>) -> PoemResult<Response> {
    require_admin_session(req, &state)?;
    let store = accounts::load(&state.data_root).await;
    Ok(Response::builder()
        .status(poem::http::StatusCode::OK)
        .content_type("application/json")
        .body(serde_json::to_vec(&store.pending_requests).unwrap_or_default()))
}

#[derive(Deserialize)]
struct DecideAccessRequestPayload {
    approve: bool,
    #[serde(default)]
    allow_view: bool,
    #[serde(default)]
    allow_edit: bool,
}

/// `POST /api/accounts/requests/:id/decide` — 申請を審査する(管理者のみ)。
/// 承認時、カタログ全体の`access::AccessConfig::accounts`に閲覧/編集
/// 許可を書き込む。`accounts_locked`中は管理者メール以外の承認を
/// 拒否する(`RGit`/`RS-Chiketto`の`*_ACCOUNTS_LOCKED`と同じ方針)。
#[handler]
async fn decide_access_request(
    req: &Request,
    PathExtractor(id): PathExtractor<String>,
    state: Data<&AppState>,
    body: poem::web::Json<DecideAccessRequestPayload>,
) -> PoemResult<Response> {
    require_admin_session(req, &state)?;
    let mut store = accounts::load(&state.data_root).await;
    let Some(pos) = store.pending_requests.iter().position(|r| r.id == id) else {
        return Ok(Response::builder().status(poem::http::StatusCode::NOT_FOUND).body("request not found"));
    };
    let request = store.pending_requests.remove(pos);

    if body.approve && state.accounts_locked && request.email != state.admin_email {
        accounts::save(&state.data_root, &store)
            .await
            .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
        return Ok(Response::builder()
            .status(poem::http::StatusCode::FORBIDDEN)
            .body("account registration is currently restricted to the administrator email only"));
    }

    if body.approve {
        store.emails.insert(request.email.clone());
    }
    accounts::save(&state.data_root, &store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;

    if body.approve {
        let mut config = access::load(&state.data_root).await;
        config.accounts.insert(request.email.clone(), access::AccountPermission { allow_view: body.allow_view, allow_edit: body.allow_edit });
        access::save(&state.data_root, &config)
            .await
            .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    }

    if let Some(smtp) = state.smtp.clone() {
        if let Err(e) = mail::send_access_decision(smtp, request.email.clone(), body.approve).await {
            tracing::warn!("failed to notify requester of decision: {e}");
        }
    }
    Ok(Response::builder().status(poem::http::StatusCode::OK).body(if body.approve { "approved" } else { "denied" }))
}

fn env_data_dir() -> PathBuf {
    std::env::var("RSEC_DATA_DIR").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("./data"))
}

/// ルーティング定義を`main()`とテスト(`poem::test::TestClient`)の両方から
/// 再利用できるように切り出したもの。
fn build_routes(state: AppState) -> impl poem::Endpoint {
    Route::new()
        .at("/healthz", get(healthz))
        .at("/api/auth/request-otp", post(request_otp))
        .at("/api/auth/verify-otp", post(verify_otp))
        .at("/api/auth/logout", post(logout))
        .at("/api/accounts", get(list_accounts).post(add_account))
        .at("/api/accounts/request", post(request_access))
        .at("/api/accounts/requests", get(list_access_requests))
        .at("/api/accounts/requests/:id/decide", post(decide_access_request))
        .at("/api/products", get(list_products).post(create_product))
        .at("/api/products/:id", get(get_product).put(update_product).delete(delete_product))
        .data(state)
        .with(Tracing)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let data_root = env_data_dir();
    tokio::fs::create_dir_all(&data_root).await?;
    tracing::info!("rs-ec v0.1.0 starting, data_root={:?}", data_root);

    let admin_email = std::env::var("RSEC_ADMIN_EMAIL").unwrap_or_else(|_| "admin@example.com".to_string());
    let smtp = mail::SmtpConfig::from_env();
    if smtp.is_none() {
        tracing::warn!("RSEC_SMTP_* not fully configured; /api/auth/request-otp will return 503");
    }
    let accounts_locked = std::env::var("RSEC_ACCOUNTS_LOCKED").map(|v| v != "false" && v != "0").unwrap_or(true);
    if accounts_locked {
        tracing::info!("account registration is locked to the admin email only (RSEC_ACCOUNTS_LOCKED=false to lift)");
    }
    let state = AppState { data_root, auth: Arc::new(auth::AuthStore::default()), admin_email, smtp, accounts_locked };

    let app = build_routes(state);

    let port = std::env::var("RSEC_PORT").unwrap_or_else(|_| "8102".to_string());
    let addr = format!("0.0.0.0:{port}");
    tracing::info!("listening on {addr}");
    Server::new(TcpListener::bind(addr)).run(app).await?;
    Ok(())
}

#[cfg(test)]
mod handler_tests {
    //! `poem::test::TestClient`を使ったハンドラレベルの統合テスト
    //! (`RS-Chiketto`と同じ流儀)。`RSEC_DATA_DIR`環境変数には依存せず、
    //! テストごとに独立した一時ディレクトリを直接`AppState`へ渡す。

    use super::*;
    use poem::test::TestClient;

    const ADMIN_EMAIL: &str = "admin@example.com";

    fn temp_dir(label: &str) -> PathBuf {
        let unique = accounts::generate_request_id();
        std::env::temp_dir().join(format!("rsec-handler-test-{label}-{unique}"))
    }

    async fn make_state(label: &str, accounts_locked: bool) -> AppState {
        let data_root = temp_dir(label);
        tokio::fs::create_dir_all(&data_root).await.unwrap();
        AppState { data_root, auth: Arc::new(auth::AuthStore::default()), admin_email: ADMIN_EMAIL.to_string(), smtp: None, accounts_locked }
    }

    fn admin_token(state: &AppState) -> String {
        state.auth.create_session(ADMIN_EMAIL)
    }

    #[tokio::test]
    async fn unauthenticated_list_products_returns_401_when_catalog_is_private() {
        // access.rsの既定は`Mode::Private`のため、未ログインでの一覧取得は
        // 401(RS-Chikettoのproject単位フィルタとは異なり、カタログ全体が
        // 単一リソースのため空配列ではなく401を返す設計、main.rs参照)。
        let state = make_state("list-private", true).await;
        let app = build_routes(state);
        let client = TestClient::new(app);

        let resp = client.get("/api/products").send().await;
        resp.assert_status(poem::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn admin_can_create_and_list_products() {
        let state = make_state("admin-crud", true).await;
        let token = admin_token(&state);
        let app = build_routes(state);
        let client = TestClient::new(app);

        let resp = client
            .post("/api/products")
            .header("Authorization", format!("Bearer {token}"))
            .body_json(&serde_json::json!({ "name": "Widget", "description": "A widget", "price_cents": 1999, "stock": 10 }))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::CREATED);

        let resp = client.get("/api/products").header("Authorization", format!("Bearer {token}")).send().await;
        resp.assert_status_is_ok();
        let body: Vec<Product> = resp.json().await.value().deserialize();
        assert_eq!(body.len(), 1);
        assert_eq!(body[0].name, "Widget");
        assert!(matches!(body[0].status, ProductStatus::Draft));
    }

    #[tokio::test]
    async fn update_and_delete_product_round_trip() {
        let state = make_state("update-delete", true).await;
        let token = admin_token(&state);
        let app = build_routes(state);
        let client = TestClient::new(app);

        let resp = client
            .post("/api/products")
            .header("Authorization", format!("Bearer {token}"))
            .body_json(&serde_json::json!({ "name": "Gadget", "price_cents": 500, "stock": 3 }))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::CREATED);
        let created: Product = resp.json().await.value().deserialize();

        let resp = client
            .put(format!("/api/products/{}", created.id))
            .header("Authorization", format!("Bearer {token}"))
            .body_json(&serde_json::json!({ "status": "on_sale", "stock": 5 }))
            .send()
            .await;
        resp.assert_status_is_ok();
        let updated: Product = resp.json().await.value().deserialize();
        assert!(matches!(updated.status, ProductStatus::OnSale));
        assert_eq!(updated.stock, 5);

        let resp = client.delete(format!("/api/products/{}", created.id)).header("Authorization", format!("Bearer {token}")).send().await;
        resp.assert_status_is_ok();

        let resp = client.get(format!("/api/products/{}", created.id)).header("Authorization", format!("Bearer {token}")).send().await;
        resp.assert_status(poem::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn self_service_account_request_returns_201_and_creates_pending_request() {
        let state = make_state("self-service-request", true).await;
        let data_root = state.data_root.clone();
        let app = build_routes(state);
        let client = TestClient::new(app);

        let resp = client
            .post("/api/accounts/request")
            .body_json(&serde_json::json!({ "email": "newcomer@example.com", "message": "please let me in" }))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::CREATED);

        let store = accounts::load(&data_root).await;
        assert_eq!(store.pending_requests.len(), 1);
        assert_eq!(store.pending_requests[0].email, "newcomer@example.com");
    }

    #[tokio::test]
    async fn admin_approving_a_request_grants_the_expected_access_config_entry() {
        let state = make_state("approve-grants-access", false).await;
        let data_root = state.data_root.clone();
        let token = admin_token(&state);
        let app = build_routes(state);
        let client = TestClient::new(app);

        client
            .post("/api/accounts/request")
            .body_json(&serde_json::json!({ "email": "member@example.com" }))
            .send()
            .await
            .assert_status(poem::http::StatusCode::CREATED);

        let store = accounts::load(&data_root).await;
        let request_id = store.pending_requests[0].id.clone();

        let resp = client
            .post(format!("/api/accounts/requests/{request_id}/decide"))
            .header("Authorization", format!("Bearer {token}"))
            .body_json(&serde_json::json!({ "approve": true, "allow_view": true, "allow_edit": false }))
            .send()
            .await;
        resp.assert_status_is_ok();

        let updated_store = accounts::load(&data_root).await;
        assert!(updated_store.emails.contains("member@example.com"));
        assert!(updated_store.pending_requests.is_empty());

        let config = access::load(&data_root).await;
        let perm = config.accounts.get("member@example.com").expect("member should have an access grant");
        assert!(perm.allow_view);
        assert!(!perm.allow_edit);
    }

    #[tokio::test]
    async fn accounts_locked_rejects_non_admin_approval_with_403() {
        let state = make_state("locked-rejects-approval", true).await;
        let data_root = state.data_root.clone();
        let token = admin_token(&state);
        let app = build_routes(state);
        let client = TestClient::new(app);

        client
            .post("/api/accounts/request")
            .body_json(&serde_json::json!({ "email": "outsider@example.com" }))
            .send()
            .await
            .assert_status(poem::http::StatusCode::CREATED);

        let store = accounts::load(&data_root).await;
        let request_id = store.pending_requests[0].id.clone();

        let resp = client
            .post(format!("/api/accounts/requests/{request_id}/decide"))
            .header("Authorization", format!("Bearer {token}"))
            .body_json(&serde_json::json!({ "approve": true }))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::FORBIDDEN);

        let after = accounts::load(&data_root).await;
        assert!(!after.emails.contains("outsider@example.com"));
    }
}
