//! 产品价格服务实现
//!
//! 使用 product_price 表（最新行即当前价格）。

use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::sync::Arc;

use crate::repositories::{Executor, PaginatedResult, ProductPriceRepo};
use crate::service::{
    AllPriceHistoryQuery, PriceHistoryQuery, PriceLogEntry, PriceLogWithProduct,
    ProductPriceService,
};

/// 产品价格服务实现
pub struct ProductPriceServiceImpl {
    pool: Arc<PgPool>,
}

impl ProductPriceServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProductPriceService for ProductPriceServiceImpl {
    async fn update_price(
        &self,
        product_id: i64,
        new_price: Decimal,
        operator_id: Option<i64>,
        remark: Option<&str>,
        executor: Executor<'_>,
    ) -> Result<()> {
        // 条件 INSERT：价格相同则不插入，单次 DB 往返
        ProductPriceRepo::update_price(executor, product_id, new_price, operator_id, remark)
            .await?;

        Ok(())
    }

    async fn get_price_history(
        &self,
        query: PriceHistoryQuery,
        _pool: &PgPool,
    ) -> Result<PaginatedResult<PriceLogEntry>> {
        let page = query.page.unwrap_or(1).max(1) as u32;
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100) as u32;

        let result =
            ProductPriceRepo::list_price_history(&self.pool, query.product_id, page, page_size)
                .await?;

        Ok(result)
    }

    async fn list_all_price_history(
        &self,
        query: AllPriceHistoryQuery,
        _pool: &PgPool,
    ) -> Result<PaginatedResult<PriceLogWithProduct>> {
        let result = ProductPriceRepo::list_all_price_history(&self.pool, query).await?;
        Ok(result)
    }
}
