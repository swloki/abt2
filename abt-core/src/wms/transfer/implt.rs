use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::{CreateTransferReq, InventoryTransfer, TransferFilter};
use super::repo::TransferRepo;
use super::service::TransferService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::DocumentType;
use crate::wms::enums::TransferStatus;

pub struct TransferServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
    doc_seq: Arc<dyn DocumentSequenceService>,
}

impl TransferServiceImpl {
    pub fn new(pool: Arc<PgPool>, doc_seq: Arc<dyn DocumentSequenceService>) -> Self {
        Self { pool, doc_seq }
    }
}

#[async_trait]
impl TransferService for TransferServiceImpl {
    async fn create(
        &self,
        mut ctx: ServiceContext<'_>,
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
        let doc_number = self.doc_seq.next_number(ctx.reborrow(), DocumentType::InventoryTransfer)
            .await
            .unwrap_or_else(|_| format!("TR{}", chrono::Utc::now().format("%Y%m%d%H%M%S%.f")));

        let transfer =
            TransferRepo::insert(&mut *ctx.executor, &doc_number, &req, ctx.operator_id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(transfer.id)
    }

    async fn get(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<InventoryTransfer> {
        TransferRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("调拨单"))
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: TransferFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<InventoryTransfer>> {
        TransferRepo::list(&mut *ctx.executor, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn dispatch(
        &self,
        mut ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<()> {
        let transfer = self.get(ctx.reborrow(), id).await?;

        // 状态校验：仅 Draft → InTransit
        if transfer.status != TransferStatus::Draft {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", transfer.status),
                to: "InTransit".to_string(),
            });
        }

        TransferRepo::update_status(&mut *ctx.executor, id, TransferStatus::InTransit)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    async fn complete(
        &self,
        mut ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<()> {
        let transfer = self.get(ctx.reborrow(), id).await?;

        // 状态校验：仅 InTransit → Completed
        if transfer.status != TransferStatus::InTransit {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", transfer.status),
                to: "Completed".to_string(),
            });
        }

        TransferRepo::update_status(&mut *ctx.executor, id, TransferStatus::Completed)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    async fn cancel(
        &self,
        mut ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<()> {
        let transfer = self.get(ctx.reborrow(), id).await?;

        // 状态校验：仅 Draft → Cancelled
        if transfer.status != TransferStatus::Draft {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", transfer.status),
                to: "Cancelled".to_string(),
            });
        }

        TransferRepo::update_status(&mut *ctx.executor, id, TransferStatus::Cancelled)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }
}
