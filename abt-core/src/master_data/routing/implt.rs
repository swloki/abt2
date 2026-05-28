use sqlx::PgPool;

use super::model::*;
use super::repo::RoutingRepo;
use super::service::RoutingService;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::{new_domain_event_bus, service::DomainEventBus};
use crate::shared::event_bus::EventPublishRequest;
use crate::shared::types::{
    DomainError, PageParams, PaginatedResult, PgExecutor, Result, ServiceContext,
};

pub struct RoutingServiceImpl {
    repo: RoutingRepo,
    pool: PgPool,
}

impl RoutingServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: RoutingRepo,
            pool,
        }
    }
}

#[async_trait::async_trait]
impl RoutingService for RoutingServiceImpl {
    async fn list(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: RoutingQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Routing>> {
        self.repo.query(db, &query, &page).await
    }

    async fn get_detail(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<RoutingDetail> {
        let routing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Routing"))?;

        let steps = self.repo.find_steps(db, id).await?;

        Ok(RoutingDetail { routing, steps })
    }

    async fn create(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateRoutingReq,
    ) -> Result<i64> {
        let id = self.repo.create(db, &req, ctx.operator_id).await?;

        self.repo.insert_steps(db, id, &req.steps).await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "Routing", entity_id: id, action: AuditAction::Create, changes: None, context: None })
            .await?;

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::RoutingCreated,
                    aggregate_type: "Routing".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({ "name": req.name, "step_count": req.steps.len() }),
                    idempotency_key: None,
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
        req: UpdateRoutingReq,
    ) -> Result<()> {
        let _existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Routing"))?;

        let has_changes = req.name.is_some() || req.description.is_some() || req.steps.is_some();
        if !has_changes {
            return Ok(());
        }

        self.repo.update(db, id, &req, ctx.operator_id).await?;

        if let Some(ref steps) = req.steps {
            self.repo.delete_steps(db, id).await?;
            self.repo.insert_steps(db, id, steps).await?;
        }

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "Routing", entity_id: id, action: AuditAction::Update, changes: None, context: None })
            .await?;

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::RoutingUpdated,
                    aggregate_type: "Routing".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({}),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let _existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Routing"))?;

        let bom_bindings = self.repo.list_boms_by_routing(db, id).await?;
        if !bom_bindings.is_empty() {
            return Err(DomainError::business_rule(
                "该工艺路线已被产品绑定，无法删除",
            ));
        }

        self.repo.delete_steps(db, id).await?;

        self.repo.delete(db, id).await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "Routing", entity_id: id, action: AuditAction::Delete, changes: None, context: None })
            .await?;

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::RoutingDeleted,
                    aggregate_type: "Routing".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({}),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn find_matching_routing(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        process_codes: Vec<String>,
    ) -> Result<Option<RoutingDetail>> {
        let routing_id = self
            .repo
            .find_matching_by_process_codes(db, &process_codes)
            .await?;

        match routing_id {
            Some(id) => {
                let routing = self
                    .repo
                    .find_by_id(db, id)
                    .await?
                    .ok_or_else(|| DomainError::not_found("Routing"))?;

                let steps = self.repo.find_steps(db, id).await?;

                Ok(Some(RoutingDetail { routing, steps }))
            }
            None => Ok(None),
        }
    }

    async fn set_bom_routing(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
        routing_id: i64,
    ) -> Result<()> {
        let _routing = self
            .repo
            .find_by_id(db, routing_id)
            .await?
            .ok_or_else(|| DomainError::not_found("Routing"))?;

        self.repo
            .set_bom_routing(db, &product_code, routing_id, ctx.operator_id)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx,
                    db,
                    RecordAuditLogReq {
                        entity_type: "BomRouting",
                        entity_id: routing_id,
                        action: AuditAction::Update,
                        changes: None,
                        context: None,
                    },
                )
            .await?;

        new_domain_event_bus(self.pool.clone())
            .publish(ctx, db, EventPublishRequest {
                event_type: DomainEventType::BomRoutingChanged,
                aggregate_type: "BomRouting".to_string(),
                aggregate_id: routing_id,
                payload: serde_json::json!({ "product_code": product_code, "routing_id": routing_id }),
                idempotency_key: None,
            }).await?;

        Ok(())
    }

    async fn get_bom_routing(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
    ) -> Result<Option<RoutingDetail>> {
        let bom_routing = self.repo.get_bom_routing(db, &product_code).await?;

        match bom_routing {
            Some(br) => {
                let routing = self.repo.find_by_id(db, br.routing_id).await?;

                match routing {
                    Some(r) => {
                        let steps = self.repo.find_steps(db, br.routing_id).await?;
                        Ok(Some(RoutingDetail { routing: r, steps }))
                    }
                    None => {
                        self.repo.delete_bom_routing(db, &product_code).await?;
                        Ok(None)
                    }
                }
            }
            None => Ok(None),
        }
    }

    async fn list_boms_by_routing(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        routing_id: i64,
    ) -> Result<Vec<BomRouting>> {
        self.repo.list_boms_by_routing(db, routing_id).await
    }
}
