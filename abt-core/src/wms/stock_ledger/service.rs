use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

use super::model::{StockFilter, StockLedger, UpsertStockReq};

/// 预计可用量分解（参考 ERPNext bin.projected_qty 公式）
/// projected = actual + on_order_po + in_progress_wo - reserved
#[derive(Debug, Clone)]
pub struct ProjectedQty {
    /// 当前实物库存（stock_ledger.quantity）
    pub actual: Decimal,
    /// 在途采购量（PO quantity - received_qty, 状态 Confirmed/PartiallyReceived）
    pub on_order_po: Decimal,
    /// 在制工单量（WO planned_qty - completed_qty, 状态 Released/InProduction）
    pub in_progress_wo: Decimal,
    /// 硬预留量（inventory_reservations Active）
    pub reserved: Decimal,
    /// 净预计可用量 = actual + on_order_po + in_progress_wo - reserved
    pub projected: Decimal,
}

#[async_trait]
pub trait StockLedgerService: Send + Sync {
    async fn upsert(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: UpsertStockReq,
    ) -> Result<()>;

    async fn query(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: StockFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<StockLedger>>;

    async fn query_available(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<Decimal>;

    /// 批量查询可用量（消除 N+1，单 warehouse；调用方按 warehouse 分组调用）
    async fn query_available_batch(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_ids: &[i64],
        warehouse_id: Option<i64>,
    ) -> Result<std::collections::HashMap<i64, Decimal>>;

    /// 预计可用量（参考 ERPNext projected_qty 公式）
    /// projected = actual + on_order_po + in_progress_wo - reserved
    async fn query_projected_qty(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<ProjectedQty>;

    /// 批量查询预计可用量（消除 N+1）
    async fn query_projected_qty_batch(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_ids: &[i64],
        warehouse_id: Option<i64>,
    ) -> Result<std::collections::HashMap<i64, ProjectedQty>>;
}
