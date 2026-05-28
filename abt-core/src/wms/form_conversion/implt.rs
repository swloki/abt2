use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::{ConversionFilter, CreateConversionReq, FormConversion};
use super::repo::FormConversionRepo;
use super::service::FormConversionService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::PaginatedResult;
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::DocumentType;
use crate::wms::enums::ConversionStatus;

pub struct FormConversionServiceImpl {
    pool: PgPool,
}

impl FormConversionServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl FormConversionService for FormConversionServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateConversionReq,
    ) -> Result<i64> {
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::FormConversion)
            .await
            .unwrap_or_else(|_| format!("FC{}", chrono::Utc::now().format("%Y%m%d%H%M%S%3f")));

        let conversion =
            FormConversionRepo::insert(&mut *db, &doc_number, &req, ctx.operator_id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(conversion.id)
    }

    async fn get(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<FormConversion> {
        FormConversionRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("FormConversion"))
    }

    async fn list(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: ConversionFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<FormConversion>> {
        FormConversionRepo::list(&mut *db, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn complete(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let conversion = self.get(ctx, db, id).await?;

        if conversion.status != ConversionStatus::Draft {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", conversion.status),
                to: "Completed".to_string(),
            });
        }

        FormConversionRepo::update_status(&mut *db, id, ConversionStatus::Completed)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    async fn cancel(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let conversion = self.get(ctx, db, id).await?;

        if conversion.status != ConversionStatus::Draft {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", conversion.status),
                to: "Cancelled".to_string(),
            });
        }

        FormConversionRepo::update_status(&mut *db, id, ConversionStatus::Cancelled)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }
}
