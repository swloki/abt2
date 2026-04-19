use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use crate::models::*;
use crate::repositories::{BomCategoryRepo, Executor};
use crate::service::BomCategoryService;
use common::error::ServiceError;

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
        if BomCategoryRepo::is_name_exists(&mut *executor, &req.bom_category_name).await? {
            return Err(ServiceError::Conflict {
                resource: "BomCategory".into(),
                message: format!("分类名称 '{}' 已存在", req.bom_category_name),
            }.into());
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
        let _existing = BomCategoryRepo::find_by_id(&mut *executor, bom_category_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "BomCategory".into(),
                id: bom_category_id.to_string(),
            })?;

        if req.bom_category_name != _existing.bom_category_name
            && BomCategoryRepo::is_name_exists(&mut *executor, &req.bom_category_name).await? {
                return Err(ServiceError::Conflict {
                    resource: "BomCategory".into(),
                    message: format!("分类名称 '{}' 已存在", req.bom_category_name),
                }.into());
            }

        BomCategoryRepo::update(executor, bom_category_id, &req).await?;
        Ok(())
    }

    async fn delete(
        &self,
        bom_category_id: i64,
        executor: Executor<'_>,
    ) -> Result<()> {
        BomCategoryRepo::find_by_id(&mut *executor, bom_category_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "BomCategory".into(),
                id: bom_category_id.to_string(),
            })?;

        if BomCategoryRepo::has_boms(&mut *executor, bom_category_id).await? {
            return Err(ServiceError::BusinessValidation {
                message: "该分类下存在 BOM，无法删除".into(),
            }.into());
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
}
