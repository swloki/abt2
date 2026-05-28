use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use sqlx::postgres::PgPool;

use super::model::{CreatePurchaseQuotationRequest, PurchaseQuotation, PurchaseQuotationQuery, QuotationComparison};
use super::repo::{PurchaseQuotationItemRepo, PurchaseQuotationRepo};
use super::service::PurchaseQuotationService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::types::PgExecutor;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::idempotency::service::IdempotencyService;
use crate::purchase::enums::PurchaseQuotationStatus;
use crate::shared::idempotency::service::key_to_i64;
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

const ENTITY_TYPE: &str = "PurchaseQuotation";

pub struct PurchaseQuotationServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
    doc_seq: Arc<dyn DocumentSequenceService>,
    state_machine: Arc<dyn StateMachineService>,
    event_bus: Arc<dyn DomainEventBus>,
    audit_log: Arc<dyn AuditLogService>,
    #[allow(dead_code)]
    idempotency: Arc<dyn IdempotencyService>,
}

impl PurchaseQuotationServiceImpl {
    pub fn new(
        pool: PgPool,
        doc_seq: Arc<dyn DocumentSequenceService>,
        state_machine: Arc<dyn StateMachineService>,
        event_bus: Arc<dyn DomainEventBus>,
        audit_log: Arc<dyn AuditLogService>,
        idempotency: Arc<dyn IdempotencyService>,
    ) -> Self {
        Self { pool, doc_seq, state_machine, event_bus, audit_log, idempotency }
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
            if !self.idempotency.check_and_mark(ctx, db, hash, "PurchaseQuotation:create").await? {
                return Err(DomainError::duplicate("PurchaseQuotation"));
            }
        }
        // 1. 生成单据编号
        let doc_number = self.doc_seq
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

        // 4. 审计日志
        self.audit_log
            .record(ctx, db, ENTITY_TYPE, id, AuditAction::Create, None, None)
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
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))
    }

    async fn activate(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        idempotency_key: Option<String>,
    ) -> Result<()> {
        if let Some(ref key) = idempotency_key {
            let hash = key_to_i64(key);
            if !self.idempotency.check_and_mark(ctx, db, hash, "PurchaseQuotation:activate").await? {
                return Err(DomainError::duplicate("PurchaseQuotation"));
            }
        }
        // 1. 获取当前记录（用于乐观锁）
        let quotation = PurchaseQuotationRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))?;

        // 2. 状态转换 Draft -> Active
        self.state_machine
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
        self.event_bus
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

        // 3. 审计日志
        self.audit_log
            .record(ctx, db, ENTITY_TYPE, id, AuditAction::Transition, None, None)
            .await?;

        Ok(())
    }

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        query: PurchaseQuotationQuery,
    ) -> Result<PaginatedResult<PurchaseQuotation>> {
        let params = PageParams::new(1, 20);
        let scope = (ctx.data_scope, ctx.operator_id, ctx.department_id);
        let (items, total) = PurchaseQuotationRepo::query(&mut *db, &query, &params, scope)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(PaginatedResult::new(items, total, params.page, params.page_size))
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
}
