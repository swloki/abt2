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

        // 负库存前置预检（P0-2）：消耗型事务扣减时，先校验仓库级可用量，
        // 给出"物料X在Y库可用量N不足以扣减M"的明确错误（而非 upsert 深处的泛化错误）。
        // 注意：Adjustment（盘点调账/手工调整）不走此预检——它是对账面的事实修正，
        // 由 upsert 后置硬阻断兜底防止真实负库存。
        if req.quantity < Decimal::ZERO && is_consumption_txn(req.transaction_type) {
            let available = new_stock_ledger_service(self.pool.clone())
                .query_available(ctx, db, req.product_id, Some(req.warehouse_id))
                .await?;
            let required = req.quantity.abs();
            if available < required {
                return Err(DomainError::insufficient_stock(
                    req.product_id,
                    req.warehouse_id,
                    available,
                    required,
                ));
            }
        }

        let txn = InventoryTransactionRepo::insert(&mut *db, &req, ctx.operator_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 自动更新库存台账（设计要求：record -> auto update StockLedger）。
        // 修复：zone/bin 缺失时不再静默跳过，而是按 product×warehouse 解析默认库位
        // （FIFO 有库存优先），确保领料/形态转换等不传库位的调用方也能正确动账。
        let (zone_id, bin_id) = match (req.zone_id, req.bin_id) {
            (Some(z), Some(b)) => (Some(z), Some(b)),
            _ => {
                let resolved = crate::wms::stock_ledger::repo::StockLedgerRepo::resolve_default_bin(
                    &mut *db,
                    req.product_id,
                    req.warehouse_id,
                )
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                match resolved {
                    Some((z, b)) => (Some(z), Some(b)),
                    None => (req.zone_id, req.bin_id),
                }
            }
        };
        if let (Some(zone_id), Some(bin_id)) = (zone_id, bin_id) {
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

        // 减库存后检查安全库存预警（P0-4）：best-effort，预警失败不回滚库存事务，仅记录日志。
        if req.quantity < Decimal::ZERO && is_consumption_txn(req.transaction_type) {
            use crate::wms::low_stock_alert::service::LowStockAlertService;
            if let Err(e) =
                crate::wms::low_stock_alert::new_low_stock_alert_service(self.pool.clone())
                    .check_and_record(ctx, db, req.product_id, req.warehouse_id)
                    .await
            {
                tracing::warn!(error = %e, "low stock alert check failed (best-effort)");
            }
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

/// 是否为"消耗型"事务（扣减实物库存）——这类负数量需前置可用量预检。
/// Adjustment 不列入：盘点/手工调整是对账面的事实修正，由 upsert 兜底。
fn is_consumption_txn(t: crate::wms::enums::TransactionType) -> bool {
    use crate::wms::enums::TransactionType;
    matches!(
        t,
        TransactionType::SalesShipment | TransactionType::MaterialIssue | TransactionType::Scrap
    )
}
