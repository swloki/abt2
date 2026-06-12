use chrono::{Local, TimeDelta};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use crate::master_data::customer::{new_customer_service, service::CustomerService};
use crate::sales::quotation::{new_quotation_service, service::QuotationService};
use crate::sales::sales_order::model::*;
use crate::sales::sales_order::repo::{SalesOrderItemRepo, SalesOrderRepo, savepoint, release_savepoint, rollback_savepoint};
use crate::sales::sales_order::service::SalesOrderService;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::document_link::{new_document_link_service, service::DocumentLinkService};
use crate::shared::document_link::model::LinkRequest;
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::enums::link_type::LinkType;
use crate::shared::enums::reservation::ReservationType;
use crate::shared::event_bus::{new_domain_event_bus, service::DomainEventBus};
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::inventory_reservation::{new_inventory_reservation_service, service::InventoryReservationService};
use crate::shared::inventory_reservation::model::ReserveRequest;
use crate::shared::state_machine::{new_state_machine_service, service::StateMachineService};
use crate::shared::types::{PgExecutor, DomainError, PageParams, PaginatedResult, ServiceContext, Result};

pub struct SalesOrderServiceImpl {
    repo: SalesOrderRepo,
    item_repo: SalesOrderItemRepo,
    pool: PgPool,
}

