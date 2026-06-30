use async_trait::async_trait;
use sqlx::postgres::PgPool;
use rust_decimal::Decimal;

use super::super::enums::*;
use super::model::*;
use super::repo::ProductionReceiptRepo;
use super::service::ProductionReceiptService;
use crate::mes::production_batch::repo::{ProductionBatchRepo, WorkOrderRoutingRepo};
use crate::mes::work_order::repo::WorkOrderRepo;
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
use crate::wms::stock_ledger::repo::StockLedgerRepo;

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

    async fn confirm(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        warehouse_id: i64,
        zone_id: Option<i64>,
        bin_id: Option<i64>,
    ) -> Result<()> {
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

        // 两步流程：仓库确认时必须指定目标仓库（生产侧 create 不填仓库）
        if warehouse_id <= 0 {
            return Err(DomainError::validation("确认入库必须指定目标仓库"));
        }

        // 1. QMS FQC 条件性门控 — 复用 get_fqc_status 统一门控语义（仅当工单工序含报检点时才要求 FQC）
        match self.get_fqc_status(ctx, db, id).await? {
            FqcGate::NotRequired | FqcGate::AllPassed => {}
            FqcGate::PendingInspection => {
                return Err(DomainError::BusinessRule(
                    "工单含报检工序，完工入库前必须完成 FQC 质检（无检验记录）".into(),
                ));
            }
            FqcGate::HasFailed => {
                return Err(DomainError::BusinessRule(
                    "FQC 质检未全部通过，不允许入库".to_string(),
                ));
            }
        }

        // 所有验证通过，标记为 Confirmed 防止部分失败后重试导致副作用重复执行
        ProductionReceiptRepo::update_status(&mut *db, id, ReceiptStatus::Confirmed)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 写入仓库确认的目标库位（create 时为空，confirm 时由仓管员指定）
        ProductionReceiptRepo::update_location(&mut *db, id, warehouse_id, zone_id, bin_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 解析产成品库存批次号：流转卡 batch_no 即产成品库存批次号
        // （批次语义统一：完工入库必须透传，保证库存台账可按流转卡/工单追溯产成品）
        let fg_batch_no: Option<String> = match receipt.batch_id {
            Some(bid) => ProductionBatchRepo::get_by_id(&mut *db, bid)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .map(|b| b.batch_no),
            None => None,
        };

        // 2. WMS inventory transaction — production receipt (入库)
        new_inventory_transaction_service(self.pool.clone())
            .record(
                ctx, db,
                RecordTransactionReq { doc_number: None, delivery_no: None, source_doc_number: Some(receipt.doc_number.clone()), transaction_type: TransactionType::ProductionReceipt, product_id: receipt.product_id,
                warehouse_id,
                zone_id,
                bin_id,
                batch_no: fg_batch_no,
                quantity: receipt.received_qty,
                unit_cost: None,
                source_type: "production_receipt".to_string(),
                source_id: id,
                remark: None, },
            )
            .await?;

        // 3. Cost entry — 从 stock_ledger 查最后已知单位成本
        let unit_cost = self.get_unit_cost(db, receipt.product_id).await.unwrap_or(Decimal::ZERO);
        let total_cost = receipt.received_qty * unit_cost;

        let period = chrono::Local::now().format("%Y-%m").to_string();
        if total_cost > Decimal::ZERO {
            new_cost_entry_service(self.pool.clone())
                .create_entries(
                    ctx, db,
                    vec![EntryRequest {
                        entity_type: CostEntityType::WorkOrder,
                        entity_id: receipt.work_order_id,
                        cost_type: CostType::Material,
                        debit_amount: total_cost,
                        credit_amount: total_cost,
                        cost_center: None,
                        profit_center: None,
                        period,
                        source_type: DocumentType::ProductionReceipt,
                        source_id: id,
                    }],
                )
                .await?;
        }

        // 4. Backflush — 纳入同一事务（修复：原来用独立连接，倒冲成功但后续失败无法回滚）
        // 传入完工入库单的仓库，倒冲从此仓扣减原料（修复：原 execute 取"系统第一个仓库"且 SQL 列名错误）
        new_backflush_service(self.pool.clone())
            .execute(ctx, db, receipt.work_order_id, receipt.received_qty, warehouse_id)
            .await
            .map_err(|e| {
                DomainError::BusinessRule(format!("倒冲失败，入库已回滚: {e:?}"))
            })?;

        ProductionReceiptRepo::set_backflush_triggered(&mut *db, id, true)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 5. 预留释放移至所有批次终态后（见下方 !has_active_batch 块）

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

            // 预留释放（仅当所有批次终态时；扁平化：已废弃 PlanItem 状态传播）
            new_inventory_reservation_service(self.pool.clone())
                .cancel_by_source(
                    ctx, db,
                    DocumentType::WorkOrder,
                    receipt.work_order_id,
                )
                .await?;
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

    async fn list_by_work_order(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<Vec<ReceiptListItem>> {
        ProductionReceiptRepo::list_by_work_order(&mut *db, work_order_id)
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

        let warehouse: Option<(String,)> = if let Some(wid) = receipt.warehouse_id {
            sqlx::query_as("SELECT name FROM warehouses WHERE id = $1")
                .bind(wid)
                .fetch_optional(&mut *db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
        } else {
            None
        };

        Ok(ReceiptDetailLookups {
            wo_doc_number: wo.map(|r| r.0),
            batch_no: batch.map(|r| r.0),
            product_name: product.map(|r| r.0),
            warehouse_name: warehouse.map(|r| r.0),
        })
    }

    async fn get_unit_cost(&self, db: PgExecutor<'_>, product_id: i64) -> Result<Decimal> {
        StockLedgerRepo::last_known_unit_cost(&mut *db, product_id).await
    }

    async fn get_fqc_status(&self, ctx: &ServiceContext, db: PgExecutor<'_>, receipt_id: i64) -> Result<FqcGate> {
        let receipt = ProductionReceiptRepo::get_by_id(&mut *db, receipt_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionReceipt"))?;

        let wo_routings = WorkOrderRoutingRepo::get_by_work_order_id(
            &mut *db, receipt.work_order_id,
        ).await.unwrap_or_default();
        let has_inspection_points = wo_routings.iter().any(|r| r.is_inspection_point);

        if !has_inspection_points {
            return Ok(FqcGate::NotRequired);
        }

        let fqc_results = new_inspection_result_service(self.pool.clone())
            .list_by_source(
                ctx, db,
                InspectionResultFilter {
                    source_type: Some(InspectionSourceType::ProductionReceipt),
                    source_id: Some(receipt_id),
                    ..Default::default()
                },
                PageParams { page: 1, page_size: 10000 },
            )
            .await
            .unwrap_or_else(|_| PaginatedResult {
                items: vec![], total: 0, page: 1, page_size: 10000, total_pages: 0,
            });

        if fqc_results.items.is_empty() {
            return Ok(FqcGate::PendingInspection);
        }

        let all_passed = fqc_results.items.iter().all(|r| {
            r.status == InspectionStatus::Completed && r.result == InspectionResultType::Pass
        });

        Ok(if all_passed { FqcGate::AllPassed } else { FqcGate::HasFailed })
    }
}
