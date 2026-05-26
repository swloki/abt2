use async_trait::async_trait;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;

use super::model::{CreateLockReq, InventoryLock, LockFilter};

#[async_trait]
pub trait InventoryLockService: Send + Sync {
    async fn create(
        &self,
        ctx: ServiceContext<'_>,
        req: CreateLockReq,
    ) -> Result<i64>;

    async fn get(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<InventoryLock>;

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: LockFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<InventoryLock>>;

    async fn release(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<()>;

    async fn cancel(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<()>;
}
