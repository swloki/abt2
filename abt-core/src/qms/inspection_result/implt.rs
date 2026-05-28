use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use sqlx::postgres::PgPool;

use super::model::*;
use super::repo;
use super::service::InspectionResultService;
use crate::qms::enums::*;
use crate::qms::inspection_specification;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::types::PgExecutor;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::idempotency::service::{key_to_i64, IdempotencyService};
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

const ENTITY_TYPE: &str = "InspectionResult";

pub struct InspectionResultServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
    doc_seq: Arc<dyn DocumentSequenceService>,
    state_machine: Arc<dyn StateMachineService>,
    event_bus: Arc<dyn DomainEventBus>,
    audit_log: Arc<dyn AuditLogService>,
    idempotency: Arc<dyn IdempotencyService>,
    spec_service: Arc<dyn inspection_specification::service::InspectionSpecificationService>,
}

impl InspectionResultServiceImpl {
    pub fn new(
        pool: Arc<PgPool>,
        doc_seq: Arc<dyn DocumentSequenceService>,
        state_machine: Arc<dyn StateMachineService>,
        event_bus: Arc<dyn DomainEventBus>,
        audit_log: Arc<dyn AuditLogService>,
        idempotency: Arc<dyn IdempotencyService>,
        spec_service: Arc<dyn inspection_specification::service::InspectionSpecificationService>,
    ) -> Self {
        Self { pool, doc_seq, state_machine, event_bus, audit_log, idempotency, spec_service }
    }
}

#[async_trait]
impl InspectionResultService for InspectionResultServiceImpl {
    /// 创建检验结果 — 仅录入来源信息和样本数量，状态为 Pending
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateInspectionResultReq,
    ) -> Result<i64> {
        // 1. 验证检验规格存在
        let spec = self.spec_service.get(ctx, db, req.spec_id).await?;
        if spec.status != SpecStatus::Active {
            return Err(DomainError::validation(format!(
                "检验规格 {} 状态不是 Active（当前: {:?}）",
                spec.doc_number, spec.status
            )));
        }

        // 2. 幂等检查
        let idem_key = format!(
            "qms:record:{}:{}:{}",
            req.source_type.as_i16(),
            req.source_id,
            spec.inspection_type.as_i16()
        );
        let hash = key_to_i64(&idem_key);
        if !self.idempotency.check_and_mark(ctx, db, hash, "InspectionResult:create").await? {
            return Err(DomainError::duplicate(ENTITY_TYPE));
        }

        // 3. 生成单据编号
        let doc_number = self
            .doc_seq
            .next_number(ctx, db, DocumentType::InspectionResult)
            .await?;

        // 4. 构建实体 — 仅来源信息和样本数量，结果字段留空/默认
        let now = chrono::Utc::now();
        let result = InspectionResult {
            id: 0,
            doc_number,
            spec_id: req.spec_id,
            source_type: req.source_type,
            source_id: req.source_id,
            inspection_type: spec.inspection_type,
            batch_no: req.batch_no,
            sample_qty: req.sample_qty,
            qualified_qty: rust_decimal::Decimal::ZERO,
            unqualified_qty: rust_decimal::Decimal::ZERO,
            result: InspectionResultType::Pass,
            check_results: vec![],
            inspector_id: 0,
            inspection_date: None,
            status: InspectionStatus::Pending,
            operator_id: ctx.operator_id,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };

        let id = repo::insert(&mut *db, &result)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 5. 审计日志
        self.audit_log
            .record(ctx, db, ENTITY_TYPE, id, AuditAction::Create, None, None)
            .await?;

        Ok(id)
    }

    async fn get(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<InspectionResult> {
        repo::find_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))
    }

    /// 记录检验结果 — Guard: qualified_qty + unqualified_qty == sample_qty
    async fn record_result(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: RecordInspectionResultReq,
    ) -> Result<QualityGateStatus> {
        // 1. 获取现有记录
        let existing = repo::find_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))?;

        if existing.status != InspectionStatus::Pending {
            return Err(DomainError::validation(format!(
                "检验结果状态为 {:?}，只有 Pending 才能记录结果",
                existing.status
            )));
        }

        // 2. 幂等检查
        let idem_key = format!("qms:record_result:{}:{}", id, req.inspector_id);
        let hash = key_to_i64(&idem_key);
        if !self.idempotency.check_and_mark(ctx, db, hash, "InspectionResult:record_result").await? {
            return Err(DomainError::duplicate(ENTITY_TYPE));
        }

        // 3. Guard condition: non-negative + sum == sample
        if req.qualified_qty < rust_decimal::Decimal::ZERO
            || req.unqualified_qty < rust_decimal::Decimal::ZERO
        {
            return Err(DomainError::validation(
                "合格数量和不合格数量不能为负数".to_string(),
            ));
        }
        if req.qualified_qty + req.unqualified_qty != existing.sample_qty {
            return Err(DomainError::validation(format!(
                "合格数量({}) + 不合格数量({}) != 样本数量({})",
                req.qualified_qty, req.unqualified_qty, existing.sample_qty
            )));
        }

        // 4. 更新检验数据并推进状态为 Completed
        let rows = repo::record_result(
            &mut *db,
            id,
            req.result.as_i16(),
            req.qualified_qty,
            req.unqualified_qty,
            req.check_results,
            req.inspector_id,
            req.inspection_date,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // 5. 状态机转换
        self.state_machine
            .transition(ctx, db, ENTITY_TYPE, id, "Completed", None)
            .await?;

        // 6. 发布领域事件
        let event_type = match req.result {
            InspectionResultType::Pass | InspectionResultType::Conditional => DomainEventType::InspectionPassed,
            InspectionResultType::Fail => DomainEventType::InspectionFailed,
        };
        self.event_bus
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type,
                    aggregate_type: ENTITY_TYPE.to_string(),
                    aggregate_id: id,
                    payload: json!({
                        "doc_number": existing.doc_number,
                        "inspection_type": existing.inspection_type.as_i16(),
                        "source_type": existing.source_type.as_i16(),
                        "source_id": existing.source_id,
                        "result": req.result.as_i16(),
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        // 7. 审计日志
        self.audit_log
            .record(ctx, db, ENTITY_TYPE, id, AuditAction::Transition, None, None)
            .await?;

        // 8. 返回 QualityGateStatus
        let gate_status = match req.result {
            InspectionResultType::Pass | InspectionResultType::Conditional => QualityGateStatus::Passed,
            InspectionResultType::Fail => QualityGateStatus::Failed,
        };
        Ok(gate_status)
    }

    async fn list_by_source(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: InspectionResultFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<InspectionResult>> {
        repo::list(&mut *db, &filter, &page)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
}
