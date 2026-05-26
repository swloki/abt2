use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;
use crate::wms::stock_ledger::model::{StockFilter, StockLedger};

use super::model::{InventoryTransaction, RecordTransactionReq, TransactionFilter};

#[async_trait]
pub trait InventoryTransactionService: Send + Sync {
    /// 追加一条库存事务（Append-only），自动更新 StockLedger
    async fn record(
        &self,
        ctx: ServiceContext<'_>,
        req: RecordTransactionReq,
    ) -> Result<i64>;

    /// 按来源查事务记录
    async fn find_by_source(
        &self,
        ctx: ServiceContext<'_>,
        source_type: &str,
        source_id: i64,
    ) -> Result<Vec<InventoryTransaction>>;

    /// 分页查询库存事务记录
    async fn query(
        &self,
        ctx: ServiceContext<'_>,
        filter: TransactionFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<InventoryTransaction>>;

    /// 分页查询库存台账（设计要求：InventoryTransactionService.query_stock）
    async fn query_stock(
        &self,
        ctx: ServiceContext<'_>,
        filter: StockFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<StockLedger>>;

    /// 查询可用量（设计要求：InventoryTransactionService.query_available）
    /// 可用量 = StockLedger.quantity - InvRes.total_reserved()
    async fn query_available(
        &self,
        ctx: ServiceContext<'_>,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<Decimal>;
}
