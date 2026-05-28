use std::sync::Arc;

use rust_decimal::Decimal;

use crate::sales::sales_order::model::SalesOrderStatus;
use crate::sales::sales_order::repo::{SalesOrderItemRepo, SalesOrderRepo};
use crate::sales::sales_order::service::SalesOrderService;
use crate::sales::shipping_request::model::*;
use crate::sales::shipping_request::repo::{ShippingRequestItemRepo, ShippingRequestRepo};
use crate::sales::shipping_request::service::ShippingRequestService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::cost_entry::model::EntryRequest;
use crate::shared::cost_entry::service::CostEntryService;
use crate::shared::document_link::model::LinkRequest;
use crate::shared::document_link::service::DocumentLinkService;
use crate::qms::inspection_result::service::InspectionResultService;
use crate::qms::inspection_result::model::InspectionResultFilter;
use crate::qms::enums::{InspectionResultType, InspectionSourceType, InspectionStatus};
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::cost::{CostEntityType, CostType};
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::enums::link_type::LinkType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::inventory_reservation::service::InventoryReservationService;
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::types::{PgExecutor,DomainError, PageParams, PaginatedResult, ServiceContext, Result};

pub struct ShippingRequestServiceImpl {
    repo: ShippingRequestRepo,
    item_repo: ShippingRequestItemRepo,
    order_repo: SalesOrderRepo,
    order_item_repo: SalesOrderItemRepo,
    doc_seq: Arc<dyn DocumentSequenceService>,
    state_machine: Arc<dyn StateMachineService>,
    audit: Arc<dyn AuditLogService>,
    event_bus: Arc<dyn DomainEventBus>,
    sales_order_svc: Arc<dyn SalesOrderService>,
    doc_link: Arc<dyn DocumentLinkService>,
    inv_res: Arc<dyn InventoryReservationService>,
    cost_entry: Arc<dyn CostEntryService>,
    qms: Arc<dyn InspectionResultService>,
}

impl ShippingRequestServiceImpl {
    pub fn new(
        repo: ShippingRequestRepo,
        item_repo: ShippingRequestItemRepo,
        order_repo: SalesOrderRepo,
        order_item_repo: SalesOrderItemRepo,
        doc_seq: Arc<dyn DocumentSequenceService>,
        state_machine: Arc<dyn StateMachineService>,
        audit: Arc<dyn AuditLogService>,
        event_bus: Arc<dyn DomainEventBus>,
        sales_order_svc: Arc<dyn SalesOrderService>,
        doc_link: Arc<dyn DocumentLinkService>,
        inv_res: Arc<dyn InventoryReservationService>,
        cost_entry: Arc<dyn CostEntryService>,
        qms: Arc<dyn InspectionResultService>,
    ) -> Self {
        Self {
            repo,
            item_repo,
            order_repo,
            order_item_repo,
            doc_seq,
            state_machine,
            audit,
            event_bus,
            sales_order_svc,
            doc_link,
            inv_res,
            cost_entry,
            qms,
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
        let order = self.sales_order_svc.find_by_id(ctx, db, req.order_id).await?;

        if order.status != SalesOrderStatus::Confirmed
            && order.status != SalesOrderStatus::InProduction
            && order.status != SalesOrderStatus::PartiallyShipped
        {
            return Err(DomainError::business_rule(
                "Order must be Confirmed, InProduction or PartiallyShipped to create shipping request",
            ));
        }

        let order_items = self
            .order_item_repo
            .find_by_order_id(db, req.order_id)
            .await
            ?;

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

        let doc_number = self
            .doc_seq
            .next_number(ctx, db, DocumentType::ShippingRequest)
            .await?;

        let id = self
            .repo
            .create(
                db,
                &doc_number,
                req.order_id,
                order.customer_id,
                req.expected_ship_date,
                req.shipping_address.as_deref().unwrap_or(""),
                "",
                ctx.operator_id,
            )
            .await
            ?;

        self.item_repo
            .create_batch(db, id, &shipping_inputs)
            .await
            ?;

        self.doc_link
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

        self.state_machine
            .transition(ctx, db, "ShippingStatus", id, "Draft", None)
            .await
            .ok();

        self.audit
            .record(
                ctx,
                db,
                "ShippingRequest",
                id,
                AuditAction::Create,
                Some(serde_json::json!({ "order_id": req.order_id })),
                None,
            )
            .await?;

        Ok(id)
    }

    async fn find_by_id(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<ShippingRequest> {
        self.repo
            .find_by_id(db, id)
            .await
            ?
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
            .await
            ?
            .ok_or_else(|| DomainError::not_found("ShippingRequest"))?;

        if existing.status != ShippingStatus::Draft {
            return Err(DomainError::business_rule("Only Draft shipping requests can be updated"));
        }

        self.repo
            .update(db, id, &req)
            .await
            ?;

        self.audit
            .record(ctx, db, "ShippingRequest", id, AuditAction::Update, None, None)
            .await?;

        Ok(())
    }

    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("ShippingRequest"))?;

        if existing.status != ShippingStatus::Draft {
            return Err(DomainError::business_rule("Only Draft shipping requests can be confirmed"));
        }

        // QMS OQC hard gate: 查询发货请求的检验结果
        let qms_results = self.qms.list_by_source(
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

        self.state_machine
            .transition(ctx, db, "ShippingStatus", id, "Confirmed", None)
            .await?;

        self.repo
            .update_status(db, id, ShippingStatus::Confirmed)
            .await
            ?;

        self.audit
            .record(
                ctx,
                db,
                "ShippingRequest",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({ "from": "Draft", "to": "Confirmed" })),
                None,
            )
            .await?;

        Ok(())
    }

