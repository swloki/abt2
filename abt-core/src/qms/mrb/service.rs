use async_trait::async_trait;

use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};
use super::model::*;

#[async_trait]
pub trait MrbService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateMrbReq,
    ) -> Result<i64, DomainError>;

    async fn get(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<Mrb, DomainError>;

    async fn submit_for_review(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<(), DomainError>;

    /// 临时 approve 方法 — WorkflowEngine 尚为 stub，直接审批通过
    async fn approve(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<(), DomainError>;

    async fn execute_disposition(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
        req: ExecuteDispositionReq,
    ) -> Result<(), DomainError>;

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: MrbFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<Mrb>, DomainError>;
}
