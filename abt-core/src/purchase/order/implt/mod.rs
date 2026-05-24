use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::postgres::PgPool;

use super::model::{
    CreateOrderItemRequest, CreatePurchaseOrderRequest, PurchaseOrder, PurchaseOrderQuery,
};
use super::repo::{PurchaseOrderItemRepo, PurchaseOrderRepo};
use super::service::PurchaseOrderService;
use crate::purchase::enums::PurchaseQuotationStatus;
use crate::purchase::quotation::repo::{PurchaseQuotationItemRepo, PurchaseQuotationRepo};
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
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

const ENTITY_TYPE: &str = "PurchaseOrder";

pub struct PurchaseOrderServiceImpl {
    #[allow(dead_code)]
    pool: Arc<PgPool>,
    doc_seq: Arc<dyn DocumentSequenceService>,
    state_machine: Arc<dyn StateMachineService>,
    event_bus: Arc<dyn DomainEventBus>,
    audit_log: Arc<dyn AuditLogService>,
    doc_link: Arc<dyn DocumentLinkService>,
}

impl PurchaseOrderServiceImpl {
    pub fn new(
        pool: Arc<PgPool>,
        doc_seq: Arc<dyn DocumentSequenceService>,
        state_machine: Arc<dyn StateMachineService>,
        event_bus: Arc<dyn DomainEventBus>,
        audit_log: Arc<dyn AuditLogService>,
        doc_link: Arc<dyn DocumentLinkService>,
    ) -> Self {
        Self {
            pool,
            doc_seq,
            state_machine,
            event_bus,
            audit_log,
            doc_link,
        }
    }
}

#[async_trait]
impl PurchaseOrderService for PurchaseOrderServiceImpl {
    async fn create(
        &self,
        mut ctx: ServiceContext<'_>,
        req: CreatePurchaseOrderRequest,
    ) -> Result<i64, DomainError> {
        // 1. 生成单据编号
        let doc_number = self
            .doc_seq
            .next_number(ctx.reborrow(), DocumentType::PurchaseOrder)
            .await?;

        // 2. 计算总金额
        let total_amount: Decimal = req.items.iter().map(|i| i.quantity * i.unit_price).sum();

        // 3. 插入主表
        let id = PurchaseOrderRepo::insert(
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
            PurchaseOrderItemRepo::insert_items(&mut *ctx.executor, id, &req.items)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        // 5. 审计日志
        self.audit_log
            .record(
                ctx.reborrow(),
                ENTITY_TYPE,
                id,
                AuditAction::Create,
                None,
                None,
            )
            .await?;

        Ok(id)
    }

    async fn create_from_quotation(
        &self,
        mut ctx: ServiceContext<'_>,
        quotation_id: i64,
    ) -> Result<i64, DomainError> {
        // 1. 获取报价单并验证状态
        let quotation = PurchaseQuotationRepo::get_by_id(&mut *ctx.executor, quotation_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found("PurchaseQuotation"))?;

        if quotation.status != PurchaseQuotationStatus::Active {
            return Err(DomainError::validation(format!(
                "报价单状态不是 Active，无法创建采购订单（当前: {:?}）",
                quotation.status
            )));
        }

        // 2. 获取报价明细
        let quotation_items =
            PurchaseQuotationItemRepo::list_by_quotation_id(&mut *ctx.executor, quotation_id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;

        // 3. 复制明细到订单明细
        let order_items: Vec<CreateOrderItemRequest> = quotation_items
            .iter()
            .enumerate()
            .map(|(idx, qi)| CreateOrderItemRequest {
                product_id: qi.product_id,
                line_no: (idx as i32) + 1,
                description: String::new(),
                quantity: qi.min_order_qty.unwrap_or(Decimal::ONE),
                unit_price: qi.unit_price,
                quotation_item_id: Some(qi.id),
                expected_delivery_date: None,
            })
            .collect();

        // 4. 计算总金额
        let total_amount: Decimal = order_items
            .iter()
            .map(|i| i.quantity * i.unit_price)
            .sum();

        // 5. 生成单据编号
        let doc_number = self
            .doc_seq
            .next_number(ctx.reborrow(), DocumentType::PurchaseOrder)
            .await?;

        // 6. 构建创建请求
        let req = CreatePurchaseOrderRequest {
            supplier_id: quotation.supplier_id,
            order_date: chrono::Local::now().date_naive(),
            expected_delivery_date: None,
            payment_terms: None,
            delivery_address: None,
            remark: format!("从报价单 {} 自动生成", quotation.doc_number),
            items: order_items.clone(),
        };

        // 7. 插入主表
        let order_id = PurchaseOrderRepo::insert(
            &mut *ctx.executor,
            &req,
            &doc_number,
            total_amount,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 8. 插入明细
        if !order_items.is_empty() {
            PurchaseOrderItemRepo::insert_items(&mut *ctx.executor, order_id, &order_items)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        // 9. 创建单据关联
        self.doc_link
            .create_links(
                ctx.reborrow(),
                vec![LinkRequest {
                    source_type: DocumentType::PurchaseOrder,
                    source_id: order_id,
                    target_type: DocumentType::PurchaseQuotation,
                    target_id: quotation_id,
                    link_type: LinkType::DerivedFrom,
                }],
            )
            .await?;

        // 10. 审计日志
        self.audit_log
            .record(
                ctx,
                ENTITY_TYPE,
                order_id,
                AuditAction::Create,
                Some(json!({ "from_quotation_id": quotation_id })),
                None,
            )
            .await?;

        Ok(order_id)
    }

    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<PurchaseOrder, DomainError> {
        PurchaseOrderRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))
    }

    async fn confirm(&self, mut ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError> {
        // 1. 获取订单及明细，校验数量和单价
        let order = PurchaseOrderRepo::get_by_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))?;

        let items = PurchaseOrderItemRepo::list_by_order_id(&mut *ctx.executor, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        for item in &items {
            if item.quantity <= Decimal::ZERO {
                return Err(DomainError::validation(format!(
                    "订单明细第 {} 行数量必须大于 0",
                    item.line_no
                )));
            }
            if item.unit_price <= Decimal::ZERO {
                return Err(DomainError::validation(format!(
                    "订单明细第 {} 行单价必须大于 0",
                    item.line_no
                )));
            }
        }

        // 2. 状态转换 Draft -> Confirmed
        self.state_machine
            .transition(ctx.reborrow(), ENTITY_TYPE, id, "Confirmed", None)
            .await?;

        // 3. 发布领域事件
        self.event_bus
            .publish(
                ctx.reborrow(),
                EventPublishRequest {
                    event_type: DomainEventType::PurchaseOrderConfirmed,
                    aggregate_type: ENTITY_TYPE.to_string(),
                    aggregate_id: id,
                    payload: json!({ "doc_number": order.doc_number }),
                    idempotency_key: None,
                },
            )
            .await?;

        // 4. 审计日志
        self.audit_log
            .record(ctx, ENTITY_TYPE, id, AuditAction::Transition, None, None)
            .await?;

        Ok(())
    }

    async fn list(
        &self,
        ctx: ServiceContext<'_>,
        query: PurchaseOrderQuery,
    ) -> Result<PaginatedResult<PurchaseOrder>, DomainError> {
        let params = PageParams::new(1, 20);
        let (items, total) = PurchaseOrderRepo::query(&mut *ctx.executor, &query, &params)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(PaginatedResult::new(items, total, params.page, params.page_size))
    }
}
