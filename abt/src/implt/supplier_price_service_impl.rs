//! 供应商价格服务实现
//!
//! 实现供应商价格管理的业务逻辑。

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::sync::Arc;

use crate::models::{SupplierPriceDetail, SupplierPriceQuery};
use crate::repositories::{
    Executor, PaginatedResult, PaginationParams, SupplierPriceRepo,
};
use crate::service::SupplierPriceService;

/// 供应商价格服务实现
pub struct SupplierPriceServiceImpl {
    pool: Arc<PgPool>,
}

impl SupplierPriceServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SupplierPriceService for SupplierPriceServiceImpl {
    async fn upsert(
        &self,
        supplier_id: i64,
        product_id: i64,
        unit_price: Decimal,
        valid_from: DateTime<Utc>,
        valid_until: DateTime<Utc>,
        operator_id: Option<i64>,
        executor: Executor<'_>,
    ) -> Result<i64> {
        let price_id = SupplierPriceRepo::insert(
            executor,
            supplier_id,
            product_id,
            unit_price,
            valid_from,
            valid_until,
            operator_id,
        )
        .await?;

        Ok(price_id)
    }

    async fn list(
        &self,
        query: SupplierPriceQuery,
    ) -> Result<PaginatedResult<SupplierPriceDetail>> {
        let page = query.page.unwrap_or(1).max(1) as u32;
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100) as u32;

        let items = SupplierPriceRepo::query(&self.pool, &query).await?;
        let total = SupplierPriceRepo::query_count(&self.pool, &query).await?;

        let pagination = PaginationParams::new(page, page_size);
        Ok(PaginatedResult::new(items, total as u64, &pagination))
    }
}
