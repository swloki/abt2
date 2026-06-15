use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::{InventoryTransaction, RecordTransactionReq, TransactionFilter};
use super::repo::InventoryTransactionRepo;
use super::service::InventoryTransactionService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;
use crate::wms::stock_ledger::model::{StockFilter, UpsertStockReq};
use crate::wms::stock_ledger::{new_stock_ledger_service, service::StockLedgerService};
use crate::wms::stock_ledger::model::StockLedger;

pub struct InventoryTransactionServiceImpl {
    pool: PgPool,
}

impl InventoryTransactionServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl InventoryTransactionService for InventoryTransactionServiceImpl {
    async fn record(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: RecordTransactionReq,
    ) -> Result<i64> {
        // 兜底生成单据号（调用方未显式提供时，按事务类型前缀 + 本地时间戳）
        let mut req = req;
        if req.doc_number.is_none() {
            req.doc_number = Some(format!(
                "{}{}",
                req.transaction_type.doc_prefix(),
                chrono::Local::now().format("%Y%m%d%H%M%S")
            ));
        }

        let txn = InventoryTransactionRepo::insert(&mut *db, &req, ctx.operator_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 自动更新库存台账（设计要求：record -> auto update StockLedger）
        if let (Some(zone_id), Some(bin_id)) = (req.zone_id, req.bin_id) {
            new_stock_ledger_service(self.pool.clone())
            .upsert(
                ctx,
                db,
                UpsertStockReq {
                    product_id: req.product_id,
                    warehouse_id: req.warehouse_id,
                    zone_id,
                    bin_id,
                    batch_no: req.batch_no.clone(),
                    qty_delta: req.quantity,
                    unit_cost: req.unit_cost,
                },
            ).await?;
        }

        Ok(txn.id)
    }

    async fn find_by_source(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        source_type: &str,
        source_id: i64,
    ) -> Result<Vec<InventoryTransaction>> {
        InventoryTransactionRepo::find_by_source(&mut *db, source_type, source_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn query(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: TransactionFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<InventoryTransaction>> {
        InventoryTransactionRepo::query(&mut *db, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn query_stock(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: StockFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<StockLedger>> {
        new_stock_ledger_service(self.pool.clone())
            .query(ctx, db, filter, page, page_size).await
    }

    async fn query_available(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<Decimal> {
        new_stock_ledger_service(self.pool.clone())
            .query_available(ctx, db, product_id, warehouse_id).await
    }
}
