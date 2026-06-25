use async_trait::async_trait;
use sqlx::postgres::PgPool;
use sqlx::Row;
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};

use super::super::enums::{PlanItemStatus, RoutingStatus, WorkOrderStatus};
use super::model::*;
use super::repo::WorkOrderRepo;
use super::service::WorkOrderService;
use crate::mes::production_plan::repo::ProductionPlanRepo;
use crate::mes::production_receipt::{new_production_receipt_service, service::ProductionReceiptService};
use crate::mes::work_report::{new_work_report_service, service::WorkReportService};
use crate::master_data::bom::{new_bom_query_service, service::BomQueryService};
use crate::master_data::work_center::{new_work_center_service, service::WorkCenterService};
use crate::master_data::routing::{new_routing_service, service::RoutingService};
use crate::master_data::product::{new_product_service, service::ProductService};
use crate::wms::material_requisition::{new_material_requisition_service, service::MaterialRequisitionService};
use crate::wms::stock_ledger::{new_stock_ledger_service, service::StockLedgerService};
use crate::mes::production_batch::model::WorkOrderRouting;
use crate::mes::production_batch::{new_production_batch_service, service::ProductionBatchService};
use crate::mes::production_batch::repo::{
    BatchRoutingProgressRepo, ProductionBatchRepo, WorkOrderRoutingRepo,
};
use crate::shared::audit_log::{new_audit_log_service, model::AuditLogQuery, service::AuditLogService, RecordAuditLogReq};
use crate::shared::document_link::repo::DocumentLinkRepo;
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::event_bus::{new_domain_event_bus, service::DomainEventBus};
use crate::shared::inventory_reservation::repo::InventoryReservationRepo;
use crate::shared::types::PgExecutor;
use crate::shared::enums::{AuditAction, DocumentType};
use crate::shared::inventory_reservation::{new_inventory_reservation_service, service::InventoryReservationService};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;
use crate::shared::types::error::DomainError;

pub struct WorkOrderServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
}

