use sqlx::PgPool;

use super::model::*;
use super::repo::LaborProcessDictRepo;
use super::service::LaborProcessDictService;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService};
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::{new_domain_event_bus, service::DomainEventBus};
use crate::shared::event_bus::EventPublishRequest;
use crate::shared::types::{PgExecutor,DomainError, PageParams, PaginatedResult, ServiceContext, Result};

pub struct LaborProcessDictServiceImpl {
    repo: LaborProcessDictRepo,
    pool: PgPool,
}

impl LaborProcessDictServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { repo: LaborProcessDictRepo, pool }
    }
}

#[async_trait::async_trait]
impl LaborProcessDictService for LaborProcessDictServiceImpl {
    async fn list(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, query: LaborProcessDictQuery, page: PageParams) -> Result<PaginatedResult<LaborProcessDict>> {
        self.repo.query(db, &query, &page)
            .await
    }

    async fn create(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CreateLaborProcessDictReq) -> Result<i64> {
        let code = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::LaborProcessDict).await?;

        let id = self.repo.create(db, &code, &req, ctx.operator_id)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, "LaborProcessDict", id, AuditAction::Create, None, None).await?;

        new_domain_event_bus(self.pool.clone())
            .publish(ctx, db, EventPublishRequest {
                event_type: DomainEventType::LaborProcessDictCreated,
                aggregate_type: "LaborProcessDict".to_string(),
                aggregate_id: id,
                payload: serde_json::json!({ "code": code, "name": req.name }),
                idempotency_key: None,
            }).await?;

        Ok(id)
    }

    async fn update(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, req: UpdateLaborProcessDictReq) -> Result<()> {
        let _existing = self.repo.find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("LaborProcessDict"))?;

        self.repo.update(db, id, &req, ctx.operator_id)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, "LaborProcessDict", id, AuditAction::Update, None, None).await?;

        new_domain_event_bus(self.pool.clone())
            .publish(ctx, db, EventPublishRequest {
                event_type: DomainEventType::LaborProcessDictUpdated,
                aggregate_type: "LaborProcessDict".to_string(),
                aggregate_id: id,
                payload: serde_json::json!({}),
                idempotency_key: None,
            }).await?;

        Ok(())
    }

    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self.repo.find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("LaborProcessDict"))?;

        if self.repo.exists_routing_step_reference(db, &existing.code)
            .await?
        {
            return Err(DomainError::business_rule(format!("工序编码 '{}' 已被工艺路线引用，无法删除", existing.code)));
        }

        self.repo.delete(db, id)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, "LaborProcessDict", id, AuditAction::Delete, None, None).await?;

        new_domain_event_bus(self.pool.clone())
            .publish(ctx, db, EventPublishRequest {
                event_type: DomainEventType::LaborProcessDictDeleted,
                aggregate_type: "LaborProcessDict".to_string(),
                aggregate_id: id,
                payload: serde_json::json!({}),
                idempotency_key: None,
            }).await?;

        Ok(())
    }
}
