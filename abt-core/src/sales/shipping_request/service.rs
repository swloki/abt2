use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

#[async_trait]
pub trait ShippingRequestService: Send + Sync {
    async fn create_from_order(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateFromOrderReq,
    ) -> Result<i64, DomainError>;

    async fn find_by_id(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<ShippingRequest, DomainError>;

    async fn update(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        req: UpdateShippingReq,
    ) -> Result<(), DomainError>;

    async fn confirm(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn pick(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn ship(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn cancel(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: ShippingQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<ShippingRequest>, DomainError>;
}
