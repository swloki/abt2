use async_trait::async_trait;
use serde_json::json;
use sqlx::postgres::PgPool;

use super::model::{CreatePurchaseQuotationRequest, PurchaseQuotation, PurchaseQuotationItem, PurchaseQuotationQuery, QuotationComparison};
use super::repo::{PurchaseQuotationItemRepo, PurchaseQuotationRepo};
use super::service::PurchaseQuotationService;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::{new_domain_event_bus, service::DomainEventBus};
use crate::shared::idempotency::{new_idempotency_service, service::{key_to_i64, IdempotencyService}};
use crate::purchase::enums::PurchaseQuotationStatus;
use crate::shared::state_machine::{new_state_machine_service, service::StateMachineService};
use crate::shared::types::PgExecutor;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

const ENTITY_TYPE: &str = "PurchaseQuotation";
const ENTITY_DISPLAY: &str = "采购报价单";

pub struct PurchaseQuotationServiceImpl {
    pool: PgPool,
}

impl PurchaseQuotationServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PurchaseQuotationService for PurchaseQuotationServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreatePurchaseQuotationRequest,
        idempotency_key: Option<String>,
    ) -> Result<i64> {
        if let Some(ref key) = idempotency_key {
            let hash = key_to_i64(key);
            if !new_idempotency_service(self.pool.clone()).check_and_mark(ctx, db, hash, "PurchaseQuotation:create").await? {
                return Err(DomainError::duplicate("PurchaseQuotation"));
            }
        }
        // buyer_id 兜底：未指定负责采购员时，默认录入人（与历史 operator_id 语义延续）
        let mut req = req;
        if req.buyer_id.is_none() {
            req.buyer_id = Some(ctx.operator_id);
        }
        // 1. 生成单据编号
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::PurchaseQuotation)
            .await?;

        // 2. 插入主表
        let id = PurchaseQuotationRepo::insert(
            &mut *db,
            &req,
            &doc_number,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 3. 插入明细
        if !req.items.is_empty() {
            PurchaseQuotationItemRepo::insert_items(
                &mut *db,
                id,
                &req.items,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        // 4. Record initial state
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, id, "Draft", None)
            .await
            .ok();

        // 5. Audit log
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: ENTITY_TYPE, entity_id: id, action: AuditAction::Create, changes: None, context: None })
            .await?;
        Ok(id)
    }

    async fn get(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<PurchaseQuotation> {
        PurchaseQuotationRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_DISPLAY))
    }

    async fn activate(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        idempotency_key: Option<String>,
    ) -> Result<()> {
        if let Some(ref key) = idempotency_key {
            let hash = key_to_i64(key);
            if !new_idempotency_service(self.pool.clone()).check_and_mark(ctx, db, hash, "PurchaseQuotation:activate").await? {
                return Err(DomainError::duplicate("PurchaseQuotation"));
            }
        }
        // 1. 获取当前记录（用于乐观锁）
        let quotation = PurchaseQuotationRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_DISPLAY))?;

        // 2. 状态转换 Draft -> Active
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, id, "Active", None)
            .await?;

        // 3. 更新实体表状态
        let rows = PurchaseQuotationRepo::update_status(
            &mut *db,
            id,
            PurchaseQuotationStatus::Active,
            &quotation.updated_at,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // 4. 发布领域事件
        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx, db,
                EventPublishRequest {
                    event_type: DomainEventType::PurchaseQuotationActivated,
                    aggregate_type: ENTITY_TYPE.to_string(),
                    aggregate_id: id,
                    payload: json!({}),
                    idempotency_key: None,
                },
            )
            .await?;

        // 5. 审计日志
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: ENTITY_TYPE, entity_id: id, action: AuditAction::Transition, changes: None, context: None })
            .await?;

        Ok(())
    }

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        query: PurchaseQuotationQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<PurchaseQuotation>> {
        // 懒过期：列表查询前把已过有效期的 Active 报价置为 Expired（幂等 UPDATE）
        PurchaseQuotationRepo::expire_overdue(&mut *db).await?;
        let scope = (ctx.data_scope, ctx.operator_id, ctx.department_id);
        let (items, total) = PurchaseQuotationRepo::query(&mut *db, &query, &page, scope)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    async fn compare(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        product_id: i64,
    ) -> Result<Vec<QuotationComparison>> {
        PurchaseQuotationRepo::compare_by_product(&mut *db, product_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
    async fn list_items(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        quotation_id: i64,
    ) -> Result<Vec<PurchaseQuotationItem>> {
        PurchaseQuotationItemRepo::list_by_quotation_id(&mut *db, quotation_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
    async fn list_items_by_quotation_ids(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        quotation_ids: &[i64],
    ) -> Result<Vec<PurchaseQuotationItem>> {
        PurchaseQuotationItemRepo::list_by_quotation_ids(&mut *db, quotation_ids)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
    async fn cancel(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        idempotency_key: Option<String>,
    ) -> Result<()> {
        if let Some(ref key) = idempotency_key {
            let hash = key_to_i64(key);
            if !new_idempotency_service(self.pool.clone()).check_and_mark(ctx, db, hash, "PurchaseQuotation:cancel").await? {
                return Err(DomainError::duplicate("PurchaseQuotation"));
            }
        }
        // 1. Fetch current record (for optimistic lock)
        let quotation = PurchaseQuotationRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_DISPLAY))?;
        // 2. State transition Draft -> Cancelled
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, id, "Cancelled", None)
            .await?;
        // 3. Update entity status
        let rows = PurchaseQuotationRepo::update_status(
            &mut *db,
            id,
            PurchaseQuotationStatus::Cancelled,
            &quotation.updated_at,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }
        // 4. Audit log
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: ENTITY_TYPE, entity_id: id, action: AuditAction::Transition, changes: None, context: None })
            .await?;
        Ok(())
    }
    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let quotation = self.get(ctx, db, id).await?;
        if quotation.status == PurchaseQuotationStatus::Active {
            return Err(DomainError::business_rule("已生效的报价不能删除"));
        }
        PurchaseQuotationRepo::soft_delete(db, id).await?;
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: ENTITY_TYPE, entity_id: id, action: AuditAction::Delete, changes: None, context: None })
            .await?;
        Ok(())
    }
}
