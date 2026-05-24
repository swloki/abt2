use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use sqlx::postgres::PgPool;

use super::model::{CreatePaymentRequestRequest, PaymentRequest};
use super::repo::PaymentRequestRepo;
use super::service::PaymentRequestService;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;

const ENTITY_TYPE: &str = "PaymentRequest";

pub struct PaymentRequestServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
    doc_seq: Arc<dyn DocumentSequenceService>,
    state_machine: Arc<dyn StateMachineService>,
    event_bus: Arc<dyn DomainEventBus>,
    audit_log: Arc<dyn AuditLogService>,
}

impl PaymentRequestServiceImpl {
    pub fn new(
        pool: Arc<PgPool>,
        doc_seq: Arc<dyn DocumentSequenceService>,
        state_machine: Arc<dyn StateMachineService>,
        event_bus: Arc<dyn DomainEventBus>,
        audit_log: Arc<dyn AuditLogService>,
    ) -> Self {
        Self {
            pool,
            doc_seq,
            state_machine,
            event_bus,
            audit_log,
        }
    }
}

#[async_trait]
impl PaymentRequestService for PaymentRequestServiceImpl {
    async fn create(
        &self,
        mut ctx: ServiceContext<'_>,
        req: CreatePaymentRequestRequest,
    ) -> Result<i64, DomainError> {
        // 1. 三单匹配校验：发票金额 vs 申请金额（简化版）
        if let Some(invoice_amount) = req.invoice_amount
            && invoice_amount < req.amount
        {
            return Err(DomainError::validation(
                "付款申请金额不能超过发票金额",
            ));
        }

        // 2. 生成单据编号
        let doc_number = self
            .doc_seq
            .next_number(ctx.reborrow(), DocumentType::PaymentRequest)
            .await?;

        // 3. 插入主表
        let id = PaymentRequestRepo::insert(
            &mut *ctx.executor,
            &req,
            &doc_number,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 4. 审计日志
        self.audit_log
            .record(
                ctx,
                ENTITY_TYPE,
                id,
                AuditAction::Create,
                None,
                None,
            )
            .await?;

        Ok(id)
    }

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<PaymentRequest, DomainError> {
        PaymentRequestRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))
    }

    async fn approve(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        // 1. 状态转换 Draft -> Approved
        self.state_machine
            .transition(ctx.reborrow(), ENTITY_TYPE, id, "Approved", None)
            .await?;

        // 2. 发布领域事件
        self.event_bus
            .publish(
                ctx.reborrow(),
                EventPublishRequest {
                    event_type: DomainEventType::PaymentRequestApproved,
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

    async fn mark_paid_by_fms(
        &self,
        mut ctx: ServiceContext<'_>,
        id: i64,
        payment_doc_no: String,
    ) -> Result<(), DomainError> {
        // 1. 获取当前记录（用于乐观锁）
        let payment = PaymentRequestRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))?;

        // 2. 状态转换 Approved -> Paid
        self.state_machine
            .transition(
                ctx.reborrow(),
                ENTITY_TYPE,
                id,
                "Paid",
                Some(&format!("FMS付款单号: {payment_doc_no}")),
            )
            .await?;

        // 3. 标记已付款（写入 FMS 付款单号）
        PaymentRequestRepo::mark_paid(
            &mut *ctx.executor,
            id,
            &payment_doc_no,
            &payment.updated_at,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 4. 审计日志
        self.audit_log
            .record(
                ctx,
                ENTITY_TYPE,
                id,
                AuditAction::Transition,
                Some(json!({ "payment_doc_no": payment_doc_no })),
                None,
            )
            .await?;

        Ok(())
    }
}
