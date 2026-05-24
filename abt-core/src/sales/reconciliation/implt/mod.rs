use std::sync::Arc;

use rust_decimal::Decimal;

use crate::sales::reconciliation::model::*;
use crate::sales::reconciliation::repo::{
    aggregate_shipping_items, ReconciliationItemRepo, ReconciliationRepo,
};
use crate::sales::reconciliation::service::ReconciliationService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::cost_entry::model::EntryRequest;
use crate::shared::cost_entry::service::CostEntryService;
use crate::shared::document_link::model::LinkRequest;
use crate::shared::document_link::service::DocumentLinkService;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::cost::CostEntityType;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::link_type::LinkType;
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

pub struct ReconciliationServiceImpl {
    repo: ReconciliationRepo,
    item_repo: ReconciliationItemRepo,
    doc_seq: Arc<dyn DocumentSequenceService>,
    state_machine: Arc<dyn StateMachineService>,
    audit: Arc<dyn AuditLogService>,
    doc_link: Arc<dyn DocumentLinkService>,
    cost_entry: Arc<dyn CostEntryService>,
}

impl ReconciliationServiceImpl {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repo: ReconciliationRepo,
        item_repo: ReconciliationItemRepo,
        doc_seq: Arc<dyn DocumentSequenceService>,
        state_machine: Arc<dyn StateMachineService>,
        audit: Arc<dyn AuditLogService>,
        doc_link: Arc<dyn DocumentLinkService>,
        cost_entry: Arc<dyn CostEntryService>,
    ) -> Self {
        Self {
            repo,
            item_repo,
            doc_seq,
            state_machine,
            audit,
            doc_link,
            cost_entry,
        }
    }
}

#[async_trait::async_trait]
impl ReconciliationService for ReconciliationServiceImpl {
    async fn create(
        &self,
        mut ctx: ServiceContext<'_>,
        customer_id: i64,
        period: String,
    ) -> Result<i64, DomainError> {
        if self
            .repo
            .exists_by_customer_period(ctx.executor, customer_id, &period)
            .await
            .map_err(DomainError::Internal)?
        {
            return Err(DomainError::duplicate(
                "Reconciliation already exists for this customer and period",
            ));
        }

        // Aggregate shipping items for this customer+period
        let aggregated = aggregate_shipping_items(ctx.executor, customer_id, &period)
            .await
            .map_err(DomainError::Internal)?;

        let total_amount: Decimal = aggregated.iter().map(|a| a.amount).sum();

        let doc_number = self
            .doc_seq
            .next_number(ctx.reborrow(), DocumentType::Reconciliation)
            .await?;

        let id = self
            .repo
            .create(
                ctx.executor,
                &doc_number,
                customer_id,
                &period,
                total_amount,
                "",
                ctx.operator_id,
            )
            .await
            .map_err(DomainError::Internal)?;

        let item_inputs: Vec<ReconciliationItemInput> = aggregated
            .iter()
            .map(|a| ReconciliationItemInput {
                shipping_request_id: a.shipping_request_id,
                sales_order_id: a.sales_order_id,
                product_id: a.product_id,
                quantity: a.quantity,
                unit_price: a.unit_price,
                amount: a.amount,
            })
            .collect();

        if !item_inputs.is_empty() {
            self.item_repo
                .create_batch(ctx.executor, id, &item_inputs)
                .await
                .map_err(DomainError::Internal)?;
        }

        // Create doc links for each unique shipping request
        let shipping_ids: Vec<i64> = aggregated
            .iter()
            .map(|a| a.shipping_request_id)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let links: Vec<LinkRequest> = shipping_ids
            .iter()
            .map(|&sid| LinkRequest {
                source_type: DocumentType::Reconciliation,
                source_id: id,
                target_type: DocumentType::ShippingRequest,
                target_id: sid,
                link_type: LinkType::Reconciles,
            })
            .collect();

        if !links.is_empty() {
            self.doc_link.create_links(ctx.reborrow(), links).await?;
        }

        self.state_machine
            .transition(ctx.reborrow(), "ReconciliationStatus", id, "Draft", None)
            .await
            .ok();

        self.audit
            .record(
                ctx.reborrow(),
                "Reconciliation",
                id,
                AuditAction::Create,
                Some(serde_json::json!({ "customer_id": customer_id, "period": period })),
                None,
            )
            .await?;

        Ok(id)
    }

