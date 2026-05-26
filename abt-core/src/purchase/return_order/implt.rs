use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::postgres::PgPool;

use super::model::{CreatePurchaseReturnRequest, PurchaseReturn};
use super::repo::{PurchaseReturnItemRepo, PurchaseReturnRepo};
use super::service::PurchaseReturnService;
use crate::purchase::enums::{PurchaseOrderStatus, PurchaseReturnStatus};
use crate::purchase::order::repo::PurchaseOrderRepo;
use crate::shared::idempotency::service::key_to_i64;
use crate::shared::audit_log::service::AuditLogService;
use crate::shared::document_link::model::LinkRequest;
use crate::shared::document_link::service::DocumentLinkService;
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::enums::link_type::LinkType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::idempotency::service::IdempotencyService;
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::Result;

const ENTITY_TYPE: &str = "PurchaseReturn";

pub struct PurchaseReturnServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
    doc_seq: Arc<dyn DocumentSequenceService>,
    state_machine: Arc<dyn StateMachineService>,
    event_bus: Arc<dyn DomainEventBus>,
    audit_log: Arc<dyn AuditLogService>,
    doc_link: Arc<dyn DocumentLinkService>,
    #[allow(dead_code)]
    idempotency: Arc<dyn IdempotencyService>,
}

impl PurchaseReturnServiceImpl {
    pub fn new(
        pool: Arc<PgPool>,
        doc_seq: Arc<dyn DocumentSequenceService>,
        state_machine: Arc<dyn StateMachineService>,
        event_bus: Arc<dyn DomainEventBus>,
        audit_log: Arc<dyn AuditLogService>,
        doc_link: Arc<dyn DocumentLinkService>,
        idempotency: Arc<dyn IdempotencyService>,
    ) -> Self {
        Self {
            pool,
            doc_seq,
            state_machine,
            event_bus,
            audit_log,
            doc_link,
            idempotency,
        }
    }
}

#[async_trait]
impl PurchaseReturnService for PurchaseReturnServiceImpl {
    async fn create(
        &self,
        mut ctx: ServiceContext<'_>,
        req: CreatePurchaseReturnRequest,
        idempotency_key: Option<String>,
    ) -> Result<i64> {
        if let Some(ref key) = idempotency_key {
            let hash = key_to_i64(key);
            if !self.idempotency.check_and_mark(ctx.reborrow(), hash, "PurchaseReturn:create").await? {
                return Err(DomainError::duplicate("PurchaseReturn"));
            }
        }
        // 1. 验证关联订单存在且状态允许退货
        let order = PurchaseOrderRepo::get_by_id(&mut *ctx.executor, req.order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("PurchaseOrder"))?;

        if !matches!(
            order.status,
            PurchaseOrderStatus::Confirmed
                | PurchaseOrderStatus::PartiallyReceived
                | PurchaseOrderStatus::Received
        ) {
            return Err(DomainError::validation(format!(
                "订单状态为 {:?}，不允许创建退货单",
                order.status
            )));
        }

        // 2. 计算退货总金额
        let total_amount: Decimal = req
            .items
            .iter()
            .map(|i| i.returned_qty * i.unit_price)
            .sum();

        // 3. 生成单据编号
        let doc_number = self
            .doc_seq
            .next_number(ctx.reborrow(), DocumentType::PurchaseReturn)
            .await?;

        // 4. 插入主表
        let id = PurchaseReturnRepo::insert(
            &mut *ctx.executor,
            &req,
            &doc_number,
            total_amount,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 5. 插入明细
        if !req.items.is_empty() {
            PurchaseReturnItemRepo::insert_items(&mut *ctx.executor, id, &req.items)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        // 6. 创建单据关联
        self.doc_link
            .create_links(
                ctx.reborrow(),
                vec![LinkRequest {
                    source_type: DocumentType::PurchaseReturn,
                    source_id: id,
                    target_type: DocumentType::PurchaseOrder,
                    target_id: req.order_id,
                    link_type: LinkType::References,
                }],
            )
            .await?;

        // 7. 审计日志
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

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<PurchaseReturn> {
        PurchaseReturnRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))
    }

    async fn confirm(&self, mut ctx: ServiceContext<'_>, id: i64, idempotency_key: Option<String>) -> Result<()> {
        if let Some(ref key) = idempotency_key {
            let hash = key_to_i64(key);
            if !self.idempotency.check_and_mark(ctx.reborrow(), hash, "PurchaseReturn:confirm").await? {
                return Err(DomainError::duplicate("PurchaseReturn"));
            }
        }
        // 1. 获取当前记录（用于乐观锁）
        let ret = PurchaseReturnRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))?;

        // 2. 状态转换 Draft -> Confirmed
        self.state_machine
            .transition(ctx.reborrow(), ENTITY_TYPE, id, "Confirmed", None)
            .await?;

        // 3. 更新实体表状态
        let rows = PurchaseReturnRepo::update_status(
            &mut *ctx.executor,
            id,
            PurchaseReturnStatus::Confirmed,
            &ret.updated_at,
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
                    event_type: DomainEventType::PurchaseReturnConfirmed,
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
