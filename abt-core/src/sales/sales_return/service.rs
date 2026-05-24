use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

#[async_trait]
pub trait SalesReturnService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateReturnReq,
    ) -> Result<i64, DomainError>;

    async fn find_by_id(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<SalesReturn, DomainError>;

    async fn approve(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn receive(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn inspect(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn complete(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn reject(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn cancel(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: ReturnQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<SalesReturn>, DomainError>;
}
