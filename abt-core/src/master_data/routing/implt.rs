use std::sync::Arc;

use super::model::*;
use super::repo::RoutingRepo;
use super::service::RoutingService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, ServiceContext};

pub struct RoutingServiceImpl {
    repo: RoutingRepo,
    audit: Arc<dyn AuditLogService>,
    event_bus: Arc<dyn DomainEventBus>,
}

impl RoutingServiceImpl {
    pub fn new(
        repo: RoutingRepo,
        audit: Arc<dyn AuditLogService>,
        event_bus: Arc<dyn DomainEventBus>,
    ) -> Self {
        Self { repo, audit, event_bus }
    }
}

#[async_trait::async_trait]
impl RoutingService for RoutingServiceImpl {
    async fn list(&self, ctx: ServiceContext<'_>, query: RoutingQuery, page: PageParams) -> Result<PaginatedResult<Routing>> {
        self.repo.query(ctx.executor, &query, &page)
            .await
    }

    async fn get_detail(&self, ctx: ServiceContext<'_>, id: i64) -> Result<RoutingDetail> {
        let routing = self.repo.find_by_id(ctx.executor, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Routing"))?;

        let steps = self.repo.find_steps(ctx.executor, id)
            .await?;

        Ok(RoutingDetail { routing, steps })
    }

    async fn create(&self, mut ctx: ServiceContext<'_>, req: CreateRoutingReq) -> Result<i64> {
        let id = self.repo.create(ctx.executor, &req, ctx.operator_id)
            .await?;

        self.repo.insert_steps(ctx.executor, id, &req.steps)
            .await?;

        self.audit.record(ctx.reborrow(), "Routing", id, AuditAction::Create, None, None).await?;

        self.event_bus.publish(ctx, EventPublishRequest {
            event_type: DomainEventType::RoutingCreated,
            aggregate_type: "Routing".to_string(),
            aggregate_id: id,
            payload: serde_json::json!({ "name": req.name, "step_count": req.steps.len() }),
            idempotency_key: None,
        }).await?;

        Ok(id)
    }

    async fn update(&self, mut ctx: ServiceContext<'_>, id: i64, req: UpdateRoutingReq) -> Result<()> {
        let _existing = self.repo.find_by_id(ctx.executor, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Routing"))?;

        let has_changes = req.name.is_some() || req.description.is_some() || req.steps.is_some();
        if !has_changes {
            return Ok(());
        }

        self.repo.update(ctx.executor, id, &req, ctx.operator_id)
            .await?;

        if let Some(ref steps) = req.steps {
            self.repo.delete_steps(ctx.executor, id)
                .await?;
            self.repo.insert_steps(ctx.executor, id, steps)
                .await?;
        }

        self.audit.record(ctx.reborrow(), "Routing", id, AuditAction::Update, None, None).await?;

        self.event_bus.publish(ctx, EventPublishRequest {
            event_type: DomainEventType::RoutingUpdated,
            aggregate_type: "Routing".to_string(),
            aggregate_id: id,
            payload: serde_json::json!({}),
            idempotency_key: None,
        }).await?;

        Ok(())
    }

    async fn delete(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<()> {
        let _existing = self.repo.find_by_id(ctx.executor, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Routing"))?;

        let bom_bindings = self.repo.list_boms_by_routing(ctx.executor, id)
            .await?;
        if !bom_bindings.is_empty() {
            return Err(DomainError::business_rule("该工艺路线已被产品绑定，无法删除"));
        }

        self.repo.delete_steps(ctx.executor, id)
            .await?;

        self.repo.delete(ctx.executor, id)
            .await?;

        self.audit.record(ctx.reborrow(), "Routing", id, AuditAction::Delete, None, None).await?;

        self.event_bus.publish(ctx, EventPublishRequest {
            event_type: DomainEventType::RoutingDeleted,
            aggregate_type: "Routing".to_string(),
            aggregate_id: id,
            payload: serde_json::json!({}),
            idempotency_key: None,
        }).await?;

        Ok(())
    }

    async fn find_matching_routing(&self, ctx: ServiceContext<'_>, process_codes: Vec<String>) -> Result<Option<RoutingDetail>> {
        let routing_id = self.repo.find_matching_by_process_codes(ctx.executor, &process_codes)
            .await?;

        match routing_id {
            Some(id) => {
                let routing = self.repo.find_by_id(ctx.executor, id)
                    .await?
                    .ok_or_else(|| DomainError::not_found("Routing"))?;

                let steps = self.repo.find_steps(ctx.executor, id)
                    .await?;

                Ok(Some(RoutingDetail { routing, steps }))
            }
            None => Ok(None),
        }
    }

    async fn set_bom_routing(&self, mut ctx: ServiceContext<'_>, product_code: String, routing_id: i64) -> Result<()> {
        let _routing = self.repo.find_by_id(ctx.executor, routing_id)
            .await?
            .ok_or_else(|| DomainError::not_found("Routing"))?;

        self.repo.set_bom_routing(ctx.executor, &product_code, routing_id, ctx.operator_id)
            .await?;

        self.audit.record(ctx.reborrow(), "BomRouting", routing_id, AuditAction::Update, None, None).await?;

        self.event_bus.publish(ctx, EventPublishRequest {
            event_type: DomainEventType::BomRoutingChanged,
            aggregate_type: "BomRouting".to_string(),
            aggregate_id: routing_id,
            payload: serde_json::json!({ "product_code": product_code, "routing_id": routing_id }),
            idempotency_key: None,
        }).await?;

        Ok(())
    }

    async fn get_bom_routing(&self, ctx: ServiceContext<'_>, product_code: String) -> Result<Option<RoutingDetail>> {
        let bom_routing = self.repo.get_bom_routing(ctx.executor, &product_code)
            .await?;

        match bom_routing {
            Some(br) => {
                let routing = self.repo.find_by_id(ctx.executor, br.routing_id)
                    .await?;

                match routing {
                    Some(r) => {
                        let steps = self.repo.find_steps(ctx.executor, br.routing_id)
                            .await?;
                        Ok(Some(RoutingDetail { routing: r, steps }))
                    }
                    None => {
                        self.repo.delete_bom_routing(ctx.executor, &product_code)
                            .await?;
                        Ok(None)
                    }
                }
            }
            None => Ok(None),
        }
    }

    async fn list_boms_by_routing(&self, ctx: ServiceContext<'_>, routing_id: i64) -> Result<Vec<BomRouting>> {
        self.repo.list_boms_by_routing(ctx.executor, routing_id)
            .await
    }
}
