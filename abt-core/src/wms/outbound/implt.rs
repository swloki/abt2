use sqlx::postgres::PgPool;
use rust_decimal::Decimal;

use crate::qms::inspection_result::{new_inspection_result_service, service::InspectionResultService};
use crate::qms::inspection_result::model::InspectionResultFilter;
use crate::qms::enums::{InspectionResultType, InspectionSourceType, InspectionStatus};
use crate::sales::sales_order::model::SalesOrderStatus;
use crate::sales::sales_order::model::ShipmentLineQty;
use crate::sales::sales_order::{new_sales_order_service, service::SalesOrderService};
use crate::wms::outbound::model::*;
use crate::wms::outbound::repo::{ShippingRequestItemRepo, ShippingRequestRepo};
use crate::wms::outbound::service::ShippingRequestService;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::document_link::{new_document_link_service, service::DocumentLinkService};
use crate::shared::document_link::model::LinkRequest;
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::enums::link_type::LinkType;
use crate::shared::event_bus::{new_domain_event_bus, service::DomainEventBus};
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::inventory_reservation::{new_inventory_reservation_service, service::InventoryReservationService};
use crate::shared::state_machine::{new_state_machine_service, service::StateMachineService};
use crate::shared::types::{PgExecutor, DomainError, PageParams, PaginatedResult, ServiceContext, Result};
use crate::wms::enums::TransactionType;
use crate::wms::inventory_transaction::{
    model::{InventoryTransaction, RecordTransactionReq}, new_inventory_transaction_service, service::InventoryTransactionService,
};

pub struct ShippingRequestServiceImpl {
    repo: ShippingRequestRepo,
    item_repo: ShippingRequestItemRepo,
    pool: PgPool,
}

