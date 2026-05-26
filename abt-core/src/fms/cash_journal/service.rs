use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PageParams, PaginatedResult, ServiceContext, Result};

#[async_trait]
pub trait CashJournalService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateCashJournalReq,
    ) -> Result<i64>;

    async fn confirm(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        idempotency_key: Option<String>,
    ) -> Result<()>;

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<CashJournal>;

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: CashJournalFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<CashJournal>>;

    async fn get_balance(
        &self,
        ctx: ServiceContext<'_>,
        period: String,
    ) -> Result<BalanceSummary>;
}