    async fn find_by_id(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<Reconciliation, DomainError> {
        self.repo
            .find_by_id(ctx.executor, id)
            .await
            .map_err(DomainError::Internal)?
            .ok_or_else(|| DomainError::not_found("Reconciliation"))
    }

    async fn send(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        let existing = self
            .repo
            .find_by_id(ctx.executor, id)
            .await
            .map_err(DomainError::Internal)?
            .ok_or_else(|| DomainError::not_found("Reconciliation"))?;

        if existing.status != ReconciliationStatus::Draft {
            return Err(DomainError::business_rule("Only Draft reconciliations can be sent"));
        }

        self.state_machine
            .transition(ctx.reborrow(), "ReconciliationStatus", id, "Sent", None)
            .await?;

        self.repo
            .update_status(ctx.executor, id, ReconciliationStatus::Sent)
            .await
            .map_err(DomainError::Internal)?;

        self.audit
            .record(
                ctx.reborrow(),
                "Reconciliation",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({ "from": "Draft", "to": "Sent" })),
                None,
            )
            .await?;

        Ok(())
    }

    async fn confirm(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        let existing = self
            .repo
            .find_by_id(ctx.executor, id)
            .await
            .map_err(DomainError::Internal)?
            .ok_or_else(|| DomainError::not_found("Reconciliation"))?;

        if existing.status != ReconciliationStatus::Sent {
            return Err(DomainError::business_rule("Only Sent reconciliations can be confirmed"));
        }

        let all_confirmed = self
            .item_repo
            .all_confirmed(ctx.executor, id)
            .await
            .map_err(DomainError::Internal)?;

        if !all_confirmed {
            return Err(DomainError::business_rule(
                "All items must be confirmed before reconciliation can be confirmed",
            ));
        }

        // confirmed_amount = total_amount (all items confirmed), difference = 0
        self.repo
            .update_amounts(ctx.executor, id, existing.total_amount, Decimal::ZERO)
            .await
            .map_err(DomainError::Internal)?;

        self.state_machine
            .transition(ctx.reborrow(), "ReconciliationStatus", id, "Confirmed", None)
            .await?;

        self.repo
            .update_status(ctx.executor, id, ReconciliationStatus::Confirmed)
            .await
            .map_err(DomainError::Internal)?;

        // Create AR voucher via CostEntry
        let items = self
            .item_repo
            .find_by_reconciliation_id(ctx.executor, id)
            .await
            .map_err(DomainError::Internal)?;

        let period = existing.period.clone();
        let mut cost_entries = Vec::with_capacity(items.len());
        for item in &items {
            cost_entries.push(EntryRequest {
                entity_type: CostEntityType::SalesOrder,
                entity_id: item.sales_order_id,
                cost_type: crate::shared::enums::cost::CostType::Material,
                debit_amount: Decimal::ZERO,
                credit_amount: item.amount,
                cost_center: None,
                profit_center: None,
                period: period.clone(),
                source_type: DocumentType::Reconciliation,
                source_id: id,
            });
        }

        if !cost_entries.is_empty() {
            self.cost_entry
                .create_entries(ctx.reborrow(), cost_entries)
                .await?;
        }

        self.audit
            .record(
                ctx.reborrow(),
                "Reconciliation",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({ "from": "Sent", "to": "Confirmed" })),
                None,
            )
            .await?;

