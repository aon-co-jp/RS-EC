//! # RS-EC (v0.1.0)
//!
//! [EC-CUBE](https://www.ec-cube.net/)(PHP製)の、ハイスピード・
//! ハイセキュリティ・省メモリなRust+[poem](https://github.com/poem-web/poem)版を目指す。
//!
//! ## 正直な開示(最重要、`RGit`/`RS-Chiketto`/`RS-Blog`と同じ流儀)
//!
//! **v0.1.0時点では、商品カタログ(Product)のCRUD、カテゴリ管理、
//! お気に入り(wishlist)、商品レビュー・評価(`src/reviews.rs`)、
//! カート(`src/cart.rs`)・注文/決済(`src/order.rs`、モック決済のみ)
//! を実装している。**
//! EC-CUBEが持つ以下の機能は**まだ一切無い**:
//!
//! - **実決済ゲートウェイ連携(Stripe等)は未実装**。`POST /api/orders/checkout`
//!   はカートを注文に確定するが、決済処理は`order::process_mock_payment`
//!   という常に成功するダミー関数(金額0円のときのみ失敗)で、実際の
//!   金銭のやり取り・カード情報の送受信は一切行わない(正直な開示)。
//! - 在庫の自動引き落としは注文確定時に減算するのみで、予約(仮引当)・
//!   キャンセル時の在庫復元・バックオーダー等は未実装。
//! - 配送(送料計算・配送状況追跡)は未実装。
//! - 会員管理(ポイント等、ログイン自体はOTP認証のみ実装)
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
mod cart;
mod categories;
mod favorites;
mod mail;
mod order;
mod reviews;

use std::path::PathBuf;
use std::sync::Arc;

