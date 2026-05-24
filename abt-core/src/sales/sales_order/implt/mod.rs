use std::sync::Arc;

use chrono::{Local, TimeDelta};
use rust_decimal::Decimal;

use crate::master_data::customer::service::CustomerService;
use crate::sales::quotation::service::QuotationService;
use crate::sales::sales_order::model::*;
use crate::sales::sales_order::repo::{SalesOrderItemRepo, SalesOrderRepo};
use crate::sales::sales_order::service::SalesOrderService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::document_link::model::LinkRequest;
use crate::shared::document_link::service::DocumentLinkService;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::enums::link_type::LinkType;
use crate::shared::enums::reservation::ReservationType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::inventory_reservation::model::ReserveRequest;
use crate::shared::inventory_reservation::service::InventoryReservationService;
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

pub struct SalesOrderServiceImpl {
    repo: SalesOrderRepo,
    item_repo: SalesOrderItemRepo,
    doc_seq: Arc<dyn DocumentSequenceService>,
    state_machine: Arc<dyn StateMachineService>,
    audit: Arc<dyn AuditLogService>,
    event_bus: Arc<dyn DomainEventBus>,
    customer_svc: Arc<dyn CustomerService>,
    quotation_svc: Arc<dyn QuotationService>,
    doc_link: Arc<dyn DocumentLinkService>,
    inv_res: Arc<dyn InventoryReservationService>,
}

