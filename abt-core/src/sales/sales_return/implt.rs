use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use crate::qms::rma::{new_rma_service, service::RmaService};
use crate::qms::rma::model::CreateRmaReq;
use crate::sales::sales_order::repo::SalesOrderItemRepo;
use crate::sales::sales_return::model::*;
use crate::sales::sales_return::repo::{SalesReturnItemRepo, SalesReturnRepo};
use crate::sales::sales_return::service::SalesReturnService;
use crate::sales::shipping_request::{new_shipping_request_service, service::ShippingRequestService};
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService};
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
use crate::shared::state_machine::{new_state_machine_service, service::StateMachineService};
use crate::shared::types::{PgExecutor, DomainError, PageParams, PaginatedResult, ServiceContext, Result};

pub struct SalesReturnServiceImpl {
    repo: SalesReturnRepo,
    item_repo: SalesReturnItemRepo,
    order_item_repo: SalesOrderItemRepo,
    pool: PgPool,
}

impl SalesReturnServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: SalesReturnRepo,
            item_repo: SalesReturnItemRepo,
            order_item_repo: SalesOrderItemRepo,
            pool,
        }
    }
}

#[async_trait::async_trait]
impl SalesReturnService for SalesReturnServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateReturnReq,
    ) -> Result<i64> {
        // Validate shipping request is Shipped
        let shipping = new_shipping_request_service(self.pool.clone()).find_by_id(ctx, db, req.shipping_request_id).await?;
        if shipping.status != crate::sales::shipping_request::model::ShippingStatus::Shipped {
            return Err(DomainError::business_rule(
                "Shipping request must be Shipped to create return",
            ));
        }

        if shipping.order_id != req.order_id {
            return Err(DomainError::validation(
                "Shipping request does not belong to the specified order",
            ));
        }

        // Get order items for validation and price lookup
        let order_items = self
            .order_item_repo
            .find_by_order_id(db, req.order_id)
            .await?;

        let mut return_inputs = Vec::with_capacity(req.items.len());
        let mut total_amount = Decimal::ZERO;

        // Get existing return items for this order to calculate already_returned_qty
        // For now, validate against shipped_qty directly
        for item in &req.items {
            let order_item = order_items
                .iter()
                .find(|oi| oi.id == item.order_item_id)
                .ok_or_else(|| {
                    DomainError::validation(format!(
                        "Order item {} not found in order {}",
                        item.order_item_id, req.order_id
                    ))
                })?;

            let max_returnable = order_item.shipped_qty - order_item.returned_qty;
            if item.returned_qty > max_returnable {
                return Err(DomainError::business_rule(format!(
                    "Item {} return qty {} exceeds returnable {}",
                    item.order_item_id, item.returned_qty, max_returnable
                )));
            }

            let amount = item.returned_qty * order_item.unit_price;
            total_amount += amount;

            return_inputs.push(ReturnItemInput {
                order_item_id: item.order_item_id,
                product_id: order_item.product_id,
                returned_qty: item.returned_qty,
                unit_price: order_item.unit_price,
                amount,
                disposition: item.disposition,
            });
        }

        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::SalesReturn)
            .await?;

        let id = self
            .repo
            .create(
                db,
                &doc_number,
                req.order_id,
                req.shipping_request_id,
                req.customer_id,
                &req.return_reason,
                total_amount,
                "",
                ctx.operator_id,
            )
            .await?;

        self.item_repo
            .create_batch(db, id, &return_inputs)
            .await?;

        new_document_link_service(self.pool.clone())
            .create_links(
                ctx,
                db,
                vec![LinkRequest {
                    source_type: DocumentType::SalesReturn,
                    source_id: id,
                    target_type: DocumentType::ShippingRequest,
                    target_id: req.shipping_request_id,
                    link_type: LinkType::References,
                }],
            )
            .await?;

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ReturnStatus", id, "Draft", None)
            .await
            .ok();

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                "SalesReturn",
                id,
                AuditAction::Create,
                Some(serde_json::json!({
                    "order_id": req.order_id,
                    "shipping_request_id": req.shipping_request_id,
                })),
                None,
            )
            .await?;

        Ok(id)
    }

    async fn find_by_id(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<SalesReturn> {
        self.repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesReturn"))
    }

    async fn approve(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesReturn"))?;

        if existing.status != ReturnStatus::Draft {
            return Err(DomainError::business_rule("Only Draft returns can be approved"));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ReturnStatus", id, "Confirmed", None)
            .await?;

        self.repo
            .update_status(db, id, ReturnStatus::Confirmed)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                "SalesReturn",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({ "from": "Draft", "to": "Confirmed" })),
                None,
            )
            .await?;

        Ok(())
    }

    async fn receive(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesReturn"))?;

        if existing.status != ReturnStatus::Confirmed {
            return Err(DomainError::business_rule("Only Confirmed returns can be received"));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ReturnStatus", id, "Received", None)
            .await?;

        self.repo
            .update_status(db, id, ReturnStatus::Received)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                "SalesReturn",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({ "from": "Confirmed", "to": "Received" })),
                None,
            )
            .await?;

        Ok(())
    }

    async fn inspect(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesReturn"))?;

        if existing.status != ReturnStatus::Received {
            return Err(DomainError::business_rule("Only Received returns can be inspected"));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ReturnStatus", id, "Inspecting", None)
            .await?;

        self.repo
            .update_status(db, id, ReturnStatus::Inspecting)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                "SalesReturn",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({ "from": "Received", "to": "Inspecting" })),
                None,
            )
            .await?;

        Ok(())
    }

    async fn complete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesReturn"))?;

        if existing.status != ReturnStatus::Inspecting {
            return Err(DomainError::business_rule("Only Inspecting returns can be completed"));
        }

        let return_items = self
            .item_repo
            .find_by_return_id(db, id)
            .await?;

        // Update order_item.returned_qty
        for item in &return_items {
            self.order_item_repo
                .update_returned_qty(db, item.order_item_id, item.returned_qty)
                .await?;
        }

        // Create reverse CostEntry (credit side) — use unit_cost to match forward COGS entry
        let order_items = self
            .order_item_repo
            .find_by_order_id(db, existing.order_id)
            .await?;

        let period = chrono::Utc::now().format("%Y-%m").to_string();
        let mut cost_entries = Vec::with_capacity(return_items.len());
        for item in &return_items {
            let unit_cost = order_items
                .iter()
                .find(|oi| oi.id == item.order_item_id)
                .map(|oi| oi.unit_cost)
                .unwrap_or(Decimal::ZERO);

            cost_entries.push(EntryRequest {
                entity_type: CostEntityType::SalesOrder,
                entity_id: existing.order_id,
                cost_type: CostType::Material,
                debit_amount: Decimal::ZERO,
                credit_amount: item.returned_qty * unit_cost,
                cost_center: None,
                profit_center: None,
                period: period.clone(),
                source_type: DocumentType::SalesReturn,
                source_id: id,
            });
        }

        if !cost_entries.is_empty() {
            new_cost_entry_service(self.pool.clone())
                .create_entries(ctx, db, cost_entries)
                .await?;
        }

        // QMS: 创建 RMA 记录关联退货
        new_rma_service(self.pool.clone()).create(
            ctx,
            db,
            CreateRmaReq {
                customer_id: 0,
                sales_order_id: None,
                shipping_request_id: None,
                product_id: 0,
                linked_inspection_result_id: None,
                defect_description: format!("SalesReturn #{}", id),
                severity: crate::qms::enums::Severity::Minor,
                remark: String::new(),
            },
        )
        .await?;

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ReturnStatus", id, "Completed", None)
            .await?;

        self.repo
            .update_status(db, id, ReturnStatus::Completed)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                "SalesReturn",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({ "from": "Inspecting", "to": "Completed" })),
                None,
            )
            .await?;

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::SalesReturnReceived,
                    aggregate_type: "SalesReturn".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({
                        "return_id": id,
                        "doc_number": existing.doc_number,
                        "order_id": existing.order_id,
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn reject(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesReturn"))?;

        if existing.status != ReturnStatus::Draft {
            return Err(DomainError::business_rule("Only Draft returns can be rejected"));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ReturnStatus", id, "Rejected", None)
            .await?;

        self.repo
            .update_status(db, id, ReturnStatus::Rejected)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                "SalesReturn",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({ "from": "Draft", "to": "Rejected" })),
                None,
            )
            .await?;

        Ok(())
    }

    async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesReturn"))?;

        if existing.status != ReturnStatus::Inspecting {
            return Err(DomainError::business_rule("Only Inspecting returns can be cancelled"));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ReturnStatus", id, "Cancelled", None)
            .await?;

        self.repo
            .update_status(db, id, ReturnStatus::Cancelled)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                "SalesReturn",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({ "from": "Inspecting", "to": "Cancelled" })),
                None,
            )
            .await?;

        Ok(())
    }

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: ReturnQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<SalesReturn>> {
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
            .ok_or_else(|| DomainError::not_found("SalesReturn"))?;

        if existing.status != ReturnStatus::Draft {
            return Err(DomainError::business_rule("仅草稿状态的退货单可以删除"));
        }

        self.repo.soft_delete(db, id).await?;

        new_audit_log_service(self.pool.clone()).record(ctx, db, "SalesReturn", id, AuditAction::Delete, None, None).await?;

        Ok(())
    }

    async fn list_items(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        return_id: i64,
    ) -> Result<Vec<SalesReturnItem>> {
        self.item_repo.find_by_return_id(db, return_id).await
    }
}
