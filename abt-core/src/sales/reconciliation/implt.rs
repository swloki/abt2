use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::sales::reconciliation::model::*;
use crate::sales::reconciliation::repo::{
    aggregate_shipping_items, ReconciliationItemRepo, ReconciliationRepo,
};
use crate::sales::reconciliation::service::ReconciliationService;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::cost_entry::{new_cost_entry_service, model::EntryRequest, service::CostEntryService};
use crate::shared::document_link::{new_document_link_service, model::LinkRequest, service::DocumentLinkService};
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::cost::CostEntityType;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::link_type::LinkType;
use crate::shared::state_machine::{new_state_machine_service, service::StateMachineService};
use crate::shared::types::{PgExecutor, DomainError, PageParams, PaginatedResult, ServiceContext, Result};
use crate::fms::cash_journal::{new_cash_journal_service, model::CreateCashJournalReq, service::CashJournalService};

pub struct ReconciliationServiceImpl {
    repo: ReconciliationRepo,
    item_repo: ReconciliationItemRepo,
    pool: PgPool,
}

impl ReconciliationServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: ReconciliationRepo,
            item_repo: ReconciliationItemRepo,
            pool,
        }
    }
}

#[async_trait::async_trait]
impl ReconciliationService for ReconciliationServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        customer_id: i64,
        period: String,
    ) -> Result<i64> {
        if self
            .repo
            .exists_by_customer_period(db, customer_id, &period)
            .await?
        {
            return Err(DomainError::duplicate(
                "Reconciliation already exists for this customer and period",
            ));
        }

        let aggregated = aggregate_shipping_items(db, customer_id, &period).await?;
        let total_amount: Decimal = aggregated.iter().map(|a| a.amount).sum();

        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::Reconciliation)
            .await?;

        let id = self
            .repo
            .create(
                db,
                &CreateReconciliationParams {
                    doc_number: &doc_number,
                    customer_id,
                    period: &period,
                    total_amount,
                    remark: "",
                    operator_id: ctx.operator_id,
                },
            )
            .await?;

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
            self.item_repo.create_batch(db, id, &item_inputs).await?;
        }

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
            new_document_link_service(self.pool.clone())
                .create_links(ctx, db, links)
                .await?;
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ReconciliationStatus", id, "Draft", None)
            .await
            .ok();

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx, db,
                    RecordAuditLogReq {
                        entity_type: "Reconciliation",
                        entity_id: id,
                        action: AuditAction::Create,
                        changes: Some(serde_json::json!({ "customer_id": customer_id, "period": period })),
                        context: None,
                    },
                )
            .await?;

        Ok(id)
    }

    async fn find_by_id(
        &self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64,
    ) -> Result<Reconciliation> {
        self.repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Reconciliation"))
    }

    async fn send(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self.repo.find_by_id(db, id).await?
            .ok_or_else(|| DomainError::not_found("Reconciliation"))?;

        if existing.status != ReconciliationStatus::Draft {
            return Err(DomainError::business_rule("Only Draft reconciliations can be sent"));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ReconciliationStatus", id, "Sent", None)
            .await?;

        self.repo.update_status(db, id, ReconciliationStatus::Sent).await?;

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx, db,
                    RecordAuditLogReq {
                        entity_type: "Reconciliation",
                        entity_id: id,
                        action: AuditAction::Transition,
                        changes: Some(serde_json::json!({ "from": "Draft", "to": "Sent" })),
                        context: None,
                    },
                )
            .await?;

        Ok(())
    }

    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self.repo.find_by_id(db, id).await?
            .ok_or_else(|| DomainError::not_found("Reconciliation"))?;

        if existing.status != ReconciliationStatus::Sent {
            return Err(DomainError::business_rule("Only Sent reconciliations can be confirmed"));
        }

        let all_confirmed = self.item_repo.all_confirmed(db, id).await?;
        if !all_confirmed {
            return Err(DomainError::business_rule(
                "All items must be confirmed before reconciliation can be confirmed",
            ));
        }

        self.repo.update_amounts(db, id, existing.total_amount, Decimal::ZERO).await?;

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ReconciliationStatus", id, "Confirmed", None)
            .await?;

        self.repo.update_status(db, id, ReconciliationStatus::Confirmed).await?;

        let items = self.item_repo.find_by_reconciliation_id(db, id).await?;
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
            new_cost_entry_service(self.pool.clone())
                .create_entries(ctx, db, cost_entries)
                .await?;
        }

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx, db,
                    RecordAuditLogReq {
                        entity_type: "Reconciliation",
                        entity_id: id,
                        action: AuditAction::Transition,
                        changes: Some(serde_json::json!({ "from": "Sent", "to": "Confirmed" })),
                        context: None,
                    },
                )
            .await?;

        Ok(())
    }

    async fn dispute(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self.repo.find_by_id(db, id).await?
            .ok_or_else(|| DomainError::not_found("Reconciliation"))?;

        if existing.status != ReconciliationStatus::Sent
            && existing.status != ReconciliationStatus::Confirmed
        {
            return Err(DomainError::business_rule(
                "Only Sent or Confirmed reconciliations can be disputed",
            ));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ReconciliationStatus", id, "Disputed", None)
            .await?;

        self.repo.update_status(db, id, ReconciliationStatus::Disputed).await?;

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx, db,
                    RecordAuditLogReq {
                        entity_type: "Reconciliation",
                        entity_id: id,
                        action: AuditAction::Transition,
                        changes: Some(serde_json::json!({ "from": existing.status.as_str(), "to": "Disputed" })),
                        context: None,
                    },
                )
            .await?;

        Ok(())
    }

    async fn reopen(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self.repo.find_by_id(db, id).await?
            .ok_or_else(|| DomainError::not_found("Reconciliation"))?;

        if existing.status != ReconciliationStatus::Disputed {
            return Err(DomainError::business_rule("Only Disputed reconciliations can be reopened"));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ReconciliationStatus", id, "Draft", None)
            .await?;

        self.repo.update_status(db, id, ReconciliationStatus::Draft).await?;

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx, db,
                    RecordAuditLogReq {
                        entity_type: "Reconciliation",
                        entity_id: id,
                        action: AuditAction::Transition,
                        changes: Some(serde_json::json!({ "from": "Disputed", "to": "Draft" })),
                        context: None,
                    },
                )
            .await?;

        Ok(())
    }

    async fn force_settle(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self.repo.find_by_id(db, id).await?
            .ok_or_else(|| DomainError::not_found("Reconciliation"))?;

        if existing.status != ReconciliationStatus::Disputed {
            return Err(DomainError::business_rule(
                "Only Disputed reconciliations can be force-settled",
            ));
        }

        self.repo
            .update_amounts(db, id, existing.total_amount - existing.difference, existing.difference)
            .await?;

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ReconciliationStatus", id, "Settled", None)
            .await?;

        self.repo.update_status(db, id, ReconciliationStatus::Settled).await?;

        let items = self.item_repo.find_by_reconciliation_id(db, id).await?;
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
            new_cost_entry_service(self.pool.clone())
                .create_entries(ctx, db, cost_entries)
                .await?;
        }

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx, db,
                    RecordAuditLogReq {
                        entity_type: "Reconciliation",
                        entity_id: id,
                        action: AuditAction::Transition,
                        changes: Some(serde_json::json!({ "from": "Disputed", "to": "Settled" })),
                        context: None,
                    },
                )
            .await?;

        Ok(())
    }

    async fn settle(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self.repo.find_by_id(db, id).await?
            .ok_or_else(|| DomainError::not_found("Reconciliation"))?;

        if existing.status != ReconciliationStatus::Confirmed {
            return Err(DomainError::business_rule(
                "Only Confirmed reconciliations can be settled",
            ));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ReconciliationStatus", id, "Settled", None)
            .await?;

        self.repo.update_status(db, id, ReconciliationStatus::Settled).await?;

        new_cash_journal_service(self.pool.clone())
            .create(
                ctx, db,
                CreateCashJournalReq {
                    journal_type: crate::fms::enums::JournalType::SalesReceipt,
                    direction: crate::fms::enums::CashDirection::Inflow,
                    amount: existing.confirmed_amount,
                    counterparty: crate::fms::enums::CounterpartyRef::Customer(existing.customer_id),
                    source_type: DocumentType::Reconciliation,
                    source_id: id,
                    bank_account: String::new(),
                    transaction_date: chrono::Local::now().date_naive(),
                    period: chrono::Local::now().format("%Y-%m").to_string(),
                    remark: String::new(),
                    lines: vec![],
                },
            )
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx, db,
                    RecordAuditLogReq {
                        entity_type: "Reconciliation",
                        entity_id: id,
                        action: AuditAction::Transition,
                        changes: Some(serde_json::json!({ "from": "Confirmed", "to": "Settled" })),
                        context: None,
                    },
                )
            .await?;

        Ok(())
    }

    async fn list(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: ReconciliationQuery, page: PageParams,
    ) -> Result<PaginatedResult<Reconciliation>> {
        self.repo
            .query(db, &filter, &page, ctx.data_scope, ctx.operator_id, ctx.department_id)
            .await
    }

    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self.repo.find_by_id(db, id).await?
            .ok_or_else(|| DomainError::not_found("Reconciliation"))?;

        if existing.status != ReconciliationStatus::Draft {
            return Err(DomainError::business_rule("仅草稿状态的对账单可以删除"));
        }

        self.repo.soft_delete(db, id).await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "Reconciliation", entity_id: id, action: AuditAction::Delete, changes: None, context: None })
            .await?;

        Ok(())
    }


    async fn list_items(
        &self, _ctx: &ServiceContext, db: PgExecutor<'_>, reconciliation_id: i64,
    ) -> Result<Vec<ReconciliationItem>> {
        self.item_repo.find_by_reconciliation_id(db, reconciliation_id).await
    }

    async fn preview(
        &self, _ctx: &ServiceContext, db: PgExecutor<'_>,
        customer_id: i64, period: String,
    ) -> Result<Vec<ReconciliationPreviewItem>> {
        let aggregated = aggregate_shipping_items(db, customer_id, &period).await?;
        Ok(aggregated.into_iter().map(|a| ReconciliationPreviewItem {
            shipping_request_id: a.shipping_request_id,
            sales_order_id: a.sales_order_id,
            product_id: a.product_id,
            quantity: a.quantity,
            unit_price: a.unit_price,
            amount: a.amount,
        }).collect())
    }
}