impl SalesOrderServiceImpl {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repo: SalesOrderRepo,
        item_repo: SalesOrderItemRepo,
        doc_seq: Arc<dyn DocumentSequenceService>,
        state_machine: Arc<dyn StateMachineService>,
        audit: Arc<dyn AuditLogService>,
        event_bus: Arc<dyn DomainEventBus>,
        customer_svc: Arc<dyn CustomerService>,
        quotation_svc: Arc<dyn QuotationService>,
        doc_link: Arc<dyn DocumentLinkService>,
        inv_res: Arc<dyn InventoryReservationService>,
    ) -> Self {
        Self {
            repo,
            item_repo,
            doc_seq,
            state_machine,
            audit,
            event_bus,
            customer_svc,
            quotation_svc,
            doc_link,
            inv_res,
        }
    }

    fn calculate_amounts(items: &[CreateSalesOrderItemReq]) -> (Decimal, Decimal) {
        let mut total_amount = Decimal::ZERO;
        let mut total_cost = Decimal::ZERO;

        for item in items {
            let discount = item.discount_rate.unwrap_or(Decimal::ZERO);
            let amount = item.quantity * item.unit_price * (Decimal::ONE - discount / Decimal::from(100));
            let cost = item.quantity * item.unit_cost.unwrap_or(Decimal::ZERO);
            total_amount += amount.round_dp(4);
            total_cost += cost.round_dp(4);
        }

        (total_amount, total_cost)
    }

    fn build_item_inputs(items: &[CreateSalesOrderItemReq]) -> Vec<SalesOrderItemInput> {
        items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let discount = item.discount_rate.unwrap_or(Decimal::ZERO);
                let amount = item.quantity * item.unit_price * (Decimal::ONE - discount / Decimal::from(100));
                SalesOrderItemInput {
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
impl SalesOrderService for SalesOrderServiceImpl {
    async fn create(
        &self,
        mut ctx: ServiceContext<'_>,
        req: CreateSalesOrderReq,
    ) -> Result<i64, DomainError> {
        self.customer_svc
            .validate_contact_ownership(ctx.reborrow(), req.customer_id, req.contact_id)
            .await?;

        let doc_number = self
            .doc_seq
            .next_number(ctx.reborrow(), DocumentType::SalesOrder)
            .await?;

        let (total_amount, total_cost) = Self::calculate_amounts(&req.items);

        let id = self
            .repo
            .create(
                ctx.executor,
                &doc_number,
                req.customer_id,
                req.contact_id,
                ctx.operator_id,
                total_amount,
                total_cost,
                req.payment_terms.as_deref().unwrap_or(""),
                req.delivery_terms.as_deref().unwrap_or(""),
                req.delivery_address.as_deref().unwrap_or(""),
                req.remark.as_deref().unwrap_or(""),
                ctx.operator_id,
            )
            .await
            .map_err(DomainError::Internal)?;

        let item_inputs = Self::build_item_inputs(&req.items);
        self.item_repo
            .create_batch(ctx.executor, id, &item_inputs)
            .await
            .map_err(DomainError::Internal)?;

        self.state_machine
            .transition(ctx.reborrow(), "SalesOrderStatus", id, "Draft", None)
            .await
            .ok();

        self.audit
            .record(ctx.reborrow(), "SalesOrder", id, AuditAction::Create, None, None)
            .await?;

        self.event_bus
            .publish(
                ctx.reborrow(),
                EventPublishRequest {
                    event_type: DomainEventType::SalesOrderCreated,
                    aggregate_type: "SalesOrder".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({
                        "sales_order_id": id,
                        "doc_number": doc_number,
                        "customer_id": req.customer_id,
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(id)
    }

    async fn create_from_quotation(
        &self,
        mut ctx: ServiceContext<'_>,
        quotation_id: i64,
    ) -> Result<i64, DomainError> {
        let quotation = self.quotation_svc.find_by_id(ctx.reborrow(), quotation_id).await?;

        if quotation.status != crate::sales::quotation::model::QuotationStatus::Accepted {
            return Err(DomainError::business_rule(
                "Only Accepted quotations can be converted to orders",
            ));
        }

        if quotation.valid_until < Local::now().date_naive() {
            return Err(DomainError::business_rule("Quotation has expired"));
        }

        let quotation_items = self.quotation_svc.list_items(ctx.reborrow(), quotation_id).await?;

        let order_items: Vec<CreateSalesOrderItemReq> = quotation_items
            .iter()
            .map(|qi| CreateSalesOrderItemReq {
                product_id: qi.product_id,
                description: Some(qi.description.clone()),
                quantity: qi.quantity,
                unit: Some(qi.unit.clone()),
                unit_price: qi.unit_price,
                unit_cost: Some(qi.unit_cost),
                discount_rate: Some(qi.discount_rate),
                delivery_date: qi.delivery_date,
            })
            .collect();

        let doc_number = self
            .doc_seq
            .next_number(ctx.reborrow(), DocumentType::SalesOrder)
            .await?;

        let (total_amount, total_cost) = Self::calculate_amounts(&order_items);

        let id = self
            .repo
            .create(
                ctx.executor,
                &doc_number,
                quotation.customer_id,
                quotation.contact_id,
                quotation.sales_rep_id,
                total_amount,
                total_cost,
                &quotation.payment_terms,
                &quotation.delivery_terms,
                "",
                &quotation.remark,
                ctx.operator_id,
            )
            .await
            .map_err(DomainError::Internal)?;

        let item_inputs = Self::build_item_inputs(&order_items);
        self.item_repo
            .create_batch(ctx.executor, id, &item_inputs)
            .await
            .map_err(DomainError::Internal)?;

        self.doc_link
            .create_links(
                ctx.reborrow(),
                vec![LinkRequest {
                    source_type: DocumentType::SalesOrder,
                    source_id: id,
                    target_type: DocumentType::Quotation,
                    target_id: quotation_id,
                    link_type: LinkType::DerivedFrom,
                }],
            )
            .await?;

        self.state_machine
            .transition(ctx.reborrow(), "SalesOrderStatus", id, "Draft", None)
            .await
            .ok();

        self.audit
            .record(
                ctx.reborrow(),
                "SalesOrder",
                id,
                AuditAction::Create,
                Some(serde_json::json!({ "source": "quotation", "quotation_id": quotation_id })),
                None,
            )
            .await?;

        self.event_bus
            .publish(
                ctx.reborrow(),
                EventPublishRequest {
                    event_type: DomainEventType::SalesOrderCreated,
                    aggregate_type: "SalesOrder".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({
                        "sales_order_id": id,
                        "doc_number": doc_number,
                        "source_quotation_id": quotation_id,
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
    ) -> Result<SalesOrder, DomainError> {
        self.repo
            .find_by_id(ctx.executor, id)
            .await
            .map_err(DomainError::Internal)?
            .ok_or_else(|| DomainError::not_found("SalesOrder"))
    }

    async fn update_header(
        &self,
        mut ctx: ServiceContext<'_>,
        id: i64,
        req: UpdateSalesOrderReq,
    ) -> Result<(), DomainError> {
        let existing = self
            .repo
            .find_by_id(ctx.executor, id)
            .await
            .map_err(DomainError::Internal)?
            .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

        if existing.status != SalesOrderStatus::Draft {
            return Err(DomainError::business_rule("Only Draft orders can be updated"));
        }

        self.repo
            .update(ctx.executor, id, &req)
            .await
            .map_err(DomainError::Internal)?;

        self.audit
            .record(ctx.reborrow(), "SalesOrder", id, AuditAction::Update, None, None)
            .await?;

        Ok(())
    }

    async fn confirm(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        let existing = self
            .repo
            .find_by_id(ctx.executor, id)
            .await
            .map_err(DomainError::Internal)?
            .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

        if existing.status != SalesOrderStatus::Draft {
            return Err(DomainError::business_rule("Only Draft orders can be confirmed"));
        }

        self.state_machine
            .transition(ctx.reborrow(), "SalesOrderStatus", id, "Confirmed", None)
            .await?;

        let items = self
            .item_repo
            .find_by_order_id(ctx.executor, id)
            .await
            .map_err(DomainError::Internal)?;

        let ttl = chrono::Utc::now() + TimeDelta::days(7);
        let reserve_requests: Vec<ReserveRequest> = items
            .iter()
            .map(|item| ReserveRequest {
                product_id: item.product_id,
                warehouse_id: 1,
                reserved_qty: item.quantity,
                reservation_type: ReservationType::Soft,
                source_type: DocumentType::SalesOrder,
                source_id: id,
                source_line_id: Some(item.id),
                priority: 5,
                expires_at: Some(ttl),
            })
            .collect();

        self.inv_res.reserve(ctx.reborrow(), reserve_requests).await?;

        self.repo
            .update_status(ctx.executor, id, SalesOrderStatus::Confirmed)
            .await
            .map_err(DomainError::Internal)?;

        self.audit
            .record(
                ctx.reborrow(),
                "SalesOrder",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({ "from": "Draft", "to": "Confirmed" })),
                None,
            )
            .await?;

        self.event_bus
            .publish(
                ctx.reborrow(),
                EventPublishRequest {
                    event_type: DomainEventType::SalesOrderConfirmed,
                    aggregate_type: "SalesOrder".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({ "sales_order_id": id }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn start_progress(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        let existing = self
            .repo
            .find_by_id(ctx.executor, id)
            .await
            .map_err(DomainError::Internal)?
            .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

        if existing.status != SalesOrderStatus::Confirmed {
            return Err(DomainError::business_rule("Only Confirmed orders can start progress"));
        }

        self.state_machine
            .transition(ctx.reborrow(), "SalesOrderStatus", id, "InProduction", None)
            .await?;

        self.repo
            .update_status(ctx.executor, id, SalesOrderStatus::InProduction)
            .await
            .map_err(DomainError::Internal)?;

        self.audit
            .record(
                ctx.reborrow(),
                "SalesOrder",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({ "from": existing.status.as_str(), "to": "InProduction" })),
                None,
            )
            .await?;

        Ok(())
    }

    async fn complete(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        let existing = self
            .repo
            .find_by_id(ctx.executor, id)
            .await
            .map_err(DomainError::Internal)?
            .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

        if existing.status != SalesOrderStatus::Shipped {
            return Err(DomainError::business_rule("Only Shipped orders can be completed"));
        }

        let items = self
            .item_repo
            .find_by_order_id(ctx.executor, id)
            .await
            .map_err(DomainError::Internal)?;

        for item in &items {
            if item.shipped_qty < item.quantity {
                return Err(DomainError::business_rule(format!(
                    "Item {} not fully shipped: {}/{}",
                    item.line_no, item.shipped_qty, item.quantity
                )));
            }
        }

        self.state_machine
            .transition(ctx.reborrow(), "SalesOrderStatus", id, "Completed", None)
            .await?;

        self.repo
            .update_status(ctx.executor, id, SalesOrderStatus::Completed)
            .await
            .map_err(DomainError::Internal)?;

        self.audit
            .record(
                ctx.reborrow(),
                "SalesOrder",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({ "from": existing.status.as_str(), "to": "Completed" })),
                None,
            )
            .await?;

        Ok(())
    }

    async fn cancel(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        let existing = self
            .repo
            .find_by_id(ctx.executor, id)
            .await
            .map_err(DomainError::Internal)?
            .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

        if existing.status != SalesOrderStatus::Draft
            && existing.status != SalesOrderStatus::Confirmed
        {
            return Err(DomainError::business_rule(
                "Only Draft or Confirmed orders can be cancelled",
            ));
        }

        self.state_machine
            .transition(ctx.reborrow(), "SalesOrderStatus", id, "Cancelled", None)
            .await?;

        if existing.status == SalesOrderStatus::Confirmed {
            self.inv_res
                .cancel_by_source(ctx.reborrow(), DocumentType::SalesOrder, id)
                .await?;
        }

        self.repo
            .update_status(ctx.executor, id, SalesOrderStatus::Cancelled)
            .await
            .map_err(DomainError::Internal)?;

        self.audit
            .record(
                ctx.reborrow(),
                "SalesOrder",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({ "from": existing.status.as_str(), "to": "Cancelled" })),
                None,
            )
            .await?;

        self.event_bus
            .publish(
                ctx.reborrow(),
                EventPublishRequest {
                    event_type: DomainEventType::SalesOrderCancelled,
                    aggregate_type: "SalesOrder".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({ "sales_order_id": id, "doc_number": existing.doc_number }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: SalesOrderQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<SalesOrder>, DomainError> {
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
            .map_err(DomainError::Internal)
    }
}
