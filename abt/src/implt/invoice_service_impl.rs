//! 采购发票服务实现
//!
//! 实现采购发票管理的业务逻辑。

use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::sync::Arc;

use common::error::ServiceError;
use crate::models::{InvoiceDetail, InvoiceQuery};
use crate::repositories::{Executor, InvoiceRepo, PaginatedResult, PaginationParams};
use crate::service::InvoiceService;

/// 采购发票服务实现
pub struct InvoiceServiceImpl {
    pool: Arc<PgPool>,
}

impl InvoiceServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl InvoiceService for InvoiceServiceImpl {
    /// 创建发票
    async fn create(
        &self,
        invoice_no: String,
        supplier_id: i64,
        statement_id: Option<i64>,
        invoice_amount: Decimal,
        invoice_date: NaiveDate,
        remark: Option<String>,
        operator_id: Option<i64>,
        executor: Executor<'_>,
    ) -> Result<i64> {
        let invoice_id = InvoiceRepo::insert(
            executor,
            &invoice_no,
            supplier_id,
            statement_id,
            invoice_amount,
            invoice_date,
            remark.as_deref(),
            operator_id,
        )
        .await?;

        Ok(invoice_id)
    }

    /// 分页查询发票列表
    async fn list(&self, query: InvoiceQuery) -> Result<PaginatedResult<InvoiceDetail>> {
        let page = query.page.unwrap_or(1).max(1) as u32;
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100) as u32;

        let items = InvoiceRepo::query(&self.pool, &query).await?;
        let total = InvoiceRepo::query_count(&self.pool, &query).await?;

        let pagination = PaginationParams::new(page, page_size);
        Ok(PaginatedResult::new(items, total as u64, &pagination))
    }

    /// 更新发票状态（仅允许 1→2 已登记→已核验）
    async fn update_status(
        &self,
        invoice_id: i64,
        status: i16,
        executor: Executor<'_>,
    ) -> Result<()> {
        let current_status = InvoiceRepo::find_status(&self.pool, invoice_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "PurchaseInvoice".to_string(),
                id: invoice_id.to_string(),
            })?;

        if !is_valid_invoice_transition(current_status, status) {
            return Err(anyhow::Error::from(ServiceError::BusinessValidation {
                message: format!(
                    "不允许从状态【{}】变更为【{}】",
                    invoice_status_label(current_status),
                    invoice_status_label(status)
                ),
            }));
        }

        InvoiceRepo::update_status(executor, invoice_id, status).await?;
        Ok(())
    }
}

/// 发票状态转换白名单
fn is_valid_invoice_transition(from: i16, to: i16) -> bool {
    matches!(
        (from, to),
        (1, 2) // 已登记 → 已核验
    )
}

/// 发票状态标签
fn invoice_status_label(status: i16) -> &'static str {
    match status {
        1 => "已登记",
        2 => "已核验",
        _ => "未知",
    }
}
