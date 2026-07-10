use sqlx::PgPool;

use super::model::*;
use super::repo::BomOperationRepo;
use super::service::BomOperationService;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::enums::audit::AuditAction;
use crate::shared::types::{DomainError, PgExecutor, Result, ServiceContext};

pub struct BomOperationServiceImpl {
    repo: BomOperationRepo,
    pool: PgPool,
}

impl BomOperationServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: BomOperationRepo,
            pool,
        }
    }
}

#[async_trait::async_trait]
impl BomOperationService for BomOperationServiceImpl {
    async fn list_operations(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
    ) -> Result<Vec<BomOperation>> {
        self.repo.list_by_product(db, &product_code).await
    }

    async fn find_operation(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
        step_order: i32,
    ) -> Result<Option<BomOperation>> {
        self.repo.find(db, &product_code, step_order).await
    }

    async fn upsert_operation(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: UpsertBomOperationReq,
    ) -> Result<()> {
        let id = self.repo.upsert(db, &req, ctx.operator_id).await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "BomOperation",
                    entity_id: id,
                    action: AuditAction::Update,
                    changes: Some(serde_json::json!({
                        "product_code": req.product_code,
                        "step_order": req.step_order,
                        "process_code": req.process_code,
                        "output_product_id": req.output_product_id,
                        "work_center_id": req.work_center_id,
                    })),
                    context: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn delete_operation(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
        step_order: i32,
    ) -> Result<()> {
        let affected = self.repo.delete(db, &product_code, step_order).await?;
        if affected == 0 {
            return Err(DomainError::not_found("BomOperation"));
        }
        // R-5：删工序同时清对应 step_order 的计件价，防 step_order 复用错配
        sqlx::query("DELETE FROM bom_step_prices WHERE product_code = $1 AND step_order = $2")
            .bind(&product_code)
            .bind(step_order)
            .execute(&mut *db)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "BomOperation",
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

    async fn replace_operations(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
        ops: Vec<UpsertBomOperationReq>,
    ) -> Result<()> {
        let count = ops.len();
        self.repo
            .replace_all(db, &product_code, &ops, ctx.operator_id)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "BomOperation",
                    entity_id: 0,
                    action: AuditAction::Update,
                    changes: Some(serde_json::json!({
                        "product_code": product_code,
                        "replaced_count": count,
                    })),
                    context: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn apply_routing_to_bom(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
        routing_id: i64,
        force: bool,
    ) -> Result<usize> {
        let count = self.repo.count_by_product(db, &product_code).await?;
        if count > 0 && !force {
            return Err(DomainError::business_rule(
                "该 BOM 已有工序行，拒绝覆盖；如需重新从模板拷贝，请先清空现有工序（force 重拷）",
            ));
        }
        if force {
            // R-5：级联清 bom_step_prices + delete bom_operations
            sqlx::query("DELETE FROM bom_step_prices WHERE product_code = $1")
                .bind(&product_code)
                .execute(&mut *db)
                .await?;
            sqlx::query("DELETE FROM bom_operations WHERE product_code = $1")
                .bind(&product_code)
                .execute(&mut *db)
                .await?;
        }
        let n = self
            .repo
            .copy_from_routing(db, &product_code, routing_id, ctx.operator_id)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "BomOperation",
                    entity_id: 0,
                    action: AuditAction::Create,
                    changes: Some(serde_json::json!({
                        "product_code": product_code,
                        "routing_id": routing_id,
                        "force": force,
                        "copied": n,
                    })),
                    context: None,
                },
            )
            .await?;

        Ok(n as usize)
    }

    async fn resync_process_names(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<usize> {
        let n = self.repo.resync_process_names(db).await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "BomOperation",
                    entity_id: 0,
                    action: AuditAction::Update,
                    changes: Some(serde_json::json!({
                        "resync_process_names": n,
                    })),
                    context: None,
                },
            )
            .await?;

        Ok(n as usize)
    }

    async fn count_operations(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: String,
    ) -> Result<i64> {
        self.repo.count_by_product(db, &product_code).await
    }
}
