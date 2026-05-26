use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, Result, ServiceContext};

#[async_trait]
pub trait QuotationService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateQuotationReq,
    ) -> Result<i64>;

    async fn find_by_id(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<Quotation>;

    async fn update(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        req: UpdateQuotationReq,
    ) -> Result<()>;

    async fn submit(&self, ctx: ServiceContext<'_>, id: i64) -> Result<()>;

    async fn accept(&self, ctx: ServiceContext<'_>, id: i64) -> Result<()>;

    async fn reject(&self, ctx: ServiceContext<'_>, id: i64) -> Result<()>;

    async fn expire(&self, ctx: ServiceContext<'_>, id: i64) -> Result<()>;

    async fn batch_expire_overdue(&self, ctx: ServiceContext<'_>) -> Result<i32>;

    async fn list_items(
        &self,
        ctx: ServiceContext<'_>,
        quotation_id: i64,
    ) -> Result<Vec<QuotationItem>>;

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: QuotationQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Quotation>>;
}
