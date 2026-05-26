use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

#[async_trait]
pub trait ProductService: Send + Sync {
    async fn create(&self, ctx: ServiceContext<'_>, req: CreateProductReq) -> Result<i64>;
    async fn update(&self, ctx: ServiceContext<'_>, id: i64, req: UpdateProductReq) -> Result<()>;
    async fn delete(&self, ctx: ServiceContext<'_>, id: i64) -> Result<()>;
    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<Product>;
    async fn get_by_ids(&self, ctx: ServiceContext<'_>, ids: Vec<i64>) -> Result<Vec<Product>>;
    async fn list(&self, ctx: ServiceContext<'_>, filter: ProductQuery, page: PageParams) -> Result<PaginatedResult<Product>>;
    async fn check_product_usage(&self, ctx: ServiceContext<'_>, id: i64, query: UsageQuery) -> Result<PaginatedResult<UsageEntry>>;
}
