use sqlx::PgPool;

use super::model::*;
use super::repo::ProfitCenterRepo;
use super::service::ProfitCenterService;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::enums::audit::AuditAction;
use crate::shared::types::{DomainError, PgExecutor, Result, ServiceContext};

pub struct ProfitCenterServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
}

impl ProfitCenterServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl ProfitCenterService for ProfitCenterServiceImpl {
    async fn list(&self, db: PgExecutor<'_>) -> Result<Vec<ProfitCenter>> {
        ProfitCenterRepo::list_active(db).await
    }

    async fn get(&self, db: PgExecutor<'_>, id: i64) -> Result<ProfitCenter> {
        ProfitCenterRepo::find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ProfitCenter"))
    }

    async fn create(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateProfitCenterReq,
    ) -> Result<i64> {
        let id = ProfitCenterRepo::create(db, &req, ctx.operator_id).await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "ProfitCenter",
                    entity_id: id,
                    action: AuditAction::Create,
                    changes: None,
                    context: None,
                },
            )
            .await?;

        Ok(id)
    }

    async fn update(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        req: UpdateProfitCenterReq,
    ) -> Result<()> {
        ProfitCenterRepo::find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ProfitCenter"))?;

        ProfitCenterRepo::update(db, id, &req, ctx.operator_id).await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "ProfitCenter",
                    entity_id: id,
                    action: AuditAction::Update,
                    changes: None,
                    context: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        ProfitCenterRepo::find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ProfitCenter"))?;

        ProfitCenterRepo::delete(db, id).await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "ProfitCenter",
                    entity_id: id,
                    action: AuditAction::Delete,
                    changes: None,
                    context: None,
                },
            )
            .await?;

        Ok(())
    }
}
