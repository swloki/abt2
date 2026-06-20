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
use super::repo::SalesInvoiceRepo;
use super::service::SalesInvoiceService;
use super::super::entry::{new_gl_entry_service, service::GlEntryService, model::GlEntryLineInput};
use super::super::mapping::{new_gl_mapping_service, service::GlMappingService};
use super::super::invoice::InvoiceStatus;

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

        // Calculate totals
        let subtotal: Decimal = req.items.iter()
            .map(|i| i.qty * i.unit_price)
            .sum();

        // TODO: Calculate tax based on tax_rate_id (暂时为0)
        let tax_amount = Decimal::ZERO;
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

        // Batch insert items
        SalesInvoiceRepo::batch_items(db, id, &req.items).await?;

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

        // Build complete entry lines (debit + credit)
        let mut lines = vec![GlEntryLineInput {
            account_id: ar_account_id,
            debit: inv.total,
            credit: Decimal::ZERO,
            cost_center: None,
            profit_center: None,
            project_id: None,
            memo: format!("应收账款: 客户ID {}", inv.customer_id),
        }];
        lines.extend(credit_lines);

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
