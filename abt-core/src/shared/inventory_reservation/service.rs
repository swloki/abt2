use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::shared::enums::DocumentType;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::batch::BatchResult;

use super::model::ReserveRequest;

#[async_trait]
pub trait InventoryReservationService: Send + Sync {
    /// ContinueOnError 模式 — 逐条创建预留，单条失败不影响其他
    async fn reserve(
        &self,
        ctx: ServiceContext<'_>,
        requests: Vec<ReserveRequest>,
    ) -> Result<BatchResult, DomainError>;

    /// 履行预留 — UPDATE status = Fulfilled WHERE id = $1 AND status = Active
    async fn fulfill(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    /// 取消预留 — UPDATE status = Cancelled WHERE id = $1 AND status = Active
    async fn cancel(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    /// 按来源取消全部 Active 预留（用于订单取消时批量释放）
    async fn cancel_by_source(
        &self,
        ctx: ServiceContext<'_>,
        source_type: DocumentType,
        source_id: i64,
    ) -> Result<u64, DomainError>;

    /// 按来源行履行 Active 预留（用于发货时逐行 fulfill）
    async fn fulfill_by_source_line(
        &self,
        ctx: ServiceContext<'_>,
        source_type: DocumentType,
        source_line_id: i64,
    ) -> Result<(), DomainError>;

    /// 查询 product_id 的 Active 预留总量，warehouse_id 为 None 时汇总所有仓库
    async fn total_reserved(
        &self,
        ctx: ServiceContext<'_>,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<Decimal, DomainError>;
}
