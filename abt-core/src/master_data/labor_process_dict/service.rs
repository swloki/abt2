use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

#[async_trait]
pub trait LaborProcessDictService: Send + Sync {
    async fn list(&self, ctx: ServiceContext<'_>, query: LaborProcessDictQuery, page: PageParams) -> Result<PaginatedResult<LaborProcessDict>>;
    async fn create(&self, ctx: ServiceContext<'_>, req: CreateLaborProcessDictReq) -> Result<i64>;
    async fn update(&self, ctx: ServiceContext<'_>, id: i64, req: UpdateLaborProcessDictReq) -> Result<()>;
    async fn delete(&self, ctx: ServiceContext<'_>, id: i64) -> Result<()>;
}
