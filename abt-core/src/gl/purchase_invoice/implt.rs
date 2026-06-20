use sqlx::PgPool;
use rust_decimal::Decimal;

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

        // Build complete entry lines (debit + credit)
        let mut lines = debit_lines;
        lines.push(GlEntryLineInput {
            account_id: ap_account_id,
            debit: Decimal::ZERO,
            credit: inv.total,
            cost_center: None,
            profit_center: None,
            project_id: None,
            memo: format!("应付账款: 供应商ID {}", inv.supplier_id),
        });

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
