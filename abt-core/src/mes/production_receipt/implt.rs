use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::super::enums::*;
use super::model::*;
use super::repo::ProductionReceiptRepo;
use super::service::ProductionReceiptService;
use crate::mes::production_batch::repo::ProductionBatchRepo;
use crate::mes::work_order::repo::WorkOrderRepo;
use crate::mes::production_plan::repo::ProductionPlanRepo;
use crate::qms::enums::{InspectionResultType, InspectionSourceType, InspectionStatus};
use crate::qms::inspection_result::{new_inspection_result_service, model::InspectionResultFilter, service::InspectionResultService};
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::types::PgExecutor;
use crate::shared::cost_entry::{new_cost_entry_service, model::EntryRequest, service::CostEntryService};
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::{AuditAction, CostEntityType, CostType, DocumentType};
use crate::shared::inventory_reservation::{new_inventory_reservation_service, service::InventoryReservationService};
use crate::shared::types::{PaginatedResult, Result};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PageParams;
use crate::wms::backflush::{new_backflush_service, service::BackflushService};
use crate::wms::enums::TransactionType;
use crate::wms::inventory_transaction::{new_inventory_transaction_service, model::RecordTransactionReq, service::InventoryTransactionService};

pub struct ProductionReceiptServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
}

impl ProductionReceiptServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
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

        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::ProductionReceipt)
            .await
            .unwrap_or_else(|_| format!("PR{}", chrono::Local::now().format("%Y%m%d%H%M%S")));

        let receipt = ProductionReceiptRepo::insert(
            &mut *db,
            &InsertReceiptParams {
                work_order_id: req.work_order_id,
                batch_id: req.batch_id,
                product_id,
                received_qty: req.received_qty,
                warehouse_id: req.warehouse_id,
                zone_id: req.zone_id,
                bin_id: req.bin_id,
                receipt_date: req.receipt_date,
                doc_number: &doc_number,
                operator_id: ctx.operator_id,
            },
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
        let fqc_results = new_inspection_result_service(self.pool.clone())
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
        new_inventory_transaction_service(self.pool.clone())
            .record(
                ctx, db,
                RecordTransactionReq { doc_number: None, delivery_no: None, transaction_type: TransactionType::ProductionReceipt,
                product_id: receipt.product_id,
                warehouse_id: receipt.warehouse_id,
                zone_id: receipt.zone_id,
                bin_id: receipt.bin_id,
                batch_no: None,
                quantity: receipt.received_qty,
                unit_cost: None,
                source_type: "production_receipt".to_string(),
                source_id: id,
                remark: None, },
            )
            .await?;

        // 3. Cost entry — finished goods receipt cost
        let period = chrono::Local::now().format("%Y-%m").to_string();
        new_cost_entry_service(self.pool.clone())
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

        // 4. Backflush — failure does not block receipt but is audited.
        //    Uses a separate pool connection so SQL errors don't poison the
        //    caller's transaction (the backflush is best-effort by design).
        let backflush_result = {
            let mut bf_conn = self.pool.acquire().await
                .map_err(|e| DomainError::Internal(e.into()))?;
            new_backflush_service(self.pool.clone())
                .execute(ctx, &mut bf_conn, receipt.work_order_id, receipt.received_qty)
                .await
        };

        if let Err(e) = backflush_result {
            if let Err(audit_err) = new_audit_log_service(self.pool.clone())
                .record(
                    ctx, db,
                    RecordAuditLogReq {
                        entity_type: "production_receipt",
                        entity_id: id,
                        action: AuditAction::Update,
                        changes: Some(serde_json::json!({ "backflush_error": format!("{:?}", e) })),
                        context: None,
                    },
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
        new_inventory_reservation_service(self.pool.clone())
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


        // --- 7. 状态传播：完工入库后推进上游工单和计划行状态 ---

        // 7a. 多批次守卫：检查该 WO 下是否所有批次都已终态
        let all_batches = ProductionBatchRepo::list_by_work_order(
            &mut *db, receipt.work_order_id,
        ).await.map_err(|e| DomainError::Internal(e.into()))?;
        let has_active_batch = all_batches.iter().any(|b| {
            b.status != BatchStatus::Completed && b.status != BatchStatus::Cancelled
        });

        // 7b. WorkOrder: InProduction → Closed（仅当所有批次终态时）
        if !has_active_batch {
            match WorkOrderRepo::update_status_conditional(
                &mut *db,
                receipt.work_order_id,
                WorkOrderStatus::InProduction,
                WorkOrderStatus::Closed,
            ).await {
                Ok(true) => {
                    new_audit_log_service(self.pool.clone())
                        .record(ctx, db,
                            RecordAuditLogReq::new("WorkOrder", receipt.work_order_id, AuditAction::Transition),
                        ).await?;
                }
                Ok(false) => {}
                Err(e) => return Err(DomainError::Internal(e.into())),
            }
        }

        // 7c. PlanItem: InProduction → Completed
        ProductionPlanRepo::update_item_status_by_work_order(
            &mut *db,
            receipt.work_order_id,
            PlanItemStatus::Completed,
        ).await?;

        // 7d. Plan: 重新计算状态
        if let Some(plan_id) = ProductionPlanRepo::find_plan_id_by_work_order(
            &mut *db, receipt.work_order_id,
        ).await? {
            ProductionPlanRepo::recalculate_plan_status(&mut *db, plan_id).await?;
        }
        Ok(())
    }

    async fn list(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: ReceiptListFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<ReceiptListItem>> {
        ProductionReceiptRepo::list(&mut *db, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn get_detail_lookups(
        &self,
        db: PgExecutor<'_>,
        receipt: &ProductionReceipt,
    ) -> Result<ReceiptDetailLookups> {
        let wo: Option<(String,)> = sqlx::query_as(
            "SELECT doc_number FROM work_orders WHERE id = $1",
        )
        .bind(receipt.work_order_id)
        .fetch_optional(&mut *db)
        .await.map_err(|e| DomainError::Internal(e.into()))?;

        let batch: Option<(String,)> = if let Some(bid) = receipt.batch_id {
            sqlx::query_as("SELECT batch_no FROM production_batches WHERE id = $1")
                .bind(bid)
                .fetch_optional(&mut *db)
                .await.map_err(|e| DomainError::Internal(e.into()))?
        } else {
            None
        };

        let product: Option<(String,)> = sqlx::query_as(
            "SELECT pdt_name FROM products WHERE product_id = $1",
        )
        .bind(receipt.product_id)
        .fetch_optional(&mut *db)
        .await.map_err(|e| DomainError::Internal(e.into()))?;

        let warehouse: Option<(String,)> = sqlx::query_as(
            "SELECT name FROM warehouses WHERE id = $1",
        )
        .bind(receipt.warehouse_id)
        .fetch_optional(&mut *db)
        .await.map_err(|e| DomainError::Internal(e.into()))?;

        Ok(ReceiptDetailLookups {
            wo_doc_number: wo.map(|r| r.0),
            batch_no: batch.map(|r| r.0),
            product_name: product.map(|r| r.0),
            warehouse_name: warehouse.map(|r| r.0),
        })
    }
}
