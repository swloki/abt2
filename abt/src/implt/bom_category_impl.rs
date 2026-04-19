use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::sync::Arc;

use crate::models::*;
use crate::repositories::{BomCategoryRepo, Executor};
use crate::service::BomCategoryService;

pub struct BomCategoryServiceImpl {
    pool: Arc<sqlx::PgPool>,
}

impl BomCategoryServiceImpl {
    pub fn new(pool: Arc<sqlx::PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl BomCategoryService for BomCategoryServiceImpl {
    async fn create(
        &self,
        req: CreateBomCategoryRequest,
        executor: Executor<'_>,
    ) -> Result<i64> {
        // Check if name already exists
        if BomCategoryRepo::is_name_exists(self.pool.as_ref(), &req.bom_category_name).await? {
            return Err(anyhow!("BOM category name already exists: {}", req.bom_category_name));
        }

        let bom_category_id = BomCategoryRepo::insert(executor, &req).await?;
        Ok(bom_category_id)
    }

    async fn update(
        &self,
        bom_category_id: i64,
        req: UpdateBomCategoryRequest,
        executor: Executor<'_>,
    ) -> Result<()> {
        // Check if category exists
        let existing = BomCategoryRepo::find_by_id(self.pool.as_ref(), bom_category_id)
            .await?
            .ok_or_else(|| anyhow!("BOM category not found"))?;

        // Check if new name conflicts with another category
        if req.bom_category_name != existing.bom_category_name {
            if BomCategoryRepo::is_name_exists(self.pool.as_ref(), &req.bom_category_name).await? {
                return Err(anyhow!("BOM category name already exists: {}", req.bom_category_name));
            }
        }

        BomCategoryRepo::update(executor, bom_category_id, &req).await?;
        Ok(())
    }

    async fn delete(
        &self,
        bom_category_id: i64,
        executor: Executor<'_>,
    ) -> Result<()> {
        // Check if category exists
        let _existing = BomCategoryRepo::find_by_id(self.pool.as_ref(), bom_category_id)
            .await?
            .ok_or_else(|| anyhow!("BOM category not found"))?;

        // Check if there are BOMs using this category
        if BomCategoryRepo::has_boms(self.pool.as_ref(), bom_category_id).await? {
            return Err(anyhow!("Cannot delete BOM category: there are BOMs using this category"));
        }

        BomCategoryRepo::delete(executor, bom_category_id).await?;
        Ok(())
    }

    async fn get(&self, bom_category_id: i64) -> Result<Option<BomCategory>> {
        let category = BomCategoryRepo::find_by_id(self.pool.as_ref(), bom_category_id).await?;
        Ok(category)
    }

    async fn list(&self, query: BomCategoryQuery) -> Result<(Vec<BomCategory>, i64)> {
        let categories = BomCategoryRepo::query(self.pool.as_ref(), &query).await?;
        let total = BomCategoryRepo::query_count(self.pool.as_ref(), &query).await?;
        Ok((categories, total))
    }

    async fn exists_name(&self, name: &str) -> Result<bool> {
        let exists = BomCategoryRepo::is_name_exists(self.pool.as_ref(), name).await?;
        Ok(exists)
    }

    async fn has_boms(&self, bom_category_id: i64) -> Result<bool> {
        let has = BomCategoryRepo::has_boms(self.pool.as_ref(), bom_category_id).await?;
        Ok(has)
    }
}