    async fn pick(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("ShippingRequest"))?;

        if existing.status != ShippingStatus::Confirmed {
            return Err(DomainError::business_rule("Only Confirmed shipping requests can be picked"));
        }

        self.state_machine
            .transition(ctx, db, "ShippingStatus", id, "Picking", None)
            .await?;

        self.repo
            .update_status(db, id, ShippingStatus::Picking)
            .await
            ?;

        self.audit
            .record(
                ctx,
                db,
                "ShippingRequest",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({ "from": "Confirmed", "to": "Picking" })),
                None,
            )
            .await?;

        Ok(())
    }

    async fn ship(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("ShippingRequest"))?;

        if existing.status != ShippingStatus::Picking {
            return Err(DomainError::business_rule("Only Picking shipping requests can be shipped"));
        }

        let shipping_items = self
            .item_repo
            .find_by_shipping_request_id(db, id)
            .await
            ?;

        for item in &shipping_items {
            self.item_repo
                .update_shipped_qty(db, item.id, item.requested_qty)
                .await
                ?;

            self.order_item_repo
                .update_shipped_qty(db, item.order_item_id, item.requested_qty)
                .await
                ?;

            self.inv_res
                .fulfill_by_source_line(
                    ctx,
                    db,
                    DocumentType::SalesOrder,
                    item.order_item_id,
                )
                .await?;
        }

        // COGS entries
        let order_items = self
            .order_item_repo
            .find_by_order_id(db, existing.order_id)
            .await
            ?;

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
                entity_id: existing.order_id,
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
            self.cost_entry
                .create_entries(ctx, db, cost_entries)
                .await?;
        }

        self.state_machine
            .transition(ctx, db, "ShippingStatus", id, "Shipped", None)
            .await?;

        self.repo
            .update_status(db, id, ShippingStatus::Shipped)
            .await
            ?;

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
            .update_status(db, existing.order_id, new_order_status)
            .await
            ?;

        self.audit
            .record(
                ctx,
                db,
                "ShippingRequest",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({ "from": "Picking", "to": "Shipped" })),
                None,
            )
            .await?;

        self.event_bus
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
                        "order_id": existing.order_id,
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
            .await
            ?
            .ok_or_else(|| DomainError::not_found("ShippingRequest"))?;

        if existing.status != ShippingStatus::Draft && existing.status != ShippingStatus::Confirmed {
            return Err(DomainError::business_rule(
                "Only Draft or Confirmed shipping requests can be cancelled",
            ));
        }

        self.state_machine
            .transition(ctx, db, "ShippingStatus", id, "Cancelled", None)
            .await?;

        self.repo
            .update_status(db, id, ShippingStatus::Cancelled)
            .await
            ?;

        self.audit
            .record(
                ctx,
                db,
                "ShippingRequest",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({ "from": existing.status.as_str(), "to": "Cancelled" })),
                None,
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

        self.audit.record(ctx, db, "ShippingRequest", id, AuditAction::Delete, None, None).await?;

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
