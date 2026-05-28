use async_trait::async_trait;

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

    /// Internal method called by WorkflowEngine Hook (IndependentTx).
    /// Opens its own transaction from PgPool internally.
    async fn generate_payment_journal(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        expense_id: i64,
    ) -> Result<i64>;
}
