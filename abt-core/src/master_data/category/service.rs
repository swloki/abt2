use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PgExecutor,PageParams, PaginatedResult, ServiceContext, Result};

#[async_trait]
pub trait CategoryService: Send + Sync {
    async fn create(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CreateCategoryReq) -> Result<i64>;
    async fn update(&self, ctx: &ServiceContext, db: PgExecutor<'_>, category_id: i64, req: UpdateCategoryReq) -> Result<()>;
    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, category_id: i64) -> Result<()>;
    async fn get(&self, ctx: &ServiceContext, db: PgExecutor<'_>, category_id: i64) -> Result<Category>;
    async fn list(&self, ctx: &ServiceContext, db: PgExecutor<'_>, filter: CategoryQuery, page: PageParams) -> Result<PaginatedResult<Category>>;
    async fn get_tree(&self, ctx: &ServiceContext, db: PgExecutor<'_>, root_id: Option<i64>, depth_limit: Option<i32>) -> Result<Vec<CategoryTree>>;
    async fn move_to(&self, ctx: &ServiceContext, db: PgExecutor<'_>, category_id: i64, new_parent_id: i64) -> Result<()>;
    async fn assign_products(&self, ctx: &ServiceContext, db: PgExecutor<'_>, category_id: i64, product_ids: Vec<i64>) -> Result<()>;
    async fn remove_products(&self, ctx: &ServiceContext, db: PgExecutor<'_>, category_id: i64, product_ids: Vec<i64>) -> Result<()>;
}
