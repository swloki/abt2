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
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

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
        mut ctx: ServiceContext<'_>,
        req: CreateQuotationReq,
    ) -> Result<i64, DomainError> {
        if req.valid_until <= Local::now().date_naive() {
            return Err(DomainError::validation("valid_until must be after today"));
        }

        self.customer_svc
            .validate_contact_ownership(ctx.reborrow(), req.customer_id, req.contact_id)
            .await?;

        let doc_number = self
            .doc_seq
            .next_number(ctx.reborrow(), DocumentType::Quotation)
            .await?;

        let (total_amount, total_cost, estimated_margin) = Self::calculate_amounts(&req.items);

        let id = self
            .repo
            .create(
                ctx.executor,
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
            .create_batch(ctx.executor, id, &item_inputs)
            .await
            ?;

        self.state_machine
            .transition(ctx.reborrow(), "QuotationStatus", id, "Draft", None)
            .await
            .ok();

        self.audit
            .record(ctx.reborrow(), "Quotation", id, AuditAction::Create, None, None)
            .await?;

        self.event_bus
            .publish(
                ctx.reborrow(),
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
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<Quotation, DomainError> {
        self.repo
            .find_by_id(ctx.executor, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("Quotation"))
    }

    async fn update(
        &self,
        mut ctx: ServiceContext<'_>,
        id: i64,
        req: UpdateQuotationReq,
    ) -> Result<(), DomainError> {
        let existing = self
            .repo
            .find_by_id(ctx.executor, id)
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
                .delete_by_quotation_id(ctx.executor, id)
                .await
                ?;

            let item_inputs = Self::build_item_inputs(items);
            self.item_repo
                .create_batch(ctx.executor, id, &item_inputs)
                .await
                ?;

            let (total_amount, total_cost, estimated_margin) = Self::calculate_amounts(items);
            self.repo
                .update_amounts(ctx.executor, id, total_amount, total_cost, estimated_margin)
                .await
                ?;
        }

        self.repo
            .update(ctx.executor, id, &req)
            .await
            ?;

        self.audit
            .record(ctx.reborrow(), "Quotation", id, AuditAction::Update, None, None)
            .await?;

        Ok(())
    }

    async fn submit(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        let existing = self
            .repo
            .find_by_id(ctx.executor, id)
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
            .find_by_quotation_id(ctx.executor, id)
            .await
            ?;

        if items.is_empty() {
            return Err(DomainError::business_rule(
                "Cannot submit quotation without items",
            ));
        }

        self.state_machine
            .transition(ctx.reborrow(), "QuotationStatus", id, "Sent", None)
            .await?;

        self.repo
            .update_status(ctx.executor, id, QuotationStatus::Sent)
            .await
            ?;

        self.audit
            .record(
                ctx.reborrow(),
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
                ctx.reborrow(),
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

    async fn accept(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        let existing = self
            .repo
            .find_by_id(ctx.executor, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("Quotation"))?;

        if existing.status != QuotationStatus::Sent {
            return Err(DomainError::business_rule("Only Sent quotations can be accepted"));
        }

        self.state_machine
            .transition(ctx.reborrow(), "QuotationStatus", id, "Accepted", None)
            .await?;

        self.repo
            .update_status(ctx.executor, id, QuotationStatus::Accepted)
            .await
            ?;

        self.audit
            .record(
                ctx.reborrow(),
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
                ctx.reborrow(),
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

    async fn reject(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        let existing = self
            .repo
            .find_by_id(ctx.executor, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("Quotation"))?;

        if existing.status != QuotationStatus::Sent {
            return Err(DomainError::business_rule("Only Sent quotations can be rejected"));
        }

        self.state_machine
            .transition(ctx.reborrow(), "QuotationStatus", id, "Rejected", None)
            .await?;

        self.repo
            .update_status(ctx.executor, id, QuotationStatus::Rejected)
            .await
            ?;

        self.audit
            .record(
                ctx.reborrow(),
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
                ctx.reborrow(),
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

    async fn expire(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        let existing = self
            .repo
            .find_by_id(ctx.executor, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("Quotation"))?;

        if existing.status != QuotationStatus::Draft && existing.status != QuotationStatus::Sent {
            return Err(DomainError::business_rule(
                "Only Draft or Sent quotations can be expired",
            ));
        }

        self.state_machine
            .transition(ctx.reborrow(), "QuotationStatus", id, "Expired", None)
            .await?;

        self.repo
            .update_status(ctx.executor, id, QuotationStatus::Expired)
            .await
            ?;

        self.audit
            .record(
                ctx.reborrow(),
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
                ctx.reborrow(),
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

    async fn batch_expire_overdue(&self, ctx: ServiceContext<'_>) -> Result<i32, DomainError> {
        let count = self
            .repo
            .expire_overdue(ctx.executor)
            .await
            ?;
        Ok(count as i32)
    }

    async fn list_items(
        &self,
        ctx: ServiceContext<'_>,
        quotation_id: i64,
    ) -> Result<Vec<QuotationItem>, DomainError> {
        self.item_repo
            .find_by_quotation_id(ctx.executor, quotation_id)
            .await
            
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: QuotationQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Quotation>, DomainError> {
        self.repo
            .query(
                ctx.executor,
                &filter,
                &page,
                ctx.data_scope,
                ctx.operator_id,
                ctx.department_id,
            )
            .await
            
    }
}
