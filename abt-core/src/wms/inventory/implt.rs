use async_trait::async_trait;
use rust_decimal::Decimal;

use super::model::{
    InventoryDetailView, InventoryQueryFilter, StockChangeReq, StockOperationResult,
    StockTransferReq, TransactionDetailView, TransactionLogFilter,
};
use super::repo::InventoryRepo;
use super::service::InventoryService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;
use crate::wms::enums::TransactionType;
use crate::wms::inventory_transaction::model::RecordTransactionReq;
use crate::wms::inventory_transaction::repo::InventoryTransactionRepo;
use crate::wms::stock_ledger::model::UpsertStockReq;
use crate::wms::stock_ledger::repo::StockLedgerRepo;

pub struct InventoryServiceImpl;

impl InventoryServiceImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for InventoryServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InventoryService for InventoryServiceImpl {
    async fn stock_in(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: StockChangeReq,
    ) -> Result<StockOperationResult> {
        self.execute_stock_op(&mut *db, ctx.operator_id, &req, req.quantity, TransactionType::PurchaseReceipt).await
    }

    async fn stock_out(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: StockChangeReq,
    ) -> Result<StockOperationResult> {
        self.execute_stock_op(&mut *db, ctx.operator_id, &req, req.quantity, TransactionType::SalesShipment).await
    }

    async fn adjust(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: StockChangeReq,
    ) -> Result<StockOperationResult> {
        self.execute_stock_op(&mut *db, ctx.operator_id, &req, req.quantity, TransactionType::Adjustment).await
    }

