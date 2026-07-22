//! お気に入り(wishlist)。**カート・注文・決済ではない**——数量・合計金額・
//! 注文作成・決済処理は一切持たない、ログイン済みアカウントごとの
//! 「保存した商品IDリスト」のみ(正直な開示、`CLAUDE.md`/`main.rs`参照)。
//! JSONファイル永続化(`ProductStore`と同じパターン)。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FavoriteStore {
    /// account email -> favorited product ids
    pub by_email: HashMap<String, HashSet<u64>>,
}

fn favorites_path(data_root: &Path) -> PathBuf {
    data_root.join("favorites.json")
}

pub async fn load(data_root: &Path) -> FavoriteStore {
    match tokio::fs::read(favorites_path(data_root)).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => FavoriteStore::default(),
    }
}

pub async fn save(data_root: &Path, store: &FavoriteStore) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(store).expect("FavoriteStore serialization is infallible");
    tokio::fs::write(favorites_path(data_root), bytes).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join(format!("rsec-favorites-test-{}", crate::accounts::generate_request_id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let mut store = FavoriteStore::default();
        store.by_email.entry("user@example.com".to_string()).or_default().insert(7);
        save(&dir, &store).await.unwrap();

        let loaded = load(&dir).await;
        assert!(loaded.by_email.get("user@example.com").unwrap().contains(&7));
    }
}
