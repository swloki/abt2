use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, Result, ServiceContext};

#[async_trait]
pub trait SalesOrderService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateSalesOrderReq,
    ) -> Result<i64>;

    async fn create_from_quotation(
        &self,
        ctx: ServiceContext<'_>,
        quotation_id: i64,
    ) -> Result<i64>;

    async fn find_by_id(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<SalesOrder>;

    async fn update_header(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        req: UpdateSalesOrderReq,
    ) -> Result<()>;

    async fn confirm(&self, ctx: ServiceContext<'_>, id: i64) -> Result<()>;

    async fn start_progress(&self, ctx: ServiceContext<'_>, id: i64) -> Result<()>;

    async fn complete(&self, ctx: ServiceContext<'_>, id: i64) -> Result<()>;

    async fn cancel(&self, ctx: ServiceContext<'_>, id: i64) -> Result<()>;

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: SalesOrderQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<SalesOrder>>;
}
