use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::{ConversionFilter, CreateConversionReq, FormConversion};
use super::repo::FormConversionRepo;
use super::service::FormConversionService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::PaginatedResult;
use crate::wms::enums::ConversionStatus;
use crate::wms::stubs::DocumentSequenceStub;

pub struct FormConversionServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
}

impl FormConversionServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl FormConversionService for FormConversionServiceImpl {
    async fn create(
        &self,
        mut ctx: ServiceContext<'_>,
        req: CreateConversionReq,
    ) -> Result<i64, DomainError> {
        let doc_number = DocumentSequenceStub::next_number(ctx.reborrow(), "FC-")
            .await
            .unwrap_or_else(|_| format!("FC{}", chrono::Utc::now().format("%Y%m%d%H%M%S%3f")));

        let conversion =
            FormConversionRepo::insert(&mut *ctx.executor, &doc_number, &req, ctx.operator_id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(conversion.id)
    }

    async fn get(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<FormConversion, DomainError> {
        FormConversionRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("FormConversion"))
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        filter: ConversionFilter,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedResult<FormConversion>, DomainError> {
        FormConversionRepo::list(&mut *ctx.executor, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn complete(
        &self,
        mut ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<(), DomainError> {
        let conversion = self.get(ctx.reborrow(), id).await?;

        if conversion.status != ConversionStatus::Draft {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", conversion.status),
                to: "Completed".to_string(),
            });
        }

        FormConversionRepo::update_status(&mut *ctx.executor, id, ConversionStatus::Completed)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }

    async fn cancel(
        &self,
        mut ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<(), DomainError> {
        let conversion = self.get(ctx.reborrow(), id).await?;

        if conversion.status != ConversionStatus::Draft {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", conversion.status),
                to: "Cancelled".to_string(),
            });
        }

        FormConversionRepo::update_status(&mut *ctx.executor, id, ConversionStatus::Cancelled)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(())
    }
}
