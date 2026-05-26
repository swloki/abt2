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
use crate::fms::cash_journal::model::CreateCashJournalReq;
use crate::fms::cash_journal::service::CashJournalService;

pub struct ReconciliationServiceImpl {
    repo: ReconciliationRepo,
    item_repo: ReconciliationItemRepo,
    doc_seq: Arc<dyn DocumentSequenceService>,
    state_machine: Arc<dyn StateMachineService>,
    audit: Arc<dyn AuditLogService>,
    doc_link: Arc<dyn DocumentLinkService>,
    cost_entry: Arc<dyn CostEntryService>,
    cash_journal: Arc<dyn CashJournalService>,
}

impl ReconciliationServiceImpl {
    pub fn new(
        repo: ReconciliationRepo,
        item_repo: ReconciliationItemRepo,
        doc_seq: Arc<dyn DocumentSequenceService>,
        state_machine: Arc<dyn StateMachineService>,
        audit: Arc<dyn AuditLogService>,
        doc_link: Arc<dyn DocumentLinkService>,
        cost_entry: Arc<dyn CostEntryService>,
        cash_journal: Arc<dyn CashJournalService>,
    ) -> Self {
        Self {
            repo,
            item_repo,
            doc_seq,
            state_machine,
            audit,
            doc_link,
            cost_entry,
            cash_journal,
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
            ?
        {
            return Err(DomainError::duplicate(
                "Reconciliation already exists for this customer and period",
            ));
        }

        // Aggregate shipping items for this customer+period
        let aggregated = aggregate_shipping_items(ctx.executor, customer_id, &period)
            .await
            ?;

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
            ?;

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
                ?;
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
            ?
            .ok_or_else(|| DomainError::not_found("Reconciliation"))
    }

    async fn send(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        let existing = self
            .repo
            .find_by_id(ctx.executor, id)
            .await
            ?
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
            ?;

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
            ?
            .ok_or_else(|| DomainError::not_found("Reconciliation"))?;

        if existing.status != ReconciliationStatus::Sent {
            return Err(DomainError::business_rule("Only Sent reconciliations can be confirmed"));
        }

        let all_confirmed = self
            .item_repo
            .all_confirmed(ctx.executor, id)
            .await
            ?;

        if !all_confirmed {
            return Err(DomainError::business_rule(
                "All items must be confirmed before reconciliation can be confirmed",
            ));
        }

        // confirmed_amount = total_amount (all items confirmed), difference = 0
        self.repo
            .update_amounts(ctx.executor, id, existing.total_amount, Decimal::ZERO)
            .await
            ?;

        self.state_machine
            .transition(ctx.reborrow(), "ReconciliationStatus", id, "Confirmed", None)
            .await?;

        self.repo
            .update_status(ctx.executor, id, ReconciliationStatus::Confirmed)
            .await
            ?;

        // Create AR voucher via CostEntry
        let items = self
            .item_repo
            .find_by_reconciliation_id(ctx.executor, id)
            .await
            ?;

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
            ?
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
            ?;

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
            ?
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
            ?;

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
            ?
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
            ?;

        self.state_machine
            .transition(ctx.reborrow(), "ReconciliationStatus", id, "Settled", None)
            .await?;

        self.repo
            .update_status(ctx.executor, id, ReconciliationStatus::Settled)
            .await
            ?;

        // Create AR voucher via CostEntry for confirmed items (disputed path bypasses confirm)
        let items = self
            .item_repo
            .find_by_reconciliation_id(ctx.executor, id)
            .await
            ?;

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
            ?
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
            ?;

        // FMS: 对账结算时创建现金日记账 + 核销
        let _ = self.cash_journal.create(
            ctx.reborrow(),
            CreateCashJournalReq {
                journal_type: crate::fms::enums::JournalType::SalesReceipt,
                direction: crate::fms::enums::CashDirection::Inflow,
                amount: Decimal::ZERO,
                counterparty: crate::fms::enums::CounterpartyRef::Customer(0),
                source_type: DocumentType::Reconciliation,
                source_id: id,
                bank_account: String::new(),
                transaction_date: chrono::Local::now().date_naive(),
                period: chrono::Local::now().format("%Y-%m").to_string(),
                remark: String::new(),
                lines: vec![],
            },
        )
        .await;

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
            
    }
}
