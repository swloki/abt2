use sqlx::PgPool;

use super::model::*;
use super::repo::BomLaborProcessRepo;
use super::service::BomLaborProcessService;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::enums::audit::AuditAction;
use crate::shared::types::{PgExecutor,DomainError, PageParams, PaginatedResult, ServiceContext, Result};

pub struct BomLaborProcessServiceImpl {
    repo: BomLaborProcessRepo,
    pool: PgPool,
}

impl BomLaborProcessServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { repo: BomLaborProcessRepo, pool }
    }
}

#[async_trait::async_trait]
impl BomLaborProcessService for BomLaborProcessServiceImpl {
    async fn list(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, query: BomLaborProcessQuery, page: PageParams) -> Result<PaginatedResult<BomLaborProcess>> {
        self.repo.query(db, &query, &page)
            .await
    }

    async fn create(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CreateBomLaborProcessReq) -> Result<i64> {
        let id = self.repo.create(db, &req, ctx.operator_id)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "BomLaborProcess", entity_id: id, action: AuditAction::Create, changes: None, context: None }).await?;

        Ok(id)
    }

    async fn update(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, req: UpdateBomLaborProcessReq) -> Result<()> {
        let _existing = self.repo.find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("BomLaborProcess"))?;

        self.repo.update(db, id, &req, ctx.operator_id)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "BomLaborProcess", entity_id: id, action: AuditAction::Update, changes: None, context: None }).await?;

        Ok(())
    }

    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let _existing = self.repo.find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("BomLaborProcess"))?;

        self.repo.delete(db, id)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "BomLaborProcess", entity_id: id, action: AuditAction::Delete, changes: None, context: None }).await?;

        Ok(())
    }
}
