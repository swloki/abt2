use async_trait::async_trait;

use super::model::{
    CancelOutsourcingReq, ConvertToInternalReq, CreateOutsourcingOrderReq, OutsourcingOrder,
    OutsourcingOrderQuery, ReceiveOutsourcingReq, SendOutsourcingReq, UpdateOutsourcingOrderReq,
};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

#[async_trait]
pub trait OutsourcingOrderService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateOutsourcingOrderReq,
        idempotency_key: Option<String>,
    ) -> Result<i64>;

    async fn update(&self, ctx: ServiceContext<'_>, req: UpdateOutsourcingOrderReq) -> Result<()>;

    async fn send(&self, ctx: ServiceContext<'_>, req: SendOutsourcingReq) -> Result<()>;

    async fn receive(
        &self,
        ctx: ServiceContext<'_>,
        req: ReceiveOutsourcingReq,
    ) -> Result<()>;

    async fn convert_to_internal(
        &self,
        ctx: ServiceContext<'_>,
        req: ConvertToInternalReq,
    ) -> Result<i64>;

    async fn cancel(&self, ctx: ServiceContext<'_>, req: CancelOutsourcingReq) -> Result<()>;

    async fn find_by_id(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<OutsourcingOrder>;

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: OutsourcingOrderQuery,
        page: crate::shared::types::pagination::PageParams,
    ) -> Result<PaginatedResult<OutsourcingOrder>>;
}
