use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PgExecutor,PageParams, PaginatedResult, ServiceContext, Result};

#[async_trait]
pub trait ProductService: Send + Sync {
    async fn create(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CreateProductReq) -> Result<i64>;
    async fn update(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, req: UpdateProductReq) -> Result<()>;
    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
    async fn get(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<Product>;
    async fn get_by_ids(&self, ctx: &ServiceContext, db: PgExecutor<'_>, ids: Vec<i64>) -> Result<Vec<Product>>;
    async fn list(&self, ctx: &ServiceContext, db: PgExecutor<'_>, filter: ProductQuery, page: PageParams) -> Result<PaginatedResult<Product>>;
    async fn check_product_usage(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, query: UsageQuery) -> Result<PaginatedResult<UsageEntry>>;
}
