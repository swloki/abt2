use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::shared::types::{DomainError, PaginatedResult, ServiceContext};

use super::model::WatchedProductWithInventory;

#[async_trait]
pub trait ProductWatcherService: Send + Sync {
    async fn watch_product(
        &self,
        ctx: ServiceContext<'_>,
        product_id: i64,
        safety_stock_override: Option<Decimal>,
    ) -> Result<bool>;

    async fn unwatch_product(
        &self,
        ctx: ServiceContext<'_>,
        product_id: i64,
    ) -> Result<bool>;

    async fn list_watched_products(
        &self,
        ctx: ServiceContext<'_>,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<WatchedProductWithInventory>>;
}
