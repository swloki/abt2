use async_trait::async_trait;

use super::model::*;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

#[async_trait]
pub trait ReconciliationService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        customer_id: i64,
        period: String,
    ) -> Result<i64, DomainError>;

    async fn find_by_id(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<Reconciliation, DomainError>;

    async fn send(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn confirm(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn dispute(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn reopen(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn force_settle(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn settle(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: ReconciliationQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Reconciliation>, DomainError>;
}
