use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::*;
use super::repo::ProductionExceptionRepo;
use super::service::ProductionExceptionService;
use crate::mes::enums::*;
use crate::shared::types::PgExecutor;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;

pub struct ProductionExceptionServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
}

impl ProductionExceptionServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProductionExceptionService for ProductionExceptionServiceImpl {
    async fn find_by_id(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<ProductionException> {
        ProductionExceptionRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("ProductionException"))
    }

    async fn list(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: ExceptionListFilter, page: u32, page_size: u32,
    ) -> Result<crate::shared::types::PaginatedResult<ExceptionListItem>> {
        let (items, total) = ProductionExceptionRepo::list(&mut *db, &filter, page, page_size)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(crate::shared::types::PaginatedResult::new(items, total as u64, page, page_size))
    }

    async fn get_stats(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
    ) -> Result<ExceptionStats> {
        ProductionExceptionRepo::get_stats(&mut *db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn update_status(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        status: ExceptionStatus,
    ) -> Result<()> {
        ProductionExceptionRepo::update_status(&mut *db, id, status)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn list_events(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        exception_id: i64,
    ) -> Result<Vec<ExceptionEvent>> {
        ProductionExceptionRepo::list_events(&mut *db, exception_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn get_detail_lookups(
        &self,
        db: PgExecutor<'_>,
        exc: &ProductionException,
    ) -> Result<ExceptionDetailLookups> {
        let wo_doc_number = if let Some(wo_id) = exc.work_order_id {
            let row: Option<(String,)> = sqlx::query_as(
                "SELECT doc_number FROM work_orders WHERE id = $1",
            )
            .bind(wo_id)
            .fetch_optional(&mut *db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            row.map(|r| r.0)
        } else {
            None
        };

        let batch_no = if let Some(b_id) = exc.batch_id {
            let row: Option<(String,)> = sqlx::query_as(
                "SELECT batch_no FROM production_batches WHERE id = $1",
            )
            .bind(b_id)
            .fetch_optional(&mut *db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            row.map(|r| r.0)
        } else {
            None
        };

        let product_name = if let Some(p_id) = exc.product_id {
            let row: Option<(String,)> = sqlx::query_as(
                "SELECT pdt_name FROM products WHERE product_id = $1",
            )
            .bind(p_id)
            .fetch_optional(&mut *db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            row.map(|r| r.0)
        } else {
            None
        };

        let finder_name = if let Some(f_id) = exc.finder_id {
            let row: Option<(String,)> = sqlx::query_as(
                "SELECT display_name FROM users WHERE user_id = $1",
            )
            .bind(f_id)
            .fetch_optional(&mut *db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            row.map(|r| r.0)
        } else {
            None
        };

        let owner_name = if let Some(o_id) = exc.owner_id {
            let row: Option<(String,)> = sqlx::query_as(
                "SELECT display_name FROM users WHERE user_id = $1",
            )
            .bind(o_id)
            .fetch_optional(&mut *db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            row.map(|r| r.0)
        } else {
            None
        };

        Ok(ExceptionDetailLookups {
            wo_doc_number,
            batch_no,
            product_name,
            finder_name,
            owner_name,
        })
    }
}