impl WorkOrderServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WorkOrderService for WorkOrderServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateWorkOrderReq,
    ) -> Result<i64> {
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::WorkOrder)
            .await
            .unwrap_or_else(|_| format!("WO{}", chrono::Local::now().format("%Y%m%d%H%M%S")));

        let work_order = WorkOrderRepo::insert(
            &mut *db,
            &doc_number,
            &req,
            WorkOrderStatus::Draft,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        // 审计日志
        new_audit_log_service(self.pool.clone())
            .record(
                ctx, db,
                RecordAuditLogReq::new("WorkOrder", work_order.id, AuditAction::Create),
            )
            .await?;

        Ok(work_order.id)
    }

    async fn find_by_id(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<WorkOrder> {
        WorkOrderRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("WorkOrder"))
    }

    /// 下达工单：Draft/Planned -> Released
    /// - BOM 快照（冻结用料清单）
    /// - 从 Routing 创建工序（或虚拟默认工序）
    /// - 创建 ProductionBatch
    /// - backflush 模式：不预留、不创建领料单
    async fn release(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        expected_version: i32,
    ) -> Result<()> {
        // 1. 验证工单存在且状态允许下达
        let work_order = WorkOrderRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("WorkOrder"))?;

        if work_order.status != WorkOrderStatus::Draft
            && work_order.status != WorkOrderStatus::Planned
        {
            return Err(DomainError::InvalidStateTransition {
                from: work_order.status.to_string(),
                to: WorkOrderStatus::Released.to_string(),
            });
        }

        // 2. 乐观锁更新状态
        let updated =
            WorkOrderRepo::update_status_with_version(
                &mut *db,
                id,
                WorkOrderStatus::Released,
                expected_version,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if !updated {
            return Err(DomainError::ConcurrentConflict);
        }

        // 3. 获取产品信息（用于查找 BOM 和 Routing）
        let product = new_product_service(self.pool.clone())
            .get(ctx, db, work_order.product_id).await?;
        let product_code = &product.product_code;

        // 4. BOM 快照：查找产品已发布 BOM → 获取最新快照 → 写入 work_order.bom_snapshot_id
        let bom_snapshot_id = if let Some(bom_id) = new_bom_query_service(self.pool.clone())
            .find_published_bom_by_product_code(ctx, db, product_code)
            .await?
        {
            // 获取该 BOM 的最新快照
            let snapshots = new_bom_query_service(self.pool.clone())
                .get_snapshots(ctx, db, bom_id, None, Some(1))
                .await?;

            if let Some(latest_snapshot) = snapshots.into_iter().next() {
                WorkOrderRepo::update_bom_snapshot_id(&mut *db, id, latest_snapshot.snapshot_id)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                Some(latest_snapshot.snapshot_id)
            } else {
                None
            }
        } else {
            None
        };

        // 5. 工序创建：从 Routing 映射，或虚拟默认工序
        let routing_detail = new_routing_service(self.pool.clone())
            .get_bom_routing(ctx, db, product_code.to_string())
            .await?;

        // 工单已有工序（generate 时已初始化）则跳过；无则从 BOM 工艺路径初始化（兼容直接创建的旧路径）
        let existing_routings = WorkOrderRoutingRepo::get_by_work_order_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        if existing_routings.is_empty() {
            let routing_id_for_init = routing_detail.as_ref().map(|d| d.routing.id);
            new_production_batch_service(self.pool.clone())
                .init_routings_from_template(ctx, db, id, routing_id_for_init, work_order.planned_qty)
                .await?;
        }

        if let Some(ref detail) = routing_detail {
            WorkOrderRepo::update_routing_id(&mut *db, id, detail.routing.id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        // 6. 根据产品 material_consumption_mode 分流
        let consumption_mode = product.meta.material_consumption_mode;

        match consumption_mode {
            crate::master_data::product::model::MaterialConsumptionMode::Picking => {
                // picking 模式：HARD 预留组件 + 生成领料单明细行
                if let Some(snap_id) = bom_snapshot_id {
                    let snapshot_opt = new_bom_query_service(self.pool.clone())
                        .get_snapshot_by_id(ctx, db, snap_id).await?;

                    if let Some(snapshot) = snapshot_opt {
                        let leaf_nodes = snapshot.bom_detail.leaf_nodes();

                        if !leaf_nodes.is_empty() {
                            let warehouse_id = crate::wms::backflush::resolve_warehouse_id(db).await?;

                            // HARD 预留每个组件
                            let reserve_requests: Vec<crate::shared::inventory_reservation::ReserveRequest> =
                                leaf_nodes.iter().map(|node| {
                                    crate::shared::inventory_reservation::ReserveRequest {
                                        product_id: node.product_id,
                                        warehouse_id: Some(warehouse_id),
                                        reserved_qty: node.quantity * work_order.planned_qty,
                                        reservation_type: crate::shared::enums::ReservationType::Hard,
                                        source_type: DocumentType::WorkOrder,
                                        source_id: id,
                                        source_line_id: None,
                                        priority: 0,
                                        expires_at: None,
                                    }
                                }).collect();

                            let batch = new_inventory_reservation_service(self.pool.clone())
                                .reserve(ctx, db, reserve_requests)
                                .await?;
                            // 不静默丢弃失败项（与 confirm 同类修复）：缺货组件记 warn 但不阻断，
                            // 领料单仍创建（保持现状行为，仅消除静默）
                            if !batch.failed_items.is_empty() {
                                for f in &batch.failed_items {
                                    tracing::warn!(
                                        work_order_id = id,
                                        index = f.index,
                                        error = %f.error,
                                        "work order component reserve failed, requisition still created"
                                    );
                                }
                            }
                        }
                    }

                    // 创建领料单（含明细行）
                    new_material_requisition_service(self.pool.clone())
                        .create_for_work_order(ctx, db, id).await?;
                }
            }
            crate::master_data::product::model::MaterialConsumptionMode::Backflush => {
                // backflush 模式：不预留、不创建领料单
                // 倒冲在完工时按实际量自动扣减
            }
        }

        // 发布领域事件
        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx, db,
                crate::shared::event_bus::EventPublishRequest {
                    event_type: crate::shared::enums::event::DomainEventType::WOReleased,
                    aggregate_type: "WorkOrder".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({
                        "product_id": work_order.product_id,
                        "planned_qty": work_order.planned_qty,
                        "bom_snapshot_id": bom_snapshot_id,
                        "has_routing": routing_detail.is_some(),
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        // 审计日志
        new_audit_log_service(self.pool.clone())
            .record(
                ctx, db,
                RecordAuditLogReq::new("WorkOrder", id, AuditAction::Transition),
            )
            .await?;

        Ok(())
    }

    async fn mark_in_production(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let updated = WorkOrderRepo::update_status_conditional(
            &mut *db,
            id,
            WorkOrderStatus::Released,
            WorkOrderStatus::InProduction,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if updated {
            new_audit_log_service(self.pool.clone())
                .record(
                    ctx, db,
                    RecordAuditLogReq::new("WorkOrder", id, AuditAction::Transition),
                )
                .await?;
        }

        Ok(())
    }

    /// 反下达工单：Released -> Draft
    /// 安全网操作：仅在工单未开工时允许
    async fn unrelease(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        expected_version: i32,
    ) -> Result<()> {
        // 1. 加载工单，校验状态
        let work_order = WorkOrderRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("WorkOrder"))?;

        if work_order.status != WorkOrderStatus::Released {
            return Err(DomainError::BusinessRule(
                "只有已下达状态的工单才能反下达".to_string(),
            ));
        }

        // 2. 校验未开工 + 无报工记录
        let batches = ProductionBatchRepo::list_by_work_order(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let has_started = batches.iter().any(|b| b.current_step > 0);
        if has_started {
            return Err(DomainError::BusinessRule(
                "工单已开工，无法反下达".to_string(),
            ));
        }

        // 校验无报工记录（双重保险：即使 current_step=0 也可能有孤儿报工）
        let report_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM work_reports WHERE work_order_id = $1",
        )
        .bind(id)
        .fetch_one(&mut *db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if report_count > 0 {
            return Err(DomainError::BusinessRule(
                "工单已有报工记录，无法反下达".to_string(),
            ));
        }

        // 3. 取消关联的领料单（通过 document_links 双向查找）
        let requisition_ids = DocumentLinkRepo::find_linked_ids_by_type(
            &mut *db,
            DocumentType::WorkOrder,
            id,
            DocumentType::MaterialRequisition,
        )
        .await?;

        for req_id in requisition_ids {
            // 领料单取消失败（已取消/已完成）不阻断反下达主流程
            if let Err(_e) = new_material_requisition_service(self.pool.clone())
                .cancel(ctx, db, req_id).await
            {
                // 已取消或已完成的领料单会报错，继续执行
            }
        }

        // 4. 释放库存 HARD 预留（可能没有预留，忽略错误）
        if let Err(_e) = new_inventory_reservation_service(self.pool.clone())
            .cancel_by_source(ctx, db, DocumentType::WorkOrder, id).await
        {
            // backflush 模式无预留，忽略
        }

        // 5. 软删除 ProductionBatch（替代物理 DELETE）
        ProductionBatchRepo::soft_delete_by_work_order(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 6. 删除 batch_routing_progress（执行进度快照，反下达后重建）
        sqlx::query(
            "DELETE FROM batch_routing_progress WHERE batch_id IN (SELECT id FROM production_batches WHERE work_order_id = $1)",
        )
        .bind(id)
        .execute(&mut *db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 7. 删除 WorkOrderRouting（工序模板快照，反下达后可重建）
        sqlx::query("DELETE FROM work_order_routings WHERE work_order_id = $1")
            .bind(id)
            .execute(&mut *db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 7. 清除 bom_snapshot_id 和 routing_id（快照记录保留）
        sqlx::query(
            "UPDATE work_orders SET bom_snapshot_id = NULL, routing_id = NULL, updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .execute(&mut *db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 8. 工单状态 → Draft
        let updated = WorkOrderRepo::update_status_with_version(
            &mut *db,
            id,
            WorkOrderStatus::Draft,
            expected_version,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if !updated {
            return Err(DomainError::ConcurrentConflict);
        }

        // 9. 回滚关联 PlanItem 状态：Released → Planned
        if let Some(plan_item_id) = work_order.plan_item_id
            && let Err(_e) = sqlx::query(
                "UPDATE production_plan_items SET status = $2 WHERE id = $1 AND status IN ($3, $4)",
            )
            .bind(plan_item_id)
            .bind(PlanItemStatus::Planned)
            .bind(PlanItemStatus::Released)
            .bind(PlanItemStatus::InProduction)
            .execute(&mut *db)
            .await
        {
            // PlanItem 状态回滚失败不影响反下达主流程
        }

        // 10. 发布领域事件
        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx, db,
                crate::shared::event_bus::EventPublishRequest {
                    event_type: crate::shared::enums::event::DomainEventType::WOUnreleased,
                    aggregate_type: "WorkOrder".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({
                        "product_id": work_order.product_id,
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        // 11. 审计日志
        new_audit_log_service(self.pool.clone())
            .record(
                ctx, db,
                RecordAuditLogReq::new("WorkOrder", id, AuditAction::Transition),
            )
            .await?;

        Ok(())
    }

    /// 关闭工单：Released -> Closed
    async fn close(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        expected_version: i32,
    ) -> Result<()> {
        let work_order = WorkOrderRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("WorkOrder"))?;

        if work_order.status != WorkOrderStatus::Released
            && work_order.status != WorkOrderStatus::InProduction
        {
            return Err(DomainError::InvalidStateTransition {
                from: work_order.status.to_string(),
                to: WorkOrderStatus::Closed.to_string(),
            });
        }

        let batches = ProductionBatchRepo::list_by_work_order(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;


        if batches.is_empty() {
            return Err(DomainError::BusinessRule(
                "工单无生产批次，不能直接关闭。请先拆批并完成生产。".into(),
            ));
        }
        let has_incomplete = batches.iter().any(|b| {
            b.status != super::super::enums::BatchStatus::Completed
                && b.status != super::super::enums::BatchStatus::Cancelled
        });

        if has_incomplete {
            return Err(DomainError::BusinessRule(
                "All production batches must be completed before closing the work order"
                    .to_string(),
            ));
        }

        // 完工率校验（对标 Odoo button_mark_done: 校验完工量达标）
        let total_completed: Decimal = batches
            .iter()
            .filter(|b| b.status == super::super::enums::BatchStatus::Completed)
            .map(|b| b.completed_qty)
            .sum();
        if work_order.planned_qty > Decimal::ZERO {
            let completion_rate = total_completed / work_order.planned_qty;
            if completion_rate < Decimal::new(95, 2) {
                return Err(DomainError::BusinessRule(format!(
                    "完工率 {}% 低于 95%，无法关闭工单",
                    (completion_rate * Decimal::from(100)).round()
                )));
            }
        }

        let updated =
            WorkOrderRepo::update_status_with_version(
                &mut *db,
                id,
                WorkOrderStatus::Closed,
                expected_version,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if !updated {
            return Err(DomainError::ConcurrentConflict);
        }

        new_inventory_reservation_service(self.pool.clone())
            .cancel_by_source(ctx, db, DocumentType::WorkOrder, id).await?;

        // 审计日志
        new_audit_log_service(self.pool.clone())
            .record(
                ctx, db,
                RecordAuditLogReq::new("WorkOrder", id, AuditAction::Transition),
            )
            .await?;

        Ok(())
    }

    /// 取消工单：Draft/Planned/Released -> Cancelled
    async fn cancel(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        expected_version: i32,
    ) -> Result<()> {
        let work_order = WorkOrderRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("WorkOrder"))?;

        if work_order.status != WorkOrderStatus::Draft
            && work_order.status != WorkOrderStatus::Planned
            && work_order.status != WorkOrderStatus::Released
            && work_order.status != WorkOrderStatus::InProduction
        {
            return Err(DomainError::InvalidStateTransition {
                from: work_order.status.to_string(),
                to: WorkOrderStatus::Cancelled.to_string(),
            });
        }

        // 校验：已确认的完工入库记录阻止取消
        let receipt_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM production_receipts WHERE work_order_id = $1 AND deleted_at IS NULL AND status = 2",
        )
        .bind(id)
        .fetch_one(&mut *db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if receipt_count > 0 {
            return Err(DomainError::BusinessRule(format!(
                "工单已有 {} 张已确认的完工入库单，不能取消",
                receipt_count
            )));
        }

        let updated =
            WorkOrderRepo::update_status_with_version(
                &mut *db,
                id,
                WorkOrderStatus::Cancelled,
                expected_version,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if !updated {
            return Err(DomainError::ConcurrentConflict);
        }

        new_inventory_reservation_service(self.pool.clone())
            .cancel_by_source(ctx, db, DocumentType::WorkOrder, id).await?;
        WorkOrderRepo::soft_delete(&mut *db, id).await.map_err(|e| DomainError::Internal(e.into()))?;
        WorkOrderRepo::soft_delete_batches(&mut *db, id).await.map_err(|e| DomainError::Internal(e.into()))?;

        // 取消关联领料单（通过 document_links 双向查找，复用 unrelease 相同模式）
        let requisition_ids = DocumentLinkRepo::find_linked_ids_by_type(
            &mut *db,
            DocumentType::WorkOrder,
            id,
            DocumentType::MaterialRequisition,
        )
        .await?;

        for req_id in requisition_ids {
            if let Err(e) = new_material_requisition_service(self.pool.clone())
                .cancel(ctx, db, req_id)
                .await
            {
                tracing::warn!(req_id, error = %e, "领料单取消失败");
            }
        }
        // 审计日志
        new_audit_log_service(self.pool.clone())
            .record(
                ctx, db,
                RecordAuditLogReq::new("WorkOrder", id, AuditAction::Delete),
            )
            .await?;

        // 状态传播：PlanItem → Cancelled + 重新计算 Plan 状态
        ProductionPlanRepo::update_item_status_by_work_order(
            &mut *db,
            id,
            PlanItemStatus::Cancelled,
        ).await?;

        if let Some(plan_id) = ProductionPlanRepo::find_plan_id_by_work_order(
            &mut *db, id,
        ).await? {
            ProductionPlanRepo::recalculate_plan_status(
                &mut *db, plan_id,
            ).await?;
        }

        Ok(())
    }

    async fn list(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: WorkOrderFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<WorkOrder>> {
        WorkOrderRepo::list(&mut *db, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn get_product_name(&self, db: PgExecutor<'_>, product_id: i64) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT pdt_name FROM products WHERE product_id = $1",
        )
        .bind(product_id)
        .fetch_optional(&mut *db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(row.map(|r| r.0))
    }
    async fn list_by_plan(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        plan_id: i64,
    ) -> Result<Vec<WorkOrder>> {
        WorkOrderRepo::list_by_plan(&mut *db, plan_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    /// 工单工作台聚合视图（`get_hub_summary`）。
    ///
    /// 聚合顺序：find_by_id → product/work_center 名 → status_steps → source_chain
    /// → routing_matrix → reports → receipts → material(领料+availability) → info
    /// → audit_logs。
    ///
    /// material_availability 4 级算法对齐 Odoo `mrp_production.py:388-418`：
    ///   required = node.quantity × planned_qty
    ///   atp = available_atp(product_id, None)   // 判齐套严格用 ATP（双扣硬预留）
    ///   level = if atp≥required {Available}
    ///           else if atp+on_order_po≥required { 查 PO ETA ≤ scheduled_start → Expected; 否则 Late }
    ///           else {Unavailable}
    ///   整单 level = 最严重行；headline = 最严重行物料名。
    async fn get_hub_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<WorkOrderHubSummary> {
        // 0. 加载工单
        let order = WorkOrderRepo::get_by_id(&mut *db, work_order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("WorkOrder"))?;

        // 1. product / work_center 名称
        let product = new_product_service(self.pool.clone())
            .get(ctx, db, order.product_id)
            .await?;
        let product_name = product.pdt_name.clone();
        let work_center_name = if let Some(wc_id) = order.work_center_id {
            match new_work_center_service(self.pool.clone())
                .get(ctx, db, wc_id)
                .await
            {
                Ok(wc) => Some(wc.name),
                Err(_) => None,
            }
        } else {
            None
        };

        // 2. status_steps（4 步：草稿/已下达/生产中/已关闭；Cancelled 单独处理）
        let status_steps = build_status_steps(order.status);

        // 3. 批次 + 工序 + 工序进度（矩阵数据源）
        let batches = ProductionBatchRepo::list_by_work_order(&mut *db, work_order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let routings = WorkOrderRoutingRepo::get_by_work_order_id(&mut *db, work_order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let progress_rows = BatchRoutingProgressRepo::list_progress_by_work_order(
            &mut *db,
            work_order_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 4. 入库（用于 received_qty 聚合 + receipts disclosure）
        let receipts = new_production_receipt_service(self.pool.clone())
            .list_by_work_order(ctx, db, work_order_id)
            .await?;
        let received_qty: Decimal = receipts
            .iter()
            .filter(|r| r.status == crate::mes::enums::ReceiptStatus::Confirmed.as_i16())
            .map(|r| r.received_qty)
            .sum();

        // 5. source_chain
        let batch_count = batches.len() as i64;
        let source_chain = SourceChain {
            sales_order_doc: order.source_so_doc.clone(),
            customer_name: order.source_customer.clone(),
            plan_id: order.source_plan_id,
            plan_doc: order.source_plan_doc.clone(),
            batch_count,
            received_qty,
        };

        // 6. routing_matrix：routings 为列，每个批次为行
        let matrix = build_routing_matrix(&routings, &batches, &progress_rows);

        // 7. reports（聚合 total_completed/defect）
        let reports_raw = new_work_report_service(self.pool.clone())
            .list_by_work_order(ctx, db, work_order_id)
            .await?;
        let reports = build_reports(&reports_raw, &batches, &routings, db).await?;

        // 8. material：领料单 + items + availability 4 级
        let material = build_material(
            ctx,
            db,
            self.pool.clone(),
            work_order_id,
            order.bom_snapshot_id,
            order.planned_qty,
            order.scheduled_start,
            &product,
        )
        .await?;

        // 9. 进度数字（摘要带）
        let planned_qty = order.planned_qty;
        let completion_pct = if planned_qty > Decimal::ZERO {
            received_qty / planned_qty * Decimal::from(100)
        } else {
            Decimal::ZERO
        };
        let in_progress_qty = (planned_qty - order.completed_qty - order.scrap_qty).max(Decimal::ZERO);

        // 10. info disclosure
        let consumption_mode_label = match product.meta.material_consumption_mode {
            crate::master_data::product::model::MaterialConsumptionMode::Backflush => "倒冲".to_string(),
            crate::master_data::product::model::MaterialConsumptionMode::Picking => "领料".to_string(),
        };
        let routing_doc = if let Some(rid) = order.routing_id {
            new_routing_service(self.pool.clone())
                .get_detail(ctx, db, rid)
                .await
                .map(|d| d.routing.name)
                .ok()
        } else {
            None
        };
        let bom_snapshot_doc = order
            .bom_snapshot_id
            .map(|snap_id| format!("BOM-Snapshot#{}", snap_id));
        let info = HubInfo {
            bom_snapshot_doc,
            routing_doc,
            routing_step_count: routings.len(),
            consumption_mode_label,
            team_label: None,
        };

        // 11. receipts disclosure（fqc/backflush 聚合）
        let fqc_passed = receipts.iter().any(|r| r.status == 2);
        let backflush_done = receipts.iter().any(|r| r.status == 2);
        let receipt_items: Vec<HubReceiptRow> = receipts
            .iter()
            .map(|r| HubReceiptRow {
                doc_number: r.doc_number.clone(),
                batch_no: r.batch_id.map(|_| "—".to_string()).unwrap_or_default(),
                received_qty: r.received_qty,
                warehouse_name: r.warehouse_name.clone().unwrap_or_default(),
                fqc_label: if r.status == 2 { "通过".into() } else { "待检".into() },
                backflush_label: if r.status == 2 { "已倒冲".into() } else { "—".into() },
            })
            .collect();
        let receipts_block = HubReceipts {
            items: receipt_items,
            total_received: received_qty,
            fqc_passed,
            backflush_done,
        };

        // 12. audit_logs
        let logs = new_audit_log_service(self.pool.clone())
            .query_logs(
                ctx,
                db,
                AuditLogQuery {
                    entity_type: Some("WorkOrder".into()),
                    entity_id: Some(work_order_id),
                    ..Default::default()
                },
                1,
                50,
            )
            .await?;
        let audit_logs: Vec<HubAuditLog> = logs
            .items
            .iter()
            .map(|l| HubAuditLog {
                title: format!("{:?}", l.action),
                meta: l.created_at.to_rfc3339(),
                is_current: false,
            })
            .collect();

        Ok(WorkOrderHubSummary {
            order,
            product_name,
            work_center_name,
            status_steps,
            source_chain,
            material_availability: material.availability.clone(),
            completion_pct,
            received_qty,
            in_progress_qty,
            info,
            material,
            matrix,
            reports,
            receipts: receipts_block,
            audit_logs,
        })
    }

    /// 列表批量物料可用性（降级 2 级）。
    ///
    /// 逐工单取 BOM 快照叶子，`required = node.quantity × planned_qty`，
    /// `atp = available_atp(product_id, None)`。任一叶子 `atp < required` →
    /// `Unavailable`（headline = 该叶子物料名），否则 `Available`。
    /// 已关闭/取消工单：`Available` + None。工单数 = 分页规模（~20），
    /// 逐工单循环 acceptable；每工单内部 ATP 按叶子逐查。
    async fn compute_availability_batch(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_ids: &[i64],
    ) -> Result<HashMap<i64, (MaterialAvailabilityLevel, Option<String>)>> {
        let mut result: HashMap<i64, (MaterialAvailabilityLevel, Option<String>)> =
            HashMap::new();
        if work_order_ids.is_empty() {
            return Ok(result);
        }

        for &wo_id in work_order_ids {
            let order = match WorkOrderRepo::get_by_id(&mut *db, wo_id).await {
                Ok(Some(o)) => o,
                Ok(None) => continue, // 已被硬删除，跳过
                Err(e) => return Err(DomainError::Internal(e.into())),
            };

            // 已关闭/取消工单不计算（列表展示为空徽章）
            if matches!(order.status, WorkOrderStatus::Closed | WorkOrderStatus::Cancelled) {
                result.insert(wo_id, (MaterialAvailabilityLevel::Available, None));
                continue;
            }

            let snap_id = match order.bom_snapshot_id {
                Some(id) => id,
                None => {
                    result.insert(wo_id, (MaterialAvailabilityLevel::Available, None));
                    continue;
                }
            };
            let snapshot = new_bom_query_service(self.pool.clone())
                .get_snapshot_by_id(ctx, db, snap_id)
                .await?;
            let leaf_nodes = match snapshot.as_ref() {
                Some(snap) => snap.bom_detail.leaf_nodes(),
                None => {
                    result.insert(wo_id, (MaterialAvailabilityLevel::Available, None));
                    continue;
                }
            };
            if leaf_nodes.is_empty() {
                result.insert(wo_id, (MaterialAvailabilityLevel::Available, None));
                continue;
            }

            // 预取叶子物料名（headline 命中物料时用）
            let leaf_pids: Vec<i64> =
                leaf_nodes.iter().map(|n| n.product_id).collect();
            let leaf_products: HashMap<i64, crate::master_data::product::model::Product> =
                new_product_service(self.pool.clone())
                    .get_by_ids(ctx, db, leaf_pids.clone())
                    .await?
                    .into_iter()
                    .map(|p| (p.product_id, p))
                    .collect();

            // 逐叶子判定：首个 atp < required 即 Unavailable（headline=该物料名）
            let mut level = MaterialAvailabilityLevel::Available;
            let mut headline: Option<String> = None;
            for node in &leaf_nodes {
                let required = node.quantity * order.planned_qty;
                let atp =
                    InventoryReservationRepo::available_atp(&mut *db, node.product_id, None)
                        .await?;
                if atp < required {
                    level = MaterialAvailabilityLevel::Unavailable;
                    let name = leaf_products
                        .get(&node.product_id)
                        .map(|p| p.pdt_name.clone())
                        .or_else(|| node.product_code.clone())
                        .unwrap_or_else(|| format!("P{}", node.product_id));
                    headline = Some(name);
                    break; // 任一缺料即可定级，无需继续
                }
            }
            result.insert(wo_id, (level, headline));
        }

        Ok(result)
    }
}

// =============================================================================
// get_hub_summary 辅助函数
// =============================================================================

/// 构建 4 步状态条（草稿/已下达/生产中/已关闭）。
/// Cancelled 全 Pending（UI 顶部加取消标签）。
fn build_status_steps(status: WorkOrderStatus) -> Vec<StatusStep> {
    let steps_def = [
        ("draft", "草稿"),
        ("released", "已下达"),
        ("in_progress", "生产中"),
        ("closed", "已关闭"),
    ];
    let current_idx = match status {
        WorkOrderStatus::Draft | WorkOrderStatus::Planned => 0,
        WorkOrderStatus::Released => 1,
        WorkOrderStatus::InProduction => 2,
        WorkOrderStatus::Closed => 3,
        WorkOrderStatus::Cancelled => return steps_def
            .iter()
            .map(|(k, label)| StatusStep {
                key: k,
                label,
                state: StepState::Pending,
            })
            .collect(),
    };
    steps_def
        .iter()
        .enumerate()
        .map(|(idx, (k, label))| StatusStep {
            key: k,
            label,
            state: if idx < current_idx {
                StepState::Done
            } else if idx == current_idx {
                StepState::Active
            } else {
                StepState::Pending
            },
        })
        .collect()
}

/// 构建批次×工序矩阵：routings 为列（step_no 升序），每批次为行。
/// cell.status 按 RoutingStatus 映射：Done(Completed)/Active(InProgress)/Pending。
fn build_routing_matrix(
    routings: &[WorkOrderRouting],
    batches: &[crate::mes::production_batch::model::ProductionBatch],
    progress_rows: &[crate::mes::production_batch::model::BatchRoutingProgress],
) -> HubRoutingMatrix {
    let rows = batches
        .iter()
        .map(|batch| {
            let cells: Vec<RoutingMatrixCell> = routings
                .iter()
                .map(|r| {
                    let prog = progress_rows
                        .iter()
                        .find(|p| p.batch_id == batch.id && p.routing_id == r.id);
                    let status = match prog.map(|p| p.status) {
                        Some(RoutingStatus::Completed) => RoutingCellStatus::Done,
                        Some(RoutingStatus::InProgress) => RoutingCellStatus::Active,
                        Some(RoutingStatus::Skipped) => RoutingCellStatus::Done,
                        _ => RoutingCellStatus::Pending,
                    };
                    RoutingMatrixCell {
                        step_no: r.step_no,
                        status,
                        completed_qty: prog.map(|p| p.completed_qty).unwrap_or_default(),
                        defect_qty: prog.map(|p| p.defect_qty).unwrap_or_default(),
                        planned_qty: r.planned_qty,
                    }
                })
                .collect();
            RoutingMatrixRow {
                batch: batch.clone(),
                cells,
            }
        })
        .collect();
    HubRoutingMatrix {
        routings: routings.to_vec(),
        rows,
    }
}

/// 构建报工 disclosure：转 HubReportRow + 聚合 total_completed/defect。
/// batch_no / op_name / worker_name 通过 batch_id→batch_no、routing_id→process_name、
/// worker_id→users.display_name 批量查表解析。
async fn build_reports(
    reports: &[crate::mes::work_report::model::WorkReport],
    batches: &[crate::mes::production_batch::model::ProductionBatch],
    routings: &[WorkOrderRouting],
    db: &mut sqlx::postgres::PgConnection,
) -> Result<HubReports> {
    // 收集 worker_id 批量查 display_name
    let worker_ids: Vec<i64> = reports.iter().map(|r| r.worker_id).collect::<HashSet<_>>().into_iter().collect();
    let worker_names = if worker_ids.is_empty() {
        HashMap::new()
    } else {
        let rows = sqlx::query(
            "SELECT user_id, display_name FROM users WHERE user_id = ANY($1)",
        )
        .bind(&worker_ids)
        .fetch_all(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let mut m: HashMap<i64, String> = HashMap::new();
        for row in rows {
            let uid: i64 = row.try_get("user_id")
                .map_err(|e| DomainError::Internal(e.into()))?;
            let name: String = row.try_get("display_name")
                .map_err(|e| DomainError::Internal(e.into()))?;
            m.insert(uid, name);
        }
        m
    };

    let items: Vec<HubReportRow> = reports
        .iter()
        .map(|r| {
            let batch_no = batches
                .iter()
                .find(|b| b.id == r.batch_id)
                .map(|b| b.batch_no.clone())
                .unwrap_or_default();
            let op_name = routings
                .iter()
                .find(|wor| wor.id == r.routing_id)
                .map(|wor| wor.process_name.clone())
                .unwrap_or_default();
            let worker_name = worker_names
                .get(&r.worker_id)
                .cloned()
                .unwrap_or_default();
            HubReportRow {
                report_date: r.report_date,
                batch_no,
                op_name,
                completed_qty: r.completed_qty,
                defect_qty: r.defect_qty,
                worker_name,
                team_label: None,
            }
        })
        .collect();
    let total_completed: Decimal = reports.iter().map(|r| r.completed_qty).sum();
    let total_defect: Decimal = reports.iter().map(|r| r.defect_qty).sum();
    Ok(HubReports {
        total_count: items.len(),
        total_completed,
        total_defect,
        items,
    })
}

/// 构建物料 disclosure：领料单 + items + availability 4 级算法。
async fn build_material(
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    pool: sqlx::PgPool,
    work_order_id: i64,
    bom_snapshot_id: Option<i64>,
    planned_qty: Decimal,
    scheduled_start: chrono::NaiveDate,
    product: &crate::master_data::product::model::Product,
) -> Result<HubMaterial> {
    let _ = product; // 成品信息已在调用方用于 info disclosure；availability 基于 BOM 叶子
    use std::collections::HashSet;

    // 1. 领料单
    let reqs = new_material_requisition_service(pool.clone())
        .list(
            ctx,
            db,
            crate::wms::material_requisition::model::RequisitionFilter {
                work_order_id: Some(work_order_id),
                ..Default::default()
            },
            1,
            1000,
        )
        .await?;
    let mut hub_reqs: Vec<HubRequisition> = Vec::new();
    // 领料单明细所需 product_id 集合（用于 product_code/name 解析）
    let mut line_product_ids: HashSet<i64> = HashSet::new();
    let mut req_item_map: std::collections::HashMap<i64, Vec<crate::wms::material_requisition::model::MaterialReqItem>> =
        std::collections::HashMap::new();
    for req in &reqs.items {
        let items = new_material_requisition_service(pool.clone())
            .list_items(ctx, db, req.id)
            .await?;
        for it in &items {
            line_product_ids.insert(it.product_id);
        }
        req_item_map.insert(req.id, items);
    }
    // 批量查 line product 名
    let line_products: HashMap<i64, crate::master_data::product::model::Product> = if line_product_ids.is_empty() {
        HashMap::new()
    } else {
        let ids: Vec<i64> = line_product_ids.into_iter().collect();
        new_product_service(pool.clone())
            .get_by_ids(ctx, db, ids)
            .await?
            .into_iter()
            .map(|p| (p.product_id, p))
            .collect()
    };

    // 预取每行物料的 ATP（available_atp 为 async，不能在同步 map 内调用）
    let mut line_atp: HashMap<i64, Decimal> = HashMap::new();
    for &pid in line_products.keys() {
        let atp = InventoryReservationRepo::available_atp(&mut *db, pid, None).await?;
        line_atp.insert(pid, atp);
    }

    for req in &reqs.items {
        let items = req_item_map.get(&req.id).cloned().unwrap_or_default();
        let total_qty: Decimal = items.iter().map(|i| i.requested_qty).sum();
        let hub_items: Vec<HubRequisitionItem> = items
            .iter()
            .map(|i| {
                let p = line_products.get(&i.product_id);
                let (code, name) = match p {
                    Some(p) => (p.product_code.clone(), p.pdt_name.clone()),
                    None => (format!("P{}", i.product_id), "—".to_string()),
                };
                let avail = *line_atp.get(&i.product_id).unwrap_or(&Decimal::ZERO);
                HubRequisitionItem {
                    product_code: code,
                    product_name: name,
                    required_qty: i.requested_qty,
                    issued_qty: i.issued_qty,
                    available_qty: avail,
                }
            })
            .collect();
        let item_count = hub_items.len() as i64;
        let status_label = format!("{:?}", req.status);
        hub_reqs.push(HubRequisition {
            doc_number: req.doc_number.clone(),
            status_label,
            item_count,
            total_qty,
            items: hub_items,
        });
    }

    // 2. availability 4 级算法（对齐 Odoo）
    let availability = build_availability(
        ctx,
        db,
        pool.clone(),
        bom_snapshot_id,
        planned_qty,
        scheduled_start,
        &line_products,
    )
    .await?;

    Ok(HubMaterial {
        requisitions: hub_reqs,
        availability,
    })
}

/// 物料可用性 4 级算法（对齐 Odoo mrp_production.py:388-418）。
/// 无快照 → Available + 空行。
async fn build_availability(
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    pool: sqlx::PgPool,
    bom_snapshot_id: Option<i64>,
    planned_qty: Decimal,
    scheduled_start: chrono::NaiveDate,
    line_products: &HashMap<i64, crate::master_data::product::model::Product>,
) -> Result<MaterialAvailability> {
    // 无快照 → 整单 Available，空 lines
    let snapshot = match bom_snapshot_id {
        Some(snap_id) => new_bom_query_service(pool.clone())
            .get_snapshot_by_id(ctx, db, snap_id)
            .await?,
        None => None,
    };
    let leaf_nodes = match snapshot.as_ref() {
        Some(snap) => snap.bom_detail.leaf_nodes(),
        None => {
            return Ok(MaterialAvailability {
                level: MaterialAvailabilityLevel::Available,
                headline: None,
                lines: Vec::new(),
            });
        }
    };

    if leaf_nodes.is_empty() {
        return Ok(MaterialAvailability {
            level: MaterialAvailabilityLevel::Available,
            headline: None,
            lines: Vec::new(),
        });
    }

    let product_ids: Vec<i64> = leaf_nodes.iter().map(|n| n.product_id).collect();

    // ATP：逐个查（available_atp 已聚合双扣预留；为正确性优先，单工单规模可接受）
    let mut atp_map: HashMap<i64, Decimal> = HashMap::new();
    for &pid in &product_ids {
        let atp = InventoryReservationRepo::available_atp(&mut *db, pid, None).await?;
        atp_map.insert(pid, atp);
    }

    // projected（on_order_po 用于 Expected/Late 判定的补充量）
    let projected_map = new_stock_ledger_service(pool.clone())
        .query_projected_qty_batch(ctx, db, &product_ids, None)
        .await?;

    // PO ETA 批量：取 MAX(expected_delivery_date) WHERE product_id=ANY(...) AND po.status IN(2,3) AND date > CURRENT_DATE
    let po_eta_map = query_po_eta_batch(&mut *db, &product_ids).await?;

    // 逐行定级
    let mut lines: Vec<MaterialAvailabilityLine> = Vec::new();
    let mut overall_level = MaterialAvailabilityLevel::Available;
    let mut headline: Option<String> = None;
    let mut headline_severity = 0i32; // 跟踪最严重行的严重度

    for node in &leaf_nodes {
        let required = node.quantity * planned_qty;
        let atp = *atp_map.get(&node.product_id).unwrap_or(&Decimal::ZERO);
        let on_order_po = projected_map
            .get(&node.product_id)
            .map(|p| p.on_order_po)
            .unwrap_or(Decimal::ZERO);
        let projected = projected_map
            .get(&node.product_id)
            .map(|p| p.projected)
            .unwrap_or(Decimal::ZERO);

        let level = if atp >= required {
            MaterialAvailabilityLevel::Available
        } else if atp + on_order_po >= required {
            // 在途量补得齐缺口：查 MAX(expected_delivery_date) 判定 Expected / Late。
            // po_eta_map 仅保留未过期（> CURRENT_DATE）的 ETA；None 表示在途 PO 均已过期但货未到 → Late。
            match po_eta_map.get(&node.product_id) {
                Some(eta) if *eta <= scheduled_start => MaterialAvailabilityLevel::Expected,
                Some(_) => MaterialAvailabilityLevel::Late,
                None => MaterialAvailabilityLevel::Late,
            }
        } else {
            MaterialAvailabilityLevel::Unavailable
        };

        let (code, name) = match line_products.get(&node.product_id) {
            Some(p) => (p.product_code.clone(), p.pdt_name.clone()),
            None => (
                node.product_code.clone().unwrap_or_else(|| format!("P{}", node.product_id)),
                format!("P{}", node.product_id),
            ),
        };

        let severity = severity_rank(level);
        if severity > headline_severity {
            headline_severity = severity;
            headline = Some(name.clone());
        }
        overall_level = worse_level(overall_level, level);

        lines.push(MaterialAvailabilityLine {
            product_id: node.product_id,
            product_code: code,
            product_name: name,
            required_qty: required,
            issued_qty: Decimal::ZERO,
            atp,
            projected,
            level,
        });
    }

    Ok(MaterialAvailability {
        level: overall_level,
        headline,
        lines,
    })
}

/// PO ETA 批量查询：MAX(expected_delivery_date) GROUP BY product_id，
/// 仅取 status IN(2,3) 且 expected_delivery_date > CURRENT_DATE 的在途 PO。
async fn query_po_eta_batch(
    db: &mut sqlx::postgres::PgConnection,
    product_ids: &[i64],
) -> Result<HashMap<i64, chrono::NaiveDate>> {
    if product_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query(
        r#"
        SELECT poi.product_id, MAX(poi.expected_delivery_date) AS eta
        FROM purchase_order_items poi
        JOIN purchase_orders po ON po.id = poi.order_id
        WHERE poi.product_id = ANY($1)
          AND po.status IN (2, 3)
          AND poi.expected_delivery_date IS NOT NULL
          AND poi.expected_delivery_date > CURRENT_DATE
          AND po.deleted_at IS NULL
        GROUP BY poi.product_id
        "#,
    )
    .bind(product_ids)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let mut map: HashMap<i64, chrono::NaiveDate> = HashMap::new();
    for row in rows {
        let pid: i64 = row
            .try_get("product_id")
            .map_err(|e| DomainError::Internal(e.into()))?;
        let eta: chrono::NaiveDate = row
            .try_get("eta")
            .map_err(|e| DomainError::Internal(e.into()))?;
        map.insert(pid, eta);
    }
    Ok(map)
}

/// 严重度排序：Available=0 < Expected=1 < Late=2 < Unavailable=3
fn severity_rank(level: MaterialAvailabilityLevel) -> i32 {
    match level {
        MaterialAvailabilityLevel::Available => 0,
        MaterialAvailabilityLevel::Expected => 1,
        MaterialAvailabilityLevel::Late => 2,
        MaterialAvailabilityLevel::Unavailable => 3,
    }
}

/// 取两个 level 中更严重者
fn worse_level(a: MaterialAvailabilityLevel, b: MaterialAvailabilityLevel) -> MaterialAvailabilityLevel {
    if severity_rank(b) > severity_rank(a) {
        b
    } else {
        a
    }
}
