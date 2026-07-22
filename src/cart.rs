//! ショッピングカート。ログイン済みアカウント(メールアドレス)ごとに
//! 「商品ID→数量」を保持する。`favorites.rs`と同じJSONファイル永続化
//! パターン。カート自体は金額計算・注文確定を持たず、それらは
//! `order.rs`の`POST /api/orders/checkout`が担う(正直な開示: カートは
//! あくまで「確定前の一時的な選択リスト」)。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartItem {
    pub product_id: u64,
    pub quantity: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CartStore {
    /// account email -> items (product_idごとに1エントリ、数量は加算更新)
    pub by_email: HashMap<String, Vec<CartItem>>,
}

fn cart_path(data_root: &Path) -> PathBuf {
    data_root.join("cart.json")
}

pub async fn load(data_root: &Path) -> CartStore {
    match tokio::fs::read(cart_path(data_root)).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => CartStore::default(),
    }
}

pub async fn save(data_root: &Path, store: &CartStore) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(store).expect("CartStore serialization is infallible");
    tokio::fs::write(cart_path(data_root), bytes).await
}

/// カートに商品を追加する。既に同じ`product_id`があれば数量を加算する。
/// `quantity`が0以下ならエントリを削除する(数量調整のUX、負の数は
/// 呼び出し側でバリデーション済みである前提)。
pub fn upsert_item(items: &mut Vec<CartItem>, product_id: u64, quantity: u64) {
    if let Some(item) = items.iter_mut().find(|i| i.product_id == product_id) {
        item.quantity += quantity;
    } else if quantity > 0 {
        items.push(CartItem { product_id, quantity });
    }
}

pub fn remove_item(items: &mut Vec<CartItem>, product_id: u64) {
    items.retain(|i| i.product_id != product_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_adds_then_accumulates() {
        let mut items = Vec::new();
        upsert_item(&mut items, 1, 2);
        upsert_item(&mut items, 1, 3);
        upsert_item(&mut items, 2, 1);
        assert_eq!(items.iter().find(|i| i.product_id == 1).unwrap().quantity, 5);
        assert_eq!(items.iter().find(|i| i.product_id == 2).unwrap().quantity, 1);
    }

    #[test]
    fn remove_item_drops_entry() {
        let mut items = vec![CartItem { product_id: 1, quantity: 2 }];
        remove_item(&mut items, 1);
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join(format!("rsec-cart-test-{}", crate::accounts::generate_request_id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let mut store = CartStore::default();
        store.by_email.entry("user@example.com".to_string()).or_default().push(CartItem { product_id: 5, quantity: 3 });
        save(&dir, &store).await.unwrap();

        let loaded = load(&dir).await;
        let items = loaded.by_email.get("user@example.com").unwrap();
        assert_eq!(items[0].product_id, 5);
        assert_eq!(items[0].quantity, 3);
    }
}
