use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::{
    CreateManualReq, CreatePickingItemReq, CreatePickingReq, DoneItemReq, IssueMaterialReq,
    PickingFilter, ReturnMaterialReq, StockPicking, StockPickingItem,
};
use super::repo::PickingRepo;
use super::service::PickingService;
use crate::master_data::bom::{new_bom_query_service, service::BomQueryService};
use crate::master_data::product::{new_product_service, service::ProductService};
use crate::mes::production_batch::{new_production_batch_service, service::ProductionBatchService};
use crate::mes::work_order::{new_work_order_service, service::WorkOrderService};
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::cost_entry::{new_cost_entry_service, model::EntryRequest, service::CostEntryService};
use crate::shared::document_link::model::LinkRequest;
use crate::shared::document_link::{new_document_link_service, service::DocumentLinkService};
use crate::shared::enums::{AuditAction, CostEntityType, CostType, DocumentType, LinkType, ReservationType};
use crate::shared::inventory_reservation::{
    new_inventory_reservation_service, service::InventoryReservationService, ReserveRequest,
};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;
use crate::shared::types::{PgExecutor, Result};
use crate::wms::backflush::resolve_warehouse_id;
use crate::wms::enums::{PickingStatus, PickingType, TransactionType};
use crate::wms::inventory_transaction::model::RecordTransactionReq;
use crate::wms::inventory_transaction::{new_inventory_transaction_service, service::InventoryTransactionService};
use crate::wms::stock_ledger::repo::StockLedgerRepo;

pub struct PickingServiceImpl {
    pool: PgPool,
}

impl PickingServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 生成单据号（type 前缀 + 时间戳兜底）
    /// TODO（决策点 4）：接入 DocumentSequenceService，按 picking_type 分配连续序号
    fn generate_doc_number(picking_type: PickingType) -> String {
        format!(
            "{}{}",
            picking_type.doc_prefix(),
            chrono::Utc::now().format("%Y%m%d%H%M%S%.f")
        )
    }

    /// 领料 picking 的工单关联（document_link）。
    /// 借用 DocumentType::MaterialRequisition variant 做 link 类型，使 work_order cancel
    /// 反查逻辑（work_order/implt.rs cancel）无需改类型；source_id = picking_id。
    /// TODO：后续加 DocumentType::StockPicking variant 并迁移 link 类型。
    async fn link_to_work_order(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        picking_id: i64,
        work_order_id: i64,
    ) -> Result<()> {
        new_document_link_service(self.pool.clone())
            .create_links(
                ctx,
                db,
                vec![LinkRequest {
                    source_type: DocumentType::MaterialRequisition,
                    source_id: picking_id,
                    target_type: DocumentType::WorkOrder,
                    target_id: work_order_id,
                    link_type: LinkType::Fulfills,
                }],
            )
            .await?;
        Ok(())
    }
}

#[async_trait]
impl PickingService for PickingServiceImpl {
    // ── 通用作业单据 ──

    async fn create(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreatePickingReq,
    ) -> Result<i64> {
        if req.items.is_empty() {
            return Err(DomainError::Validation("作业单据至少需要一条明细".to_string()));
        }

        if let (Some(from), Some(to)) = (req.from_warehouse_id, req.to_warehouse_id)
            && from == to
        {
            return Err(DomainError::BusinessRule(
                "源仓库和目标仓库不能相同".to_string(),
            ));
        }

        let doc_number = Self::generate_doc_number(req.picking_type);
        let picking = PickingRepo::insert(&mut *db, &doc_number, &req, ctx.operator_id).await?;
        Ok(picking.id)
    }

    async fn get(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<StockPicking> {
        PickingRepo::get_by_id(&mut *db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("作业单据"))
    }

    async fn list_items(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        picking_id: i64,
    ) -> Result<Vec<StockPickingItem>> {
        PickingRepo::get_items(&mut *db, picking_id).await
    }

    async fn list(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: PickingFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<StockPicking>> {
        PickingRepo::list(&mut *db, &filter, page, page_size).await
    }

    async fn confirm(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let picking = self.get(ctx, db, id).await?;
        if picking.status != PickingStatus::Draft {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", picking.status),
                to: "Confirmed".to_string(),
            });
        }
        PickingRepo::update_status(&mut *db, id, PickingStatus::Confirmed).await?;
        Ok(())
    }

