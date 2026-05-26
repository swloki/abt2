use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

use super::model::{
    ArrivalNotice, ArrivalNoticeFilter, CreateArrivalNoticeReq, InspectArrivalNoticeReq,
    ReceiveArrivalNoticeReq,
};

#[async_trait]
pub trait ArrivalNoticeService: Send + Sync {
    /// 创建来料通知（状态为 Draft），返回通知 ID
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateArrivalNoticeReq,
    ) -> Result<i64>;

    /// 按 ID 查询来料通知，不存在则返回 NotFound
    async fn get(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<ArrivalNotice>;

    /// 分页查询来料通知
    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: ArrivalNoticeFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<ArrivalNotice>>;

    /// 收货：Draft -> Received，更新明细行 received_qty
    async fn receive(
        &self,
        ctx: ServiceContext<'_>,
        req: ReceiveArrivalNoticeReq,
    ) -> Result<()>;

    /// 检验：Received -> Inspecting -> Accepted/PartiallyAccepted/Rejected
    async fn inspect(
        &self,
        ctx: ServiceContext<'_>,
        req: InspectArrivalNoticeReq,
    ) -> Result<()>;

    /// 取消：仅 Draft 状态可取消（软删除）
    async fn cancel(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<()>;
}
