use std::sync::Arc;

use super::model::*;
use super::repo::LaborProcessDictRepo;
use super::service::LaborProcessDictService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

pub struct LaborProcessDictServiceImpl {
    repo: LaborProcessDictRepo,
    doc_seq: Arc<dyn DocumentSequenceService>,
    audit: Arc<dyn AuditLogService>,
    event_bus: Arc<dyn DomainEventBus>,
}

impl LaborProcessDictServiceImpl {
    pub fn new(
        repo: LaborProcessDictRepo,
        doc_seq: Arc<dyn DocumentSequenceService>,
        audit: Arc<dyn AuditLogService>,
        event_bus: Arc<dyn DomainEventBus>,
    ) -> Self {
        Self { repo, doc_seq, audit, event_bus }
    }
}

#[async_trait::async_trait]
impl LaborProcessDictService for LaborProcessDictServiceImpl {
    async fn list(&self, ctx: ServiceContext<'_>, query: LaborProcessDictQuery, page: PageParams) -> Result<PaginatedResult<LaborProcessDict>, DomainError> {
        self.repo.query(ctx.executor, &query, &page)
            .await.map_err(DomainError::Internal)
    }

    async fn create(&self, mut ctx: ServiceContext<'_>, req: CreateLaborProcessDictReq) -> Result<i64, DomainError> {
        let code = self.doc_seq.next_number(ctx.reborrow(), DocumentType::LaborProcessDict).await?;

        let id = self.repo.create(ctx.executor, &code, &req, ctx.operator_id)
            .await.map_err(DomainError::Internal)?;

        self.audit.record(ctx.reborrow(), "LaborProcessDict", id, AuditAction::Create, None, None).await?;

        self.event_bus.publish(ctx, EventPublishRequest {
            event_type: DomainEventType::LaborProcessDictCreated,
            aggregate_type: "LaborProcessDict".to_string(),
            aggregate_id: id,
            payload: serde_json::json!({ "code": code, "name": req.name }),
            idempotency_key: None,
        }).await?;

        Ok(id)
    }

    async fn update(&self, mut ctx: ServiceContext<'_>, id: i64, req: UpdateLaborProcessDictReq) -> Result<(), DomainError> {
        let _existing = self.repo.find_by_id(ctx.executor, id)
            .await.map_err(DomainError::Internal)?
            .ok_or_else(|| DomainError::not_found("LaborProcessDict"))?;

        self.repo.update(ctx.executor, id, &req, ctx.operator_id)
            .await.map_err(DomainError::Internal)?;

        self.audit.record(ctx.reborrow(), "LaborProcessDict", id, AuditAction::Update, None, None).await?;

        self.event_bus.publish(ctx, EventPublishRequest {
            event_type: DomainEventType::LaborProcessDictUpdated,
            aggregate_type: "LaborProcessDict".to_string(),
            aggregate_id: id,
            payload: serde_json::json!({}),
            idempotency_key: None,
        }).await?;

        Ok(())
    }

    async fn delete(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        let existing = self.repo.find_by_id(ctx.executor, id)
            .await.map_err(DomainError::Internal)?
            .ok_or_else(|| DomainError::not_found("LaborProcessDict"))?;

        if self.repo.exists_routing_step_reference(ctx.executor, &existing.code)
            .await.map_err(DomainError::Internal)?
        {
            return Err(DomainError::business_rule(format!("工序编码 '{}' 已被工艺路线引用，无法删除", existing.code)));
        }

        self.repo.delete(ctx.executor, id)
            .await.map_err(DomainError::Internal)?;

        self.audit.record(ctx.reborrow(), "LaborProcessDict", id, AuditAction::Delete, None, None).await?;

        self.event_bus.publish(ctx, EventPublishRequest {
            event_type: DomainEventType::LaborProcessDictDeleted,
            aggregate_type: "LaborProcessDict".to_string(),
            aggregate_id: id,
            payload: serde_json::json!({}),
            idempotency_key: None,
        }).await?;

        Ok(())
    }
}
