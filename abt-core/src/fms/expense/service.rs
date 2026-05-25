use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

#[async_trait]
pub trait ExpenseReimbursementService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateExpenseReq,
    ) -> Result<i64, DomainError>;

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<ExpenseReimbursement, DomainError>;

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: ExpenseFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<ExpenseReimbursement>, DomainError>;

    /// Internal method called by WorkflowEngine Hook (IndependentTx).
    /// Opens its own transaction from PgPool internally.
    async fn generate_payment_journal(
        &self,
        ctx: ServiceContext<'_>,
        expense_id: i64,
    ) -> Result<i64, DomainError>;
}
