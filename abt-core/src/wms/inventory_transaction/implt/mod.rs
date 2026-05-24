use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::{InventoryTransaction, RecordTransactionReq, TransactionFilter};
use super::repo::InventoryTransactionRepo;
use super::service::InventoryTransactionService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;
use crate::wms::stock_ledger::model::{StockFilter, UpsertStockReq};
use crate::wms::stock_ledger::StockLedgerService;
use crate::wms::stock_ledger::model::StockLedger;

pub struct InventoryTransactionServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
    stock_ledger_svc: Arc<dyn StockLedgerService>,
}

impl InventoryTransactionServiceImpl {
    pub fn new(pool: Arc<PgPool>, stock_ledger_svc: Arc<dyn StockLedgerService>) -> Self {
        Self { pool, stock_ledger_svc }
    }
}

#[async_trait]
impl InventoryTransactionService for InventoryTransactionServiceImpl {
    async fn record(
        &self,
        ctx: ServiceContext<'_>,
        req: RecordTransactionReq,
    ) -> Result<i64, DomainError> {
        let txn = InventoryTransactionRepo::insert(&mut *ctx.executor, &req, ctx.operator_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 自动更新库存台账（设计要求：record -> auto update StockLedger）
        if let (Some(zone_id), Some(bin_id)) = (req.zone_id, req.bin_id) {
            self.stock_ledger_svc.upsert(
                ctx,
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
        ctx: ServiceContext<'_>,
        source_type: &str,
        source_id: i64,
    ) -> Result<Vec<InventoryTransaction>, DomainError> {
        InventoryTransactionRepo::find_by_source(&mut *ctx.executor, source_type, source_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn query(
        &self,
        ctx: ServiceContext<'_>,
        filter: TransactionFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<InventoryTransaction>, DomainError> {
        InventoryTransactionRepo::query(&mut *ctx.executor, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn query_stock(
        &self,
        ctx: ServiceContext<'_>,
        filter: StockFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<StockLedger>, DomainError> {
        self.stock_ledger_svc.query(ctx, filter, page, page_size).await
    }

    async fn query_available(
        &self,
        ctx: ServiceContext<'_>,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<Decimal, DomainError> {
        self.stock_ledger_svc.query_available(ctx, product_id, warehouse_id).await
    }
}