    async fn set_quantity(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: StockChangeReq,
    ) -> Result<StockOperationResult> {
        let exec = &mut *db;

        let before = StockLedgerRepo::find_by_location(
            exec, req.product_id, req.warehouse_id, req.zone_id, req.bin_id, None,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        let before_qty = before.as_ref().map(|s| s.quantity).unwrap_or(Decimal::ZERO);
        let delta = req.quantity - before_qty;

        if delta == Decimal::ZERO {
            return Ok(StockOperationResult {
                transaction_id: 0,
                stock_ledger_id: before.map(|s| s.id).unwrap_or(0),
                product_id: req.product_id,
                warehouse_id: req.warehouse_id,
                zone_id: req.zone_id,
                bin_id: req.bin_id,
                before_qty,
                after_qty: before_qty,
                change_qty: Decimal::ZERO,
            });
        }

        let result = self
            .record_and_update(exec, ctx.operator_id, &req, delta, TransactionType::Adjustment)
            .await?;

        Ok(StockOperationResult {
            change_qty: delta,
            ..result
        })
    }

    async fn transfer(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: StockTransferReq,
    ) -> Result<(StockOperationResult, StockOperationResult)> {
        let exec = &mut *db;

        // out
        let out_req = StockChangeReq {
            product_id: req.product_id,
            warehouse_id: req.from_warehouse_id,
            zone_id: req.from_zone_id,
            bin_id: req.from_bin_id,
            quantity: -req.quantity,
            ref_order_type: None,
            ref_order_id: None,
            remark: req.remark.clone(),
        };
        let out_result = self
            .execute_stock_op(exec, ctx.operator_id, &out_req, -req.quantity, TransactionType::Transfer)
            .await?;

        // in
        let in_req = StockChangeReq {
            product_id: req.product_id,
            warehouse_id: req.to_warehouse_id,
            zone_id: req.to_zone_id,
            bin_id: req.to_bin_id,
            quantity: req.quantity,
            ref_order_type: None,
            ref_order_id: None,
            remark: req.remark,
        };
        let in_result = self
            .execute_stock_op(exec, ctx.operator_id, &in_req, req.quantity, TransactionType::Transfer)
            .await?;

        Ok((out_result, in_result))
    }

    async fn set_safety_stock(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        product_id: i64,
        bin_id: i64,
        safety_stock: Decimal,
    ) -> Result<()> {
        let Some((warehouse_id, zone_id, _)) =
            InventoryRepo::resolve_bin(&mut *db, bin_id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
        else {
            return Err(DomainError::not_found(format!("Bin#{bin_id}")));
        };

        StockLedgerRepo::set_safety_stock(
            &mut *db,
            product_id,
            warehouse_id,
            zone_id,
            bin_id,
            safety_stock,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))
    }

    // ── 读操作 ──

    async fn get_by_product(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        product_id: i64,
    ) -> Result<Vec<InventoryDetailView>> {
        let result = InventoryRepo::query_stock_details(
            &mut *db, Some(product_id), None, None, None, 1, 10000,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(result.items)
    }

    async fn get_by_bin(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        bin_id: i64,
    ) -> Result<Vec<InventoryDetailView>> {
        let result = InventoryRepo::query_stock_details(
            &mut *db, None, None, None, Some(bin_id), 1, 10000,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(result.items)
    }

    async fn list_low_stock(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
    ) -> Result<Vec<InventoryDetailView>> {
        InventoryRepo::list_low_stock(&mut *db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn query(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: InventoryQueryFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<InventoryDetailView>> {
        InventoryRepo::query_stock_details(
            &mut *db,
            filter.product_id,
            filter.keyword.as_deref(),
            filter.warehouse_id,
            filter.bin_id,
            page,
            page_size,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))
    }

    // ── 日志查询 ──

    async fn query_logs(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: TransactionLogFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<TransactionDetailView>> {
        InventoryRepo::query_transaction_details(&mut *db, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn list_logs_by_product(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        product_id: i64,
    ) -> Result<Vec<TransactionDetailView>> {
        InventoryRepo::list_txn_details_by_product(&mut *db, product_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn list_logs_by_bin(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        bin_id: i64,
    ) -> Result<Vec<TransactionDetailView>> {
        InventoryRepo::list_txn_details_by_bin(&mut *db, bin_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn list_logs_by_warehouse(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        warehouse_id: i64,
    ) -> Result<Vec<TransactionDetailView>> {
        InventoryRepo::list_txn_details_by_warehouse(&mut *db, warehouse_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
}

// ── 私有辅助：直接使用 repo 避免 ctx 被消费 ──

impl InventoryServiceImpl {
    /// 通用的库存操作流程：读 before → 记录事务 + 更新台账 → 读 after
    async fn execute_stock_op(
        &self,
        exec: &mut sqlx::postgres::PgConnection,
        operator_id: i64,
        req: &StockChangeReq,
        quantity: Decimal,
        txn_type: TransactionType,
    ) -> Result<StockOperationResult> {
        let result = self
            .record_and_update(exec, operator_id, req, quantity, txn_type)
            .await?;

        Ok(StockOperationResult {
            change_qty: quantity,
            ..result
        })
    }

    /// 核心流程：插入事务 + 更新台账，返回 before/after 状态
    async fn record_and_update(
        &self,
        exec: &mut sqlx::postgres::PgConnection,
        operator_id: i64,
        req: &StockChangeReq,
        quantity: Decimal,
        txn_type: TransactionType,
    ) -> Result<StockOperationResult> {
        // 1. 读 before
        let before = StockLedgerRepo::find_by_location(
            exec, req.product_id, req.warehouse_id, req.zone_id, req.bin_id, None,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        let before_qty = before.as_ref().map(|s| s.quantity).unwrap_or(Decimal::ZERO);

        // 2. 插入事务记录
        let txn_req = RecordTransactionReq {
            doc_number: None,
            transaction_type: txn_type,
            product_id: req.product_id,
            warehouse_id: req.warehouse_id,
            zone_id: Some(req.zone_id),
            bin_id: Some(req.bin_id),
            batch_no: None,
            quantity,
            unit_cost: None,
            source_type: req
                .ref_order_type
                .clone()
                .unwrap_or_else(|| "manual".to_string()),
            source_id: req
                .ref_order_id
                .as_ref()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0),
            remark: req.remark.clone(),
        };

        let txn = InventoryTransactionRepo::insert(exec, &txn_req, operator_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 3. 更新台账
        let upsert_req = UpsertStockReq {
            product_id: req.product_id,
            warehouse_id: req.warehouse_id,
            zone_id: req.zone_id,
            bin_id: req.bin_id,
            batch_no: None,
            qty_delta: quantity,
            unit_cost: None,
        };

        let updated = StockLedgerRepo::upsert(exec, &upsert_req)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(StockOperationResult {
            transaction_id: txn.id,
            stock_ledger_id: updated.id,
            product_id: req.product_id,
            warehouse_id: req.warehouse_id,
            zone_id: req.zone_id,
            bin_id: req.bin_id,
            before_qty,
            after_qty: updated.quantity,
            change_qty: quantity,
        })
    }
}
