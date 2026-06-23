use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use crate::qms::inspection_result::{new_inspection_result_service, service::InspectionResultService};
use crate::qms::inspection_result::model::InspectionResultFilter;
use crate::qms::enums::{InspectionResultType, InspectionSourceType, InspectionStatus};
use crate::sales::sales_order::model::SalesOrderStatus;
use crate::sales::sales_order::repo::{SalesOrderItemRepo, SalesOrderRepo};
use crate::sales::sales_order::{new_sales_order_service, service::SalesOrderService};
use crate::sales::shipping_request::model::*;
use crate::sales::shipping_request::repo::{ShippingRequestItemRepo, ShippingRequestRepo};
use crate::sales::shipping_request::service::ShippingRequestService;
use crate::fms::ar_ap::enums::LedgerDirection;
use crate::fms::ar_ap::repo::{ArApLedgerInsert, ArApLedgerRepo};
use crate::fms::enums::CounterpartyType;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::cost_entry::{new_cost_entry_service, service::CostEntryService};
use crate::shared::cost_entry::model::EntryRequest;
use crate::shared::document_link::{new_document_link_service, service::DocumentLinkService};
use crate::shared::document_link::model::LinkRequest;
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::cost::{CostEntityType, CostType};
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
    model::RecordTransactionReq, new_inventory_transaction_service, service::InventoryTransactionService,
};

pub struct ShippingRequestServiceImpl {
    repo: ShippingRequestRepo,
    item_repo: ShippingRequestItemRepo,
    order_repo: SalesOrderRepo,
    order_item_repo: SalesOrderItemRepo,
    pool: PgPool,
}

