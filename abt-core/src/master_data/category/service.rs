use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

#[async_trait]
pub trait CategoryService: Send + Sync {
    async fn create(&self, ctx: ServiceContext<'_>, req: CreateCategoryReq) -> Result<i64, DomainError>;
    async fn update(&self, ctx: ServiceContext<'_>, category_id: i64, req: UpdateCategoryReq) -> Result<(), DomainError>;
    async fn delete(&self, ctx: ServiceContext<'_>, category_id: i64) -> Result<(), DomainError>;
    async fn get(&self, ctx: ServiceContext<'_>, category_id: i64) -> Result<Category, DomainError>;
    async fn list(&self, ctx: ServiceContext<'_>, filter: CategoryQuery, page: PageParams) -> Result<PaginatedResult<Category>, DomainError>;
    async fn get_tree(&self, ctx: ServiceContext<'_>, root_id: Option<i64>, depth_limit: Option<i32>) -> Result<Vec<CategoryTree>, DomainError>;
    async fn move_to(&self, ctx: ServiceContext<'_>, category_id: i64, new_parent_id: i64) -> Result<(), DomainError>;
    async fn assign_products(&self, ctx: ServiceContext<'_>, category_id: i64, product_ids: Vec<i64>) -> Result<(), DomainError>;
    async fn remove_products(&self, ctx: ServiceContext<'_>, category_id: i64, product_ids: Vec<i64>) -> Result<(), DomainError>;
}
