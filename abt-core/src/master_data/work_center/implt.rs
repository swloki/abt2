use async_trait::async_trait;
use sqlx::PgPool;

use super::model::*;
use super::repo::WorkCenterRepo;
use super::service::WorkCenterService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::{PageParams, PaginatedResult};
use crate::shared::types::{PgExecutor, Result};

pub struct WorkCenterServiceImpl {
    pool: PgPool,
}

impl WorkCenterServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WorkCenterService for WorkCenterServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateWorkCenterReq,
    ) -> Result<i64> {
        let repo = WorkCenterRepo;
        let existing = repo.get_by_code(db, &req.code).await?;
        if existing.is_some() {
            return Err(DomainError::Duplicate(format!(
                "工作中心代码 {} 已存在",
                req.code
            )));
        }
        repo.create(db, &req, ctx.operator_id).await
    }

    async fn get(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<WorkCenter> {
        WorkCenterRepo
            .get_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("WorkCenter"))
    }

    async fn get_by_code(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        code: &str,
    ) -> Result<Option<WorkCenter>> {
        WorkCenterRepo.get_by_code(db, code).await
    }

    async fn list(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: WorkCenterFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<WorkCenter>> {
        WorkCenterRepo.list(db, &filter, &page).await
    }

    async fn list_active(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<Vec<WorkCenter>> {
        WorkCenterRepo.list_active(db).await
    }

    async fn update(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        req: UpdateWorkCenterReq,
    ) -> Result<()> {
        let repo = WorkCenterRepo;
        let existing = repo.get_by_id(db, id).await?;
        if existing.is_none() {
            return Err(DomainError::not_found("WorkCenter"));
        }
        repo.update(db, id, &req).await
    }

    async fn delete(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let repo = WorkCenterRepo;
        let existing = repo.get_by_id(db, id).await?;
        if existing.is_none() {
            return Err(DomainError::not_found("WorkCenter"));
        }
        repo.soft_delete(db, id).await
    }
}
