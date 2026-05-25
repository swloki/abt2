use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

#[async_trait]
pub trait LaborProcessDictService: Send + Sync {
    async fn list(&self, ctx: ServiceContext<'_>, query: LaborProcessDictQuery, page: PageParams) -> Result<PaginatedResult<LaborProcessDict>, DomainError>;
    async fn create(&self, ctx: ServiceContext<'_>, req: CreateLaborProcessDictReq) -> Result<i64, DomainError>;
    async fn update(&self, ctx: ServiceContext<'_>, id: i64, req: UpdateLaborProcessDictReq) -> Result<(), DomainError>;
    async fn delete(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
}
