//! カテゴリ管理(`Category { id, name, slug }`)。管理者のみがCRUD可能な
//! シンプルなJSONファイル永続化ストア(`ProductStore`と同じパターン)。
//! `Product`側は`categories: Vec<u64>`でこのIDを参照する(多対多)。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: u64,
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CategoryStore {
    pub next_id: u64,
    pub categories: Vec<Category>,
}

fn categories_path(data_root: &Path) -> PathBuf {
    data_root.join("categories.json")
}

pub async fn load(data_root: &Path) -> CategoryStore {
    match tokio::fs::read(categories_path(data_root)).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => CategoryStore::default(),
    }
}

pub async fn save(data_root: &Path, store: &CategoryStore) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(store).expect("CategoryStore serialization is infallible");
    tokio::fs::write(categories_path(data_root), bytes).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join(format!("rsec-categories-test-{}", crate::accounts::generate_request_id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let mut store = CategoryStore::default();
        let id = store.next_id;
        store.next_id += 1;
        store.categories.push(Category { id, name: "Books".to_string(), slug: "books".to_string() });
        save(&dir, &store).await.unwrap();

        let loaded = load(&dir).await;
        assert_eq!(loaded.categories.len(), 1);
        assert_eq!(loaded.categories[0].name, "Books");
    }
}
