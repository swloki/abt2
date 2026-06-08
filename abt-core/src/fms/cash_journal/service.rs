use async_trait::async_trait;

use rust_decimal::Decimal;
use super::model::*;
use crate::shared::types::{PgExecutor,PageParams, PaginatedResult, ServiceContext, Result};
#[async_trait]
pub trait CashJournalService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateCashJournalReq,
    ) -> Result<i64>;

    async fn confirm(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        idempotency_key: Option<String>,
    ) -> Result<()>;

    async fn get(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<CashJournal>;

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: CashJournalFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<CashJournal>>;

    async fn get_balance(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        period: String,
    ) -> Result<BalanceSummary>;

    async fn list_recent(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        limit: i64,
    ) -> Result<Vec<CashJournal>>;

    /// 按类型分布：当月各 journal_type 的合计金额
    async fn distribution_by_type(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        period: String,
    ) -> Result<Vec<(i16, Decimal)>>;

    /// 近 N 月现金流趋势: Vec<(period, inflow, outflow)>
    async fn monthly_trend(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        months_back: i32,
    ) -> Result<Vec<(String, Decimal, Decimal)>>;
}
