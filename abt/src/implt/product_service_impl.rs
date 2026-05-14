//! 产品服务实现
//!
//! 实现产品管理的业务逻辑。

use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use sqlx::PgPool;

use common::error::ServiceError;
use crate::models::{Product, ProductQuery};
use crate::repositories::{BomRepo, BomReference, Executor, ProductRepo};
use crate::service::ProductService;

/// 产品服务实现
pub struct ProductServiceImpl {
    pool: Arc<PgPool>,
}

impl ProductServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProductService for ProductServiceImpl {
    async fn create(&self, product: Product, executor: Executor<'_>) -> Result<i64> {
        let product_id = ProductRepo::insert(&mut *executor, &product.pdt_name, &product.product_code, &product.unit, product.meta)
            .await
            .map_err(|e| map_duplicate_error(e, &product.product_code))?;
        ProductRepo::upsert_term_relation(executor, product_id, product.term_id).await?;
        Ok(product_id)
    }

    async fn update(
        &self,
        product_id: i64,
        product: Product,
        executor: Executor<'_>,
    ) -> Result<()> {
        ProductRepo::update(&mut *executor, product_id, &product.pdt_name, &product.product_code, &product.unit, product.meta)
            .await
            .map_err(|e| map_duplicate_error(e, &product.product_code))?;
        ProductRepo::upsert_term_relation(executor, product_id, product.term_id).await?;
        Ok(())
    }

    async fn delete(&self, product_id: i64, executor: Executor<'_>) -> Result<()> {
        ProductRepo::delete(executor, product_id).await
    }

    async fn find(&self, product_id: i64) -> Result<Option<Product>> {
        ProductRepo::find_by_id(&self.pool, product_id).await
    }

    async fn find_by_ids(&self, product_ids: &[i64]) -> Result<Vec<Product>> {
        ProductRepo::find_by_ids(&self.pool, product_ids).await
    }

    async fn query(&self, query: ProductQuery) -> Result<(Vec<Product>, i64)> {
        let items = ProductRepo::query(&self.pool, &query).await?;
        let total = ProductRepo::query_count(&self.pool, &query).await?;
        Ok((items, total))
    }

    async fn exist_code(&self, pool: &PgPool, code: &str) -> Result<bool> {
        ProductRepo::exist_product_code(pool, code).await
    }

    async fn generate_product_code(&self, pool: &PgPool) -> Result<String> {
        loop {
            let now = SystemTime::now();
            let since_epoch = now
                .duration_since(UNIX_EPOCH)
                .context("Time went backwards")?;
            let timestamp = since_epoch.as_secs();
            let code = format!("x{}", timestamp);
            if !self.exist_code(pool, &code).await? {
                return Ok(code);
            }
            sleep(Duration::from_secs(1));
        }
    }

    async fn check_product_usage(&self, product_id: i64, page: Option<u32>, page_size: Option<u32>) -> Result<(bool, Vec<BomReference>, i64)> {
        let result = BomRepo::find_boms_using_product(&self.pool, product_id, page, page_size).await?;
        let is_used = result.total > 0;
        Ok((is_used, result.boms, result.total))
    }
}

/// 将数据库 UNIQUE 约束冲突转换为 ServiceError::Conflict，
/// 其他错误原样传递。
fn map_duplicate_error(e: anyhow::Error, product_code: &str) -> anyhow::Error {
    if let Some(sqlx::Error::Database(db_err)) = e.downcast_ref::<sqlx::Error>()
        && db_err.code().as_deref() == Some("23505")
    {
        return anyhow::Error::from(ServiceError::Conflict {
            resource: "Product".to_string(),
            message: format!("产品编码 '{}' 已存在", product_code),
        });
    }
    e
}