use poem::listener::TcpListener;
use poem::middleware::Tracing;
use poem::web::Data;
use poem::{delete, get, handler, post, web::Path as PathExtractor, EndpointExt, Request, Response, Result as PoemResult, Route, Server};
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
    /// 所属カテゴリのID群(0個以上、多対多)。`categories.rs`の
    /// `Category::id`を参照するが、外部キー制約は無し(JSONファイル
    /// 永続化のため存在しないIDを指していても検証はしない)。
    #[serde(default)]
    categories: Vec<u64>,
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
    #[serde(default)]
    categories: Vec<u64>,
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
        categories: body.categories.clone(),
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
    let category_filter: Option<u64> = req.uri().query().and_then(|q| {
        q.split('&').find_map(|pair| pair.strip_prefix("category=").and_then(|v| v.parse::<u64>().ok()))
    });
    let products: Vec<&Product> = match category_filter {
        Some(category_id) => store.products.iter().filter(|p| p.categories.contains(&category_id)).collect(),
        None => store.products.iter().collect(),
    };
    Ok(Response::builder()
        .status(poem::http::StatusCode::OK)
        .content_type("application/json")
        .body(serde_json::to_vec(&products).unwrap_or_default()))
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
    categories: Option<Vec<u64>>,
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
    if let Some(categories) = &body.categories {
        product.categories = categories.clone();
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

#[derive(Deserialize)]
struct CreateCategoryRequest {
    name: String,
    slug: String,
}

/// `POST /api/categories` — カテゴリを新規作成する(管理者のみ)。
#[handler]
async fn create_category(req: &Request, state: Data<&AppState>, body: poem::web::Json<CreateCategoryRequest>) -> PoemResult<Response> {
    require_admin_session(req, &state)?;
    if body.name.trim().is_empty() || body.slug.trim().is_empty() {
        return Ok(Response::builder().status(poem::http::StatusCode::BAD_REQUEST).body("name and slug must not be empty"));
    }
    let mut store = categories::load(&state.data_root).await;
    let id = store.next_id;
    store.next_id += 1;
    let category = categories::Category { id, name: body.name.clone(), slug: body.slug.clone() };
    store.categories.push(category.clone());
    categories::save(&state.data_root, &store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Response::builder()
        .status(poem::http::StatusCode::CREATED)
        .content_type("application/json")
        .body(serde_json::to_vec(&category).unwrap_or_default()))
}

/// `GET /api/categories` — カテゴリ一覧(誰でも閲覧可、商品カタログとは
/// 異なりアクセス制御の対象外——EC-CUBEのカテゴリツリーは通常公開情報)。
#[handler]
async fn list_categories(state: Data<&AppState>) -> PoemResult<Response> {
    let store = categories::load(&state.data_root).await;
    Ok(Response::builder()
        .status(poem::http::StatusCode::OK)
        .content_type("application/json")
        .body(serde_json::to_vec(&store.categories).unwrap_or_default()))
}

/// `DELETE /api/categories/:id` — カテゴリを削除する(管理者のみ)。
/// 削除してもそのカテゴリを参照している`Product::categories`のIDは
/// そのまま残る(外部キー制約が無いことの直接の帰結、正直な開示)。
#[handler]
async fn delete_category(req: &Request, PathExtractor(id): PathExtractor<u64>, state: Data<&AppState>) -> PoemResult<Response> {
    require_admin_session(req, &state)?;
    let mut store = categories::load(&state.data_root).await;
    let before = store.categories.len();
    store.categories.retain(|c| c.id != id);
    if store.categories.len() == before {
        return Ok(Response::builder().status(poem::http::StatusCode::NOT_FOUND).body("category not found"));
    }
    categories::save(&state.data_root, &store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Response::builder().status(poem::http::StatusCode::OK).body("deleted"))
}

#[derive(Deserialize)]
struct AddFavoriteRequest {
    product_id: u64,
}

/// `POST /api/favorites` — ログイン中アカウントのお気に入りに商品を
/// 1件追加する。**カートへの追加ではない**: 数量・合計金額・注文は
/// 一切扱わない、保存した商品IDの集合のみ(`favorites.rs`参照)。
#[handler]
async fn add_favorite(req: &Request, state: Data<&AppState>, body: poem::web::Json<AddFavoriteRequest>) -> PoemResult<Response> {
    let Some(email) = session_email(req, &state) else {
        return Err(poem::Error::from_string("login required", poem::http::StatusCode::UNAUTHORIZED));
    };
    let products = load_products(&state.data_root).await;
    if !products.products.iter().any(|p| p.id == body.product_id) {
        return Ok(Response::builder().status(poem::http::StatusCode::NOT_FOUND).body("product not found"));
    }
    let mut store = favorites::load(&state.data_root).await;
    store.by_email.entry(email).or_default().insert(body.product_id);
    favorites::save(&state.data_root, &store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Response::builder().status(poem::http::StatusCode::CREATED).body("added"))
}

/// `GET /api/favorites` — ログイン中アカウント自身のお気に入り商品一覧
/// (他アカウントの一覧は見えない)。
#[handler]
async fn list_favorites(req: &Request, state: Data<&AppState>) -> PoemResult<Response> {
    let Some(email) = session_email(req, &state) else {
        return Err(poem::Error::from_string("login required", poem::http::StatusCode::UNAUTHORIZED));
    };
    let favorite_store = favorites::load(&state.data_root).await;
    let ids = favorite_store.by_email.get(&email).cloned().unwrap_or_default();
    let products = load_products(&state.data_root).await;
    let favorited: Vec<&Product> = products.products.iter().filter(|p| ids.contains(&p.id)).collect();
    Ok(Response::builder()
        .status(poem::http::StatusCode::OK)
        .content_type("application/json")
        .body(serde_json::to_vec(&favorited).unwrap_or_default()))
}

/// `DELETE /api/favorites/:product_id` — ログイン中アカウントのお気に
/// 入りから1件削除する。
#[handler]
async fn remove_favorite(req: &Request, PathExtractor(product_id): PathExtractor<u64>, state: Data<&AppState>) -> PoemResult<Response> {
    let Some(email) = session_email(req, &state) else {
        return Err(poem::Error::from_string("login required", poem::http::StatusCode::UNAUTHORIZED));
    };
    let mut store = favorites::load(&state.data_root).await;
    let removed = store.by_email.get_mut(&email).map(|set| set.remove(&product_id)).unwrap_or(false);
    if !removed {
        return Ok(Response::builder().status(poem::http::StatusCode::NOT_FOUND).body("favorite not found"));
    }
    favorites::save(&state.data_root, &store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Response::builder().status(poem::http::StatusCode::OK).body("removed"))
}

#[derive(Deserialize)]
struct CreateReviewRequest {
    rating: u8,
    title: String,
    body: String,
}

/// `POST /api/products/:id/reviews` — ログイン中(登録アカウントまたは
/// 管理者)のみ投稿可能。`rating`は1〜5のみ許可(それ以外は`400`)。
/// 投稿直後は`approved: false`(`RS-Blog`のコメント承認制と同じ、
/// 管理者承認まで一般公開の一覧・平均評価には出ない)。
#[handler]
async fn create_review(
    req: &Request,
    PathExtractor(product_id): PathExtractor<u64>,
    state: Data<&AppState>,
    body: poem::web::Json<CreateReviewRequest>,
) -> PoemResult<Response> {
    let Some(email) = session_email(req, &state) else {
        return Err(poem::Error::from_string("login required", poem::http::StatusCode::UNAUTHORIZED));
    };
    if body.rating < 1 || body.rating > 5 {
        return Ok(Response::builder().status(poem::http::StatusCode::BAD_REQUEST).body("rating must be between 1 and 5"));
    }
    let products = load_products(&state.data_root).await;
    if !products.products.iter().any(|p| p.id == product_id) {
        return Ok(Response::builder().status(poem::http::StatusCode::NOT_FOUND).body("product not found"));
    }
    let mut store = reviews::load(&state.data_root).await;
    let id = store.next_id;
    store.next_id += 1;
    let review = reviews::Review {
        id,
        product_id,
        author_email: email,
        rating: body.rating,
        title: body.title.clone(),
        body: body.body.clone(),
        created_at: now_unix(),
        approved: false,
    };
    store.reviews.push(review.clone());
    reviews::save(&state.data_root, &store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Response::builder()
        .status(poem::http::StatusCode::CREATED)
        .content_type("application/json")
        .body(serde_json::to_vec(&review).unwrap_or_default()))
}

/// `GET /api/products/:id/reviews` — `?approved_only=true`なら誰でも
/// 承認済みレビューのみ閲覧可(公開)。それ以外(クエリ無し、または
/// `approved_only=false`)は管理者のみ、未承認を含む全件を返す。
#[handler]
async fn list_reviews(req: &Request, PathExtractor(product_id): PathExtractor<u64>, state: Data<&AppState>) -> PoemResult<Response> {
    let approved_only = req
        .uri()
        .query()
        .map(|q| q.split('&').any(|pair| pair == "approved_only=true"))
        .unwrap_or(false);
    let store = reviews::load(&state.data_root).await;
    let filtered: Vec<&reviews::Review> = if approved_only {
        store.reviews.iter().filter(|r| r.product_id == product_id && r.approved).collect()
    } else {
        match session_email(req, &state) {
            Some(email) if email == state.admin_email => {}
            Some(_) => return Err(poem::Error::from_string("insufficient permission", poem::http::StatusCode::FORBIDDEN)),
            None => return Err(poem::Error::from_string("login required", poem::http::StatusCode::UNAUTHORIZED)),
        }
        store.reviews.iter().filter(|r| r.product_id == product_id).collect()
    };
    Ok(Response::builder()
        .status(poem::http::StatusCode::OK)
        .content_type("application/json")
        .body(serde_json::to_vec(&filtered).unwrap_or_default()))
}

#[derive(Serialize)]
struct RatingSummary {
    average_rating: f32,
    review_count: u32,
}

/// `GET /api/products/:id/rating-summary` — 公開。承認済みレビューのみ
/// から平均評価・件数を計算する(未承認の1件が承認前に平均を歪めない
/// ようにするため、タスク要件)。
#[handler]
async fn rating_summary_handler(PathExtractor(product_id): PathExtractor<u64>, state: Data<&AppState>) -> PoemResult<Response> {
    let store = reviews::load(&state.data_root).await;
    let for_product: Vec<&reviews::Review> = store.reviews.iter().filter(|r| r.product_id == product_id).collect();
    let (average_rating, review_count) = reviews::rating_summary(&for_product);
    Ok(Response::builder()
        .status(poem::http::StatusCode::OK)
        .content_type("application/json")
        .body(serde_json::to_vec(&RatingSummary { average_rating, review_count }).unwrap_or_default()))
}

/// `POST /api/reviews/:id/approve` — 管理者のみ、レビューを承認して
/// 一般公開する。
#[handler]
async fn approve_review(req: &Request, PathExtractor(id): PathExtractor<u64>, state: Data<&AppState>) -> PoemResult<Response> {
    require_admin_session(req, &state)?;
    let mut store = reviews::load(&state.data_root).await;
    let Some(review) = store.reviews.iter_mut().find(|r| r.id == id) else {
        return Ok(Response::builder().status(poem::http::StatusCode::NOT_FOUND).body("review not found"));
    };
    review.approved = true;
    let updated = review.clone();
    reviews::save(&state.data_root, &store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Response::builder().status(poem::http::StatusCode::OK).content_type("application/json").body(serde_json::to_vec(&updated).unwrap_or_default()))
}

/// `DELETE /api/reviews/:id` — 管理者、またはそのレビューの投稿者本人
/// のみ削除可能。
#[handler]
async fn delete_review(req: &Request, PathExtractor(id): PathExtractor<u64>, state: Data<&AppState>) -> PoemResult<Response> {
    let Some(email) = session_email(req, &state) else {
        return Err(poem::Error::from_string("login required", poem::http::StatusCode::UNAUTHORIZED));
    };
    let mut store = reviews::load(&state.data_root).await;
    let Some(review) = store.reviews.iter().find(|r| r.id == id) else {
        return Ok(Response::builder().status(poem::http::StatusCode::NOT_FOUND).body("review not found"));
    };
    if email != state.admin_email && email != review.author_email {
        return Err(poem::Error::from_string("insufficient permission", poem::http::StatusCode::FORBIDDEN));
    }
    store.reviews.retain(|r| r.id != id);
    reviews::save(&state.data_root, &store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Response::builder().status(poem::http::StatusCode::OK).body("deleted"))
}

#[derive(Serialize)]
struct CartLine {
    product_id: u64,
    name: String,
    unit_price_cents: u64,
    quantity: u64,
    subtotal_cents: u64,
}

#[derive(Serialize)]
struct CartView {
    items: Vec<CartLine>,
    total_cents: u64,
}

async fn build_cart_view(state: &AppState, email: &str) -> CartView {
    let cart_store = cart::load(&state.data_root).await;
    let products = load_products(&state.data_root).await;
    let raw_items = cart_store.by_email.get(email).cloned().unwrap_or_default();
    let mut items = Vec::new();
    let mut total_cents: u64 = 0;
    for line in raw_items {
        if let Some(product) = products.products.iter().find(|p| p.id == line.product_id) {
            let subtotal = product.price_cents.saturating_mul(line.quantity);
            total_cents += subtotal;
            items.push(CartLine {
                product_id: product.id,
                name: product.name.clone(),
                unit_price_cents: product.price_cents,
                quantity: line.quantity,
                subtotal_cents: subtotal,
            });
        }
        // 商品が削除済みの場合はカート表示からは黙って除外する
        // (`checkout`側でも同様に扱い、注文には含めない)。
    }
    CartView { items, total_cents }
}

/// `GET /api/cart` — ログイン中アカウント自身のカート内容(商品スナップ
/// ショットと小計・合計)。カート自体は決済・注文作成を行わない。
#[handler]
async fn get_cart(req: &Request, state: Data<&AppState>) -> PoemResult<Response> {
    let Some(email) = session_email(req, &state) else {
        return Err(poem::Error::from_string("login required", poem::http::StatusCode::UNAUTHORIZED));
    };
    let view = build_cart_view(&state, &email).await;
    Ok(Response::builder().status(poem::http::StatusCode::OK).content_type("application/json").body(serde_json::to_vec(&view).unwrap_or_default()))
}

#[derive(Deserialize)]
struct AddCartItemRequest {
    product_id: u64,
    #[serde(default = "default_cart_quantity")]
    quantity: u64,
}

fn default_cart_quantity() -> u64 {
    1
}

/// `POST /api/cart/items` — カートに商品を追加(既にあれば数量を加算)。
/// `quantity`は1以上必須(それ以外は`400`)。商品の存在確認のみ行い、
/// 在庫チェックは注文確定(`checkout`)時に行う(カート追加時点では
/// 在庫を予約・引当しない、単純な実装であることの明記)。
#[handler]
async fn add_cart_item(req: &Request, state: Data<&AppState>, body: poem::web::Json<AddCartItemRequest>) -> PoemResult<Response> {
    let Some(email) = session_email(req, &state) else {
        return Err(poem::Error::from_string("login required", poem::http::StatusCode::UNAUTHORIZED));
    };
    if body.quantity == 0 {
        return Ok(Response::builder().status(poem::http::StatusCode::BAD_REQUEST).body("quantity must be at least 1"));
    }
    let products = load_products(&state.data_root).await;
    if !products.products.iter().any(|p| p.id == body.product_id) {
        return Ok(Response::builder().status(poem::http::StatusCode::NOT_FOUND).body("product not found"));
    }
    let mut store = cart::load(&state.data_root).await;
    let items = store.by_email.entry(email.clone()).or_default();
    cart::upsert_item(items, body.product_id, body.quantity);
    cart::save(&state.data_root, &store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    let view = build_cart_view(&state, &email).await;
    Ok(Response::builder().status(poem::http::StatusCode::CREATED).content_type("application/json").body(serde_json::to_vec(&view).unwrap_or_default()))
}

/// `DELETE /api/cart/items/:product_id` — カートから1商品を削除する。
#[handler]
async fn remove_cart_item(req: &Request, PathExtractor(product_id): PathExtractor<u64>, state: Data<&AppState>) -> PoemResult<Response> {
    let Some(email) = session_email(req, &state) else {
        return Err(poem::Error::from_string("login required", poem::http::StatusCode::UNAUTHORIZED));
    };
    let mut store = cart::load(&state.data_root).await;
    let Some(items) = store.by_email.get_mut(&email) else {
        return Ok(Response::builder().status(poem::http::StatusCode::NOT_FOUND).body("cart item not found"));
    };
    let before = items.len();
    cart::remove_item(items, product_id);
    if items.len() == before {
        return Ok(Response::builder().status(poem::http::StatusCode::NOT_FOUND).body("cart item not found"));
    }
    cart::save(&state.data_root, &store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Response::builder().status(poem::http::StatusCode::OK).body("removed"))
}

/// `POST /api/orders/checkout` — ログイン中アカウントのカート内容を
/// 注文として確定する。**実際の決済は一切行わず、`order::process_mock_payment`
/// という常に成功するダミー関数で決済成功を模擬する**(正直な開示、
/// `CLAUDE.md`参照)。カートが空、在庫不足の商品がある場合は`400`。
/// 成功時は各商品の在庫を数量分減算し、カートを空にする。
#[handler]
async fn checkout(req: &Request, state: Data<&AppState>) -> PoemResult<Response> {
    let Some(email) = session_email(req, &state) else {
        return Err(poem::Error::from_string("login required", poem::http::StatusCode::UNAUTHORIZED));
    };
    let mut cart_store = cart::load(&state.data_root).await;
    let raw_items = cart_store.by_email.get(&email).cloned().unwrap_or_default();
    if raw_items.is_empty() {
        return Ok(Response::builder().status(poem::http::StatusCode::BAD_REQUEST).body("cart is empty"));
    }

    let mut product_store = load_products(&state.data_root).await;
    let mut order_items = Vec::new();
    let mut total_cents: u64 = 0;
    for line in &raw_items {
        let Some(product) = product_store.products.iter().find(|p| p.id == line.product_id) else {
            return Ok(Response::builder().status(poem::http::StatusCode::BAD_REQUEST).body(format!("product {} no longer exists", line.product_id)));
        };
        if product.stock < line.quantity {
            return Ok(Response::builder()
                .status(poem::http::StatusCode::BAD_REQUEST)
                .body(format!("insufficient stock for product {} (have {}, requested {})", product.id, product.stock, line.quantity)));
        }
        let subtotal = product.price_cents.saturating_mul(line.quantity);
        total_cents += subtotal;
        order_items.push(order::OrderItem {
            product_id: product.id,
            name: product.name.clone(),
            unit_price_cents: product.price_cents,
            quantity: line.quantity,
        });
    }

    let (paid, payment_reference) = order::process_mock_payment(total_cents);
    let now = now_unix();
    let mut order_store = order::load(&state.data_root).await;
    let id = order_store.next_id;
    order_store.next_id += 1;
    let status = if paid { order::OrderStatus::Paid } else { order::OrderStatus::PaymentFailed };
    let new_order = order::Order {
        id,
        email: email.clone(),
        items: order_items,
        total_cents,
        status,
        payment_reference,
        created_at: now,
        updated_at: now,
    };

    if paid {
        // 在庫引き落とし(単純な即時減算、予約・バックオーダーは未実装)
        for line in &raw_items {
            if let Some(product) = product_store.products.iter_mut().find(|p| p.id == line.product_id) {
                product.stock -= line.quantity;
                product.updated_at = now;
                if product.stock == 0 {
                    product.status = ProductStatus::SoldOut;
                }
            }
        }
        save_products(&state.data_root, &product_store)
            .await
            .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
        cart_store.by_email.remove(&email);
        cart::save(&state.data_root, &cart_store)
            .await
            .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;
    }

    order_store.orders.push(new_order.clone());
    order::save(&state.data_root, &order_store)
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), poem::http::StatusCode::INTERNAL_SERVER_ERROR))?;

    let response_status = if paid { poem::http::StatusCode::CREATED } else { poem::http::StatusCode::PAYMENT_REQUIRED };
    Ok(Response::builder().status(response_status).content_type("application/json").body(serde_json::to_vec(&new_order).unwrap_or_default()))
}