        Ok(())
    }

    async fn dispute(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        let existing = self
            .repo
            .find_by_id(ctx.executor, id)
            .await
            .map_err(DomainError::Internal)?
            .ok_or_else(|| DomainError::not_found("Reconciliation"))?;

        if existing.status != ReconciliationStatus::Sent
            && existing.status != ReconciliationStatus::Confirmed
        {
            return Err(DomainError::business_rule(
                "Only Sent or Confirmed reconciliations can be disputed",
            ));
        }

        self.state_machine
            .transition(ctx.reborrow(), "ReconciliationStatus", id, "Disputed", None)
            .await?;

        self.repo
            .update_status(ctx.executor, id, ReconciliationStatus::Disputed)
            .await
            .map_err(DomainError::Internal)?;

        self.audit
            .record(
                ctx.reborrow(),
                "Reconciliation",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({
                    "from": existing.status.as_str(),
                    "to": "Disputed"
                })),
                None,
            )
            .await?;

        Ok(())
    }

    async fn reopen(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        let existing = self
            .repo
            .find_by_id(ctx.executor, id)
            .await
            .map_err(DomainError::Internal)?
            .ok_or_else(|| DomainError::not_found("Reconciliation"))?;

        if existing.status != ReconciliationStatus::Disputed {
            return Err(DomainError::business_rule("Only Disputed reconciliations can be reopened"));
        }

        self.state_machine
            .transition(ctx.reborrow(), "ReconciliationStatus", id, "Draft", None)
            .await?;

        self.repo
            .update_status(ctx.executor, id, ReconciliationStatus::Draft)
            .await
            .map_err(DomainError::Internal)?;

        self.audit
            .record(
                ctx.reborrow(),
                "Reconciliation",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({ "from": "Disputed", "to": "Draft" })),
                None,
            )
            .await?;

        Ok(())
    }

    async fn force_settle(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        let existing = self
            .repo
            .find_by_id(ctx.executor, id)
            .await
            .map_err(DomainError::Internal)?
            .ok_or_else(|| DomainError::not_found("Reconciliation"))?;

        if existing.status != ReconciliationStatus::Disputed {
            return Err(DomainError::business_rule(
                "Only Disputed reconciliations can be force-settled",
            ));
        }

        // Settle with difference as the confirmed amount
        self.repo
            .update_amounts(
                ctx.executor,
                id,
                existing.total_amount - existing.difference,
                existing.difference,
            )
            .await
            .map_err(DomainError::Internal)?;

        self.state_machine
            .transition(ctx.reborrow(), "ReconciliationStatus", id, "Settled", None)
            .await?;

        self.repo
            .update_status(ctx.executor, id, ReconciliationStatus::Settled)
            .await
            .map_err(DomainError::Internal)?;

        // Create AR voucher via CostEntry for confirmed items (disputed path bypasses confirm)
        let items = self
            .item_repo
            .find_by_reconciliation_id(ctx.executor, id)
            .await
            .map_err(DomainError::Internal)?;

        let period = existing.period.clone();
        let mut cost_entries = Vec::with_capacity(items.len());
        for item in &items {
            if item.confirmed {
                cost_entries.push(EntryRequest {
                    entity_type: CostEntityType::SalesOrder,
                    entity_id: item.sales_order_id,
                    cost_type: crate::shared::enums::cost::CostType::Material,
                    debit_amount: Decimal::ZERO,
                    credit_amount: item.amount,
                    cost_center: None,
                    profit_center: None,
                    period: period.clone(),
                    source_type: DocumentType::Reconciliation,
                    source_id: id,
                });
            }
        }

        if !cost_entries.is_empty() {
            self.cost_entry
                .create_entries(ctx.reborrow(), cost_entries)
                .await?;
        }

        self.audit
            .record(
                ctx.reborrow(),
                "Reconciliation",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({ "from": "Disputed", "to": "Settled" })),
                None,
            )
            .await?;

        Ok(())
    }

    async fn settle(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        let existing = self
            .repo
            .find_by_id(ctx.executor, id)
            .await
            .map_err(DomainError::Internal)?
            .ok_or_else(|| DomainError::not_found("Reconciliation"))?;

        if existing.status != ReconciliationStatus::Confirmed {
            return Err(DomainError::business_rule(
                "Only Confirmed reconciliations can be settled",
            ));
        }

        self.state_machine
            .transition(ctx.reborrow(), "ReconciliationStatus", id, "Settled", None)
            .await?;

        self.repo
            .update_status(ctx.executor, id, ReconciliationStatus::Settled)
            .await
            .map_err(DomainError::Internal)?;

        // FMS placeholder: CashJournal + WriteOff will be triggered here when FMS is implemented

        self.audit
            .record(
                ctx.reborrow(),
                "Reconciliation",
                id,
                AuditAction::Transition,
                Some(serde_json::json!({ "from": "Confirmed", "to": "Settled" })),
                None,
            )
            .await?;

        Ok(())
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: ReconciliationQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Reconciliation>, DomainError> {
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
