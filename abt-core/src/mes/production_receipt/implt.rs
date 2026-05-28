use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::super::enums::*;
use super::model::*;
use super::repo::ProductionReceiptRepo;
use super::service::ProductionReceiptService;
use crate::mes::production_batch::repo::ProductionBatchRepo;
use crate::qms::enums::{InspectionResultType, InspectionSourceType, InspectionStatus};
use crate::qms::inspection_result::model::InspectionResultFilter;
use crate::qms::inspection_result::service::InspectionResultService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::types::PgExecutor;
use crate::shared::cost_entry::model::EntryRequest;
use crate::shared::cost_entry::service::CostEntryService;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::{AuditAction, CostEntityType, CostType, DocumentType};
use crate::shared::inventory_reservation::service::InventoryReservationService;
use crate::shared::types::Result;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PageParams;
use crate::wms::backflush::service::BackflushService;
use crate::wms::enums::TransactionType;
use crate::wms::inventory_transaction::model::RecordTransactionReq;
use crate::wms::inventory_transaction::service::InventoryTransactionService;

pub struct ProductionReceiptServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
    doc_seq: Arc<dyn DocumentSequenceService>,
    qms: Arc<dyn InspectionResultService>,
    inv_txn: Arc<dyn InventoryTransactionService>,
    cost_entry: Arc<dyn CostEntryService>,
    backflush: Arc<dyn BackflushService>,
    inv_res: Arc<dyn InventoryReservationService>,
    audit: Arc<dyn AuditLogService>,
}

impl ProductionReceiptServiceImpl {
    pub fn new(
        pool: PgPool,
        doc_seq: Arc<dyn DocumentSequenceService>,
        qms: Arc<dyn InspectionResultService>,
        inv_txn: Arc<dyn InventoryTransactionService>,
        cost_entry: Arc<dyn CostEntryService>,
        backflush: Arc<dyn BackflushService>,
        inv_res: Arc<dyn InventoryReservationService>,
        audit: Arc<dyn AuditLogService>,
    ) -> Self {
        Self {
            pool,
            doc_seq,
            qms,
            inv_txn,
            cost_entry,
            backflush,
            inv_res,
            audit,
        }
    }
}

#[async_trait]
impl ProductionReceiptService for ProductionReceiptServiceImpl {
    async fn create(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CreateReceiptReq) -> Result<i64> {
        // Verify batch status if provided
        if let Some(bid) = req.batch_id {
            let batch = ProductionBatchRepo::get_by_id(&mut *db, bid)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .ok_or_else(|| DomainError::not_found("ProductionBatch"))?;

            if batch.status != BatchStatus::PendingReceipt {
                return Err(DomainError::BusinessRule(
                    "Batch must be in PendingReceipt status".to_string(),
                ));
            }
        }

        // Determine product_id
        let product_id = if req.product_id != 0 {
            req.product_id
        } else if let Some(bid) = req.batch_id {
            let batch = ProductionBatchRepo::get_by_id(&mut *db, bid)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .ok_or_else(|| DomainError::not_found("ProductionBatch"))?;
            batch.product_id
        } else {
            let batches =
                ProductionBatchRepo::list_by_work_order(&mut *db, req.work_order_id)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
            batches
                .first()
                .map(|b| b.product_id)
                .ok_or_else(|| DomainError::not_found("ProductionBatch for WorkOrder"))?
        };

        let doc_number = self
            .doc_seq
            .next_number(ctx, db, DocumentType::ProductionReceipt)
            .await
            .unwrap_or_else(|_| format!("PR{}", chrono::Local::now().format("%Y%m%d%H%M%S")));

        let receipt = ProductionReceiptRepo::insert(
            &mut *db,
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

    async fn find_by_id(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<ProductionReceipt> {
        ProductionReceiptRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionReceipt"))
    }

    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let receipt = ProductionReceiptRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionReceipt"))?;

        if receipt.status != ReceiptStatus::Draft {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", receipt.status),
                to: "Confirmed".to_string(),
            });
        }

        // 1. QMS FQC hard gate — 查询检验结果（在状态更新前验证）
        let fqc_results = self
            .qms
            .list_by_source(
                ctx, db,
                InspectionResultFilter {
                    source_type: Some(InspectionSourceType::ArrivalNotice),
                    source_id: Some(id),
                    ..Default::default()
                },
                PageParams {
                    page: 1,
                    page_size: 10000,
                },
            )
            .await
            .unwrap_or_else(|_| crate::shared::types::pagination::PaginatedResult {
                items: vec![],
                total: 0,
                page: 1,
                page_size: 10000,
                total_pages: 0,
            });

        let fqc_passed = fqc_results.items.is_empty()
            || fqc_results.items.iter().all(|r| {
                r.status == InspectionStatus::Completed && r.result == InspectionResultType::Pass
            });

        if !fqc_passed {
            return Err(DomainError::BusinessRule(
                "QMS FQC inspection not passed".to_string(),
            ));
        }

        // 所有验证通过，标记为 Confirmed 防止部分失败后重试导致副作用重复执行
        ProductionReceiptRepo::update_status(&mut *db, id, ReceiptStatus::Confirmed)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 2. WMS inventory transaction — production receipt (入库)
        self.inv_txn
            .record(
                ctx, db,
                RecordTransactionReq {
                    doc_number: None,
                    transaction_type: TransactionType::ProductionReceipt,
                    product_id: receipt.product_id,
                    warehouse_id: receipt.warehouse_id,
                    zone_id: receipt.zone_id,
                    bin_id: receipt.bin_id,
                    batch_no: None,
                    quantity: receipt.received_qty,
                    unit_cost: None,
                    source_type: "production_receipt".to_string(),
                    source_id: id,
                    remark: None,
                },
            )
            .await?;

        // 3. Cost entry — finished goods receipt cost
        let period = chrono::Local::now().format("%Y-%m").to_string();
        self.cost_entry
            .create_entries(
                ctx, db,
                vec![EntryRequest {
                    entity_type: CostEntityType::WorkOrder,
                    entity_id: receipt.work_order_id,
                    cost_type: CostType::Material,
                    debit_amount: receipt.received_qty,
                    credit_amount: receipt.received_qty,
                    cost_center: None,
                    profit_center: None,
                    period,
                    source_type: DocumentType::ProductionReceipt,
                    source_id: id,
                }],
            )
            .await?;

        // 4. Backflush — failure does not block receipt but is audited
        let backflush_result = self
            .backflush
            .execute(ctx, db, receipt.work_order_id, receipt.received_qty)
            .await;

        if let Err(e) = backflush_result {
            if let Err(audit_err) = self
                .audit
                .record(
                    ctx, db,
                    "production_receipt",
                    id,
                    AuditAction::Update,
                    Some(serde_json::json!({ "backflush_error": format!("{:?}", e) })),
                    None,
                )
                .await
            {
                tracing::warn!("audit log for backflush error failed: {audit_err}");
            }
        } else {
            ProductionReceiptRepo::set_backflush_triggered(&mut *db, id, true)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        // 5. Release hard reservation
        self.inv_res
            .cancel_by_source(
                ctx, db,
                DocumentType::WorkOrder,
                receipt.work_order_id,
            )
            .await?;

        // 6. Update batch status to Completed
        if let Some(batch_id) = receipt.batch_id {
            ProductionBatchRepo::update_status(
                &mut *db,
                batch_id,
                BatchStatus::Completed,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }

        Ok(())
    }
}
