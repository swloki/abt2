use sqlx::PgPool;

use super::model::*;
use super::repo::RoutingRepo;
use super::service::RoutingService;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
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
        let code = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::Routing)
            .await?;
        let id = self.repo.create(db, &code, &req, ctx.operator_id).await?;

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
            // 覆盖护栏：有 bom_routing_outputs 覆盖的 routing，禁止破坏性 step 删除/重排
            // （仅允许末尾 append + 改非身份属性如工时/工作中心/备注）。
            // step_order 是覆盖层关联键，delete+insert 全重建会令覆盖行错位
            // （把"焊接的产出/计件价"挂到"测试"工序上）。参照 production_batch has_report 锁定先例。
            let covered = self.repo.count_bom_outputs_by_routing(db, id).await?;
            if covered > 0 {
                let orig_steps = self.repo.find_steps(db, id).await?;
                if steps.len() < orig_steps.len() {
                    return Err(DomainError::business_rule(format!(
                        "该工艺路线已关联 {covered} 条 BOM 产出覆盖，不能删除已有工序（仅允许在末尾追加新工序）"
                    )));
                }
                for (i, orig) in orig_steps.iter().enumerate() {
                    let Some(new) = steps.get(i) else { break };
                    if new.process_code != orig.process_code {
                        return Err(DomainError::business_rule(format!(
                            "该工艺路线已关联 {covered} 条 BOM 产出覆盖，不能修改或重排已有工序「{}」（仅允许在末尾追加）",
                            orig.process_name.as_deref().unwrap_or(&orig.process_code)
                        )));
                    }
                }
            }
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

        // 唯一性：一个 BOM 只能关联一个 routing。已关联到其他 routing → 报错带名，不静默覆盖。
        if let Some(existing) = self.repo.get_bom_routing(db, &product_code).await? {
            if existing.routing_id != routing_id {
                let existing_name = self
                    .repo
                    .find_by_id(db, existing.routing_id)
                    .await?
                    .map(|r| r.name)
                    .unwrap_or_else(|| format!("#{}", existing.routing_id));
                return Err(DomainError::business_rule(format!(
                    "该 BOM（{product_code}）已关联到工艺路线「{existing_name}」，请先在那边取消关联后再绑定"
                )));
            }
            // 已关联到当前 routing → 幂等成功（不重复审计/事件）
            return Ok(());
        }

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
    async fn paginate_boms_by_routing(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        routing_id: i64,
        keyword: Option<String>,
        page: PageParams,
    ) -> Result<PaginatedResult<BomRouting>> {
        self.repo.paginate_boms_by_routing(db, routing_id, keyword.as_deref(), &page).await
    }

    async fn delete_bom_routing(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
    ) -> Result<()> {
        self.repo.delete_bom_routing(db, &product_code).await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "BomRouting",
                    entity_id: 0,
                    action: AuditAction::Delete,
                    changes: None,
                    context: None,
                },
            )
            .await?;

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::BomRoutingChanged,
                    aggregate_type: "BomRouting".to_string(),
                    aggregate_id: 0,
                    payload: serde_json::json!({ "product_code": product_code, "action": "unbind" }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(())
    }
}
