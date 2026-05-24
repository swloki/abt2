use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use sqlx::postgres::PgPool;

use super::model::{CreateMiscRequestRequest, MiscellaneousRequest};
use super::repo::{MiscRequestItemRepo, MiscRequestRepo};
use super::service::MiscellaneousRequestService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::idempotency::service::IdempotencyService;
use crate::purchase::enums::MiscRequestStatus;
use crate::shared::idempotency::service::key_to_i64;
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;

const ENTITY_TYPE: &str = "MiscellaneousRequest";

pub struct MiscellaneousRequestServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
    doc_seq: Arc<dyn DocumentSequenceService>,
    state_machine: Arc<dyn StateMachineService>,
    event_bus: Arc<dyn DomainEventBus>,
    audit_log: Arc<dyn AuditLogService>,
    #[allow(dead_code)]
    idempotency: Arc<dyn IdempotencyService>,
}

impl MiscellaneousRequestServiceImpl {
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
impl MiscellaneousRequestService for MiscellaneousRequestServiceImpl {
    async fn create(
        &self,
        mut ctx: ServiceContext<'_>,
        req: CreateMiscRequestRequest,
        idempotency_key: Option<String>,
    ) -> Result<i64, DomainError> {
        if let Some(ref key) = idempotency_key {
            let hash = key_to_i64(key);
            if !self.idempotency.check_and_mark(ctx.reborrow(), hash, "MiscellaneousRequest:create").await? {
                return Err(DomainError::duplicate("MiscellaneousRequest"));
            }
        }
        // 1. 生成单据编号
        let doc_number = self.doc_seq
            .next_number(ctx.reborrow(), DocumentType::MiscellaneousRequest)
            .await?;

        // 2. 计算明细估算总金额
        let total_amount = req.items.iter().fold(rust_decimal::Decimal::ZERO, |acc, item| {
            acc + item.quantity * item.estimated_price.unwrap_or(rust_decimal::Decimal::ZERO)
        });

        // 3. 插入主表
        let id = MiscRequestRepo::insert(
            &mut *ctx.executor,
            &req,
            &doc_number,
            total_amount,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 4. 插入明细
        if !req.items.is_empty() {
            MiscRequestItemRepo::insert_items(
                &mut *ctx.executor,
                id,
                &req.items,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }

        // 5. 审计日志
        self.audit_log
            .record(ctx.reborrow(), ENTITY_TYPE, id, AuditAction::Create, None, None)
            .await?;

        Ok(id)
    }

    async fn get(
        &self,
        ctx: ServiceContext<'_>,
        id: i64,
    ) -> Result<MiscellaneousRequest, DomainError> {
        MiscRequestRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))
    }

    async fn approve(
        &self,
        mut ctx: ServiceContext<'_>,
        id: i64,
        idempotency_key: Option<String>,
    ) -> Result<(), DomainError> {
        if let Some(ref key) = idempotency_key {
            let hash = key_to_i64(key);
            if !self.idempotency.check_and_mark(ctx.reborrow(), hash, "MiscellaneousRequest:approve").await? {
                return Err(DomainError::duplicate("MiscellaneousRequest"));
            }
        }
        // 1. 获取当前记录（用于乐观锁）
        let request = MiscRequestRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))?;

        // 2. 状态转换 Draft -> Approved
        self.state_machine
            .transition(ctx.reborrow(), ENTITY_TYPE, id, "Approved", None)
            .await?;

        // 3. 更新实体表状态
        let rows = MiscRequestRepo::update_status(
            &mut *ctx.executor,
            id,
            MiscRequestStatus::Approved,
            &request.updated_at,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // 4. 发布领域事件
        self.event_bus
            .publish(
                ctx.reborrow(),
                EventPublishRequest {
                    event_type: DomainEventType::MiscellaneousRequestApproved,
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
}
