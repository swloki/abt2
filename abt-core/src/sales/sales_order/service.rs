use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

#[async_trait]
pub trait SalesOrderService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateSalesOrderReq,
    ) -> Result<i64, DomainError>;

    async fn create_from_quotation(
        &self,
        ctx: ServiceContext<'_>,
        quotation_id: i64,
    ) -> Result<i64, DomainError>;

    async fn find_by_id(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<SalesOrder, DomainError>;

    async fn update_header(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        req: UpdateSalesOrderReq,
    ) -> Result<(), DomainError>;

    async fn confirm(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn start_progress(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn complete(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn cancel(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: SalesOrderQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<SalesOrder>, DomainError>;
}
