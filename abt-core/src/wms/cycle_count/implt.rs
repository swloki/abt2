use std::sync::Arc;

use async_trait::async_trait;

use sqlx::postgres::PgPool;

use super::model::{
    CountCycleCountReq, CreateCycleCountReq, CycleCount, CycleCountFilter,
};
use super::repo::CycleCountRepo;
use super::service::CycleCountService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::DocumentType;
use crate::wms::enums::CycleCountStatus;

pub struct CycleCountServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
    doc_seq: Arc<dyn DocumentSequenceService>,
}

impl CycleCountServiceImpl {
    pub fn new(pool: Arc<PgPool>, doc_seq: Arc<dyn DocumentSequenceService>) -> Self {
        Self { pool, doc_seq }
    }

    fn status_name(s: CycleCountStatus) -> String {
        match s {
            CycleCountStatus::Draft => "Draft".to_string(),
            CycleCountStatus::Counting => "Counting".to_string(),
            CycleCountStatus::Completed => "Completed".to_string(),
            CycleCountStatus::Adjusted => "Adjusted".to_string(),
            CycleCountStatus::Cancelled => "Cancelled".to_string(),
        }
    }
}

#[async_trait]
impl CycleCountService for CycleCountServiceImpl {
    async fn create(
        &self,
        mut ctx: ServiceContext<'_>,
        req: CreateCycleCountReq,
    ) -> Result<i64> {
        if req.items.is_empty() {
            return Err(DomainError::validation("盘点单明细不能为空"));
        }

        let doc_number = self.doc_seq.next_number(ctx.reborrow(), DocumentType::CycleCount)
            .await
            .unwrap_or_else(|_| format!("CC{}", chrono::Utc::now().format("%Y%m%d%H%M%S")));

        let count = CycleCountRepo::insert(
            &mut *ctx.executor,
            &doc_number,
            &req,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(count.id)
    }

    async fn get(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<CycleCount> {
        CycleCountRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("盘点单"))
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: CycleCountFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<CycleCount>> {
        CycleCountRepo::list(&mut *ctx.executor, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn start_count(&self, ctx: ServiceContext<'_>, id: i64) -> Result<()> {
        let count = CycleCountRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("盘点单"))?;

        if count.status != CycleCountStatus::Draft {
            return Err(DomainError::InvalidStateTransition {
                from: Self::status_name(count.status),
                to: "Counting".to_string(),
            });
        }

        CycleCountRepo::update_status(&mut *ctx.executor, id, CycleCountStatus::Counting)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    async fn count(
        &self,
        ctx: ServiceContext<'_>,
        req: CountCycleCountReq,
    ) -> Result<()> {
        let cc = CycleCountRepo::get_by_id(&mut *ctx.executor, req.id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("盘点单"))?;

        if cc.status != CycleCountStatus::Counting {
            return Err(DomainError::business_rule(format!(
                "盘点单状态为 {}，无法录入盘点数量",
                Self::status_name(cc.status)
            )));
        }

        // 一次性获取所有明细，用于计算差异
        let items = CycleCountRepo::get_items(&mut *ctx.executor, req.id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        for item in &req.items {
            let cc_item = items
                .iter()
                .find(|i| i.id == item.item_id)
                .ok_or_else(|| DomainError::not_found("盘点明细"))?;

            let variance_qty = item.counted_qty - cc_item.system_qty;

            CycleCountRepo::update_item_counted(
                &mut *ctx.executor,
                item.item_id,
                item.counted_qty,
                variance_qty,
                item.variance_reason.as_deref(),
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }

        Ok(())
    }

    async fn complete(&self, ctx: ServiceContext<'_>, id: i64) -> Result<()> {
        let count = CycleCountRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("盘点单"))?;

        if count.status != CycleCountStatus::Counting {
            return Err(DomainError::InvalidStateTransition {
                from: Self::status_name(count.status),
                to: "Completed".to_string(),
            });
        }

        CycleCountRepo::update_status(&mut *ctx.executor, id, CycleCountStatus::Completed)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    async fn adjust(&self, ctx: ServiceContext<'_>, id: i64) -> Result<()> {
        let count = CycleCountRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("盘点单"))?;

        if count.status != CycleCountStatus::Completed {
            return Err(DomainError::InvalidStateTransition {
                from: Self::status_name(count.status),
                to: "Adjusted".to_string(),
            });
        }

        CycleCountRepo::update_status(&mut *ctx.executor, id, CycleCountStatus::Adjusted)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        CycleCountRepo::mark_items_adjusted(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    async fn cancel(&self, ctx: ServiceContext<'_>, id: i64) -> Result<()> {
        let count = CycleCountRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("盘点单"))?;

        if count.status != CycleCountStatus::Draft && count.status != CycleCountStatus::Counting {
            return Err(DomainError::InvalidStateTransition {
                from: Self::status_name(count.status),
                to: "Cancelled".to_string(),
            });
        }

        CycleCountRepo::update_status(&mut *ctx.executor, id, CycleCountStatus::Cancelled)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }
}
