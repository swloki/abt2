use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

#[async_trait]
pub trait BomLaborProcessService: Send + Sync {
    async fn list(&self, ctx: ServiceContext<'_>, query: BomLaborProcessQuery, page: PageParams) -> Result<PaginatedResult<BomLaborProcess>, DomainError>;
    async fn create(&self, ctx: ServiceContext<'_>, req: CreateBomLaborProcessReq) -> Result<i64, DomainError>;
    async fn update(&self, ctx: ServiceContext<'_>, id: i64, req: UpdateBomLaborProcessReq) -> Result<(), DomainError>;
    async fn delete(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
}
