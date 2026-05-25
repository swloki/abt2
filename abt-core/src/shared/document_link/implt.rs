use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::{DocumentLink, LinkRequest};
use super::repo::DocumentLinkRepo;
use super::service::DocumentLinkService;
use crate::shared::enums::DocumentType;
use crate::shared::types::batch::BatchResult;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

pub struct DocumentLinkServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
}

impl DocumentLinkServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DocumentLinkService for DocumentLinkServiceImpl {
    async fn create_links(
        &self,
        ctx: ServiceContext<'_>,
        requests: Vec<LinkRequest>,
    ) -> Result<BatchResult, DomainError> {
        let total = requests.len() as i32;
        if requests.is_empty() {
            return Ok(BatchResult::atomic_ok(0));
        }

        DocumentLinkRepo::batch_insert(
            &mut *ctx.executor,
            &requests,
            Some(ctx.operator_id),
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(BatchResult::atomic_ok(total))
    }

    async fn find_linked(
        &self,
        ctx: ServiceContext<'_>,
        source_type: DocumentType,
        source_id: i64,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<DocumentLink>, DomainError> {
        let params = PageParams::new(page, page_size);

        let (items, total) = DocumentLinkRepo::find_linked(
            &mut *ctx.executor,
            source_type,
            source_id,
            params.page_size.into(),
            params.offset().into(),
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(PaginatedResult::new(items, total, params.page, params.page_size))
    }
}
