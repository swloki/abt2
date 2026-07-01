use async_trait::async_trait;
use rust_decimal::Decimal;
use std::collections::HashMap;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;
use crate::wms::stock_ledger::model::{StockFilter, StockLedger};

use super::model::{InventoryTransaction, RecordTransactionReq, TransactionFilter};

#[async_trait]
pub trait InventoryTransactionService: Send + Sync {
    /// 追加一条库存事务（Append-only），自动更新 StockLedger
    async fn record(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: RecordTransactionReq,
    ) -> Result<i64>;

    /// 按 id 查单条库存事务（找不到返回 NotFound）
    async fn find_by_id(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<InventoryTransaction>;

    /// 按来源查事务记录
    async fn find_by_source(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        source_type: &str,
        source_id: i64,
    ) -> Result<Vec<InventoryTransaction>>;

    /// 批量查多个 source 的库存流水（避免逐个 `find_by_source` 的 N+1）
    async fn find_by_sources(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        source_type: &str,
        source_ids: &[i64],
    ) -> Result<Vec<InventoryTransaction>>;

    /// 批量查询多个 source 的数量总和（`source_id → SUM(quantity)`），避免逐个 `find_by_source` 的 N+1
    async fn sum_quantity_by_source(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        source_type: &str,
        source_ids: &[i64],
    ) -> Result<HashMap<i64, Decimal>>;

    /// 分页查询库存事务记录
    async fn query(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: TransactionFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<InventoryTransaction>>;

    /// 分页查询库存台账（设计要求：InventoryTransactionService.query_stock）
    async fn query_stock(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: StockFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<StockLedger>>;

    /// 查询可用量（设计要求：InventoryTransactionService.query_available）
    /// 可用量 = StockLedger.quantity - InvRes.total_reserved()
    async fn query_available(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<Decimal>;

    /// 批量查可用量（消除 N+1，单 warehouse；调用方按 warehouse 分组）
    async fn query_available_batch(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        product_ids: &[i64],
        warehouse_id: Option<i64>,
    ) -> Result<std::collections::HashMap<i64, Decimal>>;
}