impl ShippingRequestServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: ShippingRequestRepo,
            item_repo: ShippingRequestItemRepo,
            order_repo: SalesOrderRepo,
            order_item_repo: SalesOrderItemRepo,
            pool,
        }
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
            && order.status != SalesOrderStatus::PartiallyShipped
        {
            return Err(DomainError::business_rule(
                "Order must be Confirmed or PartiallyShipped to create shipping request",
            ));
        }

        let order_items = self
            .order_item_repo
            .find_by_order_id(db, req.order_id)
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

        // 如果有明细行，写入
        if !req.items.is_empty() {
            let item_inputs: Vec<ShippingItemInput> = req
                .items
                .iter()
                .enumerate()
                .map(|(i, item)| ShippingItemInput {
                    line_no: (i + 1) as i32,
                    order_item_id: item.order_item_id.unwrap_or(0),
                    product_id: item.product_id.unwrap_or(0),
                    warehouse_id: item.warehouse_id,
                    requested_qty: item.requested_qty,
                    description: item.description.clone(),
                })
                .collect();
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

        // 如果传了 items，全量替换明细行
        if let Some(items) = req.items {
            self.item_repo.delete_by_shipping_request_id(db, id).await?;
            if !items.is_empty() {
                let item_inputs: Vec<ShippingItemInput> = items
                    .iter()
                    .enumerate()
                    .map(|(i, item)| ShippingItemInput {
                        line_no: (i + 1) as i32,
                        order_item_id: item.order_item_id.unwrap_or(0),
                        product_id: item.product_id.unwrap_or(0),
                        warehouse_id: item.warehouse_id,
                        requested_qty: item.requested_qty,
                        description: item.description.clone(),
                    })
                    .collect();
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

    async fn pick(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ShippingRequest"))?;

        if existing.status != ShippingStatus::Confirmed {
            return Err(DomainError::business_rule("Only Confirmed shipping requests can be picked"));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ShippingStatus", id, "Picking", None)
            .await?;

        self.repo
            .update_status(db, id, ShippingStatus::Picking)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx,
                    db,
                    RecordAuditLogReq {
                        entity_type: "ShippingRequest",
                        entity_id: id,
                        action: AuditAction::Transition,
                        changes: Some(serde_json::json!({ "from": "Confirmed", "to": "Picking" })),
                        context: None,
                    },
                )
            .await?;

        Ok(())
    }

    async fn ship(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ShippingRequest"))?;

        if existing.status != ShippingStatus::Picking {
            return Err(DomainError::business_rule("Only Picking shipping requests can be shipped"));
        }

        let order_id = existing.order_id.ok_or_else(|| {
            DomainError::business_rule("发货单缺少关联订单，无法发货")
        })?;

        let shipping_items = self
            .item_repo
            .find_by_shipping_request_id(db, id)
            .await?;

        for item in &shipping_items {
            self.item_repo
                .update_shipped_qty(db, item.id, item.requested_qty)
                .await?;

            self.order_item_repo
                .update_shipped_qty(db, item.order_item_id, item.requested_qty)
                .await?;

            new_inventory_reservation_service(self.pool.clone())
                .fulfill_by_source_line(
                    ctx,
                    db,
                    DocumentType::SalesOrder,
                    item.order_item_id,
                )
                .await?;

            // 销售出库：记 SalesShipment 库存事务（负向扣减实物库存）。
            // 修复：原 ship() 只履行预留 + 记 COGS，未扣实物库存 → 销售发货后台账无变化。
            // 先 fulfill 释放预留（ATP 回升），再出库扣减，避免与预检冲突。
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
                        warehouse_id: item.warehouse_id,
                        zone_id: None,
                        bin_id: None,
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

        // COGS entries
        let order_items = self
            .order_item_repo
            .find_by_order_id(db, order_id)
            .await?;

        let period = chrono::Utc::now().format("%Y-%m").to_string();
        let mut cost_entries = Vec::with_capacity(shipping_items.len());
        for ship_item in &shipping_items {
            let unit_cost = order_items
                .iter()
                .find(|oi| oi.id == ship_item.order_item_id)
                .map(|oi| oi.unit_cost)
                .unwrap_or(Decimal::ZERO);

            let cogs_amount = ship_item.requested_qty * unit_cost;
            cost_entries.push(EntryRequest {
                entity_type: CostEntityType::SalesOrder,
                entity_id: order_id,
                cost_type: CostType::Material,
                debit_amount: cogs_amount,
                credit_amount: Decimal::ZERO,
                cost_center: None,
                profit_center: None,
                period: period.clone(),
                source_type: DocumentType::ShippingRequest,
                source_id: id,
            });
        }

        if !cost_entries.is_empty() {
            new_cost_entry_service(self.pool.clone())
                .create_entries(ctx, db, cost_entries)
                .await?;
        }

        // 业财一体：发货即立 AR 台账（直接 insert，不经发票实体）
        // 幂等：同一发货单不重复立账
        let dup_ledger: Option<i64> = sqlx::query_scalar::<sqlx::Postgres, i64>(
            "SELECT id FROM ar_ap_ledger WHERE source_type = $1 AND source_id = $2 LIMIT 1",
        )
        .bind(DocumentType::ShippingRequest)
        .bind(id)
        .fetch_optional(&mut *db)
        .await?;

        if dup_ledger.is_none() {
            // 应收金额 = Σ 发货明细数量 × 订单行售价
            let ar_amount: Decimal = shipping_items
                .iter()
                .filter_map(|si| {
                    order_items
                        .iter()
                        .find(|oi| oi.id == si.order_item_id)
                        .map(|oi| si.requested_qty * oi.unit_price)
                })
                .sum();

            if ar_amount > Decimal::ZERO {
                // 到期日由客户 payment_terms 推导
                let (customer_currency, payment_terms): (Option<String>, Option<String>) =
                    sqlx::query_as::<sqlx::Postgres, (Option<String>, Option<String>)>(
                        "SELECT currency, payment_terms FROM customers WHERE customer_id = $1 AND deleted_at IS NULL",
                    )
                    .bind(existing.customer_id)
                    .fetch_optional(&mut *db)
                    .await?
                    .unwrap_or((None, None));
                let due_days =
                    crate::fms::ar_ap::payment_terms::parse_payment_terms_days(payment_terms.as_deref());
                let today = chrono::Local::now().date_naive();
                let due_date = today + chrono::Duration::days(due_days);
                let currency = customer_currency
                    .filter(|c| !c.is_empty())
                    .unwrap_or_else(|| "CNY".to_string());
                let period = chrono::Utc::now().format("%Y-%m").to_string();
                let doc_no = existing.doc_number.clone();
                let desc = format!("销售发货 {}", doc_no);

                let _ = ArApLedgerRepo::insert(
                    db,
                    &ArApLedgerInsert {
                        party_type: CounterpartyType::Customer,
                        party_id: existing.customer_id,
                        source_type: DocumentType::ShippingRequest,
                        source_id: id,
                        source_doc_no: &doc_no,
                        against_type: None,
                        against_id: None,
                        direction: LedgerDirection::Debit,
                        amount: ar_amount,
                        currency: &currency,
                        exchange_rate: Decimal::ONE,
                        transaction_date: today,
                        due_date: Some(due_date),
                        period: &period,
                        description: &desc,
                        operator_id: ctx.operator_id,
                    },
                )
                .await?;
            }
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ShippingStatus", id, "Shipped", None)
            .await?;

        self.repo
            .update_status(db, id, ShippingStatus::Shipped)
            .await?;

        // Update SalesOrder status: PartiallyShipped or Shipped
        let all_fully_shipped = order_items
            .iter()
            .all(|oi| oi.shipped_qty >= oi.quantity);

        let new_order_status = if all_fully_shipped {
            SalesOrderStatus::Shipped
        } else {
            SalesOrderStatus::PartiallyShipped
        };

        self.order_repo
            .update_status(db, order_id, new_order_status)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx,
                    db,
                    RecordAuditLogReq {
                        entity_type: "ShippingRequest",
                        entity_id: id,
                        action: AuditAction::Transition,
                        changes: Some(serde_json::json!({ "from": "Picking", "to": "Shipped" })),
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
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(())
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
}