    async fn cancel(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let picking = self.get(ctx, db, id).await?;
        match picking.status {
            PickingStatus::Draft | PickingStatus::Confirmed => {
                PickingRepo::update_status(&mut *db, id, PickingStatus::Cancelled).await?;
                Ok(())
            }
            other => Err(DomainError::InvalidStateTransition {
                from: format!("{other:?}"),
                to: "Cancelled".to_string(),
            }),
        }
    }

    async fn done(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        items: Vec<DoneItemReq>,
    ) -> Result<()> {
        let picking = self.get(ctx, db, id).await?;
        if picking.status != PickingStatus::Confirmed {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", picking.status),
                to: "Done".to_string(),
            });
        }

        // 通用 done：行级 qty_done + 状态转换。
        // InternalIssue 的完整业务（流水/预留/成本）由 issue() 承担（领料走 issue 入口）。
        // 其他 picking_type 的 done 分发逻辑在阶段 3-5 补全。
        for it in &items {
            PickingRepo::update_item_done(
                &mut *db,
                it.item_id,
                it.qty_done,
                it.batch_no.as_deref(),
                it.from_bin_id,
                it.to_bin_id,
            )
            .await?;
        }
        PickingRepo::set_done(&mut *db, id).await?;
        Ok(())
    }

    // ── 领料专用（InternalIssue，从 MaterialRequisitionService 迁入）──

    async fn create_for_work_order(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<i64> {
        let doc_number = Self::generate_doc_number(PickingType::InternalIssue);
        let wo = new_work_order_service(self.pool.clone())
            .find_by_id(ctx, db, work_order_id)
            .await?;

        let warehouse_id = resolve_warehouse_id(db).await?;
        let requisition_date = chrono::Local::now().date_naive();

        // BOM 快照必须存在（对标 Odoo: MO 必须有 BOM 才能产生 move）
        let snapshot_id = wo.bom_snapshot_id.ok_or_else(|| {
            DomainError::BusinessRule(
                "工单无 BOM 快照，请先确保 release 时 BOM 快照创建成功".into(),
            )
        })?;
        let snapshot = new_bom_query_service(self.pool.clone())
            .get_snapshot_by_id(ctx, db, snapshot_id)
            .await?
            .ok_or_else(|| DomainError::not_found("BomSnapshot"))?;

        let leaf_nodes = snapshot.bom_detail.leaf_nodes();
        let items: Vec<CreatePickingItemReq> = leaf_nodes
            .iter()
            .map(|node| CreatePickingItemReq {
                product_id: node.product_id,
                batch_no: None,
                qty_requested: node.quantity * wo.planned_qty,
                from_bin_id: None,
                to_bin_id: None,
                operation_id: None,
                batch_id: None,
                source_item_id: None,
                remark: None,
            })
            .collect();

        let req = CreatePickingReq {
            picking_type: PickingType::InternalIssue,
            source_type: Some("work_order".into()),
            source_id: Some(work_order_id),
            partner_id: None,
            from_warehouse_id: Some(warehouse_id),
            from_zone_id: None,
            from_bin_id: None,
            to_warehouse_id: None,
            to_zone_id: None,
            to_bin_id: None,
            scheduled_date: Some(requisition_date),
            work_order_id: Some(work_order_id),
            remark: None,
            items,
        };

        let picking = PickingRepo::insert(&mut *db, &doc_number, &req, ctx.operator_id).await?;
        self.link_to_work_order(ctx, db, picking.id, work_order_id).await?;
        Ok(picking.id)
    }

    async fn create_for_routing_step(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
        routing_id: i64,
        batch_id: Option<i64>,
    ) -> Result<i64> {
        // 1. 取工序产出品（跨模块走 ProductionBatchService trait）
        let batch_svc = new_production_batch_service(self.pool.clone());
        let routing = batch_svc
            .list_routings(ctx, db, work_order_id)
            .await?
            .into_iter()
            .find(|r| r.id == routing_id)
            .ok_or_else(|| DomainError::not_found("WorkOrderRouting"))?;
        let output_product_id = routing.product_id.ok_or_else(|| {
            DomainError::BusinessRule("该工序未配置产出品，无法工序级领料".into())
        })?;

        // 2. 工单成品 → 成品已发布 BOM
        let wo = new_work_order_service(self.pool.clone())
            .find_by_id(ctx, db, work_order_id)
            .await?;
        let fg_product = new_product_service(self.pool.clone())
            .get(ctx, db, wo.product_id)
            .await?;
        let bom_svc = new_bom_query_service(self.pool.clone());
        let fg_bom_id = bom_svc
            .find_published_bom_by_product_code(ctx, db, &fg_product.product_code)
            .await?
            .ok_or_else(|| DomainError::BusinessRule("工单成品无已发布 BOM，无法工序级领料".into()))?;

        // 3. 定位产出品节点，取其直接子级
        let children = bom_svc
            .get_direct_children_by_product(ctx, db, fg_bom_id, output_product_id)
            .await?;
        if children.is_empty() {
            return Err(DomainError::BusinessRule(
                "产出品在成品 BOM 中无直接子级物料，无法工序级领料（散料请走完工倒冲）".into(),
            ));
        }

        // 4. 数量基数：batch_id 优先，否则工单 planned_qty
        let base_qty = if let Some(bid) = batch_id {
            batch_svc.find_by_id(ctx, db, bid).await?.batch_qty
        } else {
            wo.planned_qty
        };

        // 5. 建 InternalIssue picking（items 挂 operation_id + batch_id）
        let doc_number = Self::generate_doc_number(PickingType::InternalIssue);
        let warehouse_id = resolve_warehouse_id(db).await?;
        let items: Vec<CreatePickingItemReq> = children
            .iter()
            .map(|node| CreatePickingItemReq {
                product_id: node.product_id,
                batch_no: None,
                qty_requested: node.quantity * base_qty,
                from_bin_id: None,
                to_bin_id: None,
                operation_id: Some(routing_id),
                batch_id,
                source_item_id: None,
                remark: None,
            })
            .collect();
        let req = CreatePickingReq {
            picking_type: PickingType::InternalIssue,
            source_type: Some("work_order".into()),
            source_id: Some(work_order_id),
            partner_id: None,
            from_warehouse_id: Some(warehouse_id),
            from_zone_id: None,
            from_bin_id: None,
            to_warehouse_id: None,
            to_zone_id: None,
            to_bin_id: None,
            scheduled_date: Some(chrono::Local::now().date_naive()),
            work_order_id: Some(work_order_id),
            remark: None,
            items,
        };
        let picking = PickingRepo::insert(&mut *db, &doc_number, &req, ctx.operator_id).await?;

        // 6. 单据关联
        self.link_to_work_order(ctx, db, picking.id, work_order_id).await?;

        tracing::info!(
            work_order_id, routing_id, batch_id, fg_bom_id, output_product_id,
            "routing-step picking created"
        );
        Ok(picking.id)
    }

    async fn create_manual(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateManualReq,
    ) -> Result<i64> {
        if req.items.is_empty() {
            return Err(DomainError::validation("请至少添加一条领料明细"));
        }

        let doc_number = Self::generate_doc_number(PickingType::InternalIssue);
        let items: Vec<CreatePickingItemReq> = req
            .items
            .iter()
            .map(|it| CreatePickingItemReq {
                product_id: it.product_id,
                batch_no: None,
                qty_requested: it.requested_qty,
                from_bin_id: None,
                to_bin_id: None,
                operation_id: None,
                batch_id: None,
                source_item_id: None,
                remark: None,
            })
            .collect();
        let picking_req = CreatePickingReq {
            picking_type: PickingType::InternalIssue,
            source_type: Some("none".into()),
            source_id: None,
            partner_id: None,
            from_warehouse_id: Some(req.warehouse_id),
            from_zone_id: None,
            from_bin_id: None,
            to_warehouse_id: None,
            to_zone_id: None,
            to_bin_id: None,
            scheduled_date: Some(req.requisition_date),
            work_order_id: None,
            remark: req.remark.clone(),
            items,
        };
        let picking = PickingRepo::insert(&mut *db, &doc_number, &picking_req, ctx.operator_id).await?;
        Ok(picking.id)
    }

    /// 发料（Confirmed → Done/Confirmed）：写 MaterialIssue 流水 + 消耗 HARD 预留 + 记工单成本 + 审计
    async fn issue(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: IssueMaterialReq,
    ) -> Result<()> {
        let picking = self.get(ctx, db, req.id).await?;
        if picking.status != PickingStatus::Confirmed {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", picking.status),
                to: "Issued".to_string(),
            });
        }

        let existing_items = PickingRepo::get_items(&mut *db, req.id).await?;
        let warehouse_id = picking
            .from_warehouse_id
            .ok_or_else(|| DomainError::BusinessRule("领料单无源仓库".into()))?;

        // 批量预加载涉及产品的最后已知单位成本（消除循环内 N+1）
        let cost_product_ids: Vec<i64> = req
            .items
            .iter()
            .filter_map(|item| {
                existing_items
                    .iter()
                    .find(|i| i.id == item.item_id)
                    .map(|i| i.product_id)
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        let unit_cost_map =
            StockLedgerRepo::last_known_unit_cost_batch(&mut *db, &cost_product_ids)
                .await
                .unwrap_or_default();

        let mut total_cost_amount = Decimal::ZERO;

        for item in &req.items {
            let found = existing_items.iter().find(|i| i.id == item.item_id);
            let Some(found_item) = found else {
                return Err(DomainError::not_found(format!("PickingItem {}", item.item_id)));
            };

            // 更新行级 qty_done + from_bin_id
            PickingRepo::update_item_done(
                &mut *db,
                item.item_id,
                item.issued_qty,
                None,
                item.bin_id,
                None,
            )
            .await?;

            let unit_cost = unit_cost_map
                .get(&found_item.product_id)
                .copied()
                .unwrap_or(Decimal::ZERO);
            total_cost_amount += item.issued_qty * unit_cost;

            // MaterialIssue 流水（负数出库）
            new_inventory_transaction_service(self.pool.clone())
                .record(
                    ctx,
                    db,
                    RecordTransactionReq {
                        doc_number: Some(picking.doc_number.clone()),
                        delivery_no: None,
                        source_doc_number: None,
                        transaction_type: TransactionType::MaterialIssue,
                        product_id: found_item.product_id,
                        warehouse_id,
                        zone_id: None,
                        bin_id: item.bin_id,
                        batch_no: None,
                        quantity: -item.issued_qty,
                        unit_cost: Some(unit_cost),
                        source_type: "stock_picking".to_string(),
                        source_id: req.id,
                        remark: None,
                    },
                )
                .await?;

            // 消耗库存预留（对标 Odoo move._action_done 消费 reservation）
            if let Some(wo_id) = picking.work_order_id.filter(|&w| w > 0) {
                new_inventory_reservation_service(self.pool.clone())
                    .consume(
                        ctx,
                        db,
                        DocumentType::WorkOrder,
                        wo_id,
                        found_item.product_id,
                        item.issued_qty,
                    )
                    .await?;
            }
        }

        // 判断是否全部发完 → Done 否则保持 Confirmed（行级 qty_done 部分填）
        let issued_map: std::collections::HashMap<i64, Decimal> =
            req.items.iter().map(|r| (r.item_id, r.issued_qty)).collect();
        let all_fully_issued = existing_items.iter().all(|i| {
            let issued = issued_map.get(&i.id).copied().unwrap_or(i.qty_done);
            issued >= i.qty_requested
        });
        if all_fully_issued {
            PickingRepo::set_done(&mut *db, req.id).await?;
        }

        // 领料出库 → 创建材料成本分录（真实金额 = qty × unit_cost）
        if let Some(wo_id) = picking.work_order_id.filter(|&w| w > 0)
            && total_cost_amount > Decimal::ZERO
        {
            let period = chrono::Local::now().format("%Y-%m").to_string();
            new_cost_entry_service(self.pool.clone())
                .create_entries(
                    ctx,
                    db,
                    vec![EntryRequest {
                        entity_type: CostEntityType::WorkOrder,
                        entity_id: wo_id,
                        cost_type: CostType::Material,
                        debit_amount: total_cost_amount,
                        credit_amount: total_cost_amount,
                        cost_center: None,
                        profit_center: None,
                        period,
                        // 借用 MaterialRequisition variant（TODO: 加 StockPicking）
                        source_type: DocumentType::MaterialRequisition,
                        source_id: req.id,
                    }],
                )
                .await?;
        }

        // 审计日志
        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq::new("StockPicking", req.id, AuditAction::Transition),
            )
            .await?;

        Ok(())
    }

    /// 退料：Done/Confirmed → 退料入库（正数流水）+ 恢复预留 + 行级 qty_done 扣减
    async fn return_materials(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: ReturnMaterialReq,
    ) -> Result<()> {
        let picking = self.get(ctx, db, req.requisition_id).await?;
        // 仅已发料（Done / Confirmed 含 qty_done）的可退料
        if picking.status == PickingStatus::Draft || picking.status == PickingStatus::Cancelled {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", picking.status),
                to: "Returned".to_string(),
            });
        }

        let existing_items = PickingRepo::get_items(&mut *db, req.requisition_id).await?;
        let warehouse_id = picking
            .from_warehouse_id
            .ok_or_else(|| DomainError::BusinessRule("领料单无源仓库".into()))?;

        for item in &req.items {
            let Some(found_item) = existing_items.iter().find(|i| i.id == item.item_id) else {
                return Err(DomainError::not_found(format!("PickingItem {}", item.item_id)));
            };
            if item.return_qty > found_item.qty_done {
                return Err(DomainError::validation(format!(
                    "退料量 {} 超过已发料量 {}",
                    item.return_qty, found_item.qty_done
                )));
            }

            // 退料入库流水（正数）
            new_inventory_transaction_service(self.pool.clone())
                .record(
                    ctx,
                    db,
                    RecordTransactionReq {
                        doc_number: Some(picking.doc_number.clone()),
                        delivery_no: None,
                        source_doc_number: None,
                        transaction_type: TransactionType::MaterialIssue,
                        product_id: found_item.product_id,
                        warehouse_id,
                        zone_id: None,
                        bin_id: item.bin_id,
                        batch_no: None,
                        quantity: item.return_qty,
                        unit_cost: None,
                        source_type: "material_return".to_string(),
                        source_id: req.requisition_id,
                        remark: Some(req.reason.clone()),
                    },
                )
                .await?;

            // 行级 qty_done 扣减
            let new_qty_done = found_item.qty_done - item.return_qty;
            PickingRepo::update_item_done(
                &mut *db,
                item.item_id,
                new_qty_done,
                None,
                item.bin_id,
                None,
            )
            .await?;

            // 工单驱动领料在 issue 时 consume 了 HARD 预留；退料恢复等量预留
            if let Some(wo_id) = picking.work_order_id.filter(|&w| w > 0) {
                new_inventory_reservation_service(self.pool.clone())
                    .reserve(
                        ctx,
                        db,
                        vec![ReserveRequest {
                            product_id: found_item.product_id,
                            warehouse_id: Some(warehouse_id),
                            reserved_qty: item.return_qty,
                            reservation_type: ReservationType::Hard,
                            source_type: DocumentType::WorkOrder,
                            source_id: wo_id,
                            source_line_id: None,
                            priority: 0,
                            expires_at: None,
                        }],
                    )
                    .await?;
            }
        }

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq::new("StockPicking", req.requisition_id, AuditAction::Update),
            )
            .await?;
        Ok(())
    }

    async fn list_items_by_req_ids(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        requisition_ids: &[i64],
    ) -> Result<Vec<StockPickingItem>> {
        PickingRepo::get_items_by_picking_ids(&mut *db, requisition_ids).await
    }

    async fn list_requisitioned_routing_ids(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        batch_id: i64,
    ) -> Result<Vec<i64>> {
        PickingRepo::find_routing_ids_by_batch(&mut *db, batch_id).await
    }
}
