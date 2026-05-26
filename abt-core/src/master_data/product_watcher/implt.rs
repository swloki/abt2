use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::shared::types::{PaginatedResult, ServiceContext, Result};

use super::model::WatchedProductWithInventory;
use super::repo::ProductWatcherRepo;
use super::service::ProductWatcherService;

pub struct ProductWatcherServiceImpl;

impl ProductWatcherServiceImpl {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProductWatcherService for ProductWatcherServiceImpl {
    async fn watch_product(
        &self,
        ctx: ServiceContext<'_>,
        product_id: i64,
        safety_stock_override: Option<Decimal>,
    ) -> Result<bool> {
        ProductWatcherRepo::upsert(ctx.executor, ctx.operator_id, product_id, safety_stock_override)
            .await
            
    }

    async fn unwatch_product(
        &self,
        ctx: ServiceContext<'_>,
        product_id: i64,
    ) -> Result<bool> {
        ProductWatcherRepo::delete(ctx.executor, ctx.operator_id, product_id)
            .await
            
    }

    async fn list_watched_products(
        &self,
        ctx: ServiceContext<'_>,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<WatchedProductWithInventory>> {
        ProductWatcherRepo::find_by_user_with_inventory(ctx.executor, ctx.operator_id, page, page_size)
            .await
            
    }
}
