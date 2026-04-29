//! 劳务工序服务实现

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;

use crate::models::*;
use crate::repositories::{Executor, LaborProcessRepo};
use crate::service::LaborProcessService;

pub struct LaborProcessServiceImpl {
    pool: PgPool,
}

impl LaborProcessServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LaborProcessService for LaborProcessServiceImpl {
    // ========================================================================
    // 查询
    // ========================================================================

    async fn list(&self, query: ListLaborProcessQuery) -> Result<(Vec<BomLaborProcess>, i64)> {
        let page = query.page.max(1);
        let page_size = query.page_size.clamp(1, 100);
        let kw = query.keyword.as_deref();
        let items = LaborProcessRepo::find_by_product_code(
            &self.pool, &query.product_code, kw, page, page_size,
        )
        .await?;
        let total = LaborProcessRepo::count_by_product_code(
            &self.pool, &query.product_code, kw,
        )
        .await?;
        Ok((items, total))
    }

    // ========================================================================
    // 写入
    // ========================================================================

    async fn create(&self, req: CreateLaborProcessReq, executor: Executor<'_>) -> Result<i64> {
        LaborProcessRepo::insert(
            executor,
            &req.product_code,
            req.process_code.as_deref(),
            &req.name,
            req.unit_price,
            req.quantity,
            req.sort_order,
            req.remark.as_deref(),
        )
        .await
    }

    async fn update(&self, req: UpdateLaborProcessReq, executor: Executor<'_>) -> Result<()> {
        LaborProcessRepo::update(
            executor,
            req.id,
            &req.product_code,
            req.process_code.as_deref(),
            &req.name,
            req.unit_price,
            req.quantity,
            req.sort_order,
            req.remark.as_deref(),
        )
        .await
    }

    async fn delete(&self, id: i64, product_code: &str, executor: Executor<'_>) -> Result<u64> {
        LaborProcessRepo::delete(executor, id, product_code).await
    }

}
