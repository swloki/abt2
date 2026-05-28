use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::{CreateLockReq, InventoryLock, LockFilter};
use super::repo::InventoryLockRepo;
use super::service::InventoryLockService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::DocumentType;
use crate::wms::enums::LockStatus;

pub struct InventoryLockServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
    doc_seq: Arc<dyn DocumentSequenceService>,
}

impl InventoryLockServiceImpl {
    pub fn new(pool: PgPool, doc_seq: Arc<dyn DocumentSequenceService>) -> Self {
        Self { pool, doc_seq }
    }
}

#[async_trait]
impl InventoryLockService for InventoryLockServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateLockReq,
    ) -> Result<i64> {
        if req.locked_qty <= rust_decimal::Decimal::ZERO {
            return Err(DomainError::validation("锁定数量必须大于零"));
        }

        let doc_number = self.doc_seq.next_number(ctx, db, DocumentType::InventoryLock)
            .await
            .unwrap_or_else(|_| format!("LK{}", chrono::Utc::now().format("%Y%m%d%H%M%S")));

        let lock = InventoryLockRepo::insert(&mut *db, &doc_number, &req, ctx.operator_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(lock.id)
    }

    async fn get(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<InventoryLock> {
        InventoryLockRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(format!("InventoryLock#{id}")))
    }

    async fn list(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: LockFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<InventoryLock>> {
        InventoryLockRepo::list(&mut *db, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    /// 释放：Active → Released
    /// 设计：release = 正常释放
    async fn release(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let lock = self.get(ctx, db, id).await?;

        if lock.status != LockStatus::Active {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", lock.status),
                to: "Released".to_string(),
            });
        }

        InventoryLockRepo::update_status(&mut *db, id, LockStatus::Released)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    /// 作废：Active → Cancelled
    /// 设计：cancel = 管理员作废（预留量退回）
    async fn cancel(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let lock = self.get(ctx, db, id).await?;

        if lock.status != LockStatus::Active {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", lock.status),
                to: "Cancelled".to_string(),
            });
        }

        InventoryLockRepo::update_status(&mut *db, id, LockStatus::Cancelled)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }
}
