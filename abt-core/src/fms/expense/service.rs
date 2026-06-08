use async_trait::async_trait;
use rust_decimal::Decimal;

use super::model::*;
use crate::shared::types::{PgExecutor,PageParams, PaginatedResult, ServiceContext, Result};

#[async_trait]
pub trait ExpenseReimbursementService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateExpenseReq,
    ) -> Result<i64>;

    async fn get(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<ExpenseReimbursement>;

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: ExpenseFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<ExpenseReimbursement>>;

    async fn list_items(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        reimbursement_id: i64,
    ) -> Result<Vec<ExpenseReimbursementItem>>;

    /// Internal method called by WorkflowEngine Hook (IndependentTx).
    /// Opens its own transaction from PgPool internally.
    async fn generate_payment_journal(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        expense_id: i64,
    ) -> Result<i64>;

    async fn list_pending(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        limit: i64,
    ) -> Result<Vec<ExpenseReimbursement>>;

    /// 待审报销统计: (count, total_amount)
    async fn pending_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<(i64, Decimal)>;
}
