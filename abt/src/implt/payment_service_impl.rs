use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::sync::Arc;

use common::error::ServiceError;
use crate::models::{PaymentDetail, PaymentQuery, PurchasePayment};
use crate::repositories::{
    DocumentSequenceRepo, Executor, PaginatedResult, PaginationParams, PaymentRepo,
};
use crate::service::PaymentService;

pub struct PaymentServiceImpl {
    pool: Arc<PgPool>,
}

impl PaymentServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PaymentService for PaymentServiceImpl {
    /// 创建付款（自动生成付款编号）
    async fn create(
        &self,
        supplier_id: i64,
        invoice_id: Option<i64>,
        payment_amount: Decimal,
        payment_method: Option<String>,
        remark: Option<String>,
        operator_id: Option<i64>,
        executor: Executor<'_>,
    ) -> Result<i64> {
        let payment_no = DocumentSequenceRepo::next_number(&mut *executor, "PP").await?;

        let payment_id = PaymentRepo::insert(
            executor,
            &payment_no,
            supplier_id,
            invoice_id,
            payment_amount,
            payment_method.as_deref(),
            remark.as_deref(),
            operator_id,
        )
        .await?;

        Ok(payment_id)
    }

    async fn get_by_id(&self, payment_id: i64) -> Result<Option<PurchasePayment>> {
        PaymentRepo::find_by_id(&self.pool, payment_id).await
    }

    async fn list(&self, query: PaymentQuery) -> Result<PaginatedResult<PaymentDetail>> {
        let page = query.page.unwrap_or(1).max(1) as u32;
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100) as u32;

        let items = PaymentRepo::query(&self.pool, &query).await?;
        let total = PaymentRepo::query_count(&self.pool, &query).await?;

        let pagination = PaginationParams::new(page, page_size);
        Ok(PaginatedResult::new(items, total as u64, &pagination))
    }

    /// 更新付款状态（1→2→3：待审批→已审批→已付款）
    async fn update_status(
        &self,
        payment_id: i64,
        status: i16,
        executor: Executor<'_>,
    ) -> Result<()> {
        let current_status = PaymentRepo::find_status(&self.pool, payment_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "PurchasePayment".to_string(),
                id: payment_id.to_string(),
            })?;

        if !is_valid_payment_transition(current_status, status) {
            return Err(anyhow::Error::from(ServiceError::BusinessValidation {
                message: format!(
                    "不允许从状态【{}】变更为【{}】",
                    payment_status_label(current_status),
                    payment_status_label(status)
                ),
            }));
        }

        PaymentRepo::update_status(executor, payment_id, status).await?;
        Ok(())
    }
}

/// 付款状态转换白名单
fn is_valid_payment_transition(from: i16, to: i16) -> bool {
    matches!(
        (from, to),
        (1, 2) // 待审批 → 已审批
        | (2, 3) // 已审批 → 已付款
    )
}

/// 付款状态标签
fn payment_status_label(status: i16) -> &'static str {
    match status {
        1 => "待审批",
        2 => "已审批",
        3 => "已付款",
        _ => "未知",
    }
}
