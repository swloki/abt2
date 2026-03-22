//! 产品价格服务实现
//!
//! 实现价格管理和历史记录的业务逻辑。

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
        // 1. 获取当前价格
        let current_price = ProductPriceRepo::get_price(executor, product_id).await?;

        // 2. 如果价格相同，不更新
        if let Some(current) = current_price
            && current == new_price
        {
            return Ok(());
        }

        // 3. 更新产品价格
        ProductPriceRepo::update_price(executor, product_id, new_price).await?;

        // 4. 记录价格历史
        ProductPriceRepo::insert_price_log(
            executor,
            product_id,
            current_price,
            new_price,
            operator_id,
            remark,
        )
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
