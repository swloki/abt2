use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::postgres::PgPool;

use super::model::{CreatePaymentRequestRequest, PaymentRequest};
use super::repo::PaymentRequestRepo;
use super::service::PaymentRequestService;
use crate::purchase::enums::PaymentStatus;
use crate::purchase::reconciliation::repo::PurchaseReconciliationRepo;
use crate::shared::idempotency::service::key_to_i64;
use crate::shared::types::PgExecutor;
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
use crate::shared::types::Result;

const ENTITY_TYPE: &str = "PaymentRequest";

/// 三单匹配容差率 ±0.5%
const TOLERANCE_RATE: Decimal = Decimal::from_parts(5, 0, 0, false, 3); // 0.005

fn within_tolerance(a: Decimal, b: Decimal) -> bool {
    if b == Decimal::ZERO {
        return a == Decimal::ZERO;
    }
    let diff = (a - b).abs();
    let threshold = b * TOLERANCE_RATE;
    diff <= threshold
}

pub struct PaymentRequestServiceImpl {
    #[allow(dead_code)]
    pool: PgPool,
    doc_seq: Arc<dyn DocumentSequenceService>,
    state_machine: Arc<dyn StateMachineService>,
    event_bus: Arc<dyn DomainEventBus>,
    audit_log: Arc<dyn AuditLogService>,
    #[allow(dead_code)]
    idempotency: Arc<dyn IdempotencyService>,
}

impl PaymentRequestServiceImpl {
    pub fn new(
        pool: PgPool,
        doc_seq: Arc<dyn DocumentSequenceService>,
        state_machine: Arc<dyn StateMachineService>,
        event_bus: Arc<dyn DomainEventBus>,
        audit_log: Arc<dyn AuditLogService>,
        idempotency: Arc<dyn IdempotencyService>,
    ) -> Self {
        Self {
            pool,
            doc_seq,
            state_machine,
            event_bus,
            audit_log,
            idempotency,
        }
    }
}

#[async_trait]
impl PaymentRequestService for PaymentRequestServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreatePaymentRequestRequest,
        idempotency_key: Option<String>,
    ) -> Result<i64> {
        if let Some(ref key) = idempotency_key {
            let hash = key_to_i64(key);
            if !self.idempotency.check_and_mark(ctx, db, hash, "PaymentRequest:create").await? {
                return Err(DomainError::duplicate("PaymentRequest"));
            }
        }
        // 1. 三单匹配校验
        // 1a. 若关联对账单，查对账单 confirmed_amount 作为收货侧金额
        if let Some(recon_id) = req.reconciliation_id {
            let recon = PurchaseReconciliationRepo::get_by_id(&mut *db, recon_id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .ok_or_else(|| DomainError::not_found("PurchaseReconciliation"))?;

            // 付款金额 vs 对账确认金额（收货侧）容差校验
            if !within_tolerance(req.amount, recon.confirmed_amount) {
                return Err(DomainError::validation(format!(
                    "付款金额 {} 与对账确认金额 {} 偏差超过容差 ±0.5%",
                    req.amount, recon.confirmed_amount
                )));
            }
        }

        // 1b. 发票金额 vs 付款金额容差校验
        if let Some(invoice_amount) = req.invoice_amount
            && !within_tolerance(req.amount, invoice_amount)
        {
            return Err(DomainError::validation(format!(
                "付款金额 {} 与发票金额 {} 偏差超过容差 ±0.5%",
                req.amount, invoice_amount
            )));
        }

        // 2. 生成单据编号
        let doc_number = self
            .doc_seq
            .next_number(ctx, db, DocumentType::PaymentRequest)
            .await?;

        // 3. 插入主表
        let id = PaymentRequestRepo::insert(
            &mut *db,
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
                db,
                ENTITY_TYPE,
                id,
                AuditAction::Create,
                None,
                None,
            )
            .await?;

        Ok(id)
    }

    async fn get(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<PaymentRequest> {
        PaymentRequestRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))
    }

    async fn approve(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, idempotency_key: Option<String>) -> Result<()> {
        if let Some(ref key) = idempotency_key {
            let hash = key_to_i64(key);
            if !self.idempotency.check_and_mark(ctx, db, hash, "PaymentRequest:approve").await? {
                return Err(DomainError::duplicate("PaymentRequest"));
            }
        }
        // 1. 获取当前记录（用于乐观锁）
        let payment = PaymentRequestRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))?;

        // 2. 状态转换 Draft -> Approved
        self.state_machine
            .transition(ctx, db, ENTITY_TYPE, id, "Approved", None)
            .await?;

        // 3. 更新实体表状态
        let rows = PaymentRequestRepo::update_status(
            &mut *db,
            id,
            PaymentStatus::Approved,
            &payment.updated_at,
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
            .record(ctx, db, ENTITY_TYPE, id, AuditAction::Transition, None, None)
            .await?;

        Ok(())
    }

    async fn mark_paid_by_fms(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        payment_doc_no: String,
        idempotency_key: Option<String>,
    ) -> Result<()> {
        if let Some(ref key) = idempotency_key {
            let hash = key_to_i64(key);
            if !self.idempotency.check_and_mark(ctx, db, hash, "PaymentRequest:mark_paid_by_fms").await? {
                return Err(DomainError::duplicate("PaymentRequest"));
            }
        }
        // 1. 获取当前记录（用于乐观锁）
        let payment = PaymentRequestRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))?;

        // 2. 状态转换 Approved -> Paid
        self.state_machine
            .transition(
                ctx, db,
                ENTITY_TYPE,
                id,
                "Paid",
                Some(&format!("FMS付款单号: {payment_doc_no}")),
            )
            .await?;

        // 3. 标记已付款（写入 FMS 付款单号）
        let rows = PaymentRequestRepo::mark_paid(
            &mut *db,
            id,
            &payment_doc_no,
            &payment.updated_at,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // 4. 审计日志
        self.audit_log
            .record(
                ctx,
                db,
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
