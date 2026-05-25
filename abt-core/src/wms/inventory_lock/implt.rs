use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::{CreateLockReq, InventoryLock, LockFilter};
use super::repo::InventoryLockRepo;
use super::service::InventoryLockService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;
use crate::wms::enums::LockStatus;
use crate::wms::stubs::DocumentSequenceStub;

pub struct InventoryLockServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
}

impl InventoryLockServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl InventoryLockService for InventoryLockServiceImpl {
    async fn create(
        &self,
        mut ctx: ServiceContext<'_>,
        req: CreateLockReq,
    ) -> Result<i64, DomainError> {
        if req.locked_qty <= rust_decimal::Decimal::ZERO {
            return Err(DomainError::validation("锁定数量必须大于零"));
        }

        let doc_number = DocumentSequenceStub::next_number(ctx.reborrow(), "LK-")
            .await
            .unwrap_or_else(|_| format!("LK{}", chrono::Utc::now().format("%Y%m%d%H%M%S")));

        let lock = InventoryLockRepo::insert(&mut *ctx.executor, &doc_number, &req, ctx.operator_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(lock.id)
    }

    async fn get(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<InventoryLock, DomainError> {
        InventoryLockRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(format!("InventoryLock#{id}")))
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: LockFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<InventoryLock>, DomainError> {
        InventoryLockRepo::list(&mut *ctx.executor, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    /// 释放：Active → Released
    /// 设计：release = 正常释放
    async fn release(
        &self,
        mut ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<(), DomainError> {
        let lock = self.get(ctx.reborrow(), id).await?;

        if lock.status != LockStatus::Active {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", lock.status),
                to: "Released".to_string(),
            });
        }

        InventoryLockRepo::update_status(&mut *ctx.executor, id, LockStatus::Released)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    /// 作废：Active → Cancelled
    /// 设计：cancel = 管理员作废（预留量退回）
    async fn cancel(
        &self,
        mut ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<(), DomainError> {
        let lock = self.get(ctx.reborrow(), id).await?;

        if lock.status != LockStatus::Active {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", lock.status),
                to: "Cancelled".to_string(),
            });
        }

        InventoryLockRepo::update_status(&mut *ctx.executor, id, LockStatus::Cancelled)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }
}
