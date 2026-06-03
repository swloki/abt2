use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::postgres::PgPool;

use super::model::{
    CreateOrderItemRequest, CreatePurchaseOrderRequest, PurchaseOrder, PurchaseOrderItem, PurchaseOrderQuery,
};
use super::repo::{PurchaseOrderItemRepo, PurchaseOrderRepo};
use super::service::PurchaseOrderService;
use crate::master_data::supplier::service::SupplierService;
use crate::master_data::supplier::{new_supplier_service, model::SupplierStatus};
use crate::purchase::enums::{PurchaseOrderStatus, PurchaseQuotationStatus};
use crate::purchase::quotation::repo::{PurchaseQuotationItemRepo, PurchaseQuotationRepo};
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
use crate::shared::types::Result;
use crate::shared::types::pagination::{PageParams, PaginatedResult};

const ENTITY_TYPE: &str = "PurchaseOrder";

pub struct PurchaseOrderServiceImpl {
    pool: PgPool,
}

impl PurchaseOrderServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PurchaseOrderService for PurchaseOrderServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreatePurchaseOrderRequest,
        idempotency_key: Option<String>,
    ) -> Result<i64> {
        if let Some(ref key) = idempotency_key {
            let hash = key_to_i64(key);
            if !new_idempotency_service(self.pool.clone()).check_and_mark(ctx, db, hash, "PurchaseOrder:create").await? {
                return Err(DomainError::duplicate("PurchaseOrder"));
            }
        }
        // 1. 生成单据编号
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::PurchaseOrder)
            .await?;

        // 2. 计算总金额
        let total_amount: Decimal = req.items.iter().map(|i| i.quantity * i.unit_price).sum();

        // 3. 插入主表
        let id = PurchaseOrderRepo::insert(
            &mut *db,
            &req,
            &doc_number,
            total_amount,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 4. 插入明细
        if !req.items.is_empty() {
            PurchaseOrderItemRepo::insert_items(&mut *db, id, &req.items)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        // 5. 审计日志
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: ENTITY_TYPE, entity_id: id, action: AuditAction::Create, changes: None, context: None })
            .await?;

        // 6. 初始状态日志
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, id, "Draft", None)
            .await
            .ok();

        Ok(id)
    }

    async fn create_from_quotation(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        quotation_id: i64,
        idempotency_key: Option<String>,
    ) -> Result<i64> {
        if let Some(ref key) = idempotency_key {
            let hash = key_to_i64(key);
            if !new_idempotency_service(self.pool.clone()).check_and_mark(ctx, db, hash, "PurchaseOrder:create_from_quotation").await? {
                return Err(DomainError::duplicate("PurchaseOrder"));
            }
        }
        // 1. 获取报价单并验证状态
        let quotation = PurchaseQuotationRepo::get_by_id(&mut *db, quotation_id)
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
            PurchaseQuotationItemRepo::list_by_quotation_id(&mut *db, quotation_id)
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
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::PurchaseOrder)
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
            &mut *db,
            &req,
            &doc_number,
            total_amount,
            ctx.operator_id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        // 8. 插入明细
        if !order_items.is_empty() {
            PurchaseOrderItemRepo::insert_items(&mut *db, order_id, &order_items)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }

        // 9. 创建单据关联
        new_document_link_service(self.pool.clone())
            .create_links(
                ctx, db,
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
        new_audit_log_service(self.pool.clone())
            .record(
                    ctx,
                    db,
                    RecordAuditLogReq {
                        entity_type: ENTITY_TYPE,
                        entity_id: order_id,
                        action: AuditAction::Create,
                        changes: Some(json!({ "from_quotation_id": quotation_id })),
                        context: None,
                    },
                )
            .await?;

        Ok(order_id)
    }

    async fn get(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<PurchaseOrder> {
        PurchaseOrderRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))
    }

    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, idempotency_key: Option<String>) -> Result<()> {
        if let Some(ref key) = idempotency_key {
            let hash = key_to_i64(key);
            if !new_idempotency_service(self.pool.clone()).check_and_mark(ctx, db, hash, "PurchaseOrder:confirm").await? {
                return Err(DomainError::duplicate("PurchaseOrder"));
            }
        }
        // 1. 获取订单及明细
        let order = PurchaseOrderRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))?;

        let items = PurchaseOrderItemRepo::list_by_order_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 2. 校验供应商状态 ∉ {Blacklisted, Disqualified}
        let supplier = new_supplier_service(self.pool.clone()).get(ctx, db, order.supplier_id).await?;
        if matches!(supplier.status, SupplierStatus::Blacklisted | SupplierStatus::Disqualified) {
            return Err(DomainError::validation(format!(
                "供应商状态为 {:?}，无法确认订单",
                supplier.status
            )));
        }

        // 3. 校验所有明细 quantity > 0 且 unit_price > 0
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

        // 4. 若关联 Quotation，校验 quotation.status == Active 且 valid_until >= today
        if let Some(qi_id) = items.iter().find_map(|i| i.quotation_item_id) {
            let quotation = PurchaseQuotationRepo::get_by_item_id(&mut *db, qi_id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;

            if let Some(q) = quotation {
                if q.status != PurchaseQuotationStatus::Active {
                    return Err(DomainError::validation(format!(
                        "关联报价单 {} 状态不是 Active（当前: {:?}）",
                        q.doc_number, q.status
                    )));
                }
                let today = chrono::Local::now().date_naive();
                if q.valid_until < today {
                    return Err(DomainError::validation(format!(
                        "关联报价单 {} 已过期（有效期至 {}）",
                        q.doc_number, q.valid_until
                    )));
                }
            }
        }

        // 5. 状态转换 Draft -> Confirmed
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, id, "Confirmed", None)
            .await?;

        // 5.1 更新实体表状态
        let rows = PurchaseOrderRepo::update_status(
            &mut *db,
            id,
            PurchaseOrderStatus::Confirmed,
            &order.updated_at,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // 6. 发布领域事件
        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx, db,
                EventPublishRequest {
                    event_type: DomainEventType::PurchaseOrderConfirmed,
                    aggregate_type: ENTITY_TYPE.to_string(),
                    aggregate_id: id,
                    payload: json!({ "doc_number": order.doc_number }),
                    idempotency_key: None,
                },
            )
            .await?;

        // 7. 审计日志
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: ENTITY_TYPE, entity_id: id, action: AuditAction::Transition, changes: None, context: None })
            .await?;

        Ok(())
    }

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        query: PurchaseOrderQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<PurchaseOrder>> {
        let scope = (ctx.data_scope, ctx.operator_id, ctx.department_id);
        let (items, total) = PurchaseOrderRepo::query(&mut *db, &query, &page, scope)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    async fn list_items(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, order_id: i64) -> Result<Vec<PurchaseOrderItem>> {
        PurchaseOrderItemRepo::list_by_order_id(&mut *db, order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    }

    async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, idempotency_key: Option<String>) -> Result<()> {
        if let Some(ref key) = idempotency_key {
            let hash = key_to_i64(key);
            if !new_idempotency_service(self.pool.clone()).check_and_mark(ctx, db, hash, "PurchaseOrder:cancel").await? {
                return Err(DomainError::duplicate("PurchaseOrder"));
            }
        }

        let order = PurchaseOrderRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_TYPE))?;

        if order.status != PurchaseOrderStatus::Draft {
            return Err(DomainError::validation(format!(
                "只有 Draft 状态的订单才能取消（当前: {:?}）",
                order.status
            )));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, id, "Cancelled", None)
            .await?;

        let rows = PurchaseOrderRepo::update_status(
            &mut *db,
            id,
            PurchaseOrderStatus::Cancelled,
            &order.updated_at,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx, db,
                EventPublishRequest {
                    event_type: DomainEventType::PurchaseOrderCancelled,
                    aggregate_type: ENTITY_TYPE.to_string(),
                    aggregate_id: id,
                    payload: json!({ "doc_number": order.doc_number }),
                    idempotency_key: None,
                },
            )
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: ENTITY_TYPE, entity_id: id, action: AuditAction::Transition, changes: None, context: None })
            .await?;

        Ok(())
    }
}
