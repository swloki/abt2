use std::sync::Arc;

use chrono::Local;
use rust_decimal::Decimal;

use crate::master_data::customer::service::CustomerService;
use crate::sales::quotation::model::*;
use crate::sales::quotation::repo::{QuotationItemRepo, QuotationRepo};
use crate::sales::quotation::service::QuotationService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::types::{PgExecutor,DomainError, PageParams, PaginatedResult, ServiceContext, Result};

pub struct QuotationServiceImpl {
    repo: QuotationRepo,
    item_repo: QuotationItemRepo,
    doc_seq: Arc<dyn DocumentSequenceService>,
    state_machine: Arc<dyn StateMachineService>,
    audit: Arc<dyn AuditLogService>,
    event_bus: Arc<dyn DomainEventBus>,
    customer_svc: Arc<dyn CustomerService>,
}

impl QuotationServiceImpl {
    pub fn new(
        repo: QuotationRepo,
        item_repo: QuotationItemRepo,
        doc_seq: Arc<dyn DocumentSequenceService>,
        state_machine: Arc<dyn StateMachineService>,
        audit: Arc<dyn AuditLogService>,
        event_bus: Arc<dyn DomainEventBus>,
        customer_svc: Arc<dyn CustomerService>,
    ) -> Self {
        Self {
            repo,
            item_repo,
            doc_seq,
            state_machine,
            audit,
            event_bus,
            customer_svc,
        }
    }

    fn calculate_amounts(items: &[CreateQuotationItemReq]) -> (Decimal, Decimal, Decimal) {
        let mut total_amount = Decimal::ZERO;
        let mut total_cost = Decimal::ZERO;

        for item in items {
            let discount = item.discount_rate.unwrap_or(Decimal::ZERO);
            let amount = item.quantity * item.unit_price * (Decimal::ONE - discount / Decimal::from(100));
            let cost = item.quantity * item.unit_cost.unwrap_or(Decimal::ZERO);
            total_amount += amount.round_dp(4);
            total_cost += cost.round_dp(4);
        }

        let margin = if total_amount > Decimal::ZERO {
            ((total_amount - total_cost) / total_amount * Decimal::from(100)).round_dp(2)
        } else {
            Decimal::ZERO
        };

        (total_amount, total_cost, margin)
    }

    fn build_item_inputs(items: &[CreateQuotationItemReq]) -> Vec<QuotationItemInput> {
        items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let discount = item.discount_rate.unwrap_or(Decimal::ZERO);
                let amount = item.quantity * item.unit_price * (Decimal::ONE - discount / Decimal::from(100));
                QuotationItemInput {
                    line_no: (i + 1) as i32,
                    product_id: item.product_id,
                    description: item.description.clone().unwrap_or_default(),
                    quantity: item.quantity,
                    unit: item.unit.clone().unwrap_or_default(),
                    unit_price: item.unit_price,
                    unit_cost: item.unit_cost.unwrap_or(Decimal::ZERO),
                    discount_rate: item.discount_rate.unwrap_or(Decimal::ZERO),
                    amount: amount.round_dp(4),
                    delivery_date: item.delivery_date,
                }
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl QuotationService for QuotationServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateQuotationReq,
    ) -> Result<i64> {
        if req.valid_until <= Local::now().date_naive() {
            return Err(DomainError::validation("valid_until must be after today"));
        }

        if req.customer_id > 0 && req.contact_id > 0 {
            self.customer_svc
                .validate_contact_ownership(ctx, db, req.customer_id, req.contact_id)
                .await?;
        }

        if req.items.is_empty() {
            return Err(DomainError::validation("报价单必须包含至少一个产品"));
        }

        if req.items.iter().all(|i| i.unit_price == Decimal::ZERO) {
            return Err(DomainError::validation("报价单中所有产品的单价不能都为 0"));
        }

        let doc_number = self
            .doc_seq
            .next_number(ctx, db, DocumentType::Quotation)
            .await?;

        let (total_amount, total_cost, estimated_margin) = Self::calculate_amounts(&req.items);

        let id = self
            .repo
            .create(
                db,
                &doc_number,
                &req,
                ctx.operator_id,
                total_amount,
                total_cost,
                estimated_margin,
                ctx.operator_id,
            )
            .await
            ?;

        let item_inputs = Self::build_item_inputs(&req.items);
        self.item_repo
            .create_batch(db, id, &item_inputs)
            .await
            ?;

        self.state_machine
            .transition(ctx, db, "QuotationStatus", id, "Draft", None)
            .await
            .ok();

        self.audit
            .record(ctx, db, "Quotation", id, AuditAction::Create, None, None)
            .await?;

        self.event_bus
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::QuotationCreated,
                    aggregate_type: "Quotation".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({
                        "quotation_id": id,
                        "doc_number": doc_number,
                        "customer_id": req.customer_id,
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(id)
    }

    async fn find_by_id(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<Quotation> {
        self.repo
            .find_by_id(db, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("Quotation"))
    }

    async fn update(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateQuotationReq,
    ) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("Quotation"))?;

        if existing.status != QuotationStatus::Draft {
            return Err(DomainError::business_rule(
                "Only Draft quotations can be updated",
            ));
        }

        if let Some(valid_until) = req.valid_until
            && valid_until <= Local::now().date_naive()
        {
            return Err(DomainError::validation("valid_until must be after today"));
        }

        if let Some(ref items) = req.items {
            self.item_repo
                .delete_by_quotation_id(db, id)
                .await
                ?;

            let item_inputs = Self::build_item_inputs(items);
            self.item_repo
                .create_batch(db, id, &item_inputs)
                .await
                ?;

            let (total_amount, total_cost, estimated_margin) = Self::calculate_amounts(items);
            self.repo
                .update_amounts(db, id, total_amount, total_cost, estimated_margin)
                .await
                ?;
        }

        self.repo
            .update(db, id, &req)
            .await
            ?;

        self.audit
            .record(ctx, db, "Quotation", id, AuditAction::Update, None, None)
            .await?;

        Ok(())
    }

    async fn submit(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("Quotation"))?;

        if existing.status != QuotationStatus::Draft {
            return Err(DomainError::business_rule(
                "Only Draft quotations can be submitted",
            ));
        }

        let items = self
            .item_repo
            .find_by_quotation_id(db, id)
            .await
            ?;

        if items.is_empty() {
            return Err(DomainError::business_rule(
                "Cannot submit quotation without items",
            ));
        }

        self.state_machine
            .transition(ctx, db, "QuotationStatus", id, "Sent", None)
            .await?;

        self.repo
            .update_status(db, id, QuotationStatus::Sent)
            .await
            ?;

        self.audit
            .record(
                ctx,
                db,
                "Quotation",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({
                    "from": "Draft",
                    "to": "Sent",
                })),
                None,
            )
            .await?;

