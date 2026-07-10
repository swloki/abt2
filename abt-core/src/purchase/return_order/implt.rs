use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::postgres::PgPool;

use super::model::{CreatePurchaseReturnRequest, PurchaseReturn, PurchaseReturnItem, PurchaseReturnQuery};
use super::repo::{PurchaseReturnItemRepo, PurchaseReturnRepo};
use super::service::PurchaseReturnService;
use crate::purchase::enums::{PurchaseOrderStatus, PurchaseReturnStatus};
use crate::purchase::order::repo::{PurchaseOrderItemRepo, PurchaseOrderRepo};
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::document_link::{new_document_link_service, model::LinkRequest, service::DocumentLinkService};
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::enums::link_type::LinkType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::{new_domain_event_bus, service::DomainEventBus};
use crate::shared::idempotency::{new_idempotency_service, service::{key_to_i64, IdempotencyService}};
use crate::shared::state_machine::{new_state_machine_service, service::StateMachineService};
use crate::shared::types::PgExecutor;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::{PageParams, PaginatedResult};
use crate::shared::types::Result;

const ENTITY_TYPE: &str = "PurchaseReturn";
const ENTITY_DISPLAY: &str = "采购退货单";

pub struct PurchaseReturnServiceImpl {
    pool: PgPool,
}

impl PurchaseReturnServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PurchaseReturnService for PurchaseReturnServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreatePurchaseReturnRequest,
        idempotency_key: Option<String>,
    ) -> Result<i64> {
        if let Some(ref key) = idempotency_key {
            let hash = key_to_i64(key);
            if !new_idempotency_service(self.pool.clone()).check_and_mark(ctx, db, hash, "PurchaseReturn:create").await? {
                return Err(DomainError::duplicate("PurchaseReturn"));
            }
        }
        // 1. 验证关联订单存在且状态允许退货
        let order = PurchaseOrderRepo::get_by_id(&mut *db, req.order_id)
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

        // 1.5 校验退货数量不超过可退数量
        let po_items = PurchaseOrderItemRepo::list_by_order_id(&mut *db, req.order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        for item in &req.items {
            let po_item = po_items.iter().find(|p| p.id == item.order_item_id)
                .ok_or_else(|| DomainError::validation(format!(
                    "退货明细关联的订单行 {} 不存在", item.order_item_id
                )))?;

            let max_returnable = po_item.received_qty - po_item.returned_qty;
            if item.returned_qty > max_returnable {
                return Err(DomainError::validation(format!(
                    "退货数量 {} 超过可退数量 {}（已收 {} - 已退 {}）",
                    item.returned_qty, max_returnable, po_item.received_qty, po_item.returned_qty
                )));
            }
        }

        // 2. 计算退货总金额
        let total_amount: Decimal = req
            .items
            .iter()
            .map(|i| i.returned_qty * i.unit_price)
            .sum();

        // 3. 生成单据编号
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::PurchaseReturn)
            .await?;

        // 4. 插入主表
        let id = PurchaseReturnRepo::insert(
            &mut *db,
            &req,
            &doc_number,
            total_amount,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 5. 插入明细
        if !req.items.is_empty() {
            PurchaseReturnItemRepo::insert_items(&mut *db, id, &req.items)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        // 6. 创建单据关联
        new_document_link_service(self.pool.clone())
            .create_links(
                ctx, db,
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
        new_audit_log_service(self.pool.clone())
            .record(
                    ctx,
                    db,
                    RecordAuditLogReq {
                        entity_type: ENTITY_TYPE,
                        entity_id: id,
                        action: AuditAction::Create,
                        changes: None,
                        context: None,
                    },
                )
            .await?;

        // 8. 初始状态日志
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, id, "Draft", None)
            .await?;

        Ok(id)
    }

    async fn get(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<PurchaseReturn> {
        PurchaseReturnRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_DISPLAY))
    }

    async fn list(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: PurchaseReturnQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<PurchaseReturn>> {
        let (items, total) = PurchaseReturnRepo::query(&mut *db, &query, &page)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    async fn list_items(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, return_id: i64) -> Result<Vec<PurchaseReturnItem>> {
        PurchaseReturnItemRepo::list_by_return_id(&mut *db, return_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn list_items_by_return_ids(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        return_ids: &[i64],
    ) -> Result<Vec<PurchaseReturnItem>> {
        PurchaseReturnItemRepo::list_by_return_ids(&mut *db, return_ids)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, idempotency_key: Option<String>) -> Result<()> {
        if let Some(ref key) = idempotency_key {
            let hash = key_to_i64(key);
            if !new_idempotency_service(self.pool.clone()).check_and_mark(ctx, db, hash, "PurchaseReturn:confirm").await? {
                return Err(DomainError::duplicate("PurchaseReturn"));
            }
        }
        // 1. 获取当前记录（用于乐观锁）
        let ret = PurchaseReturnRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_DISPLAY))?;

        // 2. 状态转换 Draft -> Confirmed
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, id, "Confirmed", None)
            .await?;

        // 3. 更新实体表状态
        let rows = PurchaseReturnRepo::update_status(
            &mut *db,
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
        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx, db,
                EventPublishRequest {
                    event_type: DomainEventType::PurchaseReturnConfirmed,
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

    async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, idempotency_key: Option<String>) -> Result<()> {
        if let Some(ref key) = idempotency_key {
            let hash = key_to_i64(key);
            if !new_idempotency_service(self.pool.clone()).check_and_mark(ctx, db, hash, "PurchaseReturn:cancel").await? {
                return Err(DomainError::duplicate("PurchaseReturn"));
            }
        }
        // 1. 获取当前记录（用于乐观锁）
        let ret = PurchaseReturnRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_DISPLAY))?;

        // 2. 状态转换 Draft -> Cancelled
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, id, "Cancelled", None)
            .await?;

        // 3. 更新实体表状态
        let rows = PurchaseReturnRepo::update_status(
            &mut *db,
            id,
            PurchaseReturnStatus::Cancelled,
            &ret.updated_at,
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
                    event_type: DomainEventType::PurchaseReturnCancelled,
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
}