impl ShippingRequestServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: ShippingRequestRepo,
            item_repo: ShippingRequestItemRepo,
            pool,
        }
    }

    /// 草稿明细的 product_id 解析：前端基于「选订单行」提交，product_id 隐含于 order_item。
    /// 若未显式传 product_id，则按 order_item_id 反查 sales_order_items 填充；
    /// 最终 product_id 仍为 0 则报错（杜绝 product_id=0 脏数据，见 SR-2026-06-000043 事故）。
    async fn resolve_draft_items(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: Option<i64>,
        items: &[CreateDraftItemReq],
    ) -> Result<Vec<ShippingItemInput>> {
        let order_items = if let Some(oid) = order_id {
            new_sales_order_service(self.pool.clone())
                .list_items(ctx, db, oid)
                .await?
        } else {
            Vec::new()
        };
        let item_inputs: Vec<ShippingItemInput> = items
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
                ShippingItemInput {
                    line_no: (i + 1) as i32,
                    order_item_id: item.order_item_id.unwrap_or(0),
                    product_id,
                    warehouse_id: item.warehouse_id,
                    requested_qty: item.requested_qty,
                    description: item.description.clone(),
                }
            })
            .collect();
        if item_inputs.iter().any(|i| i.product_id == 0) {
            return Err(DomainError::validation(
                "发货明细必须关联订单行或指定商品（product_id 缺失，无法确定发货商品）",
            ));
        }
        Ok(item_inputs)
    }

    /// 发货核心：扣库存 + 释放预留 + 回写销售订单 + 转 Shipped + 发布 ShipmentShipped 事件。
    /// `wh_bin`: shipping_item_id → (warehouse_id, bin_id)，由 direct_ship 从选仓参数组装。
    /// AR/COGS 由 fms 异步消费 ShipmentShipped 事件立账，与仓库无关。
    async fn do_ship(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        existing: &ShippingRequest,
        shipping_items: &[ShippingRequestItem],
        order_id: i64,
        wh_bin: &std::collections::HashMap<i64, (Option<i64>, Option<i64>)>,
    ) -> Result<()> {
        let id = existing.id;
        for item in shipping_items {
            // 发货单自身 shipped_qty
            self.item_repo
                .update_shipped_qty(db, item.id, item.requested_qty)
                .await?;

            new_inventory_reservation_service(self.pool.clone())
                .fulfill_by_source_line(ctx, db, DocumentType::SalesOrder, item.order_item_id)
                .await?;

            // 销售出库：SalesShipment 库存事务（负向扣减实物库存）
            let (wh_id, bin_id) = wh_bin.get(&item.id).cloned().unwrap_or((None, None));
            let wh_id = wh_id.ok_or_else(|| {
                DomainError::business_rule(format!("发货行 {} 未指定仓库，无法出库", item.id))
            })?;
            new_inventory_transaction_service(self.pool.clone())
                .record(
                    ctx,
                    db,
                    RecordTransactionReq {
                        doc_number: None,
                        delivery_no: None,
                        source_doc_number: Some(existing.doc_number.clone()),
                        transaction_type: TransactionType::SalesShipment,
                        product_id: item.product_id,
                        warehouse_id: wh_id,
                        zone_id: None,
                        bin_id,
                        batch_no: None,
                        quantity: -item.requested_qty,
                        unit_cost: None,
                        source_type: "shipping".to_string(),
                        source_id: id,
                        remark: None,
                    },
                )
                .await?;
        }

        // 回写销售订单 shipped_qty + 重算头状态（跨域 SalesOrderService::record_shipment）
        let lines: Vec<ShipmentLineQty> = shipping_items
            .iter()
            .map(|i| ShipmentLineQty {
                order_item_id: i.order_item_id,
                shipped_qty: i.requested_qty,
            })
            .collect();
        new_sales_order_service(self.pool.clone())
            .record_shipment(ctx, db, order_id, &lines)
            .await?;

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ShippingStatus", id, "Shipped", None)
            .await?;

        self.repo
            .update_status(db, id, ShippingStatus::Shipped)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "ShippingRequest",
                    entity_id: id,
                    action: AuditAction::Transition,
                    changes: Some(serde_json::json!({ "from": "Confirmed", "to": "Shipped" })),
                    context: None,
                },
            )
            .await?;

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::ShipmentShipped,
                    aggregate_type: "ShippingRequest".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({
                        "shipping_request_id": id,
                        "doc_number": existing.doc_number,
                        "order_id": order_id,
                        "customer_id": existing.customer_id,
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl ShippingRequestService for ShippingRequestServiceImpl {
    async fn create_from_order(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateFromOrderReq,
    ) -> Result<i64> {
        let order = new_sales_order_service(self.pool.clone()).find_by_id(ctx, db, req.order_id).await?;

        if order.status != SalesOrderStatus::Confirmed
            && order.status != SalesOrderStatus::ReadyToShip
            && order.status != SalesOrderStatus::PartiallyShipped
        {
            return Err(DomainError::business_rule(
                "Order must be Confirmed, ReadyToShip or PartiallyShipped to create shipping request",
            ));
        }

        let order_items = new_sales_order_service(self.pool.clone())
            .list_items(ctx, db, req.order_id)
            .await?;

        let mut shipping_inputs = Vec::with_capacity(req.items.len());
        for (i, item) in req.items.iter().enumerate() {
            let order_item = order_items
                .iter()
                .find(|oi| oi.id == item.order_item_id)
                .ok_or_else(|| {
                    DomainError::validation(format!(
                        "Order item {} not found in order {}",
                        item.order_item_id, req.order_id
                    ))
                })?;

            let remaining = order_item.quantity - order_item.shipped_qty;
            if item.requested_qty > remaining {
                return Err(DomainError::business_rule(format!(
                    "Item {} requested qty {} exceeds remaining {}",
                    item.order_item_id, item.requested_qty, remaining
                )));
            }

            shipping_inputs.push(ShippingItemInput {
                line_no: (i + 1) as i32,
                order_item_id: item.order_item_id,
                product_id: order_item.product_id,
                warehouse_id: item.warehouse_id,
                requested_qty: item.requested_qty,
                description: order_item.description.clone(),
            });
        }

        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::ShippingRequest)
            .await?;

        let id = self
            .repo
            .create(
                db,
                &CreateShippingRequestParams {
                    doc_number: &doc_number,
                    order_id: Some(req.order_id),
                    customer_id: order.customer_id,
                    expected_ship_date: req.expected_ship_date,
                    shipping_address: req.shipping_address.as_deref().unwrap_or(""),
                    carrier: "",
                    remark: "",
                    operator_id: ctx.operator_id,
                },
            )
            .await?;

        self.item_repo
            .create_batch(db, id, &shipping_inputs)
            .await?;

        new_document_link_service(self.pool.clone())
            .create_links(
                ctx,
                db,
                vec![LinkRequest {
                    source_type: DocumentType::ShippingRequest,
                    source_id: id,
                    target_type: DocumentType::SalesOrder,
                    target_id: req.order_id,
                    link_type: LinkType::Triggers,
                }],
            )
            .await?;

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ShippingStatus", id, "Draft", None)
            .await
            .ok();

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx,
                    db,
                    RecordAuditLogReq {
                        entity_type: "ShippingRequest",
                        entity_id: id,
                        action: AuditAction::Create,
                        changes: Some(serde_json::json!({ "order_id": req.order_id })),
                        context: None,
                    },
                )
            .await?;

        Ok(id)
    }

    async fn request_from_order(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
        items: Vec<RequestShippingItemReq>,
    ) -> Result<i64> {
        let so_svc = new_sales_order_service(self.pool.clone());
        let order = so_svc.find_by_id(ctx, db, order_id).await?;

        // 允许 Confirmed/ReadyToShip/PartiallyShipped/ShippingRequested（后者支持追加申请）
        if !matches!(
            order.status,
            SalesOrderStatus::Confirmed
                | SalesOrderStatus::ReadyToShip
                | SalesOrderStatus::PartiallyShipped
                | SalesOrderStatus::ShippingRequested
        ) {
            return Err(DomainError::business_rule("订单当前状态不允许申请发货"));
        }

        let order_items = so_svc.list_items(ctx, db, order_id).await?;
        let mut shipping_inputs = Vec::with_capacity(items.len());
        for (i, item) in items.iter().enumerate() {
            if item.requested_qty <= Decimal::ZERO {
                return Err(DomainError::validation("申请数量必须大于 0"));
            }
            let order_item = order_items
                .iter()
                .find(|oi| oi.id == item.order_item_id)
                .ok_or_else(|| {
                    DomainError::validation(format!("订单行 {} 不存在", item.order_item_id))
                })?;
            let remaining = order_item.quantity - order_item.shipped_qty;
            if item.requested_qty > remaining {
                return Err(DomainError::business_rule(format!(
                    "订单行 {} 申请数量 {} 超过未发数量 {}",
                    item.order_item_id, item.requested_qty, remaining
                )));
            }
            shipping_inputs.push(ShippingItemInput {
                line_no: (i + 1) as i32,
                order_item_id: item.order_item_id,
                product_id: order_item.product_id,
                warehouse_id: None, // 销售不指定仓库，仓库拣货时手选
                requested_qty: item.requested_qty,
                description: order_item.description.clone(),
            });
        }

        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::ShippingRequest)
            .await?;
        let id = self
            .repo
            .create(
                db,
                &CreateShippingRequestParams {
                    doc_number: &doc_number,
                    order_id: Some(order_id),
                    customer_id: order.customer_id,
                    expected_ship_date: None,
                    shipping_address: order.delivery_address.as_str(),
                    carrier: "",
                    remark: "",
                    operator_id: ctx.operator_id,
                },
            )
            .await?;
        self.item_repo.create_batch(db, id, &shipping_inputs).await?;

        new_document_link_service(self.pool.clone())
            .create_links(
                ctx, db,
                vec![LinkRequest {
                    source_type: DocumentType::ShippingRequest,
                    source_id: id,
                    target_type: DocumentType::SalesOrder,
                    target_id: order_id,
                    link_type: LinkType::Triggers,
                }],
            )
            .await?;

        // 跳过 Draft → 直接 Confirmed（入 work-center 待发货队列）
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ShippingStatus", id, "Confirmed", None)
            .await
            .ok();
        self.repo.update_status(db, id, ShippingStatus::Confirmed).await?;

        // 回写订单状态 → recalc_header_status 叠加判定 ShippingRequested
        so_svc.recalc_header_status(ctx, db, order_id).await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx, db,
                RecordAuditLogReq {
                    entity_type: "ShippingRequest",
                    entity_id: id,
                    action: AuditAction::Create,
                    changes: Some(serde_json::json!({ "order_id": order_id, "via": "request_from_order" })),
                    context: None,
                },
            )
            .await?;

        Ok(id)
    }

    async fn save_draft(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateDraftReq,
    ) -> Result<i64> {
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::ShippingRequest)
            .await?;

        let id = self
            .repo
            .create(
                db,
                &CreateShippingRequestParams {
                    doc_number: &doc_number,
                    order_id: req.order_id,
                    customer_id: req.customer_id,
                    expected_ship_date: req.expected_ship_date,
                    shipping_address: req.shipping_address.as_deref().unwrap_or(""),
                    carrier: req.carrier.as_deref().unwrap_or(""),
                    remark: req.remark.as_deref().unwrap_or(""),
                    operator_id: ctx.operator_id,
                },
            )
            .await?;

        // 如果有明细行，写入（product_id 由 resolve_draft_items 反查填充并校验）
        if !req.items.is_empty() {
            let item_inputs = self.resolve_draft_items(ctx, db, req.order_id, &req.items).await?;
            self.item_repo.create_batch(db, id, &item_inputs).await?;
        }

        // 如果关联了订单，建立文档链接
        if let Some(order_id) = req.order_id {
            new_document_link_service(self.pool.clone())
                .create_links(
                    ctx,
                    db,
                    vec![LinkRequest {
                        source_type: DocumentType::ShippingRequest,
                        source_id: id,
                        target_type: DocumentType::SalesOrder,
                        target_id: order_id,
                        link_type: LinkType::Triggers,
                    }],
                )
                .await?;
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ShippingStatus", id, "Draft", None)
            .await
            .ok();

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "ShippingRequest",
                    entity_id: id,
                    action: AuditAction::Create,
                    changes: Some(serde_json::json!({
                        "order_id": req.order_id,
                        "customer_id": req.customer_id,
                        "is_draft": true,
                    })),
                    context: None,
                },
            )
            .await?;

        Ok(id)
    }

    async fn update_draft(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateDraftReq,
    ) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ShippingRequest"))?;

        if existing.status != ShippingStatus::Draft {
            return Err(DomainError::business_rule("仅草稿状态的发货单可以编辑"));
        }

        self.repo
            .update_draft_fields(
                db,
                id,
                req.order_id,
                req.customer_id,
                req.expected_ship_date,
                req.shipping_address.as_deref(),
                req.carrier.as_deref(),
                req.remark.as_deref(),
            )
            .await?;

        // 如果传了 items，全量替换明细行（product_id 由 resolve_draft_items 反查填充并校验）
        if let Some(items) = req.items {
            self.item_repo.delete_by_shipping_request_id(db, id).await?;
            if !items.is_empty() {
                let order_id_for_items = req.order_id.or(existing.order_id);
                let item_inputs = self.resolve_draft_items(ctx, db, order_id_for_items, &items).await?;
                self.item_repo.create_batch(db, id, &item_inputs).await?;
            }
        }

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "ShippingRequest",
                    entity_id: id,
                    action: AuditAction::Update,
                    changes: None,
                    context: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn find_by_id(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<ShippingRequest> {
        self.repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ShippingRequest"))
    }

    async fn update(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateShippingReq,
    ) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ShippingRequest"))?;

        if existing.status != ShippingStatus::Draft {
            return Err(DomainError::business_rule("Only Draft shipping requests can be updated"));
        }

        self.repo
            .update(db, id, &req)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "ShippingRequest", entity_id: id, action: AuditAction::Update, changes: None, context: None })
            .await?;

        Ok(())
    }

    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ShippingRequest"))?;

        if existing.status != ShippingStatus::Draft {
            return Err(DomainError::business_rule("Only Draft shipping requests can be confirmed"));
        }

        if existing.order_id.is_none() {
            return Err(DomainError::business_rule("草稿必须关联销售订单后才能确认"));
        }

        // QMS OQC hard gate: 查询发货请求的检验结果
        let qms_results = new_inspection_result_service(self.pool.clone()).list_by_source(
            ctx,
            db,
            InspectionResultFilter {
                source_type: Some(InspectionSourceType::ShippingRequest),
                source_id: Some(id),
                ..Default::default()
            },
            crate::shared::types::pagination::PageParams { page: 1, page_size: 100 },
        )
        .await?;

        if !qms_results.items.is_empty() {
            let all_passed = qms_results.items.iter().all(|r| {
                r.status == InspectionStatus::Completed && r.result == InspectionResultType::Pass
            });
            if !all_passed {
                return Err(DomainError::business_rule("OQC 检验未通过，无法发货"));
            }
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ShippingStatus", id, "Confirmed", None)
            .await?;

        self.repo
            .update_status(db, id, ShippingStatus::Confirmed)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx,
                    db,
                    RecordAuditLogReq {
                        entity_type: "ShippingRequest",
                        entity_id: id,
                        action: AuditAction::Transition,
                        changes: Some(serde_json::json!({ "from": "Draft", "to": "Confirmed" })),
                        context: None,
                    },
                )
            .await?;

        Ok(())
    }

    async fn direct_ship(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        warehouse_id: i64,
        bin_id: Option<i64>,
    ) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ShippingRequest"))?;

        // 直发：仅 Confirmed 单可用（拣货已移除，无 Picking 中间态）
        if existing.status != ShippingStatus::Confirmed {
            return Err(DomainError::business_rule(
                "只有已确认的发货单可发货",
            ));
        }

        let order_id = existing.order_id.ok_or_else(|| {
            DomainError::business_rule("发货单缺少关联订单，无法发货")
        })?;

        let shipping_items = self
            .item_repo
            .find_by_shipping_request_id(db, id)
            .await?;

        // 所有行统一用调用方指定的仓库/库位（选仓 drawer传入）
        let wh_bin: std::collections::HashMap<i64, (Option<i64>, Option<i64>)> = shipping_items
            .iter()
            .map(|it| (it.id, (Some(warehouse_id), bin_id)))
            .collect();

        self.do_ship(ctx, db, &existing, &shipping_items, order_id, &wh_bin)
            .await
    }

    async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ShippingRequest"))?;

        if existing.status != ShippingStatus::Draft && existing.status != ShippingStatus::Confirmed {
            return Err(DomainError::business_rule(
                "Only Draft or Confirmed shipping requests can be cancelled",
            ));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ShippingStatus", id, "Cancelled", None)
            .await?;

        self.repo
            .update_status(db, id, ShippingStatus::Cancelled)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx,
                    db,
                    RecordAuditLogReq {
                        entity_type: "ShippingRequest",
                        entity_id: id,
                        action: AuditAction::Transition,
                        changes: Some(serde_json::json!({ "from": existing.status.as_str(), "to": "Cancelled" })),
                        context: None,
                    },
                )
            .await?;

        Ok(())
    }

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: ShippingQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<ShippingRequest>> {
        self.repo
            .query(
                db,
                &filter,
                &page,
                ctx.data_scope,
                ctx.operator_id,
                ctx.department_id,
            )
            .await

    }

    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self.repo.find_by_id(db, id).await?
            .ok_or_else(|| DomainError::not_found("ShippingRequest"))?;

        if existing.status != ShippingStatus::Draft {
            return Err(DomainError::business_rule("仅草稿状态的发货单可以删除"));
        }

        self.repo.soft_delete(db, id).await?;

        new_audit_log_service(self.pool.clone()).record(ctx, db, RecordAuditLogReq { entity_type: "ShippingRequest", entity_id: id, action: AuditAction::Delete, changes: None, context: None }).await?;

        Ok(())
    }

    async fn list_items(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        shipping_request_id: i64,
    ) -> Result<Vec<ShippingRequestItem>> {
        self.item_repo.find_by_shipping_request_id(db, shipping_request_id).await
    }

    async fn hub_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<ShippingHubSummary> {
        let items = self.item_repo.find_by_shipping_request_id(db, id).await?;
        let pending_ship_qty: rust_decimal::Decimal =
            items.iter().map(|i| i.requested_qty).sum();
        let shipped_qty: rust_decimal::Decimal =
            items.iter().map(|i| i.shipped_qty).sum();

        // 缺货判定：任一明细 ATP < 待发量（requested - shipped）即缺货。
        // 批量 ATP（按 warehouse 分组，每组一条 query_available_batch），替代逐明细 query_available 的 N+1。
        let txn_svc = new_inventory_transaction_service(self.pool.clone());
        let mut by_warehouse: std::collections::HashMap<Option<i64>, Vec<&ShippingRequestItem>> =
            std::collections::HashMap::new();
        for item in &items {
            if item.requested_qty - item.shipped_qty <= rust_decimal::Decimal::ZERO {
                continue;
            }
            by_warehouse.entry(item.warehouse_id).or_default().push(item);
        }
        let mut shortage = None;
        'outer: for (wh, wh_items) in by_warehouse {
            let pids: Vec<i64> = wh_items.iter().map(|i| i.product_id).collect();
            let atp_map = match txn_svc
                .query_available_batch(ctx, db, &pids, wh)
                .await
            {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!(error = %e, warehouse_id = wh, "hub_summary: query_available_batch failed");
                    continue;
                }
            };
            for it in wh_items {
                let remaining = it.requested_qty - it.shipped_qty;
                let atp = atp_map
                    .get(&it.product_id)
                    .copied()
                    .unwrap_or(rust_decimal::Decimal::ZERO);
                if atp < remaining {
                    shortage = Some(ShortageSignal {
                        product_id: it.product_id,
                        product_name: format!("产品 #{}", it.product_id),
                        requested_qty: it.requested_qty,
                        available_qty: atp,
                    });
                    break 'outer;
                }
            }
        }

        Ok(ShippingHubSummary {
            pending_ship_qty,
            shipped_qty,
            shortage,
        })
    }

    async fn list_transactions(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        page: PageParams,
    ) -> Result<PaginatedResult<InventoryTransaction>> {
        // 数据库分页（source_type="shipping"，与 ship() record 的 source_type 对齐），
        // 替代 find_by_source 拉全量 + 内存 skip/take
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
}