        self.event_bus
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::QuotationSubmitted,
                    aggregate_type: "Quotation".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({
                        "quotation_id": id,
                        "doc_number": existing.doc_number,
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn accept(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("Quotation"))?;

        if existing.status != QuotationStatus::Sent {
            return Err(DomainError::business_rule("Only Sent quotations can be accepted"));
        }

        self.state_machine
            .transition(ctx, db, "QuotationStatus", id, "Accepted", None)
            .await?;

        self.repo
            .update_status(db, id, QuotationStatus::Accepted)
            .await
            ?;

        self.audit
            .record(
                ctx,
                db,
                "Quotation",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({
                    "from": existing.status.as_str(),
                    "to": "Accepted",
                })),
                None,
            )
            .await?;

        self.event_bus
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::QuotationAccepted,
                    aggregate_type: "Quotation".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({
                        "quotation_id": id,
                        "doc_number": existing.doc_number,
                        "customer_id": existing.customer_id,
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
            .await
            ?
            .ok_or_else(|| DomainError::not_found("Quotation"))?;

        if existing.status != QuotationStatus::Sent {
            return Err(DomainError::business_rule("Only Sent quotations can be rejected"));
        }

        self.state_machine
            .transition(ctx, db, "QuotationStatus", id, "Rejected", None)
            .await?;

        self.repo
            .update_status(db, id, QuotationStatus::Rejected)
            .await
            ?;

        self.audit
            .record(
                ctx,
                db,
                "Quotation",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({
                    "from": existing.status.as_str(),
                    "to": "Rejected",
                })),
                None,
            )
            .await?;

        self.event_bus
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::QuotationRejected,
                    aggregate_type: "Quotation".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({
                        "quotation_id": id,
                        "doc_number": existing.doc_number,
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn expire(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("Quotation"))?;

        if existing.status != QuotationStatus::Draft && existing.status != QuotationStatus::Sent {
            return Err(DomainError::business_rule(
                "Only Draft or Sent quotations can be expired",
            ));
        }

        self.state_machine
            .transition(ctx, db, "QuotationStatus", id, "Expired", None)
            .await?;

        self.repo
            .update_status(db, id, QuotationStatus::Expired)
            .await
            ?;

        self.audit
            .record(
                ctx,
                db,
                "Quotation",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({
                    "from": existing.status.as_str(),
                    "to": "Expired",
                })),
                None,
            )
            .await?;

        self.event_bus
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::QuotationExpired,
                    aggregate_type: "Quotation".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({
                        "quotation_id": id,
                        "doc_number": existing.doc_number,
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn batch_expire_overdue(&self, _ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<i32> {
        let count = self
            .repo
            .expire_overdue(db)
            .await
            ?;
        Ok(count as i32)
    }

    async fn list_items(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        quotation_id: i64,
    ) -> Result<Vec<QuotationItem>> {
        self.item_repo
            .find_by_quotation_id(db, quotation_id)
            .await
            .map_err(Into::into)
    }

    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Quotation"))?;

        if existing.status != QuotationStatus::Draft {
            return Err(DomainError::business_rule("仅草稿状态的报价单可以删除"));
        }

        self.repo.soft_delete(db, id).await?;

        self.audit
            .record(
                ctx,
                db,
                "Quotation",
                id,
                AuditAction::Delete,
                Some(serde_json::json!({ "doc_number": existing.doc_number })),
                None,
            )
            .await?;

        self.event_bus
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::QuotationDeleted,
                    aggregate_type: "Quotation".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({ "quotation_id": id, "doc_number": existing.doc_number }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: QuotationQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Quotation>> {
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
            .map_err(Into::into)
    }
}
