use async_trait::async_trait;

use super::model::{
    CancelOutsourcingReq, ConvertToInternalReq, CreateOutsourcingOrderReq, OutsourcingMaterial,
    OutsourcingOrder, OutsourcingOrderQuery, ReceiveOutsourcingReq, SendOutsourcingReq,
    UpdateOutsourcingOrderReq, WorkOrderOutsourcingSummary,
};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;
use crate::wms::inventory_transaction::model::InventoryTransaction;

#[async_trait]
pub trait OutsourcingOrderService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateOutsourcingOrderReq,
        idempotency_key: Option<String>,
    ) -> Result<i64>;

    async fn update(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: UpdateOutsourcingOrderReq) -> Result<()>;

    async fn send(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: SendOutsourcingReq) -> Result<()>;

    async fn receive(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: ReceiveOutsourcingReq,
    ) -> Result<()>;

    async fn convert_to_internal(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: ConvertToInternalReq,
    ) -> Result<i64>;

    async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CancelOutsourcingReq) -> Result<()>;

    async fn find_by_id(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<OutsourcingOrder>;

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: OutsourcingOrderQuery,
        page: crate::shared::types::pagination::PageParams,
    ) -> Result<PaginatedResult<OutsourcingOrder>>;

    /// 查询委外单的发料明细列表
    async fn list_materials(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        outsourcing_id: i64,
    ) -> Result<Vec<OutsourcingMaterial>>;

    /// 查询委外单关联的库存收发记录（发料/收货流水，来自关联的 WMS 调拨单）
    async fn list_inventory_records(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        outsourcing_id: i64,
    ) -> Result<Vec<InventoryTransaction>>;

    /// 工单委外摘要（关联工单联动：产品/数量/交期/客户 + 工序列表）
    async fn outsourcing_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<WorkOrderOutsourcingSummary>;

    /// 查某工单某工序的活跃委外单（非取消/非转自制）。drawer 委外工序动作位判定用。
    async fn find_active_for_routing(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
        routing_id: i64,
        batch_id: Option<i64>,
    ) -> Result<Vec<OutsourcingOrder>>;
}
