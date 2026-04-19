use anyhow::Result;
use async_trait::async_trait;

use crate::models::{BomCategory, BomCategoryQuery, CreateBomCategoryRequest, UpdateBomCategoryRequest};
use crate::repositories::Executor;

#[async_trait]
pub trait BomCategoryService: Send + Sync {
    async fn create(
        &self,
        req: CreateBomCategoryRequest,
        executor: Executor<'_>,
    ) -> Result<i64>;

    async fn update(
        &self,
        bom_category_id: i64,
        req: UpdateBomCategoryRequest,
        executor: Executor<'_>,
    ) -> Result<()>;

    async fn delete(
        &self,
        bom_category_id: i64,
        executor: Executor<'_>,
    ) -> Result<()>;

    async fn get(&self, bom_category_id: i64) -> Result<Option<BomCategory>>;

    async fn list(&self, query: BomCategoryQuery) -> Result<(Vec<BomCategory>, i64)>;

    async fn exists_name(&self, name: &str) -> Result<bool>;

    async fn has_boms(&self, bom_category_id: i64) -> Result<bool>;
}
