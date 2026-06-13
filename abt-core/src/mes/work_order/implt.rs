use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::super::enums::{PlanItemStatus, WorkOrderStatus};
use super::model::*;
use super::repo::WorkOrderRepo;
use super::service::WorkOrderService;
use crate::mes::production_plan::repo::ProductionPlanRepo;
use crate::master_data::bom::{new_bom_query_service, service::BomQueryService};
use crate::master_data::routing::{new_routing_service, service::RoutingService};
use crate::master_data::product::{new_product_service, service::ProductService};
use crate::wms::material_requisition::{new_material_requisition_service, service::MaterialRequisitionService};
use crate::mes::production_batch::model::WorkOrderRouting;
use crate::mes::production_batch::repo::{ProductionBatchRepo, WorkOrderRoutingRepo};
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::event_bus::{new_domain_event_bus, service::DomainEventBus};
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

        // 先确定每道工序的 (step_no, process_name)：有工艺则映射各工序，否则用单道虚拟默认工序
        let steps: Vec<(i32, String)> = match routing_detail.as_ref() {
            Some(detail) => detail.steps.iter().map(|step| (
                step.step_order,
                step.process_name.clone().unwrap_or_else(|| step.process_code.clone()),
            )).collect(),
            None => vec![(1, "生产".to_string())],
        };

        let routing_steps: Vec<WorkOrderRouting> = steps
            .into_iter()
            .map(|(step_no, process_name)| WorkOrderRouting {
                id: 0,
                work_order_id: id,
                step_no,
                process_name,
                work_center_id: None,
                standard_time: None,
                standard_cost: None,
                unit_price: None,
                allowed_loss_rate: None,
                planned_qty: work_order.planned_qty,
                completed_qty: Decimal::ZERO,
                defect_qty: Decimal::ZERO,
                status: super::super::enums::RoutingStatus::Pending,
                is_outsourced: false,
                is_inspection_point: false,
            })
            .collect();

        WorkOrderRoutingRepo::insert_for_work_order(&mut *db, &routing_steps)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        if let Some(ref detail) = routing_detail {
            WorkOrderRepo::update_routing_id(&mut *db, id, detail.routing.id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        // 6. 创建至少 1 个 ProductionBatch
        let batch_req = crate::mes::production_batch::model::CreateBatchReq {
            work_order_id: id,
            product_id: work_order.product_id,
            batch_qty: work_order.planned_qty,
            team_id: None,
        };

        let batch_no = new_document_sequence_service(self.pool.clone())
            .next_number(
                ctx, db,
                DocumentType::WorkOrder,
            )
            .await
            .unwrap_or_else(|_| format!("{}-01", work_order.doc_number));

        let card_sn = format!("SN-{}", chrono::Local::now().format("%Y%m%d%H%M%S%3f"));

        ProductionBatchRepo::insert(
            &mut *db,
            &batch_req,
            &batch_no,
            &card_sn,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 7. 根据产品 material_consumption_mode 分流
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

        // 2. 校验未开工（所有批次 current_step == 0）
        let batches = ProductionBatchRepo::list_by_work_order(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let has_started = batches.iter().any(|b| b.current_step > 0);
        if has_started {
            return Err(DomainError::BusinessRule(
                "工单已开工，无法反下达".to_string(),
            ));
        }

        // 3. 取消关联的领料单（通过 document_links 双向查找）
        let requisition_ids: Vec<i64> = sqlx::query_scalar(
            r#"SELECT source_id FROM document_links
               WHERE target_type = $2 AND target_id = $1 AND source_type = $3
               UNION
               SELECT target_id FROM document_links
               WHERE source_type = $2 AND source_id = $1 AND target_type = $3"#,
        )
        .bind(id)
        .bind(DocumentType::WorkOrder)
        .bind(DocumentType::MaterialRequisition)
        .fetch_all(&mut *db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

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

        // 5. 删除 ProductionBatch
        sqlx::query("DELETE FROM production_batches WHERE work_order_id = $1")
            .bind(id)
            .execute(&mut *db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 6. 删除 WorkOrderRouting
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
}
