use sqlx::PgPool;
use rust_decimal::Decimal;

use crate::fms::ar_ap::enums::LedgerDirection;
use crate::fms::ar_ap::repo::{ArApLedgerInsert, ArApLedgerRepo};
use crate::fms::ar_ap::model::SettleReq;
use crate::fms::ar_ap::new_ar_ap_service;
use crate::fms::ar_ap::service::ArApService;
use crate::fms::cash_journal::model::*;
use crate::fms::cash_journal::repo::{CashJournalLineRepo, CashJournalRepo};
use crate::fms::cash_journal::service::CashJournalService;
use crate::fms::enums::{CounterpartyType, JournalType};
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::document_sequence::new_document_sequence_service;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::event_bus::new_domain_event_bus;
use crate::shared::idempotency::service::IdempotencyService;
use crate::shared::idempotency::new_idempotency_service;
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::state_machine::new_state_machine_service;
use crate::shared::types::{PgExecutor,DomainError, PageParams, PaginatedResult, ServiceContext, Result};
use crate::fms::enums::JournalStatus;

pub struct CashJournalServiceImpl {
    pool: PgPool,
}

impl CashJournalServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl CashJournalService for CashJournalServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateCashJournalReq,
    ) -> Result<i64> {
        if req.amount <= rust_decimal::Decimal::ZERO {
            return Err(DomainError::validation("amount must be greater than zero"));
        }
        if req.lines.is_empty() {
            return Err(DomainError::validation("at least one journal line is required"));
        }

        // Header amount must equal sum of line debit (and credit) totals
        let (total_debit, total_credit) = req.lines.iter().fold(
            (rust_decimal::Decimal::ZERO, rust_decimal::Decimal::ZERO),
            |(d, c), l| (d + l.debit_amount, c + l.credit_amount),
        );
        if total_debit != req.amount || total_credit != req.amount {
            return Err(DomainError::validation(
                "header amount must equal line debit and credit totals",
            ));
        }

        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::CashJournal)
            .await?;

        let id = CashJournalRepo::create(db, &doc_number, &req, ctx.operator_id)
            .await
            ?;

        CashJournalLineRepo::batch_insert(db, id, &req.lines)
            .await
            ?;

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "JournalStatus", id, "Draft", None)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "CashJournal", entity_id: id, action: AuditAction::Create, changes: None, context: None })
            .await?;

        Ok(id)
    }

    async fn confirm(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        idempotency_key: Option<String>,
    ) -> Result<()> {
        // Step 1: Idempotency check
        if let Some(ref key) = idempotency_key {
            let hash = crate::shared::idempotency::service::key_to_i64(key);
            let is_first = new_idempotency_service(self.pool.clone())
                .check_and_mark(ctx, db, hash, "CashJournal:confirm")
                .await?;
            if !is_first {
                return Ok(());
            }
        }

        // Step 2: Lock journal FOR UPDATE
        let journal = CashJournalRepo::get_for_update(db, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("CashJournal"))?;

        if journal.status != super::super::enums::JournalStatus::Draft {
            return Err(DomainError::business_rule(
                "Only Draft journals can be confirmed",
            ));
        }

        // Step 3: Aggregate lines debit/credit
        let (total_debit, total_credit) = CashJournalLineRepo::sum_debit_credit(db, id)
            .await
            ?;

        // Step 4: Validate balanced entry with non-zero totals
        if total_debit != total_credit {
            return Err(DomainError::business_rule("UnbalancedEntry"));
        }
        if total_debit == rust_decimal::Decimal::ZERO {
            return Err(DomainError::business_rule("ZeroEntry"));
        }

        // Step 5: State transition
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "JournalStatus", id, "Confirmed", None)
            .await?;

        // Update status with optimistic lock
        let rows = CashJournalRepo::update_status(
            db,
            id,
            super::super::enums::JournalStatus::Confirmed,
            journal.version,
        )
        .await
        ?;

        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx, db,
                    RecordAuditLogReq {
                        entity_type: "CashJournal",
                        entity_id: id,
                        action: AuditAction::Transition,
                        changes: Some(serde_json::json!({ "from": "Draft", "to": "Confirmed", })),
                        context: None,
                    },
                )
            .await?;

        // 业财一体：生成 AR/AP 台账记录 + 自动核销（同事务）
        match journal.journal_type {
            JournalType::SalesReceipt => {
                let is_auto_settle_source = matches!(
                    journal.source_type,
                    DocumentType::ShippingRequest
                        | DocumentType::ArrivalNotice
                        | DocumentType::OutsourcingOrder
                );
                let party_type = CounterpartyType::Customer;
                let party_id = journal.counterparty_id;

                // 查询客户币种
                let currency: String = sqlx::query_scalar::<sqlx::Postgres, Option<String>>(
                    "SELECT currency FROM customers WHERE customer_id = $1 AND deleted_at IS NULL",
                )
                .bind(party_id)
                .fetch_optional(&mut *db)
                .await?
                .flatten()
                .filter(|c| !c.is_empty())
                .unwrap_or_else(|| "CNY".to_string());

                // 台账：AR 减少（Credit）
                ArApLedgerRepo::insert(
                    db,
                    &ArApLedgerInsert {
                        party_type,
                        party_id,
                        source_type: DocumentType::CashJournal,
                        source_id: journal.id,
                        source_doc_no: &journal.doc_number,
                        against_type: if is_auto_settle_source { Some(journal.source_type) } else { None },
                        against_id: if is_auto_settle_source { Some(journal.source_id) } else { None },
                        direction: LedgerDirection::Credit,  // AR 减少
                        amount: journal.amount,
                        currency: &currency,
                        exchange_rate: Decimal::ONE,
                        transaction_date: journal.transaction_date,
                        due_date: None,
                        period: &journal.period,
                        description: &format!("收款确认 {}", journal.doc_number),
                        operator_id: ctx.operator_id,
                    },
                )
                .await?;

                // 自动核销：如果源单据是发票
                if is_auto_settle_source {
                    new_ar_ap_service(self.pool.clone())
                        .settle(ctx, db, SettleReq {
                            payment_source_type: DocumentType::CashJournal,
                            payment_source_id: journal.id,
                            invoice_source_type: journal.source_type,
                            invoice_source_id: journal.source_id,
                            amount: journal.amount,
                        })
                        .await?;
                }
            }
            JournalType::PurchasePayment => {
                let is_auto_settle_source = matches!(
                    journal.source_type,
                    DocumentType::ShippingRequest
                        | DocumentType::ArrivalNotice
                        | DocumentType::OutsourcingOrder
                );
                let party_type = CounterpartyType::Supplier;
                let party_id = journal.counterparty_id;

                // 查询供应商币种
                let currency: String = sqlx::query_scalar::<sqlx::Postgres, Option<String>>(
                    "SELECT currency FROM suppliers WHERE supplier_id = $1 AND deleted_at IS NULL",
                )
                .bind(party_id)
                .fetch_optional(&mut *db)
                .await?
                .flatten()
                .filter(|c| !c.is_empty())
                .unwrap_or_else(|| "CNY".to_string());

                // 台账：AP 减少（Debit）
                ArApLedgerRepo::insert(
                    db,
                    &ArApLedgerInsert {
                        party_type,
                        party_id,
                        source_type: DocumentType::CashJournal,
                        source_id: journal.id,
                        source_doc_no: &journal.doc_number,
                        against_type: if is_auto_settle_source { Some(journal.source_type) } else { None },
                        against_id: if is_auto_settle_source { Some(journal.source_id) } else { None },
                        direction: LedgerDirection::Debit,  // AP 减少
                        amount: journal.amount,
                        currency: &currency,
                        exchange_rate: Decimal::ONE,
                        transaction_date: journal.transaction_date,
                        due_date: None,
                        period: &journal.period,
                        description: &format!("付款确认 {}", journal.doc_number),
                        operator_id: ctx.operator_id,
                    },
                )
                .await?;

                // 自动核销
                if is_auto_settle_source {
                    new_ar_ap_service(self.pool.clone())
                        .settle(ctx, db, SettleReq {
                            payment_source_type: DocumentType::CashJournal,
                            payment_source_id: journal.id,
                            invoice_source_type: journal.source_type,
                            invoice_source_id: journal.source_id,
                            amount: journal.amount,
                        })
                        .await?;
                }
            }
            _ => {
                // Expense/Payroll/Other 无需 AR/AP 台账
            }
        }

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx, db,
                EventPublishRequest {
                    event_type: DomainEventType::CashJournalConfirmed,
                    aggregate_type: "CashJournal".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({
                        "cash_journal_id": id,
                        "doc_number": journal.doc_number,
                        "amount": journal.amount,
                        "period": journal.period,
                    }),
                    idempotency_key,
                },
            )
            .await?;

        Ok(())
    }

    async fn get(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<CashJournal> {
        CashJournalRepo::get_by_id(db, id)
            .await
            ?
            .ok_or_else(|| DomainError::not_found("CashJournal"))
    }

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: CashJournalFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<CashJournal>> {
        let (items, total) =
            CashJournalRepo::query(
                db,
                &filter,
                &page,
                ctx.data_scope,
                ctx.operator_id,
                ctx.department_id,
            )
            .await
            ?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    async fn get_balance(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        period: String,
    ) -> Result<BalanceSummary> {
        let (total_inflow, total_outflow) =
            CashJournalRepo::sum_balance_by_period(db, &period)
                .await
                ?;

        let net_balance = total_inflow - total_outflow;

        Ok(BalanceSummary {
            total_inflow,
            total_outflow,
            net_balance,
            currency: "CNY".to_string(),
        })
    }

    async fn list_recent(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        limit: i64,
    ) -> Result<Vec<CashJournal>> {
        let filter = CashJournalFilter { status: vec![JournalStatus::Confirmed], ..Default::default() };
        let page = PageParams::new(1, limit as u32);
        let (items, _) = CashJournalRepo::query(
            db, &filter, &page, ctx.data_scope, ctx.operator_id, ctx.department_id,
        ).await?;
        Ok(items)
    }

    async fn distribution_by_type(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        period: String,
    ) -> Result<Vec<(i16, Decimal)>> {
        CashJournalRepo::distribution_by_type(db, &period).await
    }

    async fn monthly_trend(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        months_back: i32,
    ) -> Result<Vec<(String, Decimal, Decimal)>> {
        CashJournalRepo::monthly_trend(db, months_back).await
    }

    async fn search_counterparties(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        counterparty_type: crate::fms::enums::CounterpartyType,
        keyword: &str,
        limit: i64,
    ) -> Result<Vec<CounterpartyResult>> {
        let like = format!("%{}%", keyword);
        match counterparty_type {
            crate::fms::enums::CounterpartyType::Customer => {
                let rows = sqlx::query_as::<sqlx::Postgres, (i64, String, String)>(
                    "SELECT customer_id, customer_name, customer_code FROM customers \
                     WHERE deleted_at IS NULL AND (customer_name ILIKE $1 OR customer_code ILIKE $1) \
                     ORDER BY customer_name LIMIT $2",
                )
                .bind(&like)
                .bind(limit)
                .fetch_all(db)
                .await?;
                Ok(rows
                    .into_iter()
                    .map(|(id, name, code)| CounterpartyResult { id, name, code })
                    .collect())
            }
            crate::fms::enums::CounterpartyType::Supplier => {
                let rows = sqlx::query_as::<sqlx::Postgres, (i64, String, String)>(
                    "SELECT supplier_id, supplier_name, supplier_code FROM suppliers \
                     WHERE deleted_at IS NULL AND (supplier_name ILIKE $1 OR supplier_code ILIKE $1) \
                     ORDER BY supplier_name LIMIT $2",
                )
                .bind(&like)
                .bind(limit)
                .fetch_all(db)
                .await?;
                Ok(rows
                    .into_iter()
                    .map(|(id, name, code)| CounterpartyResult { id, name, code })
                    .collect())
            }
            crate::fms::enums::CounterpartyType::Employee => {
                let rows = sqlx::query_as::<sqlx::Postgres, (i64, String, String)>(
                    "SELECT user_id, COALESCE(display_name, username), username FROM users \
                     WHERE is_active = TRUE AND (COALESCE(display_name, username) ILIKE $1 OR username ILIKE $1) \
                     ORDER BY COALESCE(display_name, username) LIMIT $2",
                )
                .bind(&like)
                .bind(limit)
                .fetch_all(db)
                .await?;
                Ok(rows
                    .into_iter()
                    .map(|(id, name, code)| CounterpartyResult { id, name, code })
                    .collect())
            }
            crate::fms::enums::CounterpartyType::Other => Ok(vec![]),
        }
    }

    async fn search_accounts(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        keyword: &str,
        limit: i64,
    ) -> Result<Vec<AccountResult>> {
        let like = format!("%{}%", keyword);
        let rows = sqlx::query_as::<sqlx::Postgres, (i64, String, String)>(
            "SELECT id, code, name FROM gl_accounts \
             WHERE deleted_at IS NULL AND is_detail = TRUE \
             AND (code ILIKE $1 OR name ILIKE $1) \
             ORDER BY code LIMIT $2",
        )
        .bind(&like)
        .bind(limit)
        .fetch_all(db)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(id, code, name)| AccountResult { id, code, name })
            .collect())
    }
}