impl SalesOrderServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: SalesOrderRepo,
            item_repo: SalesOrderItemRepo,
            pool,
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
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateSalesOrderReq,
    ) -> Result<i64> {
        new_customer_service(self.pool.clone())
            .validate_contact_ownership(ctx, db, req.customer_id, req.contact_id)
            .await?;

        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::SalesOrder)
            .await?;

        let (total_amount, total_cost) = Self::calculate_amounts(&req.items);

        let id = self
            .repo
            .create(
                db,
                &CreateSalesOrderParams {
                    doc_number: &doc_number,
                    customer_id: req.customer_id,
                    contact_id: req.contact_id,
                    sales_rep_id: ctx.operator_id,
                    total_amount,
                    total_cost,
                    payment_terms: req.payment_terms.as_deref().unwrap_or(""),
                    delivery_terms: req.delivery_terms.as_deref().unwrap_or(""),
                    delivery_address: req.delivery_address.as_deref().unwrap_or(""),
                    remark: req.remark.as_deref().unwrap_or(""),
                    operator_id: ctx.operator_id,
                },
            )
            .await?;

        let item_inputs = Self::build_item_inputs(&req.items);
        self.item_repo
            .create_batch(db, id, &item_inputs)
            .await?;

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "SalesOrderStatus", id, "Draft", None)
            .await
            .ok();

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "SalesOrder", entity_id: id, action: AuditAction::Create, changes: None, context: None })
            .await?;

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
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
        ctx: &ServiceContext, db: PgExecutor<'_>,
        quotation_id: i64,
    ) -> Result<i64> {
        let quotation = new_quotation_service(self.pool.clone()).find_by_id(ctx, db, quotation_id).await?;

        if quotation.status != crate::sales::quotation::model::QuotationStatus::Accepted {
            return Err(DomainError::business_rule(
                "Only Accepted quotations can be converted to orders",
            ));
        }

        if quotation.valid_until < Local::now().date_naive() {
            return Err(DomainError::business_rule("Quotation has expired"));
        }

        let quotation_items = new_quotation_service(self.pool.clone()).list_items(ctx, db, quotation_id).await?;

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

        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::SalesOrder)
            .await?;

        let (total_amount, total_cost) = Self::calculate_amounts(&order_items);

        let id = self
            .repo
            .create(
                db,
                &CreateSalesOrderParams {
                    doc_number: &doc_number,
                    customer_id: quotation.customer_id,
                    contact_id: quotation.contact_id,
                    sales_rep_id: quotation.sales_rep_id,
                    total_amount,
                    total_cost,
                    payment_terms: &quotation.payment_terms,
                    delivery_terms: &quotation.delivery_terms,
                    delivery_address: "",
                    remark: &quotation.remark,
                    operator_id: ctx.operator_id,
                },
            )
            .await?;

        let item_inputs = Self::build_item_inputs(&order_items);
        self.item_repo
            .create_batch(db, id, &item_inputs)
            .await?;

        new_document_link_service(self.pool.clone())
            .create_links(
                ctx,
                db,
                vec![LinkRequest {
                    source_type: DocumentType::SalesOrder,
                    source_id: id,
                    target_type: DocumentType::Quotation,
                    target_id: quotation_id,
                    link_type: LinkType::DerivedFrom,
                }],
            )
            .await?;

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "SalesOrderStatus", id, "Draft", None)
            .await
            .ok();

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx,
                    db,
                    RecordAuditLogReq {
                        entity_type: "SalesOrder",
                        entity_id: id,
                        action: AuditAction::Create,
                        changes: Some(serde_json::json!({ "source": "quotation", "quotation_id": quotation_id })),
                        context: None,
                    },
                )
            .await?;

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
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
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<SalesOrder> {
        self.repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesOrder"))
    }

    async fn update_header(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateSalesOrderReq,
    ) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

        if existing.status != SalesOrderStatus::Draft {
            return Err(DomainError::business_rule("Only Draft orders can be updated"));
        }

        self.repo
            .update(db, id, &req)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "SalesOrder", entity_id: id, action: AuditAction::Update, changes: None, context: None })
            .await?;

        Ok(())
    }

    async fn update(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateSalesOrderReq,
        items: Vec<CreateSalesOrderItemReq>,
    ) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

        if existing.status != SalesOrderStatus::Draft {
            return Err(DomainError::business_rule("仅草稿状态的订单可以编辑"));
        }

        self.repo.update(db, id, &req).await?;

        self.item_repo.delete_by_order_id(db, id).await?;

        let item_inputs = Self::build_item_inputs(&items);
        self.item_repo
            .create_batch(db, id, &item_inputs)
            .await?;

        let (total_amount, total_cost) = Self::calculate_amounts(&items);
        self.repo
            .update_amounts(db, id, total_amount, total_cost)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "SalesOrder", entity_id: id, action: AuditAction::Update, changes: None, context: None })
            .await?;

        Ok(())
    }

    async fn list_items(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<Vec<SalesOrderItem>> {
        self.item_repo
            .find_by_order_id(db, order_id)
            .await
    }

    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

        if existing.status != SalesOrderStatus::Draft {
            return Err(DomainError::business_rule("Only Draft orders can be confirmed"));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "SalesOrderStatus", id, "Confirmed", None)
            .await?;

        // 立即更新业务表状态（不依赖后续 reserve 等操作）
        self.repo
            .update_status(db, id, SalesOrderStatus::Confirmed)
            .await?;

        let items = self
            .item_repo
            .find_by_order_id(db, id)
            .await?;

        // Reserve inventory in a savepoint so failures don't abort the main transaction
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

        savepoint(db, "sp_reserve").await.ok();
        match new_inventory_reservation_service(self.pool.clone()).reserve(ctx, db, reserve_requests).await {
            Ok(batch) if batch.failed_items.is_empty() => {
                release_savepoint(db, "sp_reserve").await.ok();
            }
            Ok(batch) => {
                tracing::warn!(
                    "inventory reservation partial failure: {}/{} succeeded",
                    batch.success_count, batch.total
                );
                rollback_savepoint(db, "sp_reserve").await.ok();
            }
            Err(e) => {
                tracing::warn!("inventory reserve error: {e}");
                rollback_savepoint(db, "sp_reserve").await.ok();
            }
        }

        savepoint(db, "sp_audit").await.ok();
        if let Err(e) = new_audit_log_service(self.pool.clone())
            .record(
                    ctx,
                    db,
                    RecordAuditLogReq {
                        entity_type: "SalesOrder",
                        entity_id: id,
                        action: AuditAction::Transition,
                        changes: Some(serde_json::json!({ "from": "Draft", "to": "Confirmed" })),
                        context: None,
                    },
                )
            .await
        {
            tracing::warn!("audit record failed: {e}");
            rollback_savepoint(db, "sp_audit").await.ok();
        } else {
            release_savepoint(db, "sp_audit").await.ok();
        }

        savepoint(db, "sp_event").await.ok();
        if let Err(e) = new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::SalesOrderConfirmed,
                    aggregate_type: "SalesOrder".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({ "sales_order_id": id }),
                    idempotency_key: None,
                },
            )
            .await
        {
            tracing::warn!("event publish failed: {e}");
            rollback_savepoint(db, "sp_event").await.ok();
        } else {
            release_savepoint(db, "sp_event").await.ok();
        }

        Ok(())
    }

    async fn complete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

        if existing.status != SalesOrderStatus::Shipped {
            return Err(DomainError::business_rule("Only Shipped orders can be completed"));
        }

        let items = self
            .item_repo
            .find_by_order_id(db, id)
            .await?;

        for item in &items {
            if item.shipped_qty < item.quantity {
                return Err(DomainError::business_rule(format!(
                    "Item {} not fully shipped: {}/{}",
                    item.line_no, item.shipped_qty, item.quantity
                )));
            }
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "SalesOrderStatus", id, "Completed", None)
            .await?;

        self.repo
            .update_status(db, id, SalesOrderStatus::Completed)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx,
                    db,
                    RecordAuditLogReq {
                        entity_type: "SalesOrder",
                        entity_id: id,
                        action: AuditAction::Transition,
                        changes: Some(serde_json::json!({ "from": existing.status.as_str(), "to": "Completed" })),
                        context: None,
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
            .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

        if existing.status != SalesOrderStatus::Draft
            && existing.status != SalesOrderStatus::Confirmed
        {
            return Err(DomainError::business_rule(
                "Only Draft or Confirmed orders can be cancelled",
            ));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "SalesOrderStatus", id, "Cancelled", None)
            .await?;

        if existing.status == SalesOrderStatus::Confirmed {
            new_inventory_reservation_service(self.pool.clone())
                .cancel_by_source(ctx, db, DocumentType::SalesOrder, id)
                .await?;
        }

        self.repo
            .update_status(db, id, SalesOrderStatus::Cancelled)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx,
                    db,
                    RecordAuditLogReq {
                        entity_type: "SalesOrder",
                        entity_id: id,
                        action: AuditAction::Transition,
                        changes: Some(serde_json::json!({ "from": existing.status.as_str(), "to": "Cancelled" })),
                        context: None,
                    },
                )
            .await?;

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
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

    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

        if existing.status != SalesOrderStatus::Draft {
            return Err(DomainError::business_rule("仅草稿状态的订单可以删除"));
        }

        self.repo.soft_delete(db, id).await?;

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx,
                    db,
                    RecordAuditLogReq {
                        entity_type: "SalesOrder",
                        entity_id: id,
                        action: AuditAction::Delete,
                        changes: Some(serde_json::json!({ "doc_number": existing.doc_number })),
                        context: None,
                    },
                )
            .await?;

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::SalesOrderDeleted,
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
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: SalesOrderQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<SalesOrder>> {
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
}
