//! 产品关注服务接口

use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::models::WatchedProductWithInventory;

#[async_trait]
pub trait ProductWatcherService: Send + Sync {
    async fn watch_product(
        &self,
        user_id: i64,
        product_id: i64,
        safety_stock_override: Option<Decimal>,
    ) -> Result<bool>;

    async fn unwatch_product(&self, user_id: i64, product_id: i64) -> Result<bool>;

    async fn list_watched_products(
        &self,
        user_id: i64,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<WatchedProductWithInventory>, i64)>;
}
