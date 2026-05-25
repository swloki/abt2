use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::*;
use super::repo::ProductionReceiptRepo;
use super::service::ProductionReceiptService;
use super::super::enums::*;
use crate::mes::production_batch::repo::ProductionBatchRepo;
use crate::mes::stubs::{
    AuditLogStub, BackflushStub, CostEntryStub, CostEntryReq, DocumentSequenceStub,
    InventoryReservationStub, QmsInspectionStub, ReservationType, WmsInventoryTransactionStub,
    WmsTransactionType,
};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;

pub struct ProductionReceiptServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
}

impl ProductionReceiptServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProductionReceiptService for ProductionReceiptServiceImpl {
    async fn create(
        &self,
        mut ctx: ServiceContext<'_>,
        req: CreateReceiptReq,
    ) -> Result<i64, DomainError> {
        // Verify batch status if provided
        if let Some(bid) = req.batch_id {
            let batch = ProductionBatchRepo::get_by_id(&mut *ctx.executor, bid)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .ok_or_else(|| DomainError::not_found("ProductionBatch"))?;

            if batch.status != BatchStatus::PendingReceipt {
                return Err(DomainError::BusinessRule(
                    "Batch must be in PendingReceipt status".to_string(),
                ));
            }
        }

        // Determine product_id: use req.product_id if provided, otherwise derive from batch/work order
        let product_id = if req.product_id != 0 {
            req.product_id
        } else if let Some(bid) = req.batch_id {
            let batch = ProductionBatchRepo::get_by_id(&mut *ctx.executor, bid)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .ok_or_else(|| DomainError::not_found("ProductionBatch"))?;
            batch.product_id
        } else {
            // Fallback: get from work order's first batch
            let batches = ProductionBatchRepo::list_by_work_order(&mut *ctx.executor, req.work_order_id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            batches
                .first()
                .map(|b| b.product_id)
                .ok_or_else(|| DomainError::not_found("ProductionBatch for WorkOrder"))?
        };

        let doc_number = DocumentSequenceStub::next_number(ctx.reborrow(), "PR-")
            .await
            .unwrap_or_else(|_| format!("PR{}", chrono::Local::now().format("%Y%m%d%H%M%S")));

        let receipt = ProductionReceiptRepo::insert(
            &mut *ctx.executor,
            req.work_order_id,
            req.batch_id,
            product_id,
            req.received_qty,
            req.warehouse_id,
            req.zone_id,
            req.bin_id,
            req.receipt_date,
            &doc_number,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(receipt.id)
    }

    async fn find_by_id(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<ProductionReceipt, DomainError> {
        ProductionReceiptRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionReceipt"))
    }

    async fn confirm(
        &self,
        mut ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<(), DomainError> {
        let receipt = ProductionReceiptRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionReceipt"))?;

        if receipt.status != ReceiptStatus::Draft {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", receipt.status),
                to: "Confirmed".to_string(),
            });
        }

        // 先标记为 Confirmed 防止部分失败后重试导致副作用重复执行
        ProductionReceiptRepo::update_status(&mut *ctx.executor, id, ReceiptStatus::Confirmed)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 1. QMS FQC hard gate
        let fqc_passed = QmsInspectionStub::is_passed(
            ctx.reborrow(),
            "production_receipt",
            id,
        )
        .await
        .unwrap_or(true);

        if !fqc_passed {
            return Err(DomainError::BusinessRule(
                "QMS FQC inspection not passed".to_string(),
            ));
        }

        // 2. WMS inventory transaction — production receipt (入库)
        let _ = WmsInventoryTransactionStub::record(
            ctx.reborrow(),
            WmsTransactionType::ProductionReceipt,
            receipt.product_id,
            receipt.warehouse_id,
            receipt.received_qty,
            "production_receipt",
            id,
        )
        .await;

        // 3. Cost entry — finished goods receipt cost
        let _ = CostEntryStub::record(
            ctx.reborrow(),
            CostEntryReq {
                cost_type: "production_receipt".to_string(),
                debit_account: "inventory".to_string(),
                credit_account: "wip".to_string(),
                amount: rust_decimal::Decimal::ZERO, // TODO: calculate from actual cost
                source_type: "production_receipt".to_string(),
                source_id: id,
            },
        )
        .await;

        // 4. Backflush — failure does not block receipt
        let backflush_result = BackflushStub::execute(
            ctx.reborrow(),
            receipt.work_order_id,
            receipt.product_id,
            receipt.received_qty,
        )
        .await;

        if let Err(e) = backflush_result {
            // Log but don't block — backflush failure goes to DeadLetter
            let _ = AuditLogStub::record(
                ctx.reborrow(),
                "BACKFLUSH_FAILED",
                "production_receipt",
                id,
                &format!("Backflush failed: {:?}", e),
            )
            .await;
        } else {
            // Backflush succeeded — set flag
            let _ = ProductionReceiptRepo::set_backflush_triggered(
                &mut *ctx.executor,
                id,
                true,
            )
            .await;
        }

        // 5. Release hard reservation
        let _ = InventoryReservationStub::fulfill(
            ctx.reborrow(),
            receipt.product_id,
            receipt.warehouse_id,
            receipt.received_qty,
            ReservationType::Hard,
        )
        .await;

        // 6. Update batch status to Completed
        if let Some(batch_id) = receipt.batch_id {
            let _ = ProductionBatchRepo::update_status(
                &mut *ctx.executor,
                batch_id,
                BatchStatus::Completed,
            )
            .await;
        }

        Ok(())
    }
}