/// `GET /api/orders` — ログイン中アカウント自身の注文一覧(管理者は
/// 常に全アカウントの注文一覧が見える)。
#[handler]
async fn list_orders(req: &Request, state: Data<&AppState>) -> PoemResult<Response> {
    let Some(email) = session_email(req, &state) else {
        return Err(poem::Error::from_string("login required", poem::http::StatusCode::UNAUTHORIZED));
    };
    let store = order::load(&state.data_root).await;
    let orders: Vec<&order::Order> = if email == state.admin_email {
        store.orders.iter().collect()
    } else {
        store.orders.iter().filter(|o| o.email == email).collect()
    };
    Ok(Response::builder().status(poem::http::StatusCode::OK).content_type("application/json").body(serde_json::to_vec(&orders).unwrap_or_default()))
}

/// `GET /api/orders/:id` — 本人または管理者のみ閲覧可能。
#[handler]
async fn get_order(req: &Request, PathExtractor(id): PathExtractor<u64>, state: Data<&AppState>) -> PoemResult<Response> {
    let Some(email) = session_email(req, &state) else {
        return Err(poem::Error::from_string("login required", poem::http::StatusCode::UNAUTHORIZED));
    };
    let store = order::load(&state.data_root).await;
    let Some(found) = store.orders.iter().find(|o| o.id == id) else {
        return Ok(Response::builder().status(poem::http::StatusCode::NOT_FOUND).body("order not found"));
    };
    if found.email != email && email != state.admin_email {
        return Err(poem::Error::from_string("insufficient permission", poem::http::StatusCode::FORBIDDEN));
    }
    Ok(Response::builder().status(poem::http::StatusCode::OK).content_type("application/json").body(serde_json::to_vec(found).unwrap_or_default()))
}

