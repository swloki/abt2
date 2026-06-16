use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::TaxRate;
use super::repo::TaxRateRepo;
use super::service::TaxRateService;
use crate::shared::types::PgExecutor;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;

pub struct TaxRateServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
}

impl TaxRateServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TaxRateService for TaxRateServiceImpl {
    async fn list_active(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<Vec<TaxRate>> {
        TaxRateRepo::list_active(&mut *db).await
    }

    async fn get_by_id(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<Option<TaxRate>> {
        TaxRateRepo::get_by_id(&mut *db, id).await
    }

    async fn get_by_code(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        code: &str,
    ) -> Result<Option<TaxRate>> {
        TaxRateRepo::get_by_code(&mut *db, code).await
    }
}
