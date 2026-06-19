use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::{UpdateWmsSettingsReq, WmsSettings};
use super::repo::WmsSettingsRepo;
use super::service::WmsSettingsService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::PgExecutor;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;

pub struct WmsSettingsServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
}

impl WmsSettingsServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WmsSettingsService for WmsSettingsServiceImpl {
    async fn get(&self, _ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<WmsSettings> {
        WmsSettingsRepo::get(&mut *db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn update(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: UpdateWmsSettingsReq,
    ) -> Result<WmsSettings> {
        WmsSettingsRepo::update(&mut *db, &req)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
}
