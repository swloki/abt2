use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

#[async_trait]
pub trait CashJournalService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateCashJournalReq,
    ) -> Result<i64, DomainError>;

    async fn confirm(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        idempotency_key: Option<String>,
    ) -> Result<(), DomainError>;

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<CashJournal, DomainError>;

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: CashJournalFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<CashJournal>, DomainError>;

    async fn get_balance(
        &self,
        ctx: ServiceContext<'_>,
        period: String,
    ) -> Result<BalanceSummary, DomainError>;
}
