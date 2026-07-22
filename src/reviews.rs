//! 商品レビュー・評価(`Review`)。`RS-Blog`のコメント承認制パターン
//! (`RS-Blog/src/main.rs`)を踏襲し、投稿直後は`approved: false`で
//! 非公開、管理者が承認するまで一般公開の一覧・平均評価には出さない
//! (未モデレートのUGCはスパム・荒らしの実害があるため)。
//! 永続化は`ProductStore`/`CategoryStore`と同じJSONファイル1枚方式。
//! **決済・注文とは無関係**(購入確認なしで誰でも投稿できる、いわゆる
//! 「verified purchase」相当の仕組みは無い、正直な開示)。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub id: u64,
    pub product_id: u64,
    pub author_email: String,
    pub rating: u8,
    pub title: String,
    pub body: String,
    pub created_at: u64,
    /// 投稿直後は`false`(管理者承認待ち)。`POST /api/reviews/:id/approve`
    /// でのみ`true`になる。
    pub approved: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ReviewStore {
    pub next_id: u64,
    pub reviews: Vec<Review>,
}

fn reviews_path(data_root: &Path) -> PathBuf {
    data_root.join("reviews.json")
}

pub async fn load(data_root: &Path) -> ReviewStore {
    match tokio::fs::read(reviews_path(data_root)).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => ReviewStore::default(),
    }
}

pub async fn save(data_root: &Path, store: &ReviewStore) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(store).expect("ReviewStore serialization is infallible");
    tokio::fs::write(reviews_path(data_root), bytes).await
}

/// 承認済みレビューのみから`{average_rating, review_count}`を計算する。
/// 未承認レビュー(スパム・不適切な投稿の可能性)が承認前に平均を
/// 歪めないよう、承認済みのみを対象にする(タスク要件)。
pub fn rating_summary(reviews: &[&Review]) -> (f32, u32) {
    let approved: Vec<&&Review> = reviews.iter().filter(|r| r.approved).collect();
    let count = approved.len() as u32;
    if count == 0 {
        return (0.0, 0);
    }
    let sum: u32 = approved.iter().map(|r| r.rating as u32).sum();
    (sum as f32 / count as f32, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn review(id: u64, rating: u8, approved: bool) -> Review {
        Review { id, product_id: 1, author_email: "a@example.com".to_string(), rating, title: "t".to_string(), body: "b".to_string(), created_at: 0, approved }
    }

    #[tokio::test]
    async fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join(format!("rsec-reviews-test-{}", crate::accounts::generate_request_id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let mut store = ReviewStore::default();
        let id = store.next_id;
        store.next_id += 1;
        store.reviews.push(review(id, 5, false));
        save(&dir, &store).await.unwrap();

        let loaded = load(&dir).await;
        assert_eq!(loaded.reviews.len(), 1);
        assert_eq!(loaded.reviews[0].rating, 5);
    }

    #[test]
    fn rating_summary_excludes_unapproved() {
        let r1 = review(1, 5, true);
        let r2 = review(2, 3, true);
        let r3 = review(3, 1, false); // unapproved spam, must not count
        let refs: Vec<&Review> = vec![&r1, &r2, &r3];
        let (avg, count) = rating_summary(&refs);
        assert_eq!(count, 2);
        assert!((avg - 4.0).abs() < f32::EPSILON);
    }

    #[test]
    fn rating_summary_with_no_approved_reviews_is_zero() {
        let r1 = review(1, 5, false);
        let refs: Vec<&Review> = vec![&r1];
        let (avg, count) = rating_summary(&refs);
        assert_eq!(count, 0);
        assert_eq!(avg, 0.0);
    }
}
