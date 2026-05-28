use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::{CostEntry, EntryRequest};
use super::repo::CostEntryRepo;
use super::service::CostEntryService;
use crate::shared::enums::CostEntityType;
use crate::shared::types::PgExecutor;
use crate::shared::types::batch::BatchResult;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

pub struct CostEntryServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
}

impl CostEntryServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CostEntryService for CostEntryServiceImpl {
    /// Atomic 模式 — 双层记账必须完整
    async fn create_entries(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        entries: Vec<EntryRequest>,
    ) -> Result<BatchResult> {
        let total = entries.len() as i32;
        if entries.is_empty() {
            return Ok(BatchResult::atomic_ok(0));
        }

        CostEntryRepo::batch_insert(&mut *db, &entries)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(BatchResult::atomic_ok(total))
    }

    async fn find_by_entity(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        entity_type: CostEntityType,
        entity_id: i64,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<CostEntry>> {
        let params = PageParams::new(page, page_size);

        let (items, total) = CostEntryRepo::find_by_entity(
            &mut *db,
            entity_type.as_i16(),
            entity_id,
            params.page_size.into(),
            params.offset().into(),
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(PaginatedResult::new(items, total, params.page, params.page_size))
    }
}
