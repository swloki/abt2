use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use sqlx::postgres::PgPool;

use super::model::{CreatePurchaseQuotationRequest, PurchaseQuotation, PurchaseQuotationQuery, QuotationComparison};
use super::repo::{PurchaseQuotationItemRepo, PurchaseQuotationRepo};
use super::service::PurchaseQuotationService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::idempotency::service::IdempotencyService;
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

const ENTITY_TYPE: &str = "PurchaseQuotation";

pub struct PurchaseQuotationServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
    doc_seq: Arc<dyn DocumentSequenceService>,
    state_machine: Arc<dyn StateMachineService>,
    event_bus: Arc<dyn DomainEventBus>,
    audit_log: Arc<dyn AuditLogService>,
    #[allow(dead_code)]
    idempotency: Arc<dyn IdempotencyService>,
}

impl PurchaseQuotationServiceImpl {
    pub fn new(
        pool: Arc<PgPool>,
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
        mut ctx: ServiceContext<'_>,
        req: CreatePurchaseQuotationRequest,
        idempotency_key: Option<String>,
    ) -> Result<i64, DomainError> {
        let _ = idempotency_key;
        // 1. 生成单据编号
        let doc_number = self.doc_seq
            .next_number(ctx.reborrow(), DocumentType::PurchaseQuotation)
            .await?;

        // 2. 插入主表
        let id = PurchaseQuotationRepo::insert(
            &mut *ctx.executor,
            &req,
            &doc_number,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 3. 插入明细
        if !req.items.is_empty() {
            PurchaseQuotationItemRepo::insert_items(
                &mut *ctx.executor,
                id,
                &req.items,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }

        // 4. 审计日志
        self.audit_log
            .record(ctx.reborrow(), ENTITY_TYPE, id, AuditAction::Create, None, None)
            .await?;

        Ok(id)
    }

    async fn get(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<PurchaseQuotation, DomainError> {
        PurchaseQuotationRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))
    }

    async fn activate(
        &self,
        mut ctx: ServiceContext<'_>,
        id: i64,
        idempotency_key: Option<String>,
    ) -> Result<(), DomainError> {
        let _ = idempotency_key;
        // 1. 状态转换 Draft -> Active
        self.state_machine
            .transition(ctx.reborrow(), ENTITY_TYPE, id, "Active", None)
            .await?;

        // 2. 发布领域事件
        self.event_bus
            .publish(
                ctx.reborrow(),
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
            .record(ctx, ENTITY_TYPE, id, AuditAction::Transition, None, None)
            .await?;

        Ok(())
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        query: PurchaseQuotationQuery,
    ) -> Result<PaginatedResult<PurchaseQuotation>, DomainError> {
        let params = PageParams::new(1, 20);
        let scope = (ctx.data_scope, ctx.operator_id, ctx.department_id);
        let (items, total) = PurchaseQuotationRepo::query(&mut *ctx.executor, &query, &params, scope)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(PaginatedResult::new(items, total, params.page, params.page_size))
    }

    async fn compare(
        &self,
        ctx: ServiceContext<'_>,
        product_id: i64,
    ) -> Result<Vec<QuotationComparison>, DomainError> {
        PurchaseQuotationRepo::compare_by_product(&mut *ctx.executor, product_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }
}
