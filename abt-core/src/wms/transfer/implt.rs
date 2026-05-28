use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::{CreateTransferReq, InventoryTransfer, TransferFilter};
use super::repo::TransferRepo;
use super::service::TransferService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::DocumentType;
use crate::wms::enums::TransferStatus;

pub struct TransferServiceImpl {
    pool: PgPool,
}

impl TransferServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TransferService for TransferServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateTransferReq,
    ) -> Result<i64> {
        // 校验：至少一条明细
        if req.items.is_empty() {
            return Err(DomainError::Validation("调拨单至少需要一条明细".to_string()));
        }

        // 校验：源仓库和目标仓库不能相同
        if req.from_warehouse_id == req.to_warehouse_id {
            return Err(DomainError::BusinessRule(
                "源仓库和目标仓库不能相同".to_string(),
            ));
        }

        // 生成单据编号
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::InventoryTransfer)
            .await
            .unwrap_or_else(|_| format!("TR{}", chrono::Utc::now().format("%Y%m%d%H%M%S%.f")));

        let transfer =
            TransferRepo::insert(&mut *db, &doc_number, &req, ctx.operator_id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(transfer.id)
    }

    async fn get(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<InventoryTransfer> {
        TransferRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("调拨单"))
    }

    async fn list(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: TransferFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<InventoryTransfer>> {
        TransferRepo::list(&mut *db, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn dispatch(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let transfer = self.get(ctx, db, id).await?;

        // 状态校验：仅 Draft → InTransit
        if transfer.status != TransferStatus::Draft {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", transfer.status),
                to: "InTransit".to_string(),
            });
        }

        TransferRepo::update_status(&mut *db, id, TransferStatus::InTransit)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    async fn complete(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let transfer = self.get(ctx, db, id).await?;

        // 状态校验：仅 InTransit → Completed
        if transfer.status != TransferStatus::InTransit {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", transfer.status),
                to: "Completed".to_string(),
            });
        }

        TransferRepo::update_status(&mut *db, id, TransferStatus::Completed)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    async fn cancel(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let transfer = self.get(ctx, db, id).await?;

        // 状态校验：仅 Draft → Cancelled
        if transfer.status != TransferStatus::Draft {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", transfer.status),
                to: "Cancelled".to_string(),
            });
        }

        TransferRepo::update_status(&mut *db, id, TransferStatus::Cancelled)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }
}
