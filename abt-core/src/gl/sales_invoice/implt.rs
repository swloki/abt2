use chrono::Datelike;
use sqlx::PgPool;
use rust_decimal::Decimal;

use crate::fms::ar_ap::enums::LedgerDirection;
use crate::fms::ar_ap::model::SettleReq;
use crate::fms::ar_ap::new_ar_ap_service;
use crate::fms::ar_ap::repo::{ArApLedgerInsert, ArApLedgerRepo};
use crate::fms::ar_ap::service::ArApService;
use crate::fms::enums::CounterpartyType;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::state_machine::{new_state_machine_service, service::StateMachineService};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, PgExecutor, Result};

use super::model::*;
use super::repo::SalesInvoiceRepo;
use super::service::SalesInvoiceService;
use super::super::entry::{new_gl_entry_service, service::GlEntryService, model::GlEntryLineInput};
use super::super::mapping::{new_gl_mapping_service, service::GlMappingService};
use super::super::invoice::InvoiceStatus;
use crate::purchase::tax::service::TaxRateService;

pub struct SalesInvoiceServiceImpl {
    pool: PgPool,
}

impl SalesInvoiceServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl SalesInvoiceService for SalesInvoiceServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateSalesInvoiceReq,
    ) -> Result<i64> {
        // Validation
        if req.items.is_empty() {
            return Err(DomainError::validation("at least one invoice item is required"));
        }

        // Validate each item qty and price are positive
        for item in &req.items {
            if item.qty <= Decimal::ZERO {
                return Err(DomainError::validation("item quantity must be greater than zero"));
            }
            if item.unit_price < Decimal::ZERO {
                return Err(DomainError::validation("item unit price cannot be negative"));
            }
        }

        // Calculate per-item line_tax based on tax_rate_id, then aggregate totals.
        // 价外税：line_tax = qty * unit_price * rate / 100，四舍五入到 2 位小数。
        let tax_svc = crate::purchase::tax::new_tax_rate_service(self.pool.clone());
        let mut line_taxes: Vec<Decimal> = Vec::with_capacity(req.items.len());
        let mut subtotal = Decimal::ZERO;
        let mut tax_amount = Decimal::ZERO;
        for item in &req.items {
            let line_subtotal = item.qty * item.unit_price;
            let rate = match item.tax_rate_id {
                Some(tid) => tax_svc
                    .get_by_id(ctx, db, tid)
                    .await?
                    .map(|t| t.rate)
                    .unwrap_or(Decimal::ZERO),
                None => Decimal::ZERO,
            };
            let line_tax = (line_subtotal * rate / Decimal::from(100)).round_dp(2);
            line_taxes.push(line_tax);
            subtotal += line_subtotal;
            tax_amount += line_tax;
        }
        let total = subtotal + tax_amount;

        // Generate doc number
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::SalesInvoice)
            .await?;

        // Create invoice
        let id = SalesInvoiceRepo::create(
            db,
            &doc_number,
            &req,
            subtotal,
            tax_amount,
            total,
            ctx.operator_id,
        )
        .await?;

        // Batch insert items (with precomputed line_tax)
        SalesInvoiceRepo::batch_items(db, id, &req.items, &line_taxes).await?;

        // State machine transition to Draft
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "SalesInvoiceStatus", id, "Draft", None)
            .await?;

        // Audit log
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq {
                entity_type: "SalesInvoice",
                entity_id: id,
                action: AuditAction::Create,
                changes: None,
                context: None,
            })
            .await?;

        Ok(id)
    }

    async fn post(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        // Fetch invoice
        let inv = SalesInvoiceRepo::get_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesInvoice"))?;

        // Only Draft can be posted
        if inv.status != InvoiceStatus::Draft {
            return Err(DomainError::business_rule("Only Draft invoices can be posted"));
        }

        // Fetch items
        let items = SalesInvoiceRepo::list_items(db, id).await?;

        // Resolve accounts using GlMappingService
        let mapping_svc = new_gl_mapping_service(self.pool.clone());

        // 借方：应收账款
        let ar_account_id = mapping_svc.resolve(ctx, db, "default_ar", None).await?;

        // 贷方：主营业务收入 + 销项税额
        let mut credit_lines: Vec<GlEntryLineInput> = Vec::new();
        let mut total_tax = Decimal::ZERO;

        for item in &items {
            // Resolve revenue account (try product-level, fallback to default)
            let revenue_account_id = match mapping_svc
                .resolve(ctx, db, "default_revenue", Some(item.product_id))
                .await
            {
                Ok(id) => id,
                Err(_) => mapping_svc.resolve(ctx, db, "default_revenue", None).await?,
            };

            credit_lines.push(GlEntryLineInput {
                account_id: revenue_account_id,
                debit: Decimal::ZERO,
                credit: item.line_subtotal,
                cost_center: None,
                profit_center: None,
                project_id: None,
                memo: format!("产品收入: 产品ID {}", item.product_id),
            });

            total_tax += item.line_tax;
        }

        // 销项税额（如果有）
        if total_tax > Decimal::ZERO {
            let tax_account_id = mapping_svc.resolve(ctx, db, "default_tax_output", None).await?;
            credit_lines.push(GlEntryLineInput {
                account_id: tax_account_id,
                debit: Decimal::ZERO,
                credit: total_tax,
                cost_center: None,
                profit_center: None,
                project_id: None,
                memo: "销项税额".to_string(),
            });
        }

        // Build complete entry lines
        // 红字发票：借贷方向对调
        let (ar_debit, ar_credit, ledger_dir) = if inv.is_return {
            (Decimal::ZERO, inv.total, LedgerDirection::Credit)
        } else {
            (inv.total, Decimal::ZERO, LedgerDirection::Debit)
        };

        let mut lines = vec![GlEntryLineInput {
            account_id: ar_account_id,
            debit: ar_debit,
            credit: ar_credit,
            cost_center: None,
            profit_center: None,
            project_id: None,
            memo: format!("应收账款: 客户ID {}", inv.customer_id),
        }];

        // 红字发票：收入/税金行也反转
        if inv.is_return {
            let mut debit_lines: Vec<GlEntryLineInput> = credit_lines
                .into_iter()
                .map(|l| GlEntryLineInput {
                    debit: l.credit,
                    credit: Decimal::ZERO,
                    ..l
                })
                .collect();
            lines.append(&mut debit_lines);
        } else {
            lines.extend(credit_lines);
        }

        // Post GL entry (一步建 posted 凭证)
        let entry_id = new_gl_entry_service(self.pool.clone())
            .post_from_source(
                ctx,
                db,
                DocumentType::SalesInvoice,
                id,
                inv.issue_date,
                format!("销售发票 {}", inv.doc_number),
                lines,
            )
            .await?;

        // State machine transition
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "SalesInvoiceStatus", id, "Posted", None)
            .await?;

        // Update status with optimistic lock
        let rows = SalesInvoiceRepo::update_status(db, id, InvoiceStatus::Posted, inv.version).await?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // Attach GL entry ID
        SalesInvoiceRepo::attach_gl_entry(db, id, entry_id).await?;

        // 业财一体：生成 AR 台账记录（同事务，不可变）
        // 读取客户 payment_terms 计算到期日 + 客户币种
        let (customer_currency, payment_terms): (Option<String>, Option<String>) =
            sqlx::query_as::<sqlx::Postgres, (Option<String>, Option<String>)>(
                "SELECT currency, payment_terms FROM customers WHERE customer_id = $1 AND deleted_at IS NULL",
            )
            .bind(inv.customer_id)
            .fetch_optional(&mut *db)
            .await?
            .unwrap_or((None, None));

        let due_days = crate::fms::ar_ap::payment_terms::parse_payment_terms_days(payment_terms.as_deref());
        let due_date = inv.issue_date + chrono::Duration::days(due_days);
        let currency = customer_currency.filter(|c| !c.is_empty()).unwrap_or_else(|| "CNY".to_string());

        SalesInvoiceRepo::update_financial_fields(db, id, due_date, inv.total).await?;

        let period = format!("{}-{:02}", inv.issue_date.year(), inv.issue_date.month());
        ArApLedgerRepo::insert(
            db,
            &ArApLedgerInsert {
                party_type: CounterpartyType::Customer,
                party_id: inv.customer_id,
                account_id: ar_account_id,
                source_type: DocumentType::SalesInvoice,
                source_id: id,
                source_doc_no: &inv.doc_number,
                against_type: None,
                against_id: None,
                direction: ledger_dir,
                amount: inv.total,
                currency: &currency,
                exchange_rate: Decimal::ONE,
                transaction_date: inv.issue_date,
                due_date: Some(due_date),
                period: &period,
                gl_entry_id: Some(entry_id),
                description: &format!("销售发票 {}", inv.doc_number),
                operator_id: ctx.operator_id,
            },
        )
        .await?;

        // Audit log
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq {
                entity_type: "SalesInvoice",
                entity_id: id,
                action: AuditAction::Transition,
                changes: Some(serde_json::json!({
                    "from": "Draft",
                    "to": "Posted",
                    "gl_entry_id": entry_id
                })),
                context: None,
            })
            .await?;

        Ok(())
    }

    async fn create_return(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        original_id: i64,
    ) -> Result<i64> {
        // Fetch original invoice
        let (inv, items) = self.get(ctx, db, original_id).await?;

        // Only Posted invoices can be returned
        if inv.status != InvoiceStatus::Posted {
            return Err(DomainError::business_rule("Only Posted invoices can be returned"));
        }
        if inv.is_return {
            return Err(DomainError::business_rule("Cannot return a red invoice"));
        }

        // Generate new doc number
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::SalesInvoice)
            .await?;

        // Create red invoice record
        let red_id = SalesInvoiceRepo::create_red(
            db,
            &doc_number,
            &inv,
            ctx.operator_id,
        )
        .await?;

        // Copy items (same items, same amounts — reversal is in GL)
        let item_inputs: Vec<SalesInvoiceItemInput> = items
            .iter()
            .map(|i| SalesInvoiceItemInput {
                product_id: i.product_id,
                qty: i.qty,
                unit_price: i.unit_price,
                tax_rate_id: i.tax_rate_id,
            })
            .collect();
        let line_taxes: Vec<Decimal> = items.iter().map(|i| i.line_tax).collect();
        SalesInvoiceRepo::batch_items(db, red_id, &item_inputs, &line_taxes).await?;

        // State machine
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "SalesInvoiceStatus", red_id, "Draft", None)
            .await?;

        // Post the red invoice (reverses GL and AR/AP)
        self.post(ctx, db, red_id).await?;

        // Auto-settle red invoice against original
        new_ar_ap_service(self.pool.clone())
            .settle(ctx, db, SettleReq {
                payment_source_type: DocumentType::SalesInvoice, // red invoice acts as the "payment" side
                payment_source_id: red_id,
                invoice_source_type: DocumentType::SalesInvoice,
                invoice_source_id: original_id,
                amount: inv.total.min(inv.total), // full amount
            })
            .await?;

        Ok(red_id)
    }

    async fn cancel(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        // Fetch invoice
        let inv = SalesInvoiceRepo::get_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesInvoice"))?;

        // Only Posted can be cancelled
        if inv.status != InvoiceStatus::Posted {
            return Err(DomainError::business_rule("Only Posted invoices can be cancelled"));
        }

        // Sync cancel GL entry
        if let Some(gl_entry_id) = inv.gl_entry_id {
            new_gl_entry_service(self.pool.clone())
                .cancel(ctx, db, gl_entry_id)
                .await?;
        }

        // State machine transition
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "SalesInvoiceStatus", id, "Cancelled", None)
            .await?;

        // Update status with optimistic lock
        let rows = SalesInvoiceRepo::update_status(db, id, InvoiceStatus::Cancelled, inv.version).await?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // Audit log
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq {
                entity_type: "SalesInvoice",
                entity_id: id,
                action: AuditAction::Transition,
                changes: Some(serde_json::json!({
                    "from": "Posted",
                    "to": "Cancelled",
                    "gl_entry_id": inv.gl_entry_id
                })),
                context: None,
            })
            .await?;

        Ok(())
    }

    async fn get(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<(SalesInvoice, Vec<SalesInvoiceItem>)> {
        let invoice = SalesInvoiceRepo::get_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesInvoice"))?;

        let items = SalesInvoiceRepo::list_items(db, id).await?;

        Ok((invoice, items))
    }

    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: SalesInvoiceFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<SalesInvoice>> {
        let (items, total) = SalesInvoiceRepo::query(
            db,
            &filter,
            &page,
            ctx.data_scope,
            ctx.operator_id,
            ctx.department_id,
        )
        .await?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }
}