/// トップページ(`GET /`)のHTMLランディングページ。
/// ブラウザで実インスタンスへアクセスしたユーザーへ、アプリの概要・
/// 実装済みAPI一覧・カート/注文/決済が一切無いことの正直な開示・
/// ダウンロードリンクを示す(JSON APIのみで何も表示されないUXバグの修正)。
const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="ja">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>RS-EC</title>
<style>
  body { font-family: system-ui, sans-serif; max-width: 780px; margin: 2rem auto; padding: 0 1rem; line-height: 1.6; color: #222; }
  h1 { margin-bottom: 0; }
  .tagline { color: #666; margin-top: 0.2rem; }
  code { background: #f2f2f2; padding: 0.1rem 0.35rem; border-radius: 3px; }
  table { border-collapse: collapse; width: 100%; margin: 1rem 0; }
  th, td { text-align: left; padding: 0.4rem 0.6rem; border-bottom: 1px solid #ddd; font-size: 0.92rem; }
  .warn { background: #fff8e1; border: 1px solid #ffe08a; border-radius: 6px; padding: 0.8rem 1rem; }
  .danger { background: #fdecea; border: 1px solid #f5b8b0; border-radius: 6px; padding: 0.8rem 1rem; }
  .btn { display: inline-block; background: #2d6cdf; color: #fff; padding: 0.5rem 1rem; border-radius: 6px; text-decoration: none; margin-right: 0.5rem; }
  footer { color: #888; font-size: 0.85rem; margin-top: 2rem; }
</style>
</head>
<body>
<h1>RS-EC</h1>
<p class="tagline">EC-CUBE相当のECエンジン — Rust + poem(RPoem)製、高速・高セキュリティ・省メモリ志向。v0.1.0。</p>

<div class="danger">
<strong>重要: 決済・カート機能はまだ一切ありません</strong>
<p>
  v0.1.0時点では商品カタログ(Product)のCRUDとOTPログイン(管理者+
  登録アカウント)のみを実装しています。<strong>カート・注文・決済連携
  (Stripe等)は一切実装していません。</strong>支払い情報・カード情報を
  扱う処理はコード内に一切存在しません。
</p>
</div>

<h2>これは何?</h2>
<p>
  <a href="https://www.ec-cube.net/">EC-CUBE</a>のRust版を目指すプロジェクトです。
  実決済連携までを将来目標としていますが、現時点では商品カタログ管理のみです。
</p>

<h2>使い方: 現在はJSON APIのみ(ブラウザUIはまだありません)</h2>
<p>このページ以外はすべてJSON APIです。以下のエンドポイントに対して<code>curl</code>や外部クライアントからアクセスしてください。</p>
<table>
<tr><th>メソッド / パス</th><th>説明</th></tr>
<tr><td><code>GET /healthz</code></td><td>ヘルスチェック</td></tr>
<tr><td><code>POST /api/auth/request-otp</code></td><td>ログイン用ワンタイムパスワードをメール送信(管理者+登録アカウント)</td></tr>
<tr><td><code>POST /api/auth/verify-otp</code></td><td>OTPを検証してセッショントークンを発行</td></tr>
<tr><td><code>POST /api/auth/logout</code></td><td>ログアウト(トークン失効)</td></tr>
<tr><td><code>GET /api/accounts</code> / <code>POST /api/accounts</code></td><td>登録済みメールアドレスの一覧・追加(管理者のみ)</td></tr>
<tr><td><code>POST /api/accounts/request</code></td><td>アクセス許可の自己申請(認証不要)</td></tr>
<tr><td><code>GET /api/accounts/requests</code></td><td>申請一覧(管理者のみ)</td></tr>
<tr><td><code>POST /api/accounts/requests/:id/decide</code></td><td>申請の審査・承認/却下(管理者のみ)</td></tr>
<tr><td><code>GET /api/products</code> / <code>POST /api/products</code></td><td>商品一覧取得(<code>?category=&lt;id&gt;</code>で絞り込み可) / 新規作成</td></tr>
<tr><td><code>GET /api/products/:id</code></td><td>商品詳細取得</td></tr>
<tr><td><code>PUT /api/products/:id</code></td><td>商品更新(在庫・ステータス・所属カテゴリ変更含む)</td></tr>
<tr><td><code>DELETE /api/products/:id</code></td><td>商品削除</td></tr>
<tr><td><code>GET /api/categories</code> / <code>POST /api/categories</code></td><td>カテゴリ一覧取得(公開) / 新規作成(管理者のみ)</td></tr>
<tr><td><code>DELETE /api/categories/:id</code></td><td>カテゴリ削除(管理者のみ)</td></tr>
<tr><td><code>GET /api/favorites</code> / <code>POST /api/favorites</code></td><td>自分のお気に入り商品一覧取得 / 追加(要ログイン、<strong>カートではない</strong>)</td></tr>
<tr><td><code>DELETE /api/favorites/:product_id</code></td><td>お気に入りから削除(要ログイン)</td></tr>
<tr><td><code>POST /api/products/:id/reviews</code></td><td>レビュー投稿(要ログイン、1〜5の評価必須、投稿直後は未承認)</td></tr>
<tr><td><code>GET /api/products/:id/reviews?approved_only=true</code></td><td>承認済みレビュー一覧(公開)</td></tr>
<tr><td><code>GET /api/products/:id/reviews</code></td><td>全レビュー一覧(未承認含む、管理者のみ)</td></tr>
<tr><td><code>GET /api/products/:id/rating-summary</code></td><td>承認済みレビューのみで算出した平均評価・件数(公開)</td></tr>
<tr><td><code>POST /api/reviews/:id/approve</code></td><td>レビューを承認して公開(管理者のみ)</td></tr>
<tr><td><code>DELETE /api/reviews/:id</code></td><td>レビュー削除(管理者、または投稿者本人)</td></tr>
</table>

<div class="warn">
<strong>正直な開示: まだ実装していない機能</strong>
<ul>
<li><strong>カート・注文・決済連携(実決済は一切未実装。Stripe等のゲートウェイ呼び出し・カード情報の取り扱いは一切行っていない)</strong>。お気に入り機能・レビュー/評価機能はあるが、いずれも「保存した商品IDのリスト」「投稿されたテキストと星評価」に過ぎず、数量・合計金額・注文作成・購入確認(いわゆるverified purchase)を一切持たないため、購入手段では無い。</li>
<li>会員管理(ポイント等。お気に入りのみ実装済み)</li>
<li>配送・在庫管理(在庫数フィールドはあるが自動引き落とし等は無し)</li>
<li>プラグイン機構・管理画面(APIのみ、UIは無し)</li>
<li><code>aruaru-db</code>/PostgreSQL DUAL DB構成(現状はJSONファイル永続化のみ)</li>
</ul>
</div>

<h2>ダウンロード / インストール</h2>
<p>
  <a class="btn" href="https://github.com/aon-co-jp/RS-EC/releases/latest">最新リリースをダウンロード</a>
  <a class="btn" href="https://github.com/aon-co-jp/RS-EC">GitHubでソースを見る</a>
</p>
<p>Linux(静的リンクmuslバイナリ)・Windows向けにインストーラー付きビルド済みバイナリを配布しています。詳細は<a href="https://github.com/aon-co-jp/RS-EC#readme">README</a>参照。</p>

<footer>RS-EC v0.1.0 &mdash; <a href="https://github.com/aon-co-jp/RS-EC">aon-co-jp/RS-EC</a></footer>
</body>
</html>
"#;

#[handler]
async fn index() -> Response {
    Response::builder()
        .status(poem::http::StatusCode::OK)
        .content_type("text/html; charset=utf-8")
        .body(INDEX_HTML)
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
        .at("/", get(index))
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
        .at("/api/categories", get(list_categories).post(create_category))
        .at("/api/categories/:id", delete(delete_category))
        .at("/api/favorites", get(list_favorites).post(add_favorite))
        .at("/api/favorites/:product_id", delete(remove_favorite))
        .at("/api/products/:id/reviews", get(list_reviews).post(create_review))
        .at("/api/products/:id/rating-summary", get(rating_summary_handler))
        .at("/api/reviews/:id/approve", post(approve_review))
        .at("/api/reviews/:id", delete(delete_review))
        .at("/api/cart", get(get_cart))
        .at("/api/cart/items", post(add_cart_item))
        .at("/api/cart/items/:product_id", delete(remove_cart_item))
        .at("/api/orders/checkout", post(checkout))
        .at("/api/orders", get(list_orders))
        .at("/api/orders/:id", get(get_order))
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
    use crate::reviews::Review;
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
    async fn root_returns_landing_page_with_key_markers() {
        // UXバグ修正の検証: JSON APIオンリーで何も表示されなかった`GET /`が
        // アプリ名・実エンドポイント・決済未実装の開示・ダウンロードリンクを
        // 含むHTMLを返すこと。
        let state = make_state("root", true).await;
        let client = TestClient::new(build_routes(state));
        let resp = client.get("/").send().await;
        resp.assert_status_is_ok();
        let body = resp.0.into_body().into_string().await.unwrap();
        assert!(body.contains("RS-EC"));
        assert!(body.contains("/api/products"));
        assert!(body.contains("カート・注文・決済連携"));
        assert!(body.contains("https://github.com/aon-co-jp/RS-EC/releases/latest"));
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

    #[tokio::test]
    async fn category_filter_and_favorites_end_to_end() {
        // curlでのOTPログインはSMTP未設定環境では実施できないため
        // (HANDOFF既知の制約)、`TestClient`経由で管理者セッション/一般
        // アカウントセッションを直接発行し、タスクで要求されたシナリオ
        // (カテゴリ作成→商品作成→カテゴリ絞り込み→お気に入り追加→
        // 一覧→削除→空であることの確認)をエンドツーエンドで検証する。
        let state = make_state("category-favorites-e2e", false).await;
        let data_root = state.data_root.clone();
        let admin = admin_token(&state);
        let member_email = "member@example.com".to_string();
        let member_token = state.auth.create_session(&member_email);
        {
            let mut accounts_store = accounts::load(&data_root).await;
            accounts_store.emails.insert(member_email.clone());
            accounts::save(&data_root, &accounts_store).await.unwrap();
            let mut access_config = access::load(&data_root).await;
            access_config.accounts.insert(member_email.clone(), access::AccountPermission { allow_view: true, allow_edit: false });
            access::save(&data_root, &access_config).await.unwrap();
        }
        let app = build_routes(state);
        let client = TestClient::new(app);

        // 1. カテゴリ作成(管理者のみ)
        let resp = client
            .post("/api/categories")
            .header("Authorization", format!("Bearer {admin}"))
            .body_json(&serde_json::json!({ "name": "Books", "slug": "books" }))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::CREATED);
        let category: categories::Category = resp.json().await.value().deserialize();

        // 2. そのカテゴリ付きで商品作成
        let resp = client
            .post("/api/products")
            .header("Authorization", format!("Bearer {admin}"))
            .body_json(&serde_json::json!({ "name": "Rust本", "price_cents": 3000, "stock": 5, "categories": [category.id] }))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::CREATED);
        let product: Product = resp.json().await.value().deserialize();
        assert_eq!(product.categories, vec![category.id]);

        // 別カテゴリなしの商品も1件作成(フィルタで除外されることの確認用)
        client
            .post("/api/products")
            .header("Authorization", format!("Bearer {admin}"))
            .body_json(&serde_json::json!({ "name": "無関係商品", "price_cents": 100, "stock": 1 }))
            .send()
            .await
            .assert_status(poem::http::StatusCode::CREATED);

        // 3. カテゴリで絞り込み(一般アカウント、view権限のみ)
        let resp = client
            .get(format!("/api/products?category={}", category.id))
            .header("Authorization", format!("Bearer {member_token}"))
            .send()
            .await;
        resp.assert_status_is_ok();
        let filtered: Vec<Product> = resp.json().await.value().deserialize();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, product.id);

        // 4. 一般アカウントとしてお気に入りに追加
        let resp = client
            .post("/api/favorites")
            .header("Authorization", format!("Bearer {member_token}"))
            .body_json(&serde_json::json!({ "product_id": product.id }))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::CREATED);

        // 5. お気に入り一覧に出てくること(自分のものだけ)
        let resp = client.get("/api/favorites").header("Authorization", format!("Bearer {member_token}")).send().await;
        resp.assert_status_is_ok();
        let favorites_list: Vec<Product> = resp.json().await.value().deserialize();
        assert_eq!(favorites_list.len(), 1);
        assert_eq!(favorites_list[0].id, product.id);

        // 6. お気に入りから削除
        let resp = client
            .delete(format!("/api/favorites/{}", product.id))
            .header("Authorization", format!("Bearer {member_token}"))
            .send()
            .await;
        resp.assert_status_is_ok();

        // 7. 一覧が空になっていること
        let resp = client.get("/api/favorites").header("Authorization", format!("Bearer {member_token}")).send().await;
        resp.assert_status_is_ok();
        let empty: Vec<Product> = resp.json().await.value().deserialize();
        assert!(empty.is_empty());

        // カテゴリ削除(管理者のみ)の確認もついでに行う
        let resp = client.delete(format!("/api/categories/{}", category.id)).header("Authorization", format!("Bearer {admin}")).send().await;
        resp.assert_status_is_ok();
    }

    /// テスト用に商品を1件作成し、そのIDを返す小ヘルパー(レビュー系
    /// テストで繰り返し使う)。
    async fn seed_product(client: &TestClient<impl poem::Endpoint>, admin_token: &str) -> u64 {
        let resp = client
            .post("/api/products")
            .header("Authorization", format!("Bearer {admin_token}"))
            .body_json(&serde_json::json!({ "name": "Gadget", "description": "d", "price_cents": 500, "stock": 3 }))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::CREATED);
        let product: Product = resp.json().await.value().deserialize();
        product.id
    }

    #[tokio::test]
    async fn review_creation_requires_authentication() {
        let state = make_state("review-auth-required", false).await;
        let admin = admin_token(&state);
        let app = build_routes(state);
        let client = TestClient::new(app);
        let product_id = seed_product(&client, &admin).await;

        let resp = client
            .post(format!("/api/products/{product_id}/reviews"))
            .body_json(&serde_json::json!({ "rating": 5, "title": "Great", "body": "Loved it" }))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn review_creation_rejects_invalid_rating() {
        let state = make_state("review-invalid-rating", false).await;
        let admin = admin_token(&state);
        let app = build_routes(state);
        let client = TestClient::new(app);
        let product_id = seed_product(&client, &admin).await;

        for bad_rating in [0, 6] {
            let resp = client
                .post(format!("/api/products/{product_id}/reviews"))
                .header("Authorization", format!("Bearer {admin}"))
                .body_json(&serde_json::json!({ "rating": bad_rating, "title": "x", "body": "y" }))
                .send()
                .await;
            resp.assert_status(poem::http::StatusCode::BAD_REQUEST);
        }
    }

    #[tokio::test]
    async fn unapproved_reviews_are_hidden_from_public_listing_and_summary_until_approved() {
        let state = make_state("review-moderation", false).await;
        let admin = admin_token(&state);
        let member_email = "member@example.com".to_string();
        let member_token = state.auth.create_session(&member_email);
        let app = build_routes(state);
        let client = TestClient::new(app);
        let product_id = seed_product(&client, &admin).await;

        // 会員がレビューを投稿(投稿直後は未承認)
        let resp = client
            .post(format!("/api/products/{product_id}/reviews"))
            .header("Authorization", format!("Bearer {member_token}"))
            .body_json(&serde_json::json!({ "rating": 4, "title": "Nice", "body": "Works well" }))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::CREATED);
        let created: Review = resp.json().await.value().deserialize();
        assert!(!created.approved);

        // 公開一覧(approved_only=true)にはまだ出ない
        let resp = client.get(format!("/api/products/{product_id}/reviews?approved_only=true")).send().await;
        resp.assert_status_is_ok();
        let public_list: Vec<Review> = resp.json().await.value().deserialize();
        assert!(public_list.is_empty());

        // rating-summaryも未承認のみでは0件・平均0
        let resp = client.get(format!("/api/products/{product_id}/rating-summary")).send().await;
        resp.assert_status_is_ok();
        let summary: serde_json::Value = resp.json().await.value().deserialize();
        assert_eq!(summary["review_count"], 0);

        // 一般公開されていない全件一覧は管理者のみ(会員は403、未ログインは401)
        let resp = client.get(format!("/api/products/{product_id}/reviews")).send().await;
        resp.assert_status(poem::http::StatusCode::UNAUTHORIZED);
        let resp = client
            .get(format!("/api/products/{product_id}/reviews"))
            .header("Authorization", format!("Bearer {member_token}"))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::FORBIDDEN);

        // 管理者が承認
        let resp = client
            .post(format!("/api/reviews/{}/approve", created.id))
            .header("Authorization", format!("Bearer {admin}"))
            .send()
            .await;
        resp.assert_status_is_ok();
        let approved: Review = resp.json().await.value().deserialize();
        assert!(approved.approved);

        // 今度は公開一覧・rating-summaryに反映される
        let resp = client.get(format!("/api/products/{product_id}/reviews?approved_only=true")).send().await;
        resp.assert_status_is_ok();
        let public_list: Vec<Review> = resp.json().await.value().deserialize();
        assert_eq!(public_list.len(), 1);

        let resp = client.get(format!("/api/products/{product_id}/rating-summary")).send().await;
        resp.assert_status_is_ok();
        let summary: serde_json::Value = resp.json().await.value().deserialize();
        assert_eq!(summary["review_count"], 1);
        assert_eq!(summary["average_rating"], 4.0);
    }

    #[tokio::test]
    async fn rating_summary_math_is_correct_for_a_known_set_of_approved_reviews() {
        let state = make_state("rating-summary-math", false).await;
        let admin = admin_token(&state);
        let member_token = state.auth.create_session(&"member2@example.com".to_string());
        let app = build_routes(state);
        let client = TestClient::new(app);
        let product_id = seed_product(&client, &admin).await;

        // 承認済み2件(rating 5, 3) + 未承認1件(rating 1) => 平均は(5+3)/2=4.0、件数2
        for rating in [5u8, 3u8, 1u8] {
            let resp = client
                .post(format!("/api/products/{product_id}/reviews"))
                .header("Authorization", format!("Bearer {member_token}"))
                .body_json(&serde_json::json!({ "rating": rating, "title": "t", "body": "b" }))
                .send()
                .await;
            resp.assert_status(poem::http::StatusCode::CREATED);
            let created: Review = resp.json().await.value().deserialize();
            if rating != 1 {
                let resp = client
                    .post(format!("/api/reviews/{}/approve", created.id))
                    .header("Authorization", format!("Bearer {admin}"))
                    .send()
                    .await;
                resp.assert_status_is_ok();
            }
        }

        let resp = client.get(format!("/api/products/{product_id}/rating-summary")).send().await;
        resp.assert_status_is_ok();
        let summary: serde_json::Value = resp.json().await.value().deserialize();
        assert_eq!(summary["review_count"], 2);
        assert_eq!(summary["average_rating"], 4.0);
    }

    #[tokio::test]
    async fn review_delete_allowed_for_admin_and_author_but_not_other_members() {
        let state = make_state("review-delete-permissions", false).await;
        let admin = admin_token(&state);
        let author_token = state.auth.create_session(&"author@example.com".to_string());
        let other_token = state.auth.create_session(&"other@example.com".to_string());
        let app = build_routes(state);
        let client = TestClient::new(app);
        let product_id = seed_product(&client, &admin).await;

        let resp = client
            .post(format!("/api/products/{product_id}/reviews"))
            .header("Authorization", format!("Bearer {author_token}"))
            .body_json(&serde_json::json!({ "rating": 2, "title": "meh", "body": "b" }))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::CREATED);
        let review: Review = resp.json().await.value().deserialize();

        // 他人は削除不可
        let resp = client
            .delete(format!("/api/reviews/{}", review.id))
            .header("Authorization", format!("Bearer {other_token}"))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::FORBIDDEN);

        // 投稿者本人は削除可能
        let resp = client
            .delete(format!("/api/reviews/{}", review.id))
            .header("Authorization", format!("Bearer {author_token}"))
            .send()
            .await;
        resp.assert_status_is_ok();
    }

    #[tokio::test]
    async fn cart_add_checkout_decrements_stock_and_creates_paid_order() {
        // 商品登録→カート追加→注文確定(モック決済)→在庫減算・カート空化・
        // 本人のみ注文閲覧可、を一気通貫で検証する(タスク要件のE2Eシナリオ)。
        let state = make_state("cart-checkout", false).await;
        let admin = admin_token(&state);
        let member_token = state.auth.create_session("member@example.com");
        let app = build_routes(state);
        let client = TestClient::new(app);

        let resp = client
            .post("/api/products")
            .header("Authorization", format!("Bearer {admin}"))
            .body_json(&serde_json::json!({ "name": "Gadget", "price_cents": 500, "stock": 3, "status": "on_sale" }))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::CREATED);
        let product: Product = resp.json().await.value().deserialize();

        // カート未ログインは401
        let resp = client.get("/api/cart").send().await;
        resp.assert_status(poem::http::StatusCode::UNAUTHORIZED);

        // カートに2個追加
        let resp = client
            .post("/api/cart/items")
            .header("Authorization", format!("Bearer {member_token}"))
            .body_json(&serde_json::json!({ "product_id": product.id, "quantity": 2 }))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::CREATED);
        let cart_view: serde_json::Value = resp.json().await.value().deserialize();
        assert_eq!(cart_view["total_cents"], 1000);

        // 在庫を超える注文は400
        let resp = client
            .post("/api/cart/items")
            .header("Authorization", format!("Bearer {member_token}"))
            .body_json(&serde_json::json!({ "product_id": product.id, "quantity": 5 }))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::CREATED); // カート追加自体は在庫チェックしない
        let resp = client
            .post("/api/orders/checkout")
            .header("Authorization", format!("Bearer {member_token}"))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::BAD_REQUEST); // 合計7個、在庫3個のため不足

        // カートを在庫内(2個)に戻してから確定
        let resp = client
            .delete(format!("/api/cart/items/{}", product.id))
            .header("Authorization", format!("Bearer {member_token}"))
            .send()
            .await;
        resp.assert_status_is_ok();
        client
            .post("/api/cart/items")
            .header("Authorization", format!("Bearer {member_token}"))
            .body_json(&serde_json::json!({ "product_id": product.id, "quantity": 2 }))
            .send()
            .await
            .assert_status(poem::http::StatusCode::CREATED);

        let resp = client
            .post("/api/orders/checkout")
            .header("Authorization", format!("Bearer {member_token}"))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::CREATED);
        let order: order::Order = resp.json().await.value().deserialize();
        assert_eq!(order.total_cents, 1000);
        assert!(matches!(order.status, order::OrderStatus::Paid));
        assert!(order.payment_reference.starts_with("MOCK-"));

        // カートは空になっている
        let resp = client.get("/api/cart").header("Authorization", format!("Bearer {member_token}")).send().await;
        let cart_view: serde_json::Value = resp.json().await.value().deserialize();
        assert_eq!(cart_view["items"].as_array().unwrap().len(), 0);

        // 在庫が2個減って1個になっている
        let resp = client.get(format!("/api/products/{}", product.id)).header("Authorization", format!("Bearer {admin}")).send().await;
        let updated_product: Product = resp.json().await.value().deserialize();
        assert_eq!(updated_product.stock, 1);

        // 本人は注文を閲覧できる
        let resp = client
            .get(format!("/api/orders/{}", order.id))
            .header("Authorization", format!("Bearer {member_token}"))
            .send()
            .await;
        resp.assert_status_is_ok();
    }

    #[tokio::test]
    async fn other_member_cannot_view_someone_elses_order() {
        let state = make_state("order-privacy", false).await;
        let admin = admin_token(&state);
        let owner_token = state.auth.create_session("owner@example.com");
        let other_token = state.auth.create_session("other@example.com");
        let app = build_routes(state);
        let client = TestClient::new(app);

        let resp = client
            .post("/api/products")
            .header("Authorization", format!("Bearer {admin}"))
            .body_json(&serde_json::json!({ "name": "Thing", "price_cents": 200, "stock": 5, "status": "on_sale" }))
            .send()
            .await;
        let product: Product = resp.json().await.value().deserialize();

        client
            .post("/api/cart/items")
            .header("Authorization", format!("Bearer {owner_token}"))
            .body_json(&serde_json::json!({ "product_id": product.id, "quantity": 1 }))
            .send()
            .await
            .assert_status(poem::http::StatusCode::CREATED);

        let resp = client.post("/api/orders/checkout").header("Authorization", format!("Bearer {owner_token}")).send().await;
        resp.assert_status(poem::http::StatusCode::CREATED);
        let order: order::Order = resp.json().await.value().deserialize();

        let resp = client.get(format!("/api/orders/{}", order.id)).header("Authorization", format!("Bearer {other_token}")).send().await;
        resp.assert_status(poem::http::StatusCode::FORBIDDEN);

        let resp = client.get(format!("/api/orders/{}", order.id)).header("Authorization", format!("Bearer {admin}")).send().await;
        resp.assert_status_is_ok();
    }
}
