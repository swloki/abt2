use async_trait::async_trait;

use crate::shared::types::{PgExecutor,PageParams, PaginatedResult, ServiceContext, Result};
use super::model::*;

#[async_trait]
pub trait MrbService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateMrbReq,
    ) -> Result<i64>;

    async fn get(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<Mrb>;

    async fn submit_for_review(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;

    /// 临时 approve 方法 — WorkflowEngine 尚为 stub，直接审批通过
    async fn approve(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()>;

    async fn execute_disposition(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: ExecuteDispositionReq,
    ) -> Result<()>;

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: MrbFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<Mrb>>;
}
