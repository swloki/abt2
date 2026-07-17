use async_trait::async_trait;
use sqlx::postgres::PgPool;
use sqlx::Row;
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};

use super::super::enums::{RoutingStatus, WorkOrderStatus};
use super::model::*;
use super::repo::WorkOrderRepo;
use super::service::WorkOrderService;
use crate::mes::work_report::{new_work_report_service, service::WorkReportService};
use crate::master_data::bom::{new_bom_query_service, service::BomQueryService};
use crate::master_data::work_center::{new_work_center_service, service::WorkCenterService};
use crate::master_data::routing::{new_routing_service, service::RoutingService};
use crate::master_data::product::{new_product_service, service::ProductService};
use crate::sales::sales_order::{new_demand_service, service::DemandService};
use crate::om::enums::OutsourcingStatus;
use crate::om::outsourcing_order::repo::OutsourcingOrderRepo;
use crate::om::outsourcing_order::service::OutsourcingOrderService;
use crate::om::outsourcing_order::{model::CancelOutsourcingReq, new_outsourcing_order_service};
use crate::wms::picking::{new_picking_service, service::PickingService};
use crate::wms::stock_ledger::{new_stock_ledger_service, service::StockLedgerService};
use crate::mes::production_batch::model::WorkOrderRouting;
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

    /// 查产品 BOM 关联的工艺路线：若有关联则把工序模板加载到工单 + 写 `work_order.routing_id`；
    /// 无关联则跳过（留空，由下达 drawer 引导 + release 兜底）。
    ///
    /// 复用调用方传入的连接（同事务原子）：`create` 自动调用、`release` 对无工序老工单兜底。
    /// 三家 ERP（ERPNext/Odoo/OFBiz）共识——工单工序在创建时从工艺模板复制。
    async fn try_load_operations_from_bom(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
        product_id: i64,
    ) -> Result<()> {
        use crate::master_data::bom_operation::{new_bom_operation_service, service::BomOperationService};
        use crate::mes::production_batch::{new_production_batch_service, ProductionBatchService};
        let product = new_product_service(self.pool.clone())
            .get(ctx, db, product_id)
            .await?;
        let ops = new_bom_operation_service(self.pool.clone())
            .list_operations(ctx, db, product.product_code.clone())
            .await?;
        if ops.is_empty() {
            return Ok(());
        }
        new_production_batch_service(self.pool.clone())
            .load_operations_from_bom(ctx, db, work_order_id, product.product_code.clone())
            .await?;
        // routing_id 作纯溯源（D9）：优先 bom_operations.source_routing_id 首行，回退 bom_routings 绑定
        let routing_id = if let Some(srid) = ops[0].source_routing_id {
            Some(srid)
        } else {
            new_routing_service(self.pool.clone())
                .get_bom_routing(ctx, db, product.product_code.clone())
                .await.ok().flatten()
                .map(|d| d.routing.id)
        };
        if let Some(rid) = routing_id {
            WorkOrderRepo::update_routing_id(&mut *db, work_order_id, rid)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }
        Ok(())
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

        // 工序：自动从 BOM 关联的工艺路线加载（无关联则留空，由下达 drawer 引导 + release 兜底）
        self.try_load_operations_from_bom(ctx, db, work_order.id, req.product_id)
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

    async fn list_product_brief_by_ids(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        ids: &[i64],
    ) -> Result<Vec<WoProductBrief>> {
        WorkOrderRepo::find_product_brief_by_ids(&mut *db, ids)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn set_work_order_step_price(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>,
        work_order_id: i64, step_no: i32, unit_price: rust_decimal::Decimal,
    ) -> Result<()> {
        use crate::mes::production_batch::repo::WorkOrderRoutingRepo;
        use crate::master_data::bom_step_price::{new_bom_step_price_service, service::BomStepPriceService};
        // §4.4 has_report 两步解析：get_by_work_order_and_step → has_report(wor.id)
        let wor = WorkOrderRoutingRepo::get_by_work_order_and_step(&mut *db, work_order_id, step_no)
            .await?
            .ok_or_else(|| DomainError::not_found("WorkOrderRouting"))?;
        if WorkOrderRoutingRepo::has_report(&mut *db, wor.id).await? {
            return Err(DomainError::business_rule("该工序已报工，wage 已冻结，不可改价"));
        }
        let wo = WorkOrderRepo::get_by_id(&mut *db, work_order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("WorkOrder"))?;
        let product = new_product_service(self.pool.clone()).get(ctx, db, wo.product_id).await?;
        // (a) bom_step_prices upsert（真相源，跨工单共享；source_type 标工单下达 + source_wo_id 溯源）
        new_bom_step_price_service(self.pool.clone())
            .upsert_price(ctx, db, product.product_code, step_no, unit_price,
                "work_order_release".into(), Some(work_order_id)).await?;
        // (b) 本工单快照（copy-on-write 执行价；work_order_routings 无 updated_at 列）
        sqlx::query("UPDATE work_order_routings SET unit_price = $1 WHERE work_order_id = $2 AND step_no = $3")
            .bind(unit_price).bind(work_order_id).bind(step_no).execute(&mut *db).await?;
        Ok(())
    }

    /// 下达工单：Draft/Planned -> Released
    /// - BOM 快照（冻结用料清单）
    /// - 工序由用户在下达 drawer 手动从 Routing 加载（release 不再自动初始化）
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

        // 5. 工序兜底：工序快照为空（不依赖 routing_id；R-11 + 修复 routing_id=Some 但快照空的 edge case）
        //    → 从 BOM 内联工序补加载。新工单 create 时已加载（快照非空），此处跳过。
        let ops_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM work_order_routings WHERE work_order_id = $1")
            .bind(id).fetch_one(&mut *db).await.unwrap_or(0);
        if ops_count == 0 {
            self.try_load_operations_from_bom(ctx, db, id, work_order.product_id)
                .await?;
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
                    new_picking_service(self.pool.clone())
                        .create_for_work_order(ctx, db, id).await?;
                }
            }
            crate::master_data::product::model::MaterialConsumptionMode::Backflush => {
                // backflush 模式：不预留、不创建领料单
                // 倒冲在完工时按实际量自动扣减
            }
        }

        // R-11：has_routing 改读 work_order_routings 实际存在性（非 routing_id.is_some()，
        // 后者对「有 bom_operations 但无 routing_id」误判为无工艺工单）
        let has_routing: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM work_order_routings WHERE work_order_id = $1)")
            .bind(id).fetch_one(&mut *db).await.unwrap_or(false);
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
                        "has_routing": has_routing,
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

        // 校验：已完工入库的 picking 阻止取消（IncomingWorkOrder=2, Done=3）
        let receipt_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM stock_pickings WHERE source_id = $1 AND picking_type = 2 AND deleted_at IS NULL AND status = 3",
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

        // Issue #270：委外级联——已发料/已收货的委外单阻止取消工单（料在供应商手中 / 已立 AP 台账）。
        // 先查关联委外单（含所有状态）：Sent/InProduction/Delivered/Received 阻断，Draft 留待下方取消。
        let linked_osa = OutsourcingOrderRepo::query_by_work_order(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let active_osa_count = linked_osa
            .iter()
            .filter(|o| matches!(
                o.status,
                OutsourcingStatus::Sent
                    | OutsourcingStatus::InProduction
                    | OutsourcingStatus::Delivered
                    | OutsourcingStatus::Received
            ))
            .count();
        if active_osa_count > 0 {
            return Err(DomainError::BusinessRule(format!(
                "工单有 {active_osa_count} 张已发料/已收货的委外单，请先收货或取消委外单后再取消工单",
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
            if let Err(e) = new_picking_service(self.pool.clone())
                .cancel(ctx, db, req_id)
                .await
            {
                tracing::warn!(req_id, error = %e, "领料单取消失败");
            }
        }

        // Issue #270：取消 Draft 委外单（om.cancel 内部已同步取消其待发料 OutsourceIssue picking）。
        for o in &linked_osa {
            if o.status == OutsourcingStatus::Draft
                && let Err(e) = new_outsourcing_order_service(self.pool.clone())
                    .cancel(
                        ctx,
                        db,
                        CancelOutsourcingReq {
                            id: o.id,
                            expected_version: o.version,
                            remark: Some("工单取消级联".into()),
                        },
                    )
                    .await
            {
                tracing::warn!(osa_id = o.id, error = %e, "工单取消：Draft 委外单取消失败");
            }
        }

        // 回退关联需求：走 DemandService.release_back_to_pool（demand 回 Pending + 清 target_doc
        // + 发 DemandReleased 事件对称回退履行计划行/订单行），与 create_work_orders_from_demands 对称。
        new_demand_service(self.pool.clone())
            .release_back_to_pool(ctx, db, DocumentType::WorkOrder as i16, id)
            .await?;

        // 审计日志
        new_audit_log_service(self.pool.clone())
            .record(
                ctx, db,
                RecordAuditLogReq::new("WorkOrder", id, AuditAction::Delete),
            )
            .await?;

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

        // 4. 入库 picking（IncomingWorkOrder=2，用于 received_qty 聚合 + receipts disclosure）
        #[derive(sqlx::FromRow)]
        struct HubReceipt {
            doc_number: String,
            batch_id: Option<i64>,
            received_qty: Decimal,
            warehouse_name: Option<String>,
            status: i16,
        }
        let receipts: Vec<HubReceipt> = sqlx::query_as(
            "SELECT p.doc_number, pi.batch_id AS batch_id, \
             COALESCE(pi.qty_requested, 0) AS received_qty, \
             wh.name AS warehouse_name, p.status \
             FROM stock_pickings p \
             LEFT JOIN LATERAL (SELECT batch_id, qty_requested FROM stock_picking_items WHERE picking_id = p.id ORDER BY id LIMIT 1) pi ON true \
             LEFT JOIN warehouses wh ON p.to_warehouse_id = wh.id \
             WHERE p.source_id = $1 AND p.picking_type = 2 AND p.deleted_at IS NULL \
             ORDER BY p.created_at DESC",
        )
        .bind(work_order_id)
        .fetch_all(&mut *db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        // Done(3) = 已入库（原 receipt Confirmed(2) 语义）
        let received_qty: Decimal = receipts
            .iter()
            .filter(|r| r.status == 3)
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

        // 11. receipts disclosure（fqc/backflush 聚合；Done=3）
        let fqc_passed = receipts.iter().any(|r| r.status == 3);
        let backflush_done = receipts.iter().any(|r| r.status == 3);
        let receipt_items: Vec<HubReceiptRow> = receipts
            .iter()
            .map(|r| HubReceiptRow {
                doc_number: r.doc_number.clone(),
                batch_no: r.batch_id.map(|_| "—".to_string()).unwrap_or_default(),
                received_qty: r.received_qty,
                warehouse_name: r.warehouse_name.clone().unwrap_or_default(),
                fqc_label: if r.status == 3 { "通过".into() } else { "待检".into() },
                backflush_label: if r.status == 3 { "已倒冲".into() } else { "—".into() },
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
        orders: &[WorkOrder],
    ) -> Result<HashMap<i64, (MaterialAvailabilityLevel, Option<String>)>> {
        use crate::master_data::product::model::Product;
        use crate::shared::inventory_reservation::repo::InventoryReservationRepo;
        let mut result: HashMap<i64, (MaterialAvailabilityLevel, Option<String>)> =
            HashMap::new();
        if orders.is_empty() {
            return Ok(result);
        }

        let bom_svc = new_bom_query_service(self.pool.clone());
        // 阶段 1：每工单取 BOM 快照，提取叶子 (pid, qty, code)（leaf_nodes 借用 snapshot，需提取 owned）
        let mut wo_leaves: HashMap<i64, Vec<(i64, Decimal, Option<String>)>> = HashMap::new();
        for order in orders {
            // 已关闭/取消 / 无 BOM 快照：直接 Available，不计算
            if matches!(order.status, WorkOrderStatus::Closed | WorkOrderStatus::Cancelled)
                || order.bom_snapshot_id.is_none()
            {
                result.insert(order.id, (MaterialAvailabilityLevel::Available, None));
                continue;
            }
            let snapshot = bom_svc
                .get_snapshot_by_id(ctx, db, order.bom_snapshot_id.unwrap())
                .await?;
            let leaves: Vec<(i64, Decimal, Option<String>)> = match snapshot.as_ref() {
                Some(snap) => snap
                    .bom_detail
                    .leaf_nodes()
                    .iter()
                    .map(|n| (n.product_id, n.quantity, n.product_code.clone()))
                    .collect(),
                None => {
                    result.insert(order.id, (MaterialAvailabilityLevel::Available, None));
                    continue;
                }
            };
            if leaves.is_empty() {
                result.insert(order.id, (MaterialAvailabilityLevel::Available, None));
                continue;
            }
            wo_leaves.insert(order.id, leaves);
        }

        // 阶段 2：批量预取所有叶子物料名 + ATP（消除 N+1：原本每叶子一次 query → 两次 batch query）
        let all_pids: Vec<i64> = wo_leaves
            .values()
            .flatten()
            .map(|(pid, _, _)| *pid)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let leaf_products: HashMap<i64, Product> = new_product_service(self.pool.clone())
            .get_by_ids(ctx, db, all_pids.clone())
            .await?
            .into_iter()
            .map(|p| (p.product_id, p))
            .collect();
        let atp_map = InventoryReservationRepo::available_atp_batch(&mut *db, &all_pids, None)
            .await?;

        // 阶段 3：每工单 compute level（纯内存，无 query）
        for order in orders {
            let Some(leaves) = wo_leaves.get(&order.id) else {
                continue;
            };
            let mut level = MaterialAvailabilityLevel::Available;
            let mut headline: Option<String> = None;
            for (pid, qty, code) in leaves {
                let required = *qty * order.planned_qty;
                let atp = atp_map.get(pid).copied().unwrap_or(Decimal::ZERO);
                if atp < required {
                    level = MaterialAvailabilityLevel::Unavailable;
                    headline = Some(
                        leaf_products
                            .get(pid)
                            .map(|p| p.pdt_name.clone())
                            .or_else(|| code.clone())
                            .unwrap_or_else(|| format!("P{}", pid)),
                    );
                    break; // 任一缺料即可定级
                }
            }
            result.insert(order.id, (level, headline));
        }

        Ok(result)
    }

    /// 工序级齐套（#124）：工序产出品 → 子 BOM leaf_nodes → ATP/在途/ETA 四级 + 缺口明细。
    async fn compute_step_availability(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
        routing_id: i64,
        batch_id: Option<i64>,
    ) -> Result<MaterialAvailability> {
        use crate::master_data::product::model::Product;
        use crate::mes::production_batch::service::ProductionBatchService;
        // 1. 工序产出品（跨模块走 ProductionBatchService trait）
        let batch_svc = crate::mes::production_batch::new_production_batch_service(self.pool.clone());
        let routing = batch_svc
            .list_routings(ctx, db, work_order_id)
            .await?
            .into_iter()
            .find(|r| r.id == routing_id)
            .ok_or_else(|| DomainError::not_found("WorkOrderRouting"))?;
        // 无产出工序（检测/检验）→ 无消耗物料 → 齐套为空（无需料），学 Odoo 自然跳过
        let output_pid = match routing.product_id {
            Some(pid) => pid,
            None => {
                return Ok(MaterialAvailability {
                    level: MaterialAvailabilityLevel::Available,
                    headline: None,
                    lines: Vec::new(),
                });
            }
        };

        // 2. 工单成品 → 成品已发布 BOM → 产出品节点的直接子级（物料清单）
        let wo = self.find_by_id(ctx, db, work_order_id).await?;
        let fg_product = new_product_service(self.pool.clone())
            .get(ctx, db, wo.product_id)
            .await?;
        let bom_svc = new_bom_query_service(self.pool.clone());
        let fg_bom_id = bom_svc
            .find_published_bom_by_product_code(ctx, db, &fg_product.product_code)
            .await?
            .ok_or_else(|| DomainError::BusinessRule("工单成品无已发布 BOM".into()))?;
        let material_nodes = bom_svc
            .get_direct_children_by_product(ctx, db, fg_bom_id, output_pid)
            .await?;
        if material_nodes.is_empty() {
            return Ok(MaterialAvailability {
                level: MaterialAvailabilityLevel::Available,
                headline: None,
                lines: Vec::new(),
            });
        }

        // 3. base_qty（batch 优先）+ scheduled_start
        let base_qty = if let Some(bid) = batch_id {
            batch_svc.find_by_id(ctx, db, bid).await?.batch_qty
        } else {
            wo.planned_qty
        };
        let scheduled_start = wo.scheduled_start;

        // 4. line_products 批量预取
        let pids: Vec<i64> = material_nodes.iter().map(|n| n.product_id).collect();
        let line_products: HashMap<i64, Product> = new_product_service(self.pool.clone())
            .get_by_ids(ctx, db, pids)
            .await?
            .into_iter()
            .map(|p| (p.product_id, p))
            .collect();

        // 5. material_refs → 齐套计算（复用 build_availability 的核心；直接子级 quantity 相对产出品，× base_qty 即需求）
        let material_refs: Vec<&crate::master_data::bom::model::BomNode> = material_nodes.iter().collect();
        build_availability_from_leaves(
            ctx,
            db,
            self.pool.clone(),
            &material_refs,
            base_qty,
            scheduled_start,
            &line_products,
        )
        .await
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
        ("closed", "已完工"),
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

    // 1. 领料单 + 批量明细（一条 ANY 替代逐个 list_items 的 N+1）
    let req_svc = new_picking_service(pool.clone());
    let reqs = req_svc
        .list(
            ctx,
            db,
            crate::wms::picking::model::PickingFilter {
                work_order_id: Some(work_order_id),
                ..Default::default()
            },
            crate::shared::types::pagination::PageParams::new(1, 1000),
        )
        .await?;
    let mut hub_reqs: Vec<HubRequisition> = Vec::new();
    let req_ids: Vec<i64> = reqs.items.iter().map(|r| r.id).collect();
    let all_items = req_svc.list_items_by_req_ids(ctx, db, &req_ids).await?;
    // 领料单明细所需 product_id 集合（用于 product_code/name 解析）+ 按 requisition_id 分组
    let mut line_product_ids: HashSet<i64> = HashSet::new();
    let mut req_item_map: std::collections::HashMap<i64, Vec<crate::wms::picking::model::StockPickingItem>> =
        std::collections::HashMap::new();
    for it in all_items {
        line_product_ids.insert(it.product_id);
        req_item_map.entry(it.picking_id).or_default().push(it);
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

    // 批量预取每行物料的 ATP（available_atp_batch 一条查询替代逐个 available_atp 的 N+1）
    let line_atp: HashMap<i64, Decimal> = {
        let pids: Vec<i64> = line_products.keys().copied().collect();
        InventoryReservationRepo::available_atp_batch(&mut *db, &pids, None).await?
    };

    for req in &reqs.items {
        let items = req_item_map.get(&req.id).cloned().unwrap_or_default();
        let total_qty: Decimal = items.iter().map(|i| i.qty_requested).sum();
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
                    required_qty: i.qty_requested,
                    issued_qty: i.qty_done,
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

/// 工序级齐套：从 leaf_nodes（产出品子 BOM）算 MaterialAvailability（#124）。
/// 复用 build_availability 的 ATP/projected/ETA/定级逻辑，区别仅在 leaf_nodes 来源。
async fn build_availability_from_leaves(
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    pool: sqlx::PgPool,
    leaf_nodes: &[&crate::master_data::bom::model::BomNode],
    planned_qty: Decimal,
    scheduled_start: chrono::NaiveDate,
    line_products: &HashMap<i64, crate::master_data::product::model::Product>,
) -> Result<MaterialAvailability> {
    use crate::shared::inventory_reservation::repo::InventoryReservationRepo;
    if leaf_nodes.is_empty() {
        return Ok(MaterialAvailability {
            level: MaterialAvailabilityLevel::Available,
            headline: None,
            lines: Vec::new(),
        });
    }
    let product_ids: Vec<i64> = leaf_nodes.iter().map(|n| n.product_id).collect();

    let mut atp_map: HashMap<i64, Decimal> = HashMap::new();
    for &pid in &product_ids {
        let atp = InventoryReservationRepo::available_atp(&mut *db, pid, None).await?;
        atp_map.insert(pid, atp);
    }
    let projected_map = new_stock_ledger_service(pool.clone())
        .query_projected_qty_batch(ctx, db, &product_ids, None)
        .await?;
    let po_eta_map = query_po_eta_batch(&mut *db, &product_ids).await?;

    let mut lines: Vec<MaterialAvailabilityLine> = Vec::new();
    let mut overall_level = MaterialAvailabilityLevel::Available;
    let mut headline: Option<String> = None;
    let mut headline_severity = 0i32;

    for node in leaf_nodes {
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
                node.product_code
                    .clone()
                    .unwrap_or_else(|| format!("P{}", node.product_id)),
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
