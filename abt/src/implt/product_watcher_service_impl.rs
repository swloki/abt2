//! 产品关注服务实现

use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::sync::Arc;

use crate::models::WatchedProductWithInventory;
use crate::repositories::ProductWatcherRepo;
use crate::service::ProductWatcherService;

pub struct ProductWatcherServiceImpl {
    pool: Arc<PgPool>,
}

impl ProductWatcherServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProductWatcherService for ProductWatcherServiceImpl {
    async fn watch_product(
        &self,
        user_id: i64,
        product_id: i64,
        safety_stock_override: Option<Decimal>,
    ) -> Result<bool> {
        ProductWatcherRepo::upsert(&self.pool, user_id, product_id, safety_stock_override).await
    }

    async fn unwatch_product(&self, user_id: i64, product_id: i64) -> Result<bool> {
        ProductWatcherRepo::delete(&self.pool, user_id, product_id).await
    }

    async fn list_watched_products(
        &self,
        user_id: i64,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<WatchedProductWithInventory>, i64)> {
        ProductWatcherRepo::find_by_user_with_inventory(&self.pool, user_id, page, page_size).await
    }
}
