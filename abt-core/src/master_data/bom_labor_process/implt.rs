use std::sync::Arc;

use super::model::*;
use super::repo::BomLaborProcessRepo;
use super::service::BomLaborProcessService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

pub struct BomLaborProcessServiceImpl {
    repo: BomLaborProcessRepo,
    audit: Arc<dyn AuditLogService>,
}

impl BomLaborProcessServiceImpl {
    pub fn new(
        repo: BomLaborProcessRepo,
        audit: Arc<dyn AuditLogService>,
    ) -> Self {
        Self { repo, audit }
    }
}

#[async_trait::async_trait]
impl BomLaborProcessService for BomLaborProcessServiceImpl {
    async fn list(&self, ctx: ServiceContext<'_>, query: BomLaborProcessQuery, page: PageParams) -> Result<PaginatedResult<BomLaborProcess>, DomainError> {
        self.repo.query(ctx.executor, &query, &page)
            .await
    }

    async fn create(&self, mut ctx: ServiceContext<'_>, req: CreateBomLaborProcessReq) -> Result<i64, DomainError> {
        let id = self.repo.create(ctx.executor, &req, ctx.operator_id)
            .await?;

        self.audit.record(ctx.reborrow(), "BomLaborProcess", id, AuditAction::Create, None, None).await?;

        Ok(id)
    }

    async fn update(&self, mut ctx: ServiceContext<'_>, id: i64, req: UpdateBomLaborProcessReq) -> Result<(), DomainError> {
        let _existing = self.repo.find_by_id(ctx.executor, id)
            .await?
            .ok_or_else(|| DomainError::not_found("BomLaborProcess"))?;

        self.repo.update(ctx.executor, id, &req, ctx.operator_id)
            .await?;

        self.audit.record(ctx.reborrow(), "BomLaborProcess", id, AuditAction::Update, None, None).await?;

        Ok(())
    }

    async fn delete(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        let _existing = self.repo.find_by_id(ctx.executor, id)
            .await?
            .ok_or_else(|| DomainError::not_found("BomLaborProcess"))?;

        self.repo.delete(ctx.executor, id)
            .await?;

        self.audit.record(ctx.reborrow(), "BomLaborProcess", id, AuditAction::Delete, None, None).await?;

        Ok(())
    }
}
