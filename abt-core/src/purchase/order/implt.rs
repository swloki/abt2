use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::postgres::PgPool;

use super::model::{
    CreateOrderItemRequest, CreatePurchaseOrderRequest, PurchaseOrder, PurchaseOrderItem,
    PurchaseOrderQuery, UpdatePurchaseOrderRequest,
};
use super::repo::{PurchaseOrderItemRepo, PurchaseOrderRepo};
use super::service::PurchaseOrderService;
use crate::master_data::supplier::service::SupplierService;
use crate::master_data::supplier::{new_supplier_service, model::SupplierStatus};
use crate::purchase::enums::{PurchaseOrderStatus, PurchaseQuotationStatus};
use crate::purchase::quotation::repo::{PurchaseQuotationItemRepo, PurchaseQuotationRepo};
use crate::purchase::settings::repo::PurchaseSettingsRepo;
use crate::purchase::payment_schedule::{
    model::PaymentScheduleInput, new_payment_schedule_service, service::PaymentScheduleService,
};
use crate::purchase::supplier_price::repo::SupplierProductPriceRepo;
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
const ENTITY_DISPLAY: &str = "采购订单";

pub struct PurchaseOrderServiceImpl {
    pool: PgPool,
}

impl PurchaseOrderServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 幂等检查：若提供 `key`，则登记 `PurchaseOrder:<op>`；重复请求返回 duplicate 错误。
    async fn check_idempotency(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        key: Option<String>,
        op: &str,
    ) -> Result<()> {
        if let Some(k) = key {
            let hash = key_to_i64(&k);
            if !new_idempotency_service(self.pool.clone())
                .check_and_mark(ctx, db, hash, op)
                .await?
            {
                return Err(DomainError::duplicate(ENTITY_TYPE));
            }
        }
        Ok(())
    }

    /// 确保实体有初始状态日志 — 如果缺失则补录 "" → Draft。
    /// 对已有 Draft 日志的实体，再次 transition 会因无 "Draft → Draft" 规则
    /// 返回 InvalidStateTransition，这是预期行为，安全忽略。
    async fn ensure_initial_state(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        match new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, id, "Draft", None)
            .await
        {
            Ok(_) => Ok(()),
            Err(DomainError::InvalidStateTransition { .. }) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// 计算订单各项金额（total_amount, amount_untaxed, amount_tax, amount_total）
    async fn compute_amounts(
        db: &mut sqlx::postgres::PgConnection,
        items: &[CreateOrderItemRequest],
        discount_amount: Decimal,
    ) -> Result<(Decimal, Decimal, Decimal, Decimal)> {
        let tax_map = super::repo::load_tax_rate_map(db, items).await?;

        let mut total_amount = Decimal::ZERO;
        let mut amount_untaxed = Decimal::ZERO;
        let mut amount_tax = Decimal::ZERO;

        for item in items {
            let rate = item
                .tax_rate_id
                .and_then(|tid| tax_map.get(&tid).copied())
                .unwrap_or(Decimal::ZERO);
            let (amount, price_subtotal, price_tax, _) = super::model::line_amounts(
                item.quantity,
                item.unit_price,
                item.discount_pct,
                rate,
            );
            total_amount += amount;
            amount_untaxed += price_subtotal;
            amount_tax += price_tax;
        }

        amount_untaxed -= discount_amount;
        let amount_total = amount_untaxed + amount_tax;

        Ok((total_amount, amount_untaxed, amount_tax, amount_total))
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
        self.check_idempotency(ctx, db, idempotency_key, "PurchaseOrder:create").await?;
        // 1. 生成单据编号
        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::PurchaseOrder)
            .await?;

        // 2. 计算各项金额
        let (total_amount, amount_untaxed, amount_tax, amount_total) =
            Self::compute_amounts(&mut *db, &req.items, req.discount_amount).await?;

        // 2.5 校验明细：quantity > 0 且 unit_price > 0
        for (i, item) in req.items.iter().enumerate() {
            if item.quantity <= Decimal::ZERO {
                return Err(DomainError::validation(
                    format!("订单明细第 {} 行数量必须大于 0", i + 1)
                ));
            }
            if item.unit_price <= Decimal::ZERO {
                return Err(DomainError::validation(
                    format!("订单明细第 {} 行单价必须大于 0", i + 1)
                ));
            }
        }

        // 3. 插入主表
        let id = PurchaseOrderRepo::insert(
            &mut *db,
            &req,
            &doc_number,
            total_amount,
            amount_untaxed,
            amount_tax,
            amount_total,
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
            .await?;

        Ok(id)
    }

    async fn create_from_quotation(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        quotation_id: i64,
        idempotency_key: Option<String>,
    ) -> Result<i64> {
        self.check_idempotency(ctx, db, idempotency_key, "PurchaseOrder:create_from_quotation").await?;
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
            .map(|(idx, qi)| {
                let quantity = qi.min_order_qty.ok_or_else(|| DomainError::validation(
                    format!("报价明细第 {} 行未设置最小起订量，无法自动创建订单", idx + 1)
                ))?;
                if qi.unit_price <= Decimal::ZERO {
                    return Err(DomainError::validation(
                        format!("报价明细第 {} 行单价必须大于 0", idx + 1)
                    ));
                }
                Ok(CreateOrderItemRequest {
                    product_id: qi.product_id,
                    line_no: (idx as i32) + 1,
                    description: String::new(),
                    quantity,
                    unit_price: qi.unit_price,
                    quotation_item_id: Some(qi.id),
                    expected_delivery_date: None,
                    discount_pct: Decimal::ZERO,
                    tax_rate_id: None,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        // 4. 计算各项金额
        let (total_amount, amount_untaxed, amount_tax, amount_total) =
            Self::compute_amounts(&mut *db, &order_items, Decimal::ZERO).await?;

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
            currency_code: String::from("CNY"),
            currency_rate: Decimal::ONE,
            discount_amount: Decimal::ZERO,
            items: order_items.clone(),
        };

        // 7. 插入主表
        let order_id = PurchaseOrderRepo::insert(
            &mut *db,
            &req,
            &doc_number,
            total_amount,
            amount_untaxed,
            amount_tax,
            amount_total,
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

        // 11. 初始状态日志
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, order_id, "Draft", None)
            .await?;

        Ok(order_id)
    }

    async fn get(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<PurchaseOrder> {
        PurchaseOrderRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_DISPLAY))
    }

    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64, idempotency_key: Option<String>) -> Result<()> {
        self.check_idempotency(ctx, db, idempotency_key, "PurchaseOrder:confirm").await?;
        // 1. 获取订单及明细
        let order = PurchaseOrderRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_DISPLAY))?;

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

        // 4.5 价格一致性校验（maintain_same_rate）
        let settings = PurchaseSettingsRepo::get(&mut *db).await.ok();
        if let Some(ref s) = settings
            && s.maintain_same_rate
        {
            // 批量加载关联报价明细用于价格比较
            if let Some(first_qi_id) = items.iter().find_map(|i| i.quotation_item_id)
                && let Ok(Some(q)) = PurchaseQuotationRepo::get_by_item_id(&mut *db, first_qi_id).await
            {
                let quotation_items = PurchaseQuotationItemRepo::list_by_quotation_id(&mut *db, q.id)
                    .await
                    .unwrap_or_default();
                for item in &items {
                    if let Some(qi_id) = item.quotation_item_id
                        && let Some(qi) = quotation_items.iter().find(|qi| qi.id == qi_id)
                        && item.unit_price != qi.unit_price
                    {
                        return Err(DomainError::validation(format!(
                            "订单行 {} 单价 {} 与报价单单价 {} 不一致（已启用价格一致性校验）",
                            item.line_no, item.unit_price, qi.unit_price
                        )));
                    }
                }
            }
        }

        // 5. 确保有初始状态日志，然后转换 Draft -> Confirmed
        self.ensure_initial_state(ctx, db, id).await?;
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

        // 8. 生成默认付款计划（100% 单期，到期日 = order_date + 30 天）
        let schedule_input = vec![PaymentScheduleInput {
            due_date: order.order_date + chrono::Duration::days(30),
            payment_pct: Decimal::from(100),
            description: "全额付款".to_string(),
        }];
        new_payment_schedule_service(self.pool.clone())
            .generate_for_order(ctx, db, id, order.amount_total, schedule_input)
            .await?;

        // 9. 自动创建缺失的供应商价格记录（1 次预加载已存在价格，仅对缺失项逐条插入）
        {
            use std::collections::HashSet;
            let existing_prices = SupplierProductPriceRepo::list_by_supplier(
                &mut *db, order.supplier_id,
            )
            .await
            .unwrap_or_default();
            let existing_product_ids: HashSet<i64> =
                existing_prices.iter().map(|p| p.product_id).collect();
            for item in &items {
                if !existing_product_ids.contains(&item.product_id) {
                    SupplierProductPriceRepo::insert(
                        &mut *db,
                        order.supplier_id,
                        item.product_id,
                        item.unit_price,
                        &order.currency_code,
                    )
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                }
            }
        }

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
        self.check_idempotency(ctx, db, idempotency_key, "PurchaseOrder:cancel").await?;

        let order = PurchaseOrderRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_DISPLAY))?;

        if order.status != PurchaseOrderStatus::Draft {
            return Err(DomainError::validation(format!(
                "只有 Draft 状态的订单才能取消（当前: {:?}）",
                order.status
            )));
        }

        self.ensure_initial_state(ctx, db, id).await?;
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

    async fn update(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        req: UpdatePurchaseOrderRequest,
        items: Vec<CreateOrderItemRequest>,
    ) -> Result<()> {
        let existing = PurchaseOrderRepo::get_by_id(&mut *db, id)
            .await?
            .ok_or_else(|| DomainError::not_found(ENTITY_DISPLAY))?;

        if existing.status != PurchaseOrderStatus::Draft {
            return Err(DomainError::business_rule("仅草稿状态的订单可以编辑"));
        }

        // 0.5 校验明细：quantity > 0 且 unit_price > 0
        for (i, item) in items.iter().enumerate() {
            if item.quantity <= Decimal::ZERO {
                return Err(DomainError::validation(
                    format!("订单明细第 {} 行数量必须大于 0", i + 1)
                ));
            }
            if item.unit_price <= Decimal::ZERO {
                return Err(DomainError::validation(
                    format!("订单明细第 {} 行单价必须大于 0", i + 1)
                ));
            }
        }

        // 1. 更新订单头
        PurchaseOrderRepo::update_fields(&mut *db, id, &req).await?;

        // 2. 删除旧明细，插入新明细
        PurchaseOrderItemRepo::delete_by_order_id(&mut *db, id).await?;
        if !items.is_empty() {
            PurchaseOrderItemRepo::insert_items(&mut *db, id, &items).await?;
        }

        // 3. 计算并更新总金额
        let (total_amount, amount_untaxed, amount_tax, amount_total) =
            Self::compute_amounts(&mut *db, &items, req.discount_amount).await?;
        PurchaseOrderRepo::update_total_amount(
            &mut *db, id, total_amount, amount_untaxed, amount_tax, amount_total,
        )
        .await?;

        // 4. 审计日志
        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: ENTITY_TYPE,
                    entity_id: id,
                    action: AuditAction::Update,
                    changes: None,
                    context: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn update_items_after_confirm(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
        item_changes: Vec<super::model::PoItemChange>,
        _idempotency_key: Option<String>,
    ) -> Result<()> {
        use super::model::PoItemChange;

        // 1. 校验 PO 状态
        let order = PurchaseOrderRepo::get_by_id(&mut *db, order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_DISPLAY))?;

        if !matches!(
            order.status,
            PurchaseOrderStatus::Confirmed | PurchaseOrderStatus::PartiallyReceived
        ) {
            return Err(DomainError::business_rule(
                "仅 Confirmed/PartiallyReceived 状态的订单可以修改明细"
            ));
        }

        let existing_items = PurchaseOrderItemRepo::list_by_order_id(&mut *db, order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 2. 逐个处理变更
        for change in &item_changes {
            match change {
                PoItemChange::AddItem(new_item) => {
                    if new_item.quantity <= Decimal::ZERO
                        || new_item.unit_price <= Decimal::ZERO
                    {
                        return Err(DomainError::validation(
                            "追加行的数量和单价必须大于 0"
                        ));
                    }
                    let max_line_no = existing_items.iter().map(|i| i.line_no).max().unwrap_or(0);
                    PurchaseOrderItemRepo::insert_single(
                        &mut *db, order_id, max_line_no + 1, new_item,
                    )
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                }
                PoItemChange::UpdateItem {
                    item_id,
                    quantity,
                    unit_price,
                    discount_pct,
                    tax_rate_id,
                } => {
                    let item = existing_items
                        .iter()
                        .find(|i| i.id == *item_id)
                        .ok_or_else(|| DomainError::not_found("PurchaseOrderItem"))?;

                    if let Some(new_qty) = quantity
                        && *new_qty < item.received_qty
                    {
                        return Err(DomainError::validation(format!(
                            "修改后数量 {} 不能小于已收货数量 {}",
                            new_qty, item.received_qty
                        )));
                    }

                    PurchaseOrderItemRepo::update_fields_after_confirm(
                        &mut *db,
                        *item_id,
                        *quantity,
                        *unit_price,
                        *discount_pct,
                        *tax_rate_id,
                    )
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                }
                PoItemChange::RemoveItem { item_id } => {
                    let item = existing_items
                        .iter()
                        .find(|i| i.id == *item_id)
                        .ok_or_else(|| DomainError::not_found("PurchaseOrderItem"))?;

                    if item.received_qty > Decimal::ZERO {
                        return Err(DomainError::business_rule(format!(
                            "行 {} 已有收货记录，不能删除",
                            item.line_no
                        )));
                    }

                    PurchaseOrderItemRepo::delete_by_id(&mut *db, *item_id)
                        .await
                        .map_err(|e| DomainError::Internal(e.into()))?;
                }
            }
        }

        // 3. 重算总金额
        let updated_items = PurchaseOrderItemRepo::list_by_order_id(&mut *db, order_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let item_reqs: Vec<CreateOrderItemRequest> = updated_items
            .iter()
            .map(|i| CreateOrderItemRequest {
                product_id: i.product_id,
                line_no: i.line_no,
                description: i.description.clone(),
                quantity: i.quantity,
                unit_price: i.unit_price,
                quotation_item_id: i.quotation_item_id,
                expected_delivery_date: i.expected_delivery_date,
                discount_pct: i.discount_pct,
                tax_rate_id: i.tax_rate_id,
            })
            .collect();
        let (total_amount, amount_untaxed, amount_tax, amount_total) =
            Self::compute_amounts(&mut *db, &item_reqs, order.discount_amount).await?;
        PurchaseOrderRepo::update_total_amount(
            &mut *db, order_id, total_amount, amount_untaxed, amount_tax, amount_total,
        )
        .await?;

        // 4. 审计日志
        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: ENTITY_TYPE,
                    entity_id: order_id,
                    action: AuditAction::Update,
                    changes: Some(json!({ "item_changes_count": item_changes.len() })),
                    context: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn submit(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        idempotency_key: Option<String>,
    ) -> Result<()> {
        use crate::purchase::approval::{new_approval_service, service::PurchaseApprovalService};

        self.check_idempotency(ctx, db, idempotency_key, "PurchaseOrder:submit").await?;

        let order = PurchaseOrderRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_DISPLAY))?;

        if order.status != PurchaseOrderStatus::Draft {
            return Err(DomainError::business_rule("仅草稿状态的订单可以提交"));
        }

        // 查找匹配的审批规则
        let approval_rule = new_approval_service(self.pool.clone())
            .find_rule_by_amount(ctx, db, order.amount_total)
            .await?;

        if approval_rule.is_some() {
            // 需要审批 → 进入 PendingApproval
            self.ensure_initial_state(ctx, db, id).await?;
            new_state_machine_service(self.pool.clone())
                .transition(ctx, db, ENTITY_TYPE, id, "PendingApproval", None)
                .await?;
            let rows = PurchaseOrderRepo::update_status(
                &mut *db,
                id,
                PurchaseOrderStatus::PendingApproval,
                &order.updated_at,
            )
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            if rows == 0 {
                return Err(DomainError::ConcurrentConflict);
            }

            new_audit_log_service(self.pool.clone())
                .record(
                    ctx,
                    db,
                    RecordAuditLogReq {
                        entity_type: ENTITY_TYPE,
                        entity_id: id,
                        action: AuditAction::Transition,
                        changes: Some(json!({"to": "PendingApproval"})),
                        context: None,
                    },
                )
                .await?;
        } else {
            // 无需审批 → 直接确认
            self.confirm(ctx, db, id, None).await?;
        }

        Ok(())
    }

    async fn approve_po(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        idempotency_key: Option<String>,
    ) -> Result<()> {
        self.check_idempotency(ctx, db, idempotency_key, "PurchaseOrder:approve_po").await?;

        let order = PurchaseOrderRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_DISPLAY))?;

        if order.status != PurchaseOrderStatus::PendingApproval {
            return Err(DomainError::business_rule("仅待审批状态的订单可以审批通过"));
        }

        // 执行确认逻辑（复用 confirm 的完整流程）
        self.confirm(ctx, db, id, None).await?;

        Ok(())
    }

    async fn reject(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        reason: String,
        idempotency_key: Option<String>,
    ) -> Result<()> {
        self.check_idempotency(ctx, db, idempotency_key, "PurchaseOrder:reject").await?;

        let order = PurchaseOrderRepo::get_by_id(&mut *db, id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or_else(|| DomainError::not_found(ENTITY_DISPLAY))?;

        if order.status != PurchaseOrderStatus::PendingApproval {
            return Err(DomainError::business_rule("仅待审批状态的订单可以退回"));
        }

        // 状态转换 PendingApproval → Draft
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ENTITY_TYPE, id, "Draft", None)
            .await?;

        let rows = PurchaseOrderRepo::update_status(
            &mut *db,
            id,
            PurchaseOrderStatus::Draft,
            &order.updated_at,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: ENTITY_TYPE,
                    entity_id: id,
                    action: AuditAction::Transition,
                    changes: Some(json!({"to": "Draft", "reason": reason})),
                    context: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn merge_orders(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_ids: Vec<i64>,
        idempotency_key: Option<String>,
    ) -> Result<i64> {
        use std::collections::{HashMap, HashSet};

        self.check_idempotency(ctx, db, idempotency_key, "PurchaseOrder:merge").await?;

        if order_ids.len() < 2 {
            return Err(DomainError::validation("合并至少需要两个订单"));
        }

        // 1. 加载所有 PO
        let mut orders = Vec::new();
        for &oid in &order_ids {
            let order = PurchaseOrderRepo::get_by_id(&mut *db, oid)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .ok_or_else(|| DomainError::not_found(ENTITY_DISPLAY))?;
            if order.status != PurchaseOrderStatus::Draft {
                return Err(DomainError::business_rule("仅 Draft 状态的订单可以合并"));
            }
            orders.push(order);
        }

        // 2. 校验同一供应商
        let supplier_ids: HashSet<i64> = orders.iter().map(|o| o.supplier_id).collect();
        if supplier_ids.len() != 1 {
            return Err(DomainError::business_rule("合并的订单必须属于同一供应商"));
        }

        // 3. 取最早的 PO 作为目标
        orders.sort_by_key(|o| o.order_date);
        let target_id = orders[0].id;
        let target = orders[0].clone();

        // 4. 合并明细（相同 product + unit_price 合并数量）
        let mut merged: HashMap<(i64, Decimal), CreateOrderItemRequest> = HashMap::new();
        for order in &orders {
            let items = PurchaseOrderItemRepo::list_by_order_id(&mut *db, order.id)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            for item in items {
                let key = (item.product_id, item.unit_price);
                merged.entry(key)
                    .and_modify(|existing| {
                        existing.quantity += item.quantity;
                    })
                    .or_insert(CreateOrderItemRequest {
                        product_id: item.product_id,
                        line_no: 0,
                        description: item.description.clone(),
                        quantity: item.quantity,
                        unit_price: item.unit_price,
                        quotation_item_id: item.quotation_item_id,
                        expected_delivery_date: item.expected_delivery_date,
                        discount_pct: item.discount_pct,
                        tax_rate_id: item.tax_rate_id,
                    });
            }
        }

        // 5. 更新目标 PO 的明细
        let mut merged_items: Vec<CreateOrderItemRequest> = merged.into_values().collect();
        for (i, item) in merged_items.iter_mut().enumerate() {
            item.line_no = (i as i32) + 1;
        }
        PurchaseOrderItemRepo::delete_by_order_id(&mut *db, target_id).await?;
        if !merged_items.is_empty() {
            PurchaseOrderItemRepo::insert_items(&mut *db, target_id, &merged_items).await?;
        }

        // 6. 更新总金额
        let (total_amount, amount_untaxed, amount_tax, amount_total) =
            Self::compute_amounts(&mut *db, &merged_items, target.discount_amount).await?;
        PurchaseOrderRepo::update_total_amount(
            &mut *db, target_id, total_amount, amount_untaxed, amount_tax, amount_total,
        )
        .await?;

        // 7. 取消其他 PO 并创建关联
        for order in &orders {
            if order.id != target_id {
                self.cancel(ctx, db, order.id, None).await?;
                new_document_link_service(self.pool.clone())
                    .create_links(
                        ctx, db,
                        vec![LinkRequest {
                            source_type: DocumentType::PurchaseOrder,
                            source_id: target_id,
                            target_type: DocumentType::PurchaseOrder,
                            target_id: order.id,
                            link_type: LinkType::References,
                        }],
                    )
                    .await?;
            }
        }

        // 8. 审计日志
        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: ENTITY_TYPE,
                    entity_id: target_id,
                    action: AuditAction::Update,
                    changes: Some(json!({"merged_from": order_ids})),
                    context: None,
                },
            )
            .await?;

        Ok(target_id)
    }
}
