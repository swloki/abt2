use async_trait::async_trait;

use crate::shared::types::{PageParams, PaginatedResult, ServiceContext, Result};
use super::model::*;

#[async_trait]
pub trait RmaService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateRmaReq,
    ) -> Result<i64>;

    async fn get(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<Rma>;

    /// 记录根因 — 自动触发 Investigating → ActionTaken 状态转换
    async fn record_root_cause(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        req: RecordRootCauseReq,
    ) -> Result<()>;

    async fn close(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<()>;

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: RmaFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<Rma>>;
}
