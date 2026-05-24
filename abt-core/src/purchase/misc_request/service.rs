use async_trait::async_trait;

use super::model::{CreateMiscRequestRequest, MiscellaneousRequest};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;

#[async_trait]
pub trait MiscellaneousRequestService: Send + Sync {
    async fn create(
        ctx: ServiceContext<'_>,
        req: CreateMiscRequestRequest,
    ) -> Result<i64, DomainError>;

    async fn get(ctx: ServiceContext<'_>, id: i64) -> Result<MiscellaneousRequest, DomainError>;

    async fn approve(ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
}
