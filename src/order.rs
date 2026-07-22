//! 注文・決済。**実際の決済ゲートウェイ連携は無い**(CLAUDE.md/`main.rs`の
//! 正直な開示を参照)。`process_mock_payment`は金銭のやり取りを一切行わない
//! ダミー実装で、常に成功しフェイクの決済参照IDを返す。実際のカード情報・
//! 決済トークンは一切扱わない(受け取ってもいない)。
//!
//! 注文はカート(`cart.rs`)の内容を確定した時点のスナップショット
//! (商品名・単価・数量)を保持する。以後カタログ側の価格変更・商品削除の
//! 影響を受けない(EC-CUBE等一般的なECサイトと同じ「注文確定時点の価格を
//! 固定する」設計)。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    /// 決済処理中(現実装では一瞬で`Paid`か`PaymentFailed`に遷移するため
    /// 永続化されて見えることは基本無いが、将来の非同期決済ゲートウェイ
    /// 連携のために状態として用意しておく。
    Pending,
    Paid,
    PaymentFailed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderItem {
    pub product_id: u64,
    pub name: String,
    pub unit_price_cents: u64,
    pub quantity: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: u64,
    pub email: String,
    pub items: Vec<OrderItem>,
    pub total_cents: u64,
    pub status: OrderStatus,
    /// モック決済ゲートウェイが返したダミーの参照ID(実際の決済ではない)。
    pub payment_reference: String,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct OrderStore {
    pub next_id: u64,
    pub orders: Vec<Order>,
}

fn orders_path(data_root: &Path) -> PathBuf {
    data_root.join("orders.json")
}

pub async fn load(data_root: &Path) -> OrderStore {
    match tokio::fs::read(orders_path(data_root)).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => OrderStore::default(),
    }
}

pub async fn save(data_root: &Path, store: &OrderStore) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(store).expect("OrderStore serialization is infallible");
    tokio::fs::write(orders_path(data_root), bytes).await
}

/// モック決済ゲートウェイ。**実際の金銭のやり取りは一切行わない。**
/// カード情報・決済トークンなどは引数に取らない(そもそも扱わない設計)。
/// 金額が0円の注文は決済処理として不成立とみなし失敗させる以外は常に
/// 成功する(実ゲートウェイでの与信否認等はここには実装しない)。
pub fn process_mock_payment(total_cents: u64) -> (bool, String) {
    if total_cents == 0 {
        return (false, "mock-payment-rejected-zero-amount".to_string());
    }
    let reference = format!("MOCK-{:016x}", rand::random::<u64>());
    (true, reference)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_payment_succeeds_for_nonzero_amount() {
        let (ok, reference) = process_mock_payment(1000);
        assert!(ok);
        assert!(reference.starts_with("MOCK-"));
    }

    #[test]
    fn mock_payment_rejects_zero_amount() {
        let (ok, _) = process_mock_payment(0);
        assert!(!ok);
    }

    #[tokio::test]
    async fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join(format!("rsec-order-test-{}", crate::accounts::generate_request_id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let mut store = OrderStore::default();
        store.orders.push(Order {
            id: 1,
            email: "user@example.com".to_string(),
            items: vec![OrderItem { product_id: 1, name: "widget".to_string(), unit_price_cents: 500, quantity: 2 }],
            total_cents: 1000,
            status: OrderStatus::Paid,
            payment_reference: "MOCK-test".to_string(),
            created_at: 0,
            updated_at: 0,
        });
        store.next_id = 2;
        save(&dir, &store).await.unwrap();

        let loaded = load(&dir).await;
        assert_eq!(loaded.orders.len(), 1);
        assert_eq!(loaded.orders[0].total_cents, 1000);
    }
}
