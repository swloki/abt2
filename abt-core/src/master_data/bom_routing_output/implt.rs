use sqlx::PgPool;

use super::model::*;
use super::repo::BomRoutingOutputRepo;
use super::service::BomRoutingOutputService;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::enums::audit::AuditAction;
use crate::shared::types::{DomainError, PgExecutor, Result, ServiceContext};

pub struct BomRoutingOutputServiceImpl {
    repo: BomRoutingOutputRepo,
    pool: PgPool,
}

impl BomRoutingOutputServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { repo: BomRoutingOutputRepo, pool }
    }
}

#[async_trait::async_trait]
impl BomRoutingOutputService for BomRoutingOutputServiceImpl {
    async fn list_steps_with_output(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
    ) -> Result<Vec<StepWithOutput>> {
        self.repo.list_steps_with_output(db, &product_code).await
    }

    async fn upsert_output(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: UpsertBomOutputReq,
    ) -> Result<()> {
        let id = self.repo.upsert(db, &req, ctx.operator_id).await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "BomRoutingOutput",
                    entity_id: id,
                    action: AuditAction::Update,
                    changes: Some(serde_json::json!({
                        "product_code": req.product_code,
                        "routing_id": req.routing_id,
                        "step_order": req.step_order,
                        "output_product_id": req.output_product_id,
                        "unit_price": req.unit_price,
                        "work_center_id": req.work_center_id,
                    })),
                    context: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn delete_output(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
        step_order: i32,
    ) -> Result<()> {
        let affected = self.repo.delete(db, &product_code, step_order).await?;
        if affected == 0 {
            return Err(DomainError::not_found("BomRoutingOutput"));
        }

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "BomRoutingOutput",
                    entity_id: 0,
                    action: AuditAction::Delete,
                    changes: Some(serde_json::json!({
                        "product_code": product_code,
                        "step_order": step_order,
                    })),
                    context: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn find_outputs_by_product(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
    ) -> Result<Vec<BomRoutingOutput>> {
        self.repo.find_by_product(db, &product_code).await
    }
}
