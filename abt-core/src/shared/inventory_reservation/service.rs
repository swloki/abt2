use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::shared::enums::DocumentType;
use crate::shared::types::PgExecutor;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;
use crate::shared::types::batch::BatchResult;

use super::model::ReserveRequest;

#[async_trait]
pub trait InventoryReservationService: Send + Sync {
    /// ContinueOnError 模式 — 逐条创建预留，单条失败不影响其他
    async fn reserve(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        requests: Vec<ReserveRequest>,
    ) -> Result<BatchResult>;

    /// 履行预留 — UPDATE status = Fulfilled WHERE id = $1 AND status = Active
    async fn fulfill(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    /// 取消预留 — UPDATE status = Cancelled WHERE id = $1 AND status = Active
    async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;

    /// 按来源取消全部 Active 预留（用于订单取消时批量释放）
    async fn cancel_by_source(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        source_type: DocumentType,
        source_id: i64,
    ) -> Result<u64>;

    /// 按来源行履行 Active 预留（用于发货时逐行 fulfill）
    async fn fulfill_by_source_line(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        source_type: DocumentType,
        source_line_id: i64,
    ) -> Result<()>;

    /// 查询 product_id 的 Active 预留总量，warehouse_id 为 None 时汇总所有仓库
    async fn total_reserved(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<Decimal>;

    /// 按 product_id 查询 Active 预留明细（JOIN 来源单据 + 客户），供前端展示
    async fn list_active_by_product(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<Vec<super::model::ReservationDetail>>;

    /// 按来源单据查询每行实际 Active 预留量，返回 HashMap<source_line_id, qty>。
    /// confirm 预留后调用，用于计算每行 shortage（= required - actual_reserved）。
    async fn reserved_qty_by_source(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        source_type: DocumentType,
        source_id: i64,
    ) -> Result<std::collections::HashMap<i64, Decimal>>;
    /// 消耗预留 — 扣减指定来源+产品的预留量（对标 Odoo move._action_done 消费 reservation）
    async fn consume(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        source_type: DocumentType,
        source_id: i64,
        product_id: i64,
        qty: Decimal,
    ) -> Result<()>;
}
