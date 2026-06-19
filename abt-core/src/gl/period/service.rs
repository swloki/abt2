use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{PgExecutor, ServiceContext, Result};

#[async_trait]
pub trait GlPeriodService: Send + Sync {
    /// 列出期间（支持按会计年度和状态过滤）
    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: PeriodFilter,
    ) -> Result<Vec<AccountingPeriod>>;

    /// 按 entry_date 定位期间（不存在或 closed 返回错误）
    async fn resolve_open(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        entry_date: chrono::NaiveDate,
    ) -> Result<AccountingPeriod>;

    /// 关闭期间（status open→closed，乐观锁；需校验该期无 draft 凭证）
    async fn close(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        period_id: i64,
    ) -> Result<()>;
}
