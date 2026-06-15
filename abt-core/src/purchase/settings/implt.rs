use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::{PurchaseSettings, UpdatePurchaseSettingsRequest};
use super::repo::PurchaseSettingsRepo;
use super::service::PurchaseSettingsService;
use crate::shared::types::PgExecutor;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::Result;

pub struct PurchaseSettingsServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
}

impl PurchaseSettingsServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PurchaseSettingsService for PurchaseSettingsServiceImpl {
    async fn get(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<PurchaseSettings> {
        PurchaseSettingsRepo::get(&mut *db).await
    }

    async fn update(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: UpdatePurchaseSettingsRequest,
    ) -> Result<()> {
        PurchaseSettingsRepo::update(&mut *db, &req).await
    }
}
