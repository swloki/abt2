use sqlx::PgPool;
use rust_decimal::Decimal;
use std::collections::HashMap;

use super::model::*;
use super::repo::{ArApLedgerRepo, ArApSettlementRepo};
use super::service::ArApService;
use crate::fms::enums::CounterpartyType;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::enums::audit::AuditAction;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, PgExecutor, Result, ServiceContext};

pub struct ArApServiceImpl {
    pool: PgPool,
}

impl ArApServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 计算账龄分析结果
    fn compute_aging(
        rows: Vec<(i64, String, Decimal, Option<chrono::NaiveDate>)>,
        as_of_date: chrono::NaiveDate,
        buckets: &[i32],
    ) -> Vec<AgingRow> {
        // Group by party_id
        let mut party_map: HashMap<i64, (String, Decimal, Vec<Decimal>, Decimal)> = HashMap::new();
        let n = buckets.len();

        for (party_id, party_name, outstanding, due_date) in rows {
            let age_days = match due_date {
                Some(d) => (as_of_date - d).num_days(),
                None => 0,
            };

            let entry = party_map.entry(party_id).or_insert_with(|| {
                (party_name.clone(), Decimal::ZERO, vec![Decimal::ZERO; n], Decimal::ZERO)
            });

            entry.1 += outstanding;

            // Find the right bucket
            let mut placed = false;
            for (i, &days) in buckets.iter().enumerate() {
                if age_days <= days as i64 {
                    entry.2[i] += outstanding;
                    placed = true;
                    break;
                }
            }
            if !placed {
                entry.3 += outstanding; // over max
            }
        }

        party_map
            .into_iter()
            .map(|(party_id, (party_name, total_outstanding, bucket_amounts, over_max))| AgingRow {
                party_id,
                party_name,
                total_outstanding,
                buckets: bucket_amounts,
                over_max,
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl ArApService for ArApServiceImpl {
    // ---- 台账查询 ----

    async fn list_ledger(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: ArApLedgerFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<ArApLedgerRow>> {
        let (items, total) = ArApLedgerRepo::query_with_party(db, &filter, &page).await?;
        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    async fn list_ledger_details(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: ArApLedgerFilter,
    ) -> Result<Vec<ArApLedgerDetailRow>> {
        ArApLedgerRepo::query_details(db, &filter).await
    }

    async fn ledger_summary(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: ArApLedgerFilter,
    ) -> Result<LedgerSummary> {
        let today = chrono::Utc::now().date_naive();
        ArApLedgerRepo::summary(db, &filter, today).await
    }

    async fn get_party_balance(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        party_type: CounterpartyType,
        party_id: i64,
    ) -> Result<PartyBalance> {
        ArApLedgerRepo::get_party_balance(db, party_type, party_id).await
    }

    async fn batch_party_balances(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        party_type: CounterpartyType,
        party_ids: &[i64],
    ) -> Result<Vec<PartyBalance>> {
        ArApLedgerRepo::batch_party_balances(db, party_type, party_ids).await
    }

    // ---- 核销 ----

    async fn settle(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: SettleReq,
    ) -> Result<SettleResult> {
        // Validate
        if req.amount <= Decimal::ZERO {
            return Err(DomainError::validation("settlement amount must be greater than zero"));
        }

        // 1. Find the invoice ledger entry by source_type and source_id
        let invoice_ledger = ArApLedgerRepo::get_open_by_source(
            db,
            req.invoice_source_type,
            req.invoice_source_id,
        )
        .await?
        .ok_or_else(|| DomainError::not_found("invoice ledger entry (may be fully settled or not found)"))?;

        // 2. Find the payment ledger entry
        let payment_ledger = ArApLedgerRepo::get_open_by_source(
            db,
            req.payment_source_type,
            req.payment_source_id,
        )
        .await?
        .ok_or_else(|| DomainError::not_found("payment ledger entry (may be fully applied or not found)"))?;

        // 3. Validate amounts
        if req.amount > invoice_ledger.outstanding() {
            return Err(DomainError::business_rule(
                "settlement amount exceeds invoice outstanding balance",
            ));
        }
        if req.amount > payment_ledger.outstanding() {
            return Err(DomainError::business_rule(
                "settlement amount exceeds payment unapplied balance",
            ));
        }

        // 4. Update amount_applied on both ledger entries
        let new_invoice_applied = invoice_ledger.amount_applied + req.amount;
        ArApLedgerRepo::update_amount_applied(db, invoice_ledger.id, new_invoice_applied).await?;

        let new_payment_applied = payment_ledger.amount_applied + req.amount;
        ArApLedgerRepo::update_amount_applied(db, payment_ledger.id, new_payment_applied).await?;

        // 5. 计算汇兑损益（币种不同时）
        let exchange_gain_loss = if payment_ledger.currency != invoice_ledger.currency {
            // 按发票汇率折算：付款原币金额 * 发票汇率 与 付款原币金额的差额
            // 简化处理：本位币金额差异 = payment_amount * (payment_rate - invoice_rate)
            let payment_base = req.amount * payment_ledger.exchange_rate;
            let invoice_base = req.amount * invoice_ledger.exchange_rate;
            invoice_base - payment_base
        } else {
            Decimal::ZERO
        };

        // 6. Create settlement record
        let today = chrono::Utc::now().date_naive();
        let settlement_id = ArApSettlementRepo::insert(
            db,
            req.payment_source_type,
            req.payment_source_id,
            req.invoice_source_type,
            req.invoice_source_id,
            req.amount,
            payment_ledger.id,
            invoice_ledger.id,
            exchange_gain_loss,
            today,
            ctx.operator_id,
        )
        .await?;

        // 6. Audit log
        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "ArApSettlement",
                    entity_id: settlement_id,
                    action: AuditAction::Create,
                    changes: Some(serde_json::json!({
                        "payment_type": req.payment_source_type.as_i16(),
                        "payment_id": req.payment_source_id,
                        "invoice_type": req.invoice_source_type.as_i16(),
                        "invoice_id": req.invoice_source_id,
                        "amount": req.amount,
                    })),
                    context: None,
                },
            )
            .await?;

        Ok(SettleResult {
            settlement_id,
            payment_ledger_id: payment_ledger.id,
            invoice_ledger_id: invoice_ledger.id,
        })
    }

    async fn unsettle(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        settlement_id: i64,
    ) -> Result<()> {
        // Fetch settlement
        let settlement = ArApSettlementRepo::get_by_id(db, settlement_id)
            .await?
            .ok_or_else(|| DomainError::not_found("ArApSettlement"))?;

        // Reverse amount_applied
        if let Some(pid) = settlement.payment_ledger_id {
            if let Some(payment_ledger) = ArApLedgerRepo::get_by_id(db, pid).await? {
                let new_applied = payment_ledger.amount_applied - settlement.amount;
                ArApLedgerRepo::update_amount_applied(db, pid, new_applied.max(Decimal::ZERO))
                    .await?;
            }
        }

        if let Some(iid) = settlement.invoice_ledger_id {
            if let Some(invoice_ledger) = ArApLedgerRepo::get_by_id(db, iid).await? {
                let new_applied = invoice_ledger.amount_applied - settlement.amount;
                ArApLedgerRepo::update_amount_applied(db, iid, new_applied.max(Decimal::ZERO))
                    .await?;
            }
        }

        // Delete settlement record
        ArApSettlementRepo::delete(db, settlement_id).await?;

        // Audit log
        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "ArApSettlement",
                    entity_id: settlement_id,
                    action: AuditAction::Delete,
                    changes: Some(serde_json::json!({
                        "reversed": true,
                        "payment_id": settlement.payment_source_id,
                        "invoice_id": settlement.invoice_source_id,
                        "amount": settlement.amount,
                    })),
                    context: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn list_settlements(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: SettlementFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<ArApSettlement>> {
        let (items, total) = ArApSettlementRepo::query(db, &filter, &page).await?;
        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    // ---- 账龄分析 ----

    async fn ar_aging(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: AgingReq,
    ) -> Result<Vec<AgingRow>> {
        let rows = ArApLedgerRepo::aging_query(
            db,
            CounterpartyType::Customer,
            req.as_of_date,
            req.party_ids.as_deref(),
        )
        .await?;
        Ok(Self::compute_aging(rows, req.as_of_date, &req.buckets))
    }

    async fn ap_aging(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: AgingReq,
    ) -> Result<Vec<AgingRow>> {
        let rows = ArApLedgerRepo::aging_query(
            db,
            CounterpartyType::Supplier,
            req.as_of_date,
            req.party_ids.as_deref(),
        )
        .await?;
        Ok(Self::compute_aging(rows, req.as_of_date, &req.buckets))
    }

    // ---- 未清项查询 ----

    async fn list_open_invoices(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        party_type: CounterpartyType,
        party_id: i64,
    ) -> Result<Vec<OpenInvoice>> {
        ArApLedgerRepo::list_open_invoices(db, party_type, party_id).await
    }

    async fn list_unapplied_payments(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        party_type: CounterpartyType,
        party_id: i64,
    ) -> Result<Vec<UnappliedPayment>> {
        ArApLedgerRepo::list_unapplied_payments(db, party_type, party_id).await
    }
}
