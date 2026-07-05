use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use super::model::{
    CreateFromOrderReq, CreateManualReq, CreatePickingItemReq, CreatePickingReq, DoneItemReq,
    FqcGate, IssueMaterialReq, PickingFilter, ProductionReceiptDetail, ProductionReceiptFilter,
    ProductionReceiptListItem, RequestShippingItemReq, ReturnMaterialReq, ShippingHubSummary,
    ShortageSignal, ShipRowReq, StockPicking, StockPickingItem,
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
use crate::shared::types::pagination::{PageParams, PaginatedResult};
use crate::shared::types::{PgExecutor, Result};
use crate::wms::backflush::resolve_warehouse_id;
use crate::wms::enums::{PickingStatus, PickingType, TransactionType};
use crate::sales::sales_order::{new_sales_order_service, service::SalesOrderService};
use crate::sales::sales_order::model::{SalesOrderStatus, ShipmentLineQty};
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::{new_domain_event_bus, model::EventPublishRequest, service::DomainEventBus};
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

    /// 草稿明细 product_id 反查（from order_item）。product_id=0 报错（杜绝脏数据）。
    async fn resolve_draft_items(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: Option<i64>,
        items: &[super::model::CreateDraftItemReq],
    ) -> Result<Vec<super::model::ShippingItemInput>> {
        let order_items = if let Some(oid) = order_id {
            new_sales_order_service(self.pool.clone()).list_items(ctx, db, oid).await?
        } else {
            Vec::new()
        };
        let inputs: Vec<super::model::ShippingItemInput> = items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let product_id = item
                    .product_id
                    .or_else(|| {
                        order_items
                            .iter()
                            .find(|oi| oi.id == item.order_item_id.unwrap_or(0))
                            .map(|oi| oi.product_id)
                    })
                    .unwrap_or(0);
                super::model::ShippingItemInput {
                    line_no: (i + 1) as i32,
                    order_item_id: item.order_item_id.unwrap_or(0),
                    product_id,
                    warehouse_id: item.warehouse_id,
                    requested_qty: item.requested_qty,
                    description: item.description.clone(),
                }
            })
            .collect();
        if inputs.iter().any(|i| i.product_id == 0) {
            return Err(DomainError::validation(
                "发货明细必须关联订单行或指定商品（product_id 缺失，无法确定发货商品）",
            ));
        }
        Ok(inputs)
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

    async fn find_by_id(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<StockPicking> {
        self.get(ctx, db, id).await
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
        page: PageParams,
    ) -> Result<PaginatedResult<StockPicking>> {
        PickingRepo::list(&mut *db, &filter, page.page, page.page_size).await
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

    async fn delete(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let picking = self.get(ctx, db, id).await?;
        if picking.status != PickingStatus::Draft {
            return Err(DomainError::business_rule("仅草稿状态的作业单据可以删除"));
        }
        PickingRepo::soft_delete(&mut *db, id).await?;
        Ok(())
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
                batch_no: it.batch_no.clone(),
                qty_requested: it.requested_qty,
                from_bin_id: it.bin_id,
                to_bin_id: None,
                operation_id: None,
                batch_id: None,
                source_item_id: None,
                remark: None,
            })
            .collect();
        // work_order_id=Some 时关联工单（source_type/source_id/work_order_id），否则纯手动
        let source_type = match req.work_order_id {
            Some(_) => Some("work_order".to_string()),
            None => Some("none".to_string()),
        };
        let picking_req = CreatePickingReq {
            picking_type: PickingType::InternalIssue,
            source_type,
            source_id: req.work_order_id,
            partner_id: None,
            from_warehouse_id: Some(req.warehouse_id),
            from_zone_id: None,
            from_bin_id: None,
            to_warehouse_id: None,
            to_zone_id: None,
            to_bin_id: None,
            scheduled_date: Some(req.requisition_date),
            work_order_id: req.work_order_id,
            remark: req.remark.clone(),
            items,
        };
        let picking = PickingRepo::insert(&mut *db, &doc_number, &picking_req, ctx.operator_id).await?;
        Ok(picking.id)
    }

    /// 按工单聚合各产品已申请领料量（InternalIssue + 未取消）
    async fn sum_issued_qty_by_work_order(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<std::collections::HashMap<i64, rust_decimal::Decimal>> {
        PickingRepo::sum_issued_qty_by_work_order(&mut *db, work_order_id).await
    }

    /// 工单领料预览：BOM leaf_nodes × planned_qty 算需求量，附已领量（前端算待领差额 + 查可用量）
    async fn list_wo_requisition_preview(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<Vec<super::model::WoReqPreviewItem>> {
        let wo = new_work_order_service(self.pool.clone())
            .find_by_id(ctx, db, work_order_id).await?;
        let snapshot_id = wo.bom_snapshot_id.ok_or_else(|| {
            DomainError::BusinessRule("工单无 BOM 快照，请先确保 release 时 BOM 快照创建成功".into())
        })?;
        let snapshot = new_bom_query_service(self.pool.clone())
            .get_snapshot_by_id(ctx, db, snapshot_id).await?
            .ok_or_else(|| DomainError::not_found("BomSnapshot"))?;
        let planned = wo.planned_qty;
        let issued = PickingRepo::sum_issued_qty_by_work_order(&mut *db, work_order_id).await?;
        let items = snapshot
            .bom_detail
            .leaf_nodes()
            .iter()
            .map(|node| super::model::WoReqPreviewItem {
                product_id: node.product_id,
                bom_qty: node.quantity * planned,
                issued_qty: *issued.get(&node.product_id).unwrap_or(&rust_decimal::Decimal::ZERO),
            })
            .collect();
        Ok(items)
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
                .await?;

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

    // ── 调拨专用（InternalTransfer，从 TransferService 迁入）──

    /// 调拨发货：Draft → Confirmed，扣减源仓库库存（Transfer 流水负数）
    async fn dispatch(
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
        let items = PickingRepo::get_items(&mut *db, id).await?;
        let from_wh = picking
            .from_warehouse_id
            .ok_or_else(|| DomainError::BusinessRule("调拨单无源仓库".into()))?;
        let tx_svc = new_inventory_transaction_service(self.pool.clone());
        for item in &items {
            tx_svc
                .record(
                    ctx,
                    db,
                    RecordTransactionReq {
                        doc_number: Some(picking.doc_number.clone()),
                        delivery_no: None,
                        source_doc_number: None,
                        transaction_type: TransactionType::Transfer,
                        product_id: item.product_id,
                        warehouse_id: from_wh,
                        zone_id: picking.from_zone_id,
                        bin_id: picking.from_bin_id,
                        batch_no: item.batch_no.clone(),
                        quantity: -item.qty_requested,
                        unit_cost: None,
                        source_type: "stock_picking".to_string(),
                        source_id: id,
                        remark: Some("调拨发货-扣减源仓库".to_string()),
                    },
                )
                .await?;
        }
        PickingRepo::update_status(&mut *db, id, PickingStatus::Confirmed).await?;
        Ok(())
    }

    /// 调拨完成：Confirmed → Done，增加目标仓库库存（Transfer 流水正数）
    async fn complete(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let picking = self.get(ctx, db, id).await?;
        if picking.status != PickingStatus::Confirmed {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", picking.status),
                to: "Done".to_string(),
            });
        }
        let items = PickingRepo::get_items(&mut *db, id).await?;
        let to_wh = picking
            .to_warehouse_id
            .ok_or_else(|| DomainError::BusinessRule("调拨单无目标仓库".into()))?;
        let tx_svc = new_inventory_transaction_service(self.pool.clone());
        for item in &items {
            tx_svc
                .record(
                    ctx,
                    db,
                    RecordTransactionReq {
                        doc_number: Some(picking.doc_number.clone()),
                        delivery_no: None,
                        source_doc_number: None,
                        transaction_type: TransactionType::Transfer,
                        product_id: item.product_id,
                        warehouse_id: to_wh,
                        zone_id: picking.to_zone_id,
                        bin_id: picking.to_bin_id,
                        batch_no: item.batch_no.clone(),
                        quantity: item.qty_requested,
                        unit_cost: None,
                        source_type: "stock_picking".to_string(),
                        source_id: id,
                        remark: Some("调拨完成-增加目标仓库".to_string()),
                    },
                )
                .await?;
        }
        PickingRepo::set_done(&mut *db, id).await?;
        Ok(())
    }

    // ── 发货专用（OutgoingSales，从 ShippingRequestService 迁入，#146 阶段 4b）──

    async fn create_from_order(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateFromOrderReq,
    ) -> Result<i64> {
        let so_svc = new_sales_order_service(self.pool.clone());
        let order = so_svc.find_by_id(ctx, db, req.order_id).await?;
        if !matches!(
            order.status,
            SalesOrderStatus::Confirmed | SalesOrderStatus::ReadyToShip | SalesOrderStatus::PartiallyShipped
        ) {
            return Err(DomainError::business_rule(
                "订单必须为 Confirmed/ReadyToShip/PartiallyShipped 才能创建发货单",
            ));
        }
        let order_items = so_svc.list_items(ctx, db, req.order_id).await?;
        let mut item_reqs = Vec::with_capacity(req.items.len());
        for item in &req.items {
            let oi = order_items
                .iter()
                .find(|oi| oi.id == item.order_item_id)
                .ok_or_else(|| DomainError::validation(format!("订单行 {} 不存在", item.order_item_id)))?;
            let remaining = oi.quantity - oi.shipped_qty;
            if item.requested_qty > remaining {
                return Err(DomainError::business_rule(format!(
                    "订单行 {} 申请数量 {} 超过未发数量 {}",
                    item.order_item_id, item.requested_qty, remaining
                )));
            }
            item_reqs.push(CreatePickingItemReq {
                product_id: oi.product_id,
                batch_no: None,
                qty_requested: item.requested_qty,
                from_bin_id: None,
                to_bin_id: None,
                operation_id: None,
                batch_id: None,
                source_item_id: Some(item.order_item_id),
                remark: None,
            });
        }
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::ShippingRequest)
            .await?;
        let picking_req = CreatePickingReq {
            picking_type: PickingType::OutgoingSales,
            source_type: Some("sales_order".into()),
            source_id: Some(req.order_id),
            partner_id: Some(order.customer_id),
            from_warehouse_id: None,
            from_zone_id: None,
            from_bin_id: None,
            to_warehouse_id: None,
            to_zone_id: None,
            to_bin_id: None,
            scheduled_date: req.expected_ship_date,
            work_order_id: None,
            remark: req.shipping_address.clone(),
            items: item_reqs,
        };
        let picking = PickingRepo::insert(&mut *db, &doc_number, &picking_req, ctx.operator_id).await?;
        new_document_link_service(self.pool.clone())
            .create_links(ctx, db, vec![LinkRequest {
                source_type: DocumentType::ShippingRequest,
                source_id: picking.id,
                target_type: DocumentType::SalesOrder,
                target_id: req.order_id,
                link_type: LinkType::Triggers,
            }])
            .await?;
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq {
                entity_type: "StockPicking".into(),
                entity_id: picking.id,
                action: AuditAction::Create,
                changes: Some(serde_json::json!({ "order_id": req.order_id, "picking_type": "OutgoingSales" })),
                context: None,
            })
            .await?;
        Ok(picking.id)
    }

    async fn request_from_order(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
        items: Vec<RequestShippingItemReq>,
    ) -> Result<i64> {
        let so_svc = new_sales_order_service(self.pool.clone());
        let order = so_svc.find_by_id(ctx, db, order_id).await?;
        if !matches!(
            order.status,
            SalesOrderStatus::Confirmed | SalesOrderStatus::ReadyToShip
                | SalesOrderStatus::PartiallyShipped | SalesOrderStatus::ShippingRequested
        ) {
            return Err(DomainError::business_rule("订单当前状态不允许申请发货"));
        }
        let order_items = so_svc.list_items(ctx, db, order_id).await?;
        let mut item_reqs = Vec::with_capacity(items.len());
        for item in &items {
            if item.requested_qty <= Decimal::ZERO {
                return Err(DomainError::validation("申请数量必须大于 0"));
            }
            let oi = order_items
                .iter()
                .find(|oi| oi.id == item.order_item_id)
                .ok_or_else(|| DomainError::validation(format!("订单行 {} 不存在", item.order_item_id)))?;
            let remaining = oi.quantity - oi.shipped_qty;
            if item.requested_qty > remaining {
                return Err(DomainError::business_rule(format!(
                    "订单行 {} 申请数量 {} 超过未发数量 {}",
                    item.order_item_id, item.requested_qty, remaining
                )));
            }
            item_reqs.push(CreatePickingItemReq {
                product_id: oi.product_id,
                batch_no: None,
                qty_requested: item.requested_qty,
                from_bin_id: None,
                to_bin_id: None,
                operation_id: None,
                batch_id: None,
                source_item_id: Some(item.order_item_id),
                remark: None,
            });
        }
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::ShippingRequest)
            .await?;
        let picking_req = CreatePickingReq {
            picking_type: PickingType::OutgoingSales,
            source_type: Some("sales_order".into()),
            source_id: Some(order_id),
            partner_id: Some(order.customer_id),
            from_warehouse_id: None,
            from_zone_id: None,
            from_bin_id: None,
            to_warehouse_id: None,
            to_zone_id: None,
            to_bin_id: None,
            scheduled_date: None,
            work_order_id: None,
            remark: Some(order.delivery_address.clone()),
            items: item_reqs,
        };
        let picking = PickingRepo::insert(&mut *db, &doc_number, &picking_req, ctx.operator_id).await?;
        new_document_link_service(self.pool.clone())
            .create_links(ctx, db, vec![LinkRequest {
                source_type: DocumentType::ShippingRequest,
                source_id: picking.id,
                target_type: DocumentType::SalesOrder,
                target_id: order_id,
                link_type: LinkType::Triggers,
            }])
            .await?;
        // 跳 Draft → 直接 Confirmed（入待发货队列）
        PickingRepo::update_status(&mut *db, picking.id, PickingStatus::Confirmed).await?;
        // 回写 SO → recalc_header_status 叠加判定 ShippingRequested
        so_svc.recalc_header_status(ctx, db, order_id).await?;
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq {
                entity_type: "StockPicking".into(),
                entity_id: picking.id,
                action: AuditAction::Create,
                changes: Some(serde_json::json!({ "order_id": order_id, "via": "request_from_order" })),
                context: None,
            })
            .await?;
        Ok(picking.id)
    }

    async fn direct_ship(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        warehouse_id: i64,
        bin_id: Option<i64>,
    ) -> Result<()> {
        // 委托 direct_ship_rows：每行 qty=qty_requested, bin_id 全局, batch_no=None
        let items = PickingRepo::get_items(&mut *db, id).await?;
       let rows: Vec<ShipRowReq> = items
           .iter()
           .map(|item| ShipRowReq {
               picking_item_id: item.id,
                warehouse_id,
               bin_id,
               batch_no: None,
               qty: item.qty_requested,
           })
           .collect();
        self.direct_ship_rows(ctx, db, id, warehouse_id, rows).await
    }

    async fn direct_ship_rows(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        warehouse_id: i64,
        rows: Vec<ShipRowReq>,
    ) -> Result<()> {
        let picking = self.get(ctx, db, id).await?;
        if picking.status != PickingStatus::Confirmed {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", picking.status),
                to: "Done".to_string(),
            });
        }
        let order_id = picking.source_id.ok_or_else(|| {
            DomainError::business_rule("发货单缺少关联订单，无法发货")
        })?;
        PickingRepo::update_from_warehouse(&mut *db, id, warehouse_id).await?;
        let items = PickingRepo::get_items(&mut *db, id).await?;
        let item_map: std::collections::HashMap<i64, &StockPickingItem> =
            items.iter().map(|i| (i.id, i)).collect();
        let tx_svc = new_inventory_transaction_service(self.pool.clone());
        for row in &rows {
            let item = item_map.get(&row.picking_item_id).ok_or_else(|| {
                DomainError::validation(format!("发货明细行 {} 不属于此发货单", row.picking_item_id))
            })?;
            PickingRepo::update_item_done(
                &mut *db, row.picking_item_id, row.qty,
                row.batch_no.as_deref(), None, row.bin_id,
            ).await?;
            new_inventory_reservation_service(self.pool.clone())
                .fulfill_by_source_line(ctx, db, DocumentType::SalesOrder, item.source_item_id.unwrap_or(0))
                .await?;
            tx_svc
                .record(ctx, db, RecordTransactionReq {
                    doc_number: None,
                    delivery_no: None,
                    source_doc_number: Some(picking.doc_number.clone()),
                   transaction_type: TransactionType::SalesShipment,
                   product_id: item.product_id,
                    warehouse_id: row.warehouse_id,
                    zone_id: None,
                    bin_id: row.bin_id,
                    batch_no: row.batch_no.clone(),
                    quantity: -row.qty,
                    unit_cost: None,
                    source_type: "shipping".to_string(),
                    source_id: id,
                    remark: None,
                })
                .await?;
        }
        PickingRepo::set_done(&mut *db, id).await?;
        let lines: Vec<ShipmentLineQty> = rows
            .iter()
            .filter_map(|r| item_map.get(&r.picking_item_id).map(|item| ShipmentLineQty {
                order_item_id: item.source_item_id.unwrap_or(0),
                shipped_qty: r.qty,
            }))
            .collect();
        new_sales_order_service(self.pool.clone())
            .record_shipment(ctx, db, order_id, &lines)
            .await?;
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq {
                entity_type: "StockPicking".into(),
                entity_id: id,
                action: AuditAction::Transition,
                changes: Some(serde_json::json!({ "from": "Confirmed", "to": "Done" })),
                context: None,
            })
            .await?;
        new_domain_event_bus(self.pool.clone())
            .publish(ctx, db, EventPublishRequest {
                event_type: DomainEventType::ShipmentShipped,
                aggregate_type: "StockPicking".to_string(),
                aggregate_id: id,
                payload: serde_json::json!({
                    "shipping_request_id": id,
                    "doc_number": picking.doc_number,
                    "order_id": order_id,
                    "customer_id": picking.partner_id,
                }),
                idempotency_key: None,
            })
            .await?;
        Ok(())
    }

    async fn hub_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<ShippingHubSummary> {
        let items = PickingRepo::get_items(&mut *db, id).await?;
        let pending_ship_qty: Decimal = items.iter().map(|i| i.qty_requested).sum();
        let shipped_qty: Decimal = items.iter().map(|i| i.qty_done).sum();
        let picking = self.get(ctx, db, id).await?;
        // 缺货判定：任一明细 ATP < 待发量（requested - done）即缺货。批量 ATP 按 from_warehouse。
        let txn_svc = new_inventory_transaction_service(self.pool.clone());
        let pending_pids: Vec<i64> = items
            .iter()
            .filter(|i| i.qty_requested - i.qty_done > Decimal::ZERO)
            .map(|i| i.product_id)
            .collect();
        let shortage = if pending_pids.is_empty() {
            None
        } else {
            match txn_svc.query_available_batch(ctx, db, &pending_pids, picking.from_warehouse_id).await {
                Ok(atp_map) => items.iter().find_map(|it| {
                    let remaining = it.qty_requested - it.qty_done;
                    if remaining <= Decimal::ZERO {
                        return None;
                    }
                    let atp = atp_map.get(&it.product_id).copied().unwrap_or(Decimal::ZERO);
                    if atp < remaining {
                        Some(ShortageSignal {
                            product_id: it.product_id,
                            product_name: format!("产品 #{}", it.product_id),
                            requested_qty: it.qty_requested,
                            available_qty: atp,
                        })
                    } else {
                        None
                    }
                }),
                Err(e) => {
                    tracing::warn!(error = %e, "hub_summary: query_available_batch failed, shortage=None");
                    None
                }
            }
        };
        Ok(ShippingHubSummary { pending_ship_qty, shipped_qty, shortage })
    }

    async fn list_transactions(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        page: crate::shared::types::pagination::PageParams,
    ) -> Result<PaginatedResult<crate::wms::inventory_transaction::model::InventoryTransaction>> {
        // source_type="shipping"，与 direct_ship record 的 source_type 对齐（事件链/handler 兼容）
        new_inventory_transaction_service(self.pool.clone())
            .query(
                ctx, db,
                crate::wms::inventory_transaction::model::TransactionFilter {
                    source_type: Some("shipping".into()),
                    source_id: Some(id),
                    ..Default::default()
                },
                page.page,
                page.page_size,
            )
            .await
    }

    async fn save_draft(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: super::model::CreateDraftReq,
    ) -> Result<i64> {
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::ShippingRequest)
            .await?;
        let item_inputs = if req.items.is_empty() {
            Vec::new()
        } else {
            self.resolve_draft_items(ctx, db, req.order_id, &req.items).await?
        };
        // 物流字段拼接存 remark（shipping_address/carrier/remark）
        let remark = [req.shipping_address.as_deref(), req.carrier.as_deref(), req.remark.as_deref()]
            .iter()
            .filter_map(|s| *s)
            .collect::<Vec<_>>()
            .join(" | ");
        let item_reqs: Vec<CreatePickingItemReq> = item_inputs
            .iter()
            .map(|i| CreatePickingItemReq {
                product_id: i.product_id,
                batch_no: None,
                qty_requested: i.requested_qty,
                from_bin_id: None,
                to_bin_id: None,
                operation_id: None,
                batch_id: None,
                source_item_id: Some(i.order_item_id),
                remark: None,
            })
            .collect();
        let picking_req = CreatePickingReq {
            picking_type: PickingType::OutgoingSales,
            source_type: Some("sales_order".into()),
            source_id: req.order_id,
            partner_id: Some(req.customer_id),
            from_warehouse_id: None,
            from_zone_id: None,
            from_bin_id: None,
            to_warehouse_id: None,
            to_zone_id: None,
            to_bin_id: None,
            scheduled_date: req.expected_ship_date,
            work_order_id: None,
            remark: Some(remark),
            items: item_reqs,
        };
        let picking = PickingRepo::insert(&mut *db, &doc_number, &picking_req, ctx.operator_id).await?;
        if let Some(order_id) = req.order_id {
            new_document_link_service(self.pool.clone())
                .create_links(ctx, db, vec![LinkRequest {
                    source_type: DocumentType::ShippingRequest,
                    source_id: picking.id,
                    target_type: DocumentType::SalesOrder,
                    target_id: order_id,
                    link_type: LinkType::Triggers,
                }])
                .await?;
        }
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq {
                entity_type: "StockPicking".into(),
                entity_id: picking.id,
                action: AuditAction::Create,
                changes: Some(serde_json::json!({ "order_id": req.order_id, "customer_id": req.customer_id, "is_draft": true })),
                context: None,
            })
            .await?;
        Ok(picking.id)
    }

    async fn update_draft(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        req: super::model::UpdateDraftReq,
    ) -> Result<()> {
        let existing = self.get(ctx, db, id).await?;
        if existing.status != PickingStatus::Draft {
            return Err(DomainError::business_rule("仅草稿状态的发货单可以编辑"));
        }
        let remark = [req.shipping_address.as_deref(), req.carrier.as_deref(), req.remark.as_deref()]
            .iter()
            .filter_map(|s| *s)
            .collect::<Vec<_>>()
            .join(" | ");
        PickingRepo::update_draft_fields(
            &mut *db, id,
            req.order_id,
            req.customer_id,
            req.expected_ship_date,
            &remark,
        )
        .await?;
        if let Some(items) = req.items {
            PickingRepo::delete_items(&mut *db, id).await?;
            if !items.is_empty() {
                let order_id = req.order_id.or(existing.source_id);
                let item_inputs = self.resolve_draft_items(ctx, db, order_id, &items).await?;
                for input in item_inputs {
                    let item_req = CreatePickingItemReq {
                        product_id: input.product_id,
                        batch_no: None,
                        qty_requested: input.requested_qty,
                        from_bin_id: None,
                        to_bin_id: None,
                        operation_id: None,
                        batch_id: None,
                        source_item_id: Some(input.order_item_id),
                        remark: None,
                    };
                    PickingRepo::insert_item(&mut *db, id, &item_req).await?;
                }
            }
        }
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq {
                entity_type: "StockPicking".into(),
                entity_id: id,
                action: AuditAction::Update,
                changes: None,
                context: None,
            })
            .await?;
        Ok(())
    }

    async fn receive_purchase(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: super::model::ReceivePurchaseReq,
    ) -> Result<i64> {
        use crate::purchase::order::repo::{PurchaseOrderItemRepo, PurchaseOrderRepo};
        use crate::purchase::settings::repo::PurchaseSettingsRepo;
        use crate::purchase::settings::model::PurchaseSettings;
        use crate::purchase::enums::PurchaseOrderStatus;
        use crate::fms::ar_ap::repo::{ArApLedgerInsert, ArApLedgerRepo};
        use crate::fms::ar_ap::enums::LedgerDirection;
        use crate::fms::enums::CounterpartyType;
        use crate::shared::idempotency::{new_idempotency_service, service::IdempotencyService};
        use crate::shared::cost_entry::{model::EntryRequest, new_cost_entry_service, service::CostEntryService};
        use crate::shared::enums::{CostEntityType, CostType};
        use crate::wms::warehouse::{new_warehouse_service, service::WarehouseService};

        if req.rows.is_empty() {
            return Err(DomainError::validation("请至少添加一行收货明细"));
        }
        // 1. 幂等防护
        if let Some(key) = req.idempotency_key.as_deref()
            && !key.is_empty()
            && !new_idempotency_service(self.pool.clone()).try_claim(ctx, db, key).await?
        {
            return Ok(0);
        }
        // 2. 读 PO + 明细 + 超收容差设置
        let po = PurchaseOrderRepo::get_by_id(db, req.po_id).await?
            .ok_or_else(|| DomainError::not_found(format!("PurchaseOrder #{}", req.po_id)))?;
        let po_items = PurchaseOrderItemRepo::list_by_order_id(db, req.po_id).await?;
        let settings = PurchaseSettingsRepo::get(db).await.unwrap_or_else(|_| PurchaseSettings::default());

        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::StockReceipt).await?;
        let inv_svc = new_inventory_transaction_service(self.pool.clone());
        let wh_svc = new_warehouse_service(self.pool.clone());
        let prod_to_oi: std::collections::HashMap<i64, i64> =
            po_items.iter().map(|i| (i.product_id, i.id)).collect();

        // 建 IncomingPurchase picking（头仓 = 首行仓；行级 source_item_id=order_item_id）
        let first_wh = req.rows.first().map(|r| r.warehouse_id);
        let item_reqs: Vec<CreatePickingItemReq> = req.rows.iter().map(|r| CreatePickingItemReq {
            product_id: r.product_id, batch_no: r.batch_no.clone(), qty_requested: r.received_qty,
            from_bin_id: r.bin_id, to_bin_id: r.bin_id, operation_id: None, batch_id: None,
            source_item_id: Some(r.order_item_id), remark: None,
        }).collect();
        let picking_req = CreatePickingReq {
            picking_type: PickingType::IncomingPurchase,
            source_type: Some("purchase_order".into()),
            source_id: Some(req.po_id),
            partner_id: Some(po.supplier_id),
            from_warehouse_id: first_wh,
            from_zone_id: None, from_bin_id: None,
            to_warehouse_id: first_wh, to_zone_id: None, to_bin_id: None,
            scheduled_date: None, work_order_id: None,
            remark: req.remark.clone(), items: item_reqs,
        };
        let picking = PickingRepo::insert(&mut *db, &doc_number, &picking_req, ctx.operator_id).await?;

        // 3. 逐行：超收校验 → record PurchaseReceipt 流水 → PO received_qty 累加
        for row in &req.rows {
            let order_item_id = if row.order_item_id != 0 {
                row.order_item_id
            } else {
                *prod_to_oi.get(&row.product_id).ok_or_else(|| {
                    DomainError::validation(format!("收货行产品 {} 不属于采购订单 #{}", row.product_id, req.po_id))
                })?
            };
            let item = po_items.iter().find(|i| i.id == order_item_id).ok_or_else(|| {
                DomainError::validation(format!("收货行 order_item_id={} 不属于采购订单 #{}", order_item_id, req.po_id))
            })?;
            let max_qty = item.quantity
                * (Decimal::ONE + settings.over_delivery_allowance_pct / Decimal::from(100));
            if item.received_qty + row.received_qty > max_qty {
                return Err(DomainError::validation(format!(
                    "订单行 {} 收货数量超过允许上限 {}（含 {}% 容差）",
                    item.line_no, max_qty, settings.over_delivery_allowance_pct
                )));
            }
            let zone_id = wh_svc.get_or_create_default_zone(ctx, db, row.warehouse_id).await.ok().map(|z| z.id);
            let default_bin_id = if let Some(zid) = zone_id {
                wh_svc.list_bins(ctx, db, zid, None, 1, 1).await.ok().and_then(|r| r.items.first().map(|b| b.id))
            } else { None };
            inv_svc.record(ctx, db, RecordTransactionReq {
                doc_number: Some(doc_number.clone()),
                delivery_no: req.delivery_note.clone(),
                source_doc_number: Some(po.doc_number.clone()),
                transaction_type: TransactionType::PurchaseReceipt,
                product_id: row.product_id,
                warehouse_id: row.warehouse_id,
                zone_id,
                bin_id: row.bin_id.or(default_bin_id),
                batch_no: row.batch_no.clone(),
                quantity: row.received_qty,
                unit_cost: None,
                source_type: "stock_picking".to_string(),
                source_id: picking.id,
                remark: req.remark.clone(),
            }).await?;
            PurchaseOrderItemRepo::add_received_qty(db, order_item_id, row.received_qty).await?;
        }

        // 4. PO 状态流转
        let po_items_after = PurchaseOrderItemRepo::list_by_order_id(db, req.po_id).await?;
        let all_received = po_items_after.iter().all(|i| i.received_qty >= i.quantity);
        let any_received = po_items_after.iter().any(|i| i.received_qty > Decimal::ZERO);
        let target_status = if all_received {
            PurchaseOrderStatus::Received
        } else if any_received {
            PurchaseOrderStatus::PartiallyReceived
        } else {
            return Ok(picking.id);
        };
        if po.status != PurchaseOrderStatus::Received && po.status != target_status {
            let affected = PurchaseOrderRepo::update_status(db, req.po_id, target_status, &po.updated_at).await?;
            if affected == 0 {
                return Err(DomainError::ConcurrentConflict);
            }
            new_audit_log_service(self.pool.clone())
                .record(ctx, db, RecordAuditLogReq {
                    entity_type: "PurchaseOrder", entity_id: req.po_id, action: AuditAction::Transition,
                    changes: Some(serde_json::json!({
                        "from": format!("{:?}", po.status), "to": format!("{:?}", target_status),
                        "trigger": "PickingReceivePurchase",
                    })),
                    context: None,
                }).await?;
        }

        // 5. 立应付（PO 维度 upsert + rewrite）
        let ap_amount: Decimal = po_items_after.iter().map(|i| i.received_qty * i.unit_price).sum();
        if ap_amount > Decimal::ZERO {
            let period = chrono::Utc::now().format("%Y-%m").to_string();
            let today = chrono::Local::now().date_naive();
            let doc_no = po.doc_number.clone();
            let desc = format!("采购入库 {doc_no}");
            let inserted = ArApLedgerRepo::insert(db, &ArApLedgerInsert {
                party_type: CounterpartyType::Supplier, party_id: po.supplier_id,
                source_type: DocumentType::PurchaseOrder, source_id: req.po_id, source_doc_no: &doc_no,
                against_type: None, against_id: None, direction: LedgerDirection::Credit,
                amount: ap_amount, currency: "CNY", exchange_rate: Decimal::ONE,
                transaction_date: today, due_date: None, period: &period, description: &desc,
                operator_id: ctx.operator_id,
            }).await?;
            if inserted.is_none() {
                ArApLedgerRepo::rewrite_amount_by_source(db, DocumentType::PurchaseOrder, req.po_id, ap_amount).await?;
            }
        }

        // 6. 成本分录
        let total_received: Decimal = req.rows.iter().map(|r| r.received_qty).sum();
        if total_received > Decimal::ZERO {
            let period = chrono::Local::now().format("%Y-%m").to_string();
            new_cost_entry_service(self.pool.clone())
                .create_entries(ctx, db, vec![EntryRequest {
                    entity_type: CostEntityType::PurchaseOrder, entity_id: req.po_id,
                    cost_type: CostType::Material, debit_amount: total_received, credit_amount: total_received,
                    cost_center: None, profit_center: None, period,
                    source_type: DocumentType::PurchaseOrder, source_id: req.po_id,
                }]).await?;
        }

        PickingRepo::set_done(&mut *db, picking.id).await?;
        Ok(picking.id)
    }

    async fn receive_production(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        warehouse_id: i64,
        zone_id: Option<i64>,
        bin_id: Option<i64>,
    ) -> Result<()> {
        use crate::mes::production_batch::repo::{ProductionBatchRepo, WorkOrderRoutingRepo};
        use crate::mes::work_order::repo::WorkOrderRepo;
        use crate::mes::enums::{BatchStatus, WorkOrderStatus};
        use crate::qms::enums::{InspectionResultType, InspectionSourceType, InspectionStatus};
        use crate::qms::inspection_result::{new_inspection_result_service, model::InspectionResultFilter, service::InspectionResultService};
        use crate::shared::cost_entry::{model::EntryRequest, new_cost_entry_service, service::CostEntryService};
        use crate::shared::enums::{CostEntityType, CostType};
        use crate::shared::inventory_reservation::{new_inventory_reservation_service, service::InventoryReservationService};
        use crate::shared::types::pagination::PageParams;
        use crate::wms::backflush::{new_backflush_service, service::BackflushService};
        use crate::wms::stock_ledger::repo::StockLedgerRepo;

        let picking = self.get(ctx, db, id).await?;
        if picking.status != PickingStatus::Draft && picking.status != PickingStatus::Confirmed {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", picking.status),
                to: "Done".to_string(),
            });
        }
        if warehouse_id <= 0 {
            return Err(DomainError::validation("确认入库必须指定目标仓库"));
        }
        // 生产入库一步入库体验：Draft → 先转 Confirmed（写入库位）再走 7 步
        // （生产侧 create 建 Draft 不填仓库；仓库确认入库时指定目标库位 + 触发闭环）
        PickingRepo::update_to_location(&mut *db, id, warehouse_id, zone_id, bin_id).await?;
        if picking.status == PickingStatus::Draft {
            PickingRepo::update_status(&mut *db, id, PickingStatus::Confirmed).await?;
        }
        let work_order_id = picking.source_id.ok_or_else(|| {
            DomainError::business_rule("入库 picking 缺少 source_id（work_order_id）")
        })?;
        let items = PickingRepo::get_items(&mut *db, id).await?;
        let item = items.first().ok_or_else(|| DomainError::business_rule("入库 picking 无明细"))?;
        let product_id = item.product_id;
        let received_qty = item.qty_requested;
        let batch_id = item.batch_id;

        // 1. FQC 门（仅当工单工序含报检点时；InspectionResult source_id = picking.id）
        let wo_routings = WorkOrderRoutingRepo::get_by_work_order_id(&mut *db, work_order_id)
            .await.unwrap_or_default();
        let has_inspection_points = wo_routings.iter().any(|r| r.is_inspection_point);
        if has_inspection_points {
            let insp_svc = new_inspection_result_service(self.pool.clone());
            let one = PageParams::new(1, 1);
            let total = insp_svc.list_by_source(ctx, db, InspectionResultFilter {
                source_type: Some(InspectionSourceType::ProductionReceipt),
                source_id: Some(id),
                ..Default::default()
            }, one.clone()).await.map(|p| p.total).unwrap_or(0);
            if total == 0 {
                return Err(DomainError::business_rule(
                    "工单含报检工序，完工入库前必须完成 FQC 质检（无检验记录）",
                ));
            }
            let passed = insp_svc.list_by_source(ctx, db, InspectionResultFilter {
                source_type: Some(InspectionSourceType::ProductionReceipt),
                source_id: Some(id),
                status: Some(InspectionStatus::Completed),
                result: Some(InspectionResultType::Pass),
                ..Default::default()
            }, one.clone()).await.map(|p| p.total).unwrap_or(0);
            if passed != total {
                return Err(DomainError::business_rule("FQC 质检未全部通过，不允许入库"));
            }
        }

        // 解析产成品批次号（流转卡 batch_no 透传）
        let fg_batch_no: Option<String> = match batch_id {
            Some(bid) => ProductionBatchRepo::get_by_id(&mut *db, bid)
                .await.map_err(|e| DomainError::Internal(e.into()))?.map(|b| b.batch_no),
            None => None,
        };

        // 2. record ProductionReceipt 流水
        new_inventory_transaction_service(self.pool.clone())
            .record(ctx, db, RecordTransactionReq {
                doc_number: None, delivery_no: None,
                source_doc_number: Some(picking.doc_number.clone()),
                transaction_type: TransactionType::ProductionReceipt,
                product_id, warehouse_id, zone_id, bin_id,
                batch_no: fg_batch_no, quantity: received_qty, unit_cost: None,
                source_type: "stock_picking".to_string(), source_id: id, remark: None,
            }).await?;

        // 3. 成本分录（unit_cost from stock_ledger）
        let unit_cost = StockLedgerRepo::last_known_unit_cost(&mut *db, product_id).await.unwrap_or(Decimal::ZERO);
        let total_cost = received_qty * unit_cost;
        if total_cost > Decimal::ZERO {
            let period = chrono::Local::now().format("%Y-%m").to_string();
            new_cost_entry_service(self.pool.clone())
                .create_entries(ctx, db, vec![EntryRequest {
                    entity_type: CostEntityType::WorkOrder, entity_id: work_order_id,
                    cost_type: CostType::Material, debit_amount: total_cost, credit_amount: total_cost,
                    cost_center: None, profit_center: None, period,
                    source_type: DocumentType::ProductionReceipt, source_id: id,
                }]).await?;
        }

        // 4. Backflush（倒冲原料，同事务）
        new_backflush_service(self.pool.clone())
            .execute(ctx, db, work_order_id, received_qty, warehouse_id)
            .await
            .map_err(|e| DomainError::business_rule(format!("倒冲失败，入库已回滚: {e:?}")))?;

        // 5. batch Completed
        if let Some(bid) = batch_id {
            ProductionBatchRepo::update_status(&mut *db, bid, BatchStatus::Completed)
                .await.map_err(|e| DomainError::Internal(e.into()))?;
        }

        // 6. 多批次守卫 → WO InProduction→Closed + 预留释放
        let all_batches = ProductionBatchRepo::list_by_work_order(&mut *db, work_order_id)
            .await.map_err(|e| DomainError::Internal(e.into()))?;
        let has_active_batch = all_batches.iter().any(|b| {
            b.status != BatchStatus::Completed && b.status != BatchStatus::Cancelled
        });
        if !has_active_batch {
            match WorkOrderRepo::update_status_conditional(
                &mut *db, work_order_id,
                WorkOrderStatus::InProduction, WorkOrderStatus::Closed,
            ).await {
                Ok(true) => {
                    new_audit_log_service(self.pool.clone())
                        .record(ctx, db, RecordAuditLogReq {
                            entity_type: "WorkOrder".into(), entity_id: work_order_id,
                            action: AuditAction::Transition, changes: None, context: None,
                        }).await?;
                }
                Ok(false) => {}
                Err(e) => return Err(DomainError::Internal(e.into())),
            }
            new_inventory_reservation_service(self.pool.clone())
                .cancel_by_source(ctx, db, DocumentType::WorkOrder, work_order_id)
                .await?;
        }

        PickingRepo::set_done(&mut *db, id).await?;
        Ok(())
    }

    // ── 生产入库查询（IncomingWorkOrder，mes_receipt 页面用，搬自 ProductionReceiptService）──

    async fn get_fqc_status(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        picking_id: i64,
    ) -> Result<FqcGate> {
        use crate::mes::production_batch::repo::WorkOrderRoutingRepo;
        use crate::qms::enums::{InspectionResultType, InspectionSourceType, InspectionStatus};
        use crate::qms::inspection_result::{
            model::InspectionResultFilter, new_inspection_result_service,
            service::InspectionResultService,
        };
        use crate::shared::types::pagination::PageParams;

        let picking = self.get(ctx, db, picking_id).await?;
        let work_order_id = picking.source_id.ok_or_else(|| {
            DomainError::business_rule("入库 picking 缺少 source_id（work_order_id）")
        })?;

        // 仅当工单工序含报检点时才要求 FQC
        let wo_routings = WorkOrderRoutingRepo::get_by_work_order_id(&mut *db, work_order_id)
            .await
            .unwrap_or_default();
        if !wo_routings.iter().any(|r| r.is_inspection_point) {
            return Ok(FqcGate::NotRequired);
        }

        // 两次轻量 count 判定（source_id = picking.id），替代拉全量
        let insp_svc = new_inspection_result_service(self.pool.clone());
        let one = PageParams::new(1, 1);
        let total = insp_svc
            .list_by_source(
                ctx,
                db,
                InspectionResultFilter {
                    source_type: Some(InspectionSourceType::ProductionReceipt),
                    source_id: Some(picking_id),
                    ..Default::default()
                },
                one.clone(),
            )
            .await
            .map(|p| p.total)
            .unwrap_or(0);
        if total == 0 {
            return Ok(FqcGate::PendingInspection);
        }
        let passed = insp_svc
            .list_by_source(
                ctx,
                db,
                InspectionResultFilter {
                    source_type: Some(InspectionSourceType::ProductionReceipt),
                    source_id: Some(picking_id),
                    status: Some(InspectionStatus::Completed),
                    result: Some(InspectionResultType::Pass),
                    ..Default::default()
                },
                one,
            )
            .await
            .map(|p| p.total)
            .unwrap_or(0);
        Ok(if passed == total {
            FqcGate::AllPassed
        } else {
            FqcGate::HasFailed
        })
    }

    async fn get_production_detail(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        picking_id: i64,
    ) -> Result<ProductionReceiptDetail> {
        use crate::wms::stock_ledger::repo::StockLedgerRepo;

        let picking = self.get(ctx, db, picking_id).await?;
        let work_order_id = picking.source_id.ok_or_else(|| {
            DomainError::business_rule("入库 picking 缺少 source_id（work_order_id）")
        })?;
        let items = PickingRepo::get_items(&mut *db, picking_id).await?;
        let item = items
            .first()
            .ok_or_else(|| DomainError::business_rule("入库 picking 无明细"))?;
        let product_id = item.product_id;
        let received_qty = item.qty_requested;
        let batch_id = item.batch_id;

        // 关联名查询（work_order_doc / batch_no / product_name / warehouse_name）
        let wo_doc: Option<(String,)> =
            sqlx::query_as("SELECT doc_number FROM work_orders WHERE id = $1")
                .bind(work_order_id)
                .fetch_optional(&mut *db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        let batch_no: Option<(String,)> = if let Some(bid) = batch_id {
            sqlx::query_as("SELECT batch_no FROM production_batches WHERE id = $1")
                .bind(bid)
                .fetch_optional(&mut *db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
        } else {
            None
        };
        let product_name: Option<(String,)> =
            sqlx::query_as("SELECT pdt_name FROM products WHERE product_id = $1")
                .bind(product_id)
                .fetch_optional(&mut *db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        let warehouse_name: Option<(String,)> = if let Some(wid) = picking.to_warehouse_id {
            sqlx::query_as("SELECT name FROM warehouses WHERE id = $1")
                .bind(wid)
                .fetch_optional(&mut *db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
        } else {
            None
        };

        let unit_cost = StockLedgerRepo::last_known_unit_cost(&mut *db, product_id)
            .await
            .unwrap_or(Decimal::ZERO);

        Ok(ProductionReceiptDetail {
            picking,
            work_order_id,
            product_id,
            batch_id,
            received_qty,
            work_order_doc: wo_doc.map(|r| r.0),
            batch_no: batch_no.map(|r| r.0),
            product_name: product_name.map(|r| r.0),
            warehouse_name: warehouse_name.map(|r| r.0),
            unit_cost,
        })
    }

    async fn list_productions(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: ProductionReceiptFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<ProductionReceiptListItem>> {
        PickingRepo::list_productions(&mut *db, &filter, page.page, page.page_size).await
    }
}
