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
use super::repo::PurchaseInvoiceRepo;
use super::service::PurchaseInvoiceService;
use super::super::entry::{new_gl_entry_service, service::GlEntryService, model::GlEntryLineInput};
use super::super::mapping::{new_gl_mapping_service, service::GlMappingService};
use super::super::invoice::InvoiceStatus;
use crate::purchase::tax::service::TaxRateService;

pub struct PurchaseInvoiceServiceImpl {
    pool: PgPool,
}

impl PurchaseInvoiceServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl PurchaseInvoiceService for PurchaseInvoiceServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreatePurchaseInvoiceReq,
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
            .next_number(ctx, db, DocumentType::PurchaseInvoice)
            .await?;

        // Create invoice
        let id = PurchaseInvoiceRepo::create(
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
        PurchaseInvoiceRepo::batch_items(db, id, &req.items, &line_taxes).await?;

        // State machine transition to Draft
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "PurchaseInvoiceStatus", id, "Draft", None)
            .await?;

        // Audit log
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq {
                entity_type: "PurchaseInvoice",
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
        let inv = PurchaseInvoiceRepo::get_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("PurchaseInvoice"))?;

        // Only Draft can be posted
        if inv.status != InvoiceStatus::Draft {
            return Err(DomainError::business_rule("Only Draft invoices can be posted"));
        }

        // Fetch items
        let items = PurchaseInvoiceRepo::list_items(db, id).await?;

        // Resolve accounts using GlMappingService
        let mapping_svc = new_gl_mapping_service(self.pool.clone());

        // 贷方：应付账款（total）
        let ap_account_id = mapping_svc.resolve(ctx, db, "default_ap", None).await?;

        // 借方：库存商品（按产品 line_subtotal）+ 进项税额（如果有）
        let mut debit_lines: Vec<GlEntryLineInput> = Vec::new();
        let mut total_tax = Decimal::ZERO;

        for item in &items {
            // Resolve inventory account (try product-level, fallback to default)
            let inventory_account_id = match mapping_svc
                .resolve(ctx, db, "default_inventory", Some(item.product_id))
                .await
            {
                Ok(id) => id,
                Err(_) => mapping_svc.resolve(ctx, db, "default_inventory", None).await?,
            };

            debit_lines.push(GlEntryLineInput {
                account_id: inventory_account_id,
                debit: item.line_subtotal,
                credit: Decimal::ZERO,
                cost_center: None,
                profit_center: None,
                project_id: None,
                memo: format!("库存商品: 产品ID {}", item.product_id),
            });

            total_tax += item.line_tax;
        }

        // 进项税额（如果有）
        if total_tax > Decimal::ZERO {
            let tax_account_id = mapping_svc.resolve(ctx, db, "default_tax_input", None).await?;
            debit_lines.push(GlEntryLineInput {
                account_id: tax_account_id,
                debit: total_tax,
                credit: Decimal::ZERO,
                cost_center: None,
                profit_center: None,
                project_id: None,
                memo: "进项税额".to_string(),
            });
        }

        // Build complete entry lines
        // 红字发票：借贷方向对调
        let (ap_debit, ap_credit, ledger_dir) = if inv.is_return {
            (inv.total, Decimal::ZERO, LedgerDirection::Debit)
        } else {
            (Decimal::ZERO, inv.total, LedgerDirection::Credit)
        };

        let mut lines: Vec<GlEntryLineInput> = if inv.is_return {
            let mut rev_lines: Vec<GlEntryLineInput> = debit_lines
                .into_iter()
                .map(|l| GlEntryLineInput {
                    debit: Decimal::ZERO,
                    credit: l.debit,
                    ..l
                })
                .collect();
            rev_lines.push(GlEntryLineInput {
                account_id: ap_account_id,
                debit: ap_debit,
                credit: ap_credit,
                cost_center: None,
                profit_center: None,
                project_id: None,
                memo: format!("应付账款: 供应商ID {}", inv.supplier_id),
            });
            rev_lines
        } else {
            let mut normal_lines = debit_lines;
            normal_lines.push(GlEntryLineInput {
                account_id: ap_account_id,
                debit: ap_debit,
                credit: ap_credit,
                cost_center: None,
                profit_center: None,
                project_id: None,
                memo: format!("应付账款: 供应商ID {}", inv.supplier_id),
            });
            normal_lines
        };

        // Post GL entry (一步建 posted 凭证)
        let entry_id = new_gl_entry_service(self.pool.clone())
            .post_from_source(
                ctx,
                db,
                DocumentType::PurchaseInvoice,
                id,
                inv.issue_date,
                format!("采购发票 {}", inv.doc_number),
                lines,
            )
            .await?;

        // State machine transition
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "PurchaseInvoiceStatus", id, "Posted", None)
            .await?;

        // Update status with optimistic lock
        let rows = PurchaseInvoiceRepo::update_status(db, id, InvoiceStatus::Posted, inv.version).await?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // Attach GL entry ID
        PurchaseInvoiceRepo::attach_gl_entry(db, id, entry_id).await?;

        // 业财一体：生成 AP 台账记录（同事务，不可变）
        // 读取供应商 payment_terms 计算到期日 + 供应商币种
        let (supplier_currency, payment_terms): (Option<String>, Option<String>) =
            sqlx::query_as::<sqlx::Postgres, (Option<String>, Option<String>)>(
                "SELECT currency, payment_terms FROM suppliers WHERE supplier_id = $1 AND deleted_at IS NULL",
            )
            .bind(inv.supplier_id)
            .fetch_optional(&mut *db)
            .await?
            .unwrap_or((None, None));

        let due_days = crate::fms::ar_ap::payment_terms::parse_payment_terms_days(payment_terms.as_deref());
        let due_date = inv.issue_date + chrono::Duration::days(due_days);
        let currency = supplier_currency.filter(|c| !c.is_empty()).unwrap_or_else(|| "CNY".to_string());

        PurchaseInvoiceRepo::update_financial_fields(db, id, due_date, inv.total).await?;

        let period = format!("{}-{:02}", inv.issue_date.year(), inv.issue_date.month());
        ArApLedgerRepo::insert(
            db,
            &ArApLedgerInsert {
                party_type: CounterpartyType::Supplier,
                party_id: inv.supplier_id,
                account_id: ap_account_id,
                source_type: DocumentType::PurchaseInvoice,
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
                description: &format!("采购发票 {}", inv.doc_number),
                operator_id: ctx.operator_id,
            },
        )
        .await?;

        // Audit log
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq {
                entity_type: "PurchaseInvoice",
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
        let (inv, items) = self.get(ctx, db, original_id).await?;

        if inv.status != InvoiceStatus::Posted {
            return Err(DomainError::business_rule("Only Posted invoices can be returned"));
        }
        if inv.is_return {
            return Err(DomainError::business_rule("Cannot return a red invoice"));
        }

        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::PurchaseInvoice)
            .await?;

        let red_id = PurchaseInvoiceRepo::create_red(db, &doc_number, &inv, ctx.operator_id).await?;

        let item_inputs: Vec<PurchaseInvoiceItemInput> = items
            .iter()
            .map(|i| PurchaseInvoiceItemInput {
                product_id: i.product_id,
                qty: i.qty,
                unit_price: i.unit_price,
                tax_rate_id: i.tax_rate_id,
            })
            .collect();
        let line_taxes: Vec<Decimal> = items.iter().map(|i| i.line_tax).collect();
        PurchaseInvoiceRepo::batch_items(db, red_id, &item_inputs, &line_taxes).await?;

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "PurchaseInvoiceStatus", red_id, "Draft", None)
            .await?;

        self.post(ctx, db, red_id).await?;

        new_ar_ap_service(self.pool.clone())
            .settle(ctx, db, SettleReq {
                payment_source_type: DocumentType::PurchaseInvoice,
                payment_source_id: red_id,
                invoice_source_type: DocumentType::PurchaseInvoice,
                invoice_source_id: original_id,
                amount: inv.total.min(inv.total),
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
        let inv = PurchaseInvoiceRepo::get_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("PurchaseInvoice"))?;

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
            .transition(ctx, db, "PurchaseInvoiceStatus", id, "Cancelled", None)
            .await?;

        // Update status with optimistic lock
        let rows = PurchaseInvoiceRepo::update_status(db, id, InvoiceStatus::Cancelled, inv.version).await?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // Audit log
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq {
                entity_type: "PurchaseInvoice",
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
    ) -> Result<(PurchaseInvoice, Vec<PurchaseInvoiceItem>)> {
        let invoice = PurchaseInvoiceRepo::get_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("PurchaseInvoice"))?;

        let items = PurchaseInvoiceRepo::list_items(db, id).await?;

        Ok((invoice, items))
    }

    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: PurchaseInvoiceFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<PurchaseInvoice>> {
        let (items, total) = PurchaseInvoiceRepo::query(
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
