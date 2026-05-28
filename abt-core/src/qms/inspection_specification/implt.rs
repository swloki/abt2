use async_trait::async_trait;
use sqlx::postgres::PgPool;

use super::model::*;
use super::repo;
use super::service::InspectionSpecificationService;
use crate::qms::enums::*;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::types::PgExecutor;
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::state_machine::{new_state_machine_service, service::StateMachineService};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

const ENTITY_TYPE: &str = "InspectionSpecification";

pub struct InspectionSpecificationServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
}

impl InspectionSpecificationServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl InspectionSpecificationService for InspectionSpecificationServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateInspectionSpecificationReq,
    ) -> Result<i64> {
        // 1. 检查是否已存在该产品+检验类型的活跃规格
        let existing = repo::find_active_by_product_and_type(
            &mut *db,
            req.product_id,
            req.inspection_type.as_i16(),
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if existing.is_some() {
            return Err(DomainError::validation(format!(
                "产品 {} 已存在 {:?} 类型的活跃检验规格",
                req.product_id, req.inspection_type
            )));
        }

        // 2. 生成单据编号
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::InspectionSpecification)
            .await?;

        // 3. 构建实体并插入
        let now = chrono::Utc::now();
        let spec = InspectionSpecification {
            id: 0,
            doc_number,
            product_id: req.product_id,
            inspection_type: req.inspection_type,
            check_items: req.check_items,
            sample_plan: req.sample_plan,
            status: SpecStatus::Draft,
            version: 1,
            operator_id: ctx.operator_id,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };

        let id = repo::insert(&mut *db, &spec)
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
    ) -> Result<InspectionSpecification> {
        repo::find_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))
    }

    async fn find_by_product_and_type(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        product_id: i64,
        inspection_type: InspectionType,
    ) -> Result<Option<InspectionSpecification>> {
        repo::find_active_by_product_and_type(
            &mut *db,
            product_id,
            inspection_type.as_i16(),
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn update(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateInspectionSpecificationReq,
    ) -> Result<()> {
        // 1. 获取现有记录
        let existing = repo::find_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))?;

        // 2. 只有 Draft 状态才能修改
        if existing.status != SpecStatus::Draft {
            return Err(DomainError::validation(format!(
                "检验规格状态为 {:?}，只有 Draft 状态才能修改",
                existing.status
            )));
        }

        // 3. 如果包含状态变更，先走状态机校验
        if let Some(new_status) = req.status
            && new_status != existing.status {
                new_state_machine_service(self.pool.clone())
                    .transition(ctx, db, ENTITY_TYPE, id, &new_status.to_string(), None)
                    .await?;
            }

        // 4. 乐观锁更新
        let rows = repo::update_fields(
            &mut *db,
            id,
            req.check_items,
            req.sample_plan,
            req.status,
            req.expected_version,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // 5. 审计日志
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: ENTITY_TYPE, entity_id: id, action: AuditAction::Update, changes: None, context: None })
            .await?;

        Ok(())
    }

    async fn list(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: InspectionSpecFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<InspectionSpecification>> {
        repo::list(&mut *db, &filter, &page)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
}
