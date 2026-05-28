use async_trait::async_trait;
use serde_json::json;
use sqlx::postgres::PgPool;

use super::model::*;
use super::repo;
use super::service::MrbService;
use crate::qms::enums::*;
use crate::qms::inspection_result;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::types::PgExecutor;
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::{new_domain_event_bus, model::EventPublishRequest, service::DomainEventBus};
use crate::shared::state_machine::{new_state_machine_service, service::StateMachineService};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

const ENTITY_TYPE: &str = "MRB";

pub struct MrbServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
}

impl MrbServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MrbService for MrbServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateMrbReq,
    ) -> Result<i64> {
        // 1. 验证检验结果存在、已完成、且为 Fail 或 Conditional
        let ir = inspection_result::repo::find_by_id(&mut *db, req.inspection_result_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("InspectionResult"))?;

        if ir.status != InspectionStatus::Completed {
            return Err(DomainError::validation(
                "检验结果尚未完成，无法创建 MRB".to_string(),
            ));
        }

        if ir.result == InspectionResultType::Pass {
            return Err(DomainError::validation(
                "检验结果为 Pass，无需创建 MRB".to_string(),
            ));
        }

        // 2. 生成单据编号
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::Mrb)
            .await?;

        // 3. 构建实体并插入
        let now = chrono::Utc::now();
        let mrb = Mrb {
            id: 0,
            doc_number,
            inspection_result_id: req.inspection_result_id,
            product_id: req.product_id,
            defect_description: req.defect_description,
            disposition: req.disposition,
            responsible_party: req.responsible_party,
            cost_impact: req.cost_impact,
            status: MRBStatus::Draft,
            remark: req.remark,
            operator_id: ctx.operator_id,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };

        let id = repo::insert(&mut *db, &mrb)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 4. 审计日志
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: ENTITY_TYPE, entity_id: id, action: AuditAction::Create, changes: None, context: None })
            .await?;

        Ok(id)
    }

    async fn get(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<Mrb> {
        repo::find_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))
    }

    async fn submit_for_review(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, id, "UnderReview", None)
            .await?;

        let rows = repo::update_status(
            &mut *db,
            id,
            MRBStatus::UnderReview.as_i16(),
            MRBStatus::Draft.as_i16(),
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: ENTITY_TYPE, entity_id: id, action: AuditAction::Transition, changes: None, context: None })
            .await?;

        Ok(())
    }

    /// MRB approve: UnderReview -> Approved
    async fn approve(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, id, "Approved", None)
            .await?;

        let rows = repo::update_status(
            &mut *db,
            id,
            MRBStatus::Approved.as_i16(),
            MRBStatus::UnderReview.as_i16(),
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: ENTITY_TYPE, entity_id: id, action: AuditAction::Transition, changes: None, context: None })
            .await?;

        Ok(())
    }

    /// 执行处置 — 由 WorkflowHook.on_approved 回调触发
    async fn execute_disposition(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        _req: ExecuteDispositionReq,
    ) -> Result<()> {
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, id, "Completed", None)
            .await?;

        let rows = repo::update_status(
            &mut *db,
            id,
            MRBStatus::Completed.as_i16(),
            MRBStatus::Approved.as_i16(),
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // 发布 MRBDispositioned 事件
        let mrb = repo::find_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))?;

        // 推进关联检验结果到 Dispositioned
        new_state_machine_service(self.pool.clone())
            .transition(
                ctx,
                db,
                "InspectionResult",
                mrb.inspection_result_id,
                "Dispositioned",
                None,
            )
            .await?;

        let ir_rows = inspection_result::repo::update_status(
            &mut *db,
            mrb.inspection_result_id,
            InspectionStatus::Dispositioned.as_i16(),
            InspectionStatus::Completed.as_i16(),
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if ir_rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::MRBDispositioned,
                    aggregate_type: ENTITY_TYPE.to_string(),
                    aggregate_id: id,
                    payload: json!({
                        "doc_number": mrb.doc_number,
                        "disposition": mrb.disposition.as_i16(),
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: ENTITY_TYPE, entity_id: id, action: AuditAction::Transition, changes: None, context: None })
            .await?;

        Ok(())
    }

    async fn list(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: MrbFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<Mrb>> {
        repo::list(&mut *db, &filter, &page)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
}
