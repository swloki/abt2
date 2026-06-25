use chrono::Local;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use crate::master_data::customer::{new_customer_service, service::CustomerService};
use crate::master_data::product::{new_product_service, service::ProductService};
use crate::master_data::product::model::AcquireChannel;
use crate::master_data::bom::{new_bom_query_service, service::BomQueryService};
use crate::wms::stock_ledger::{new_stock_ledger_service, service::StockLedgerService};
use crate::sales::quotation::{new_quotation_service, service::QuotationService};
use crate::sales::sales_order::model::*;
use crate::sales::sales_order::repo::{DemandRepo, FulfillmentPlanLineRepo, SalesOrderItemRepo, SalesOrderRepo, savepoint, release_savepoint, rollback_savepoint};
use crate::sales::sales_order::service::{SalesOrderService, DemandService};
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::document_link::{new_document_link_service, service::DocumentLinkService};
use crate::shared::document_link::model::LinkRequest;
use crate::shared::document_sequence::{new_document_sequence_service, service::DocumentSequenceService};
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::enums::link_type::LinkType;
use crate::shared::enums::reservation::ReservationType;
use crate::shared::event_bus::{new_domain_event_bus, service::DomainEventBus};
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::inventory_reservation::{new_inventory_reservation_service, service::InventoryReservationService};
use crate::shared::inventory_reservation::model::ReserveRequest;
use crate::shared::state_machine::{new_state_machine_service, service::StateMachineService};
use crate::shared::types::{PgExecutor, DomainError, PageParams, PaginatedResult, ServiceContext, Result};

pub struct SalesOrderServiceImpl {
    repo: SalesOrderRepo,
    item_repo: SalesOrderItemRepo,
    pool: PgPool,
}

impl SalesOrderServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: SalesOrderRepo,
            item_repo: SalesOrderItemRepo,
            pool,
        }
    }

    fn calculate_amounts(items: &[CreateSalesOrderItemReq]) -> (Decimal, Decimal) {
        let mut total_amount = Decimal::ZERO;
        let mut total_cost = Decimal::ZERO;

        for item in items {
            let discount = item.discount_rate.unwrap_or(Decimal::ZERO);
            let amount = item.quantity * item.unit_price * (Decimal::ONE - discount / Decimal::from(100));
            let cost = item.quantity * item.unit_cost.unwrap_or(Decimal::ZERO);
            total_amount += amount.round_dp(4);
            total_cost += cost.round_dp(4);
        }

        (total_amount, total_cost)
    }

    fn build_item_inputs(items: &[CreateSalesOrderItemReq]) -> Vec<SalesOrderItemInput> {
        items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let discount = item.discount_rate.unwrap_or(Decimal::ZERO);
                let amount = item.quantity * item.unit_price * (Decimal::ONE - discount / Decimal::from(100));
                SalesOrderItemInput {
                    line_no: (i + 1) as i32,
                    product_id: item.product_id,
                    description: item.description.clone().unwrap_or_default(),
                    quantity: item.quantity,
                    unit: item.unit.clone().unwrap_or_default(),
                    unit_price: item.unit_price,
                    unit_cost: item.unit_cost.unwrap_or(Decimal::ZERO),
                    discount_rate: item.discount_rate.unwrap_or(Decimal::ZERO),
                    amount: amount.round_dp(4),
                    delivery_date: item.delivery_date,
                }
            })
            .collect()
    }
}

/// 幂等的订单头状态计算 — 每次订单行变更后调用
/// 关键：cancelled_qty 不等于 shipped_qty，取消不是发货
fn calc_header_status(items: &[SalesOrderItem]) -> SalesOrderStatus {
    let all_settled = items.iter().all(|i| i.is_settled());
    let any_shipped = items.iter().any(|i| i.shipped_qty > Decimal::ZERO);
    let any_open = items.iter().any(|i| i.open_qty() > Decimal::ZERO);

    // 有效行 = 非 Cancelled。从未发货 + 全部 Allocated（库存已补足）→ 待发货
    let active: Vec<&SalesOrderItem> = items
        .iter()
        .filter(|i| i.line_status != SalesOrderLineStatus::Cancelled)
        .collect();
    let all_allocated = !active.is_empty()
        && active
            .iter()
            .all(|i| i.line_status == SalesOrderLineStatus::Allocated);

    if all_settled && any_shipped {
        SalesOrderStatus::Shipped
    } else if any_shipped && any_open {
        SalesOrderStatus::PartiallyShipped
    } else if !any_shipped && all_allocated {
        SalesOrderStatus::ReadyToShip
    } else {
        SalesOrderStatus::Confirmed
    }
}

#[async_trait::async_trait]
impl SalesOrderService for SalesOrderServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateSalesOrderReq,
    ) -> Result<i64> {
        new_customer_service(self.pool.clone())
            .validate_contact_ownership(ctx, db, req.customer_id, req.contact_id)
            .await?;

        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::SalesOrder)
            .await?;

        let (total_amount, total_cost) = Self::calculate_amounts(&req.items);

        let id = self
            .repo
            .create(
                db,
                &CreateSalesOrderParams {
                    doc_number: &doc_number,
                    customer_id: req.customer_id,
                    contact_id: req.contact_id,
                    sales_rep_id: ctx.operator_id,
                    total_amount,
                    total_cost,
                    payment_terms: req.payment_terms.as_deref().unwrap_or(""),
                    delivery_terms: req.delivery_terms.as_deref().unwrap_or(""),
                    delivery_address: req.delivery_address.as_deref().unwrap_or(""),
                    remark: req.remark.as_deref().unwrap_or(""),
                    operator_id: ctx.operator_id,
                },
            )
            .await?;

        let item_inputs = Self::build_item_inputs(&req.items);
        self.item_repo
            .create_batch(db, id, &item_inputs)
            .await?;

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "SalesOrderStatus", id, "Draft", None)
            .await
            .ok();

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "SalesOrder", entity_id: id, action: AuditAction::Create, changes: None, context: None })
            .await?;

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::SalesOrderCreated,
                    aggregate_type: "SalesOrder".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({
                        "sales_order_id": id,
                        "doc_number": doc_number,
                        "customer_id": req.customer_id,
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(id)
    }

    async fn create_from_quotation(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        quotation_id: i64,
    ) -> Result<i64> {
        let quotation = new_quotation_service(self.pool.clone()).find_by_id(ctx, db, quotation_id).await?;

        if quotation.status != crate::sales::quotation::model::QuotationStatus::Accepted {
            return Err(DomainError::business_rule(
                "Only Accepted quotations can be converted to orders",
            ));
        }

        if quotation.valid_until < Local::now().date_naive() {
            return Err(DomainError::business_rule("Quotation has expired"));
        }

        let quotation_items = new_quotation_service(self.pool.clone()).list_items(ctx, db, quotation_id).await?;

        let order_items: Vec<CreateSalesOrderItemReq> = quotation_items
            .iter()
            .map(|qi| CreateSalesOrderItemReq {
                product_id: qi.product_id,
                description: Some(qi.description.clone()),
                quantity: qi.quantity,
                unit: Some(qi.unit.clone()),
                unit_price: qi.unit_price,
                unit_cost: Some(qi.unit_cost),
                discount_rate: Some(qi.discount_rate),
                delivery_date: qi.delivery_date,
            })
            .collect();

        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::SalesOrder)
            .await?;

        let (total_amount, total_cost) = Self::calculate_amounts(&order_items);

        let id = self
            .repo
            .create(
                db,
                &CreateSalesOrderParams {
                    doc_number: &doc_number,
                    customer_id: quotation.customer_id,
                    contact_id: quotation.contact_id,
                    sales_rep_id: quotation.sales_rep_id,
                    total_amount,
                    total_cost,
                    payment_terms: &quotation.payment_terms,
                    delivery_terms: &quotation.delivery_terms,
                    delivery_address: "",
                    remark: &quotation.remark,
                    operator_id: ctx.operator_id,
                },
            )
            .await?;

        let item_inputs = Self::build_item_inputs(&order_items);
        self.item_repo
            .create_batch(db, id, &item_inputs)
            .await?;

        new_document_link_service(self.pool.clone())
            .create_links(
                ctx,
                db,
                vec![LinkRequest {
                    source_type: DocumentType::SalesOrder,
                    source_id: id,
                    target_type: DocumentType::Quotation,
                    target_id: quotation_id,
                    link_type: LinkType::DerivedFrom,
                }],
            )
            .await?;

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "SalesOrderStatus", id, "Draft", None)
            .await
            .ok();

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx,
                    db,
                    RecordAuditLogReq {
                        entity_type: "SalesOrder",
                        entity_id: id,
                        action: AuditAction::Create,
                        changes: Some(serde_json::json!({ "source": "quotation", "quotation_id": quotation_id })),
                        context: None,
                    },
                )
            .await?;

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::SalesOrderCreated,
                    aggregate_type: "SalesOrder".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({
                        "sales_order_id": id,
                        "doc_number": doc_number,
                        "source_quotation_id": quotation_id,
                    }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(id)
    }

    async fn find_by_id(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<SalesOrder> {
        self.repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesOrder"))
    }

    async fn update_header(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateSalesOrderReq,
    ) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

        if existing.status != SalesOrderStatus::Draft {
            return Err(DomainError::business_rule("Only Draft orders can be updated"));
        }

        self.repo
            .update(db, id, &req)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "SalesOrder", entity_id: id, action: AuditAction::Update, changes: None, context: None })
            .await?;

        Ok(())
    }

    async fn update(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateSalesOrderReq,
        items: Vec<CreateSalesOrderItemReq>,
    ) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

        if existing.status != SalesOrderStatus::Draft {
            return Err(DomainError::business_rule("仅草稿状态的订单可以编辑"));
        }

        self.repo.update(db, id, &req).await?;

        self.item_repo.delete_by_order_id(db, id).await?;

        let item_inputs = Self::build_item_inputs(&items);
        self.item_repo
            .create_batch(db, id, &item_inputs)
            .await?;

        let (total_amount, total_cost) = Self::calculate_amounts(&items);
        self.repo
            .update_amounts(db, id, total_amount, total_cost)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq { entity_type: "SalesOrder", entity_id: id, action: AuditAction::Update, changes: None, context: None })
            .await?;

        Ok(())
    }

    async fn list_items(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<Vec<SalesOrderItem>> {
        self.item_repo
            .find_by_order_id(db, order_id)
            .await
    }

    async fn list_items_by_order_ids(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        order_ids: &[i64],
    ) -> Result<Vec<SalesOrderItem>> {
        self.item_repo
            .find_by_order_ids(db, order_ids)
            .await
    }

    async fn confirm(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        // 1. 加载并校验
        let existing = self.repo.find_by_id(db, id).await?
            .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

        if existing.status != SalesOrderStatus::Draft {
            return Err(DomainError::business_rule("Only Draft orders can be confirmed"));
        }

        let items = self.item_repo.find_by_order_id(db, id).await?;
        if items.is_empty() {
            return Err(DomainError::business_rule("Cannot confirm order without items"));
        }

        // 2. 批量查询产品获取 acquire_channel
        let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
        let products = new_product_service(self.pool.clone())
            .get_by_ids(ctx, db, product_ids).await?;
        let product_map: std::collections::HashMap<i64, AcquireChannel> = products
            .into_iter()
            .map(|p| (p.product_id, p.acquire_channel))
            .collect();

        // 3. 状态机转换
        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "SalesOrderStatus", id, "Confirmed", None)
            .await?;
        self.repo.update_status(db, id, SalesOrderStatus::Confirmed).await?;

        // 4. 逐行处理：预留 + 生成履行计划
        let mut fp_inputs: Vec<FulfillmentPlanLineInput> = Vec::with_capacity(items.len());
        let mut line_status_updates: Vec<(i64, SalesOrderLineStatus, i32)> = Vec::with_capacity(items.len());
        let mut reserve_requests: Vec<ReserveRequest> = Vec::new();

        for item in &items {
            let ac = product_map.get(&item.product_id)
                .copied()
                .unwrap_or(AcquireChannel::Legacy);

            match ac {
                AcquireChannel::NonInventory => {
                    // 费用/服务类：跳过库存，直接 Allocated
                    fp_inputs.push(FulfillmentPlanLineInput {
                        order_id: id,
                        order_line_id: item.id,
                        product_id: item.product_id,
                        acquire_channel: ac,
                        required_qty: item.quantity,
                        reserved_qty: item.quantity,
                        shortage_qty: Decimal::ZERO,
                        status: FulfillmentLineStatus::Allocated,
                        required_date: item.delivery_date,
                    });
                    line_status_updates.push((item.id, SalesOrderLineStatus::Allocated, 1));
                }
                _ => {
                    // 库存类：尝试硬预留
                    reserve_requests.push(ReserveRequest {
                        product_id: item.product_id,
                        warehouse_id: None, // 跨仓库 ATP 预留（按 product 维度汇总）
                        reserved_qty: item.quantity,
                        reservation_type: ReservationType::Hard,
                        source_type: DocumentType::SalesOrder,
                        source_id: id,
                        source_line_id: Some(item.id),
                        priority: 5,
                        expires_at: None,
                    });
                }
            }
        }

        // 5. 执行预留（支持部分预留：reserve 内部按 min(qty, ATP) 预占）
        savepoint(db, "sp_reserve").await.ok();
        match new_inventory_reservation_service(self.pool.clone())
            .reserve(ctx, db, reserve_requests.clone())
            .await
        {
            Ok(batch) => {
                // 不静默丢弃失败项：逐条记录错误详情（违反 CLAUDE.md「禁止静默丢弃错误」）
                if !batch.failed_items.is_empty() {
                    tracing::warn!(
                        order_id = id,
                        failed_count = batch.failed_items.len(),
                        "inventory reserve partial failure"
                    );
                    for f in &batch.failed_items {
                        tracing::warn!(index = f.index, error = %f.error, "reserve line failed");
                    }
                }
                release_savepoint(db, "sp_reserve").await.ok();
            }
            Err(e) => {
                tracing::warn!("inventory reserve error: {e}");
                rollback_savepoint(db, "sp_reserve").await.ok();
            }
        }

        // 5.5 查询每行实际预留量（部分预留时 reserved < required，用于精确计算 shortage）
        let reserved_map = match new_inventory_reservation_service(self.pool.clone())
            .reserved_qty_by_source(ctx, db, DocumentType::SalesOrder, id)
            .await
        {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(order_id = id, error = %e, "reserved_qty_by_source query failed");
                std::collections::HashMap::new()
            }
        };

        // 6. 为库存类行生成履行计划（根据实际预留量决定状态，支持部分预留）
        for item in &items {
            let ac = product_map.get(&item.product_id)
                .copied()
                .unwrap_or(AcquireChannel::Legacy);
            if ac == AcquireChannel::NonInventory {
                continue;
            }

            let actual_reserved = reserved_map.get(&item.id).copied().unwrap_or(Decimal::ZERO);
            let shortage_qty = (item.quantity - actual_reserved).max(Decimal::ZERO);
            let (fp_status, line_status) = if shortage_qty <= Decimal::ZERO {
                // 全部预留 → 可直接发货
                (FulfillmentLineStatus::Allocated, SalesOrderLineStatus::Allocated)
            } else {
                // 部分预留或完全缺货 → Pending，shortage 部分触发补货
                (FulfillmentLineStatus::Pending, SalesOrderLineStatus::Pending)
            };

            fp_inputs.push(FulfillmentPlanLineInput {
                order_id: id,
                order_line_id: item.id,
                product_id: item.product_id,
                acquire_channel: ac,
                required_qty: item.quantity,
                reserved_qty: actual_reserved,
                shortage_qty,
                status: fp_status,
                required_date: item.delivery_date,
            });
            line_status_updates.push((item.id, line_status, 1));
        }

        // 7. 批量写入
        if !fp_inputs.is_empty() {
            FulfillmentPlanLineRepo::create_batch(db, &fp_inputs).await?;
        }
        if !line_status_updates.is_empty() {
            self.item_repo.batch_update_line_status(db, &line_status_updates).await?;
        }

        // 8. 审计日志
        savepoint(db, "sp_audit").await.ok();
        if let Err(e) = new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq {
                entity_type: "SalesOrder",
                entity_id: id,
                action: AuditAction::Transition,
                changes: Some(serde_json::json!({ "from": "Draft", "to": "Confirmed" })),
                context: None,
            })
            .await
        {
            tracing::warn!("audit record failed: {e}");
            rollback_savepoint(db, "sp_audit").await.ok();
        } else {
            release_savepoint(db, "sp_audit").await.ok();
        }

        // 9. 领域事件
        savepoint(db, "sp_event").await.ok();
        if let Err(e) = new_domain_event_bus(self.pool.clone())
            .publish(ctx, db, EventPublishRequest {
                event_type: DomainEventType::SalesOrderConfirmed,
                aggregate_type: "SalesOrder".to_string(),
                aggregate_id: id,
                payload: serde_json::json!({ "sales_order_id": id }),
                idempotency_key: None,
            })
            .await
        {
            tracing::warn!("event publish failed: {e}");
            rollback_savepoint(db, "sp_event").await.ok();
        } else {
            release_savepoint(db, "sp_event").await.ok();
        }

        // P2: 为缺货行创建需求 + 发布 DemandCreated 事件
        let has_shortages = fp_inputs.iter().any(|l| l.shortage_qty > Decimal::ZERO);
        if has_shortages {
            savepoint(db, "sp_demands").await.ok();
            match DemandServiceImpl::new(self.pool.clone())
                .create_from_order(ctx, db, id)
                .await
            {
                Ok(demand_ids) => {
                    tracing::info!("Created {} demands for order {id}", demand_ids.len());
                    release_savepoint(db, "sp_demands").await.ok();
                }
                Err(e) => {
                    tracing::warn!("Demand creation failed for order {id}: {e}");
                    rollback_savepoint(db, "sp_demands").await.ok();
                }
            }
        }

        // 10. 重算头状态：全行 Allocated（库存已补足）则推进到 ReadyToShip，否则保持 Confirmed
        self.recalc_header_status(ctx, db, id).await?;

        Ok(())
    }

    async fn complete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

        if existing.status != SalesOrderStatus::Shipped {
            return Err(DomainError::business_rule("Only Shipped orders can be completed"));
        }

        let items = self
            .item_repo
            .find_by_order_id(db, id)
            .await?;

        for item in &items {
            if item.open_qty() > Decimal::ZERO {
                return Err(DomainError::business_rule(format!(
                    "Item {} has open qty {} (not fully shipped/cancelled)",
                    item.line_no, item.open_qty()
                )));
            }
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "SalesOrderStatus", id, "Completed", None)
            .await?;

        self.repo
            .update_status(db, id, SalesOrderStatus::Completed)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx,
                    db,
                    RecordAuditLogReq {
                        entity_type: "SalesOrder",
                        entity_id: id,
                        action: AuditAction::Transition,
                        changes: Some(serde_json::json!({ "from": existing.status.as_str(), "to": "Completed" })),
                        context: None,
                    },
                )
            .await?;

        Ok(())
    }

    async fn cancel(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

        if existing.status != SalesOrderStatus::Draft
            && existing.status != SalesOrderStatus::Confirmed
            && existing.status != SalesOrderStatus::ReadyToShip
            && existing.status != SalesOrderStatus::PartiallyShipped
        {
            return Err(DomainError::business_rule(
                "Only Draft, Confirmed, ReadyToShip or PartiallyShipped orders can be cancelled",
            ));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "SalesOrderStatus", id, "Cancelled", None)
            .await?;

        // 释放所有预留（Confirmed/ReadyToShip/PartiallyShipped 状态下才可能有预留）
        if existing.status == SalesOrderStatus::Confirmed
            || existing.status == SalesOrderStatus::ReadyToShip
            || existing.status == SalesOrderStatus::PartiallyShipped
        {
            savepoint(db, "sp_cancel_resv").await.ok();
            if let Err(e) = new_inventory_reservation_service(self.pool.clone())
                .cancel_by_source(ctx, db, DocumentType::SalesOrder, id)
                .await
            {
                tracing::warn!("cancel reservations failed: {e}");
                rollback_savepoint(db, "sp_cancel_resv").await.ok();
            } else {
                release_savepoint(db, "sp_cancel_resv").await.ok();
            }
        }

        self.repo
            .update_status(db, id, SalesOrderStatus::Cancelled)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx,
                    db,
                    RecordAuditLogReq {
                        entity_type: "SalesOrder",
                        entity_id: id,
                        action: AuditAction::Transition,
                        changes: Some(serde_json::json!({ "from": existing.status.as_str(), "to": "Cancelled" })),
                        context: None,
                    },
                )
            .await?;

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::SalesOrderCancelled,
                    aggregate_type: "SalesOrder".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({ "sales_order_id": id, "doc_number": existing.doc_number }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()> {
        let existing = self
            .repo
            .find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

        if existing.status != SalesOrderStatus::Draft {
            return Err(DomainError::business_rule("仅草稿状态的订单可以删除"));
        }

        self.repo.soft_delete(db, id).await?;

        new_audit_log_service(self.pool.clone())
            .record(
                    ctx,
                    db,
                    RecordAuditLogReq {
                        entity_type: "SalesOrder",
                        entity_id: id,
                        action: AuditAction::Delete,
                        changes: Some(serde_json::json!({ "doc_number": existing.doc_number })),
                        context: None,
                    },
                )
            .await?;

        new_domain_event_bus(self.pool.clone())
            .publish(
                ctx,
                db,
                EventPublishRequest {
                    event_type: DomainEventType::SalesOrderDeleted,
                    aggregate_type: "SalesOrder".to_string(),
                    aggregate_id: id,
                    payload: serde_json::json!({ "sales_order_id": id, "doc_number": existing.doc_number }),
                    idempotency_key: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        filter: SalesOrderQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<SalesOrder>> {
        self.repo
            .query(
                db,
                &filter,
                &page,
                ctx.data_scope,
                ctx.operator_id,
                ctx.department_id,
            )
            .await
    }

    // -- P1 新增方法 --

    async fn cancel_line(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
        line_id: i64,
        req: CancelLineReq,
    ) -> Result<()> {
        // 1. 校验订单状态
        let order = self.repo.find_by_id(db, order_id).await?
            .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

        if order.status != SalesOrderStatus::Confirmed
            && order.status != SalesOrderStatus::PartiallyShipped
        {
            return Err(DomainError::business_rule(
                "Only Confirmed or PartiallyShipped orders can cancel lines"
            ));
        }

        // 2. 校验订单行
        let items = self.item_repo.find_by_order_id(db, order_id).await?;
        let item = items.iter().find(|i| i.id == line_id)
            .ok_or_else(|| DomainError::not_found("SalesOrderItem"))?;

        if item.line_status == SalesOrderLineStatus::Shipped {
            return Err(DomainError::business_rule("Cannot cancel a shipped line"));
        }
        if item.line_status == SalesOrderLineStatus::Cancelled {
            return Err(DomainError::business_rule("Line is already cancelled"));
        }
        if req.cancelled_qty > item.open_qty() {
            return Err(DomainError::business_rule(
                format!("Cancelled qty {} exceeds open qty {}", req.cancelled_qty, item.open_qty())
            ));
        }

        // 3. 更新 cancelled_qty
        let new_line_status = if item.open_qty() - req.cancelled_qty <= Decimal::ZERO {
            SalesOrderLineStatus::Cancelled
        } else {
            item.line_status
        };

        self.item_repo.cancel_line(
            db, line_id, req.cancelled_qty, new_line_status, item.version,
        ).await?;

        // 4. 释放预留（TODO: 当前 API 仅支持按 source_id 批量取消，暂跳过单行释放）
        // 预留在整个订单取消时统一释放

        // 5. 同步头状态
        self.recalc_header_status(ctx, db, order_id).await?;

        // 6. 审计
        savepoint(db, "sp_audit").await.ok();
        if let Err(e) = new_audit_log_service(self.pool.clone())
            .record(ctx, db, RecordAuditLogReq {
                entity_type: "SalesOrderItem",
                entity_id: line_id,
                action: AuditAction::Update,
                changes: Some(serde_json::json!({
                    "action": "cancel_line",
                    "cancelled_qty": req.cancelled_qty.to_string()
                })),
                context: None,
            })
            .await
        {
            tracing::warn!("audit record failed: {e}");
            rollback_savepoint(db, "sp_audit").await.ok();
        } else {
            release_savepoint(db, "sp_audit").await.ok();
        }

        Ok(())
    }

    async fn list_fulfillment_plan(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        query: FulfillmentPlanQuery,
    ) -> Result<Vec<FulfillmentPlanLine>> {
        if let Some(order_id) = query.order_id {
            FulfillmentPlanLineRepo::find_by_order_id(db, order_id).await
        } else {
            Ok(Vec::new())
        }
    }

    async fn recalc_header_status(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<SalesOrderStatus> {
        let items = self.item_repo.find_by_order_id(db, order_id).await?;
        let new_status = calc_header_status(&items);

        // 仅当状态变化时才更新
        let order = self.repo.find_by_id(db, order_id).await?
            .ok_or_else(|| DomainError::not_found("SalesOrder"))?;

        if order.status != new_status {
            // 状态机验证转换合法性
            new_state_machine_service(self.pool.clone())
                .transition(
                    &ServiceContext::system(), db,
                    "SalesOrderStatus", order_id,
                    new_status.as_str(), None,
                )
                .await?;

            self.repo.update_status(db, order_id, new_status).await?;
        }

        Ok(new_status)
    }

    async fn record_shipment(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
        lines: &[ShipmentLineQty],
    ) -> Result<SalesOrderStatus> {
        // 超发前置校验：本次发货量不得超过 SO 行剩余可发量（quantity - shipped - cancelled）。
        // 否则 chk_soi_open_qty_nonneg 约束会在 UPDATE 时炸成 500（Opaque Internal）；
        // 这里返明确业务错误。应对「同一 SO 被多张发货单超额占用」等数据不一致场景。
        let items = self.item_repo.find_by_order_id(db, order_id).await?;
        for line in lines {
            if let Some(it) = items.iter().find(|i| i.id == line.order_item_id) {
                let open = it.quantity - it.shipped_qty - it.cancelled_qty;
                if line.shipped_qty > open {
                    return Err(DomainError::business_rule(format!(
                        "销售订单行 #{} 发货 {} 超过剩余可发量 {}（总量 {}，已发 {}，已取消 {}）",
                        line.order_item_id, line.shipped_qty, open, it.quantity, it.shipped_qty, it.cancelled_qty
                    )));
                }
            }
        }
        // 累加各行已发数量（update_shipped_qty 为 += 语义）
        for line in lines {
            self.item_repo
                .update_shipped_qty(db, line.order_item_id, line.shipped_qty)
                .await?;
        }
        // 复用头状态重算（幂等：仅状态变化才走状态机 transition + update）
        self.recalc_header_status(ctx, db, order_id).await
    }

    async fn delivery_status(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<DeliveryStatus> {
        let items = self.item_repo.find_by_order_id(db, order_id).await?;
        if items.is_empty() {
            return Ok(DeliveryStatus::None);
        }
        let total: Decimal = items.iter().map(|i| i.quantity).sum();
        let shipped: Decimal = items.iter().map(|i| i.shipped_qty).sum();
        let status = if shipped <= Decimal::ZERO {
            DeliveryStatus::None
        } else if shipped >= total {
            DeliveryStatus::Full
        } else {
            DeliveryStatus::Partial
        };
        Ok(status)
    }

    async fn reconcile_fulfillment_status(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<u32> {
        let mismatched = DemandRepo::find_mismatched(db, order_id).await?;

        let mut count = 0u32;
        for (fp_id, _demand_id) in &mismatched {
            let fp = FulfillmentPlanLineRepo::find_by_order_line_id(db, *fp_id).await?;
            if let Some(line) = fp {
                if let Err(e) = FulfillmentPlanLineRepo::update_status(
                    db, line.id, FulfillmentLineStatus::Pending, line.version,
                ).await {
                    tracing::warn!("Reconcile failed for fp_line {}: {e}", line.id);
                } else {
                    count += 1;
                }
            }
        }

        // 同步订单头状态
        self.recalc_header_status(ctx, db, order_id).await?;

        Ok(count)
    }
}

// ---------------------------------------------------------------------------
// DemandServiceImpl
// ---------------------------------------------------------------------------

pub struct DemandServiceImpl {
    pool: PgPool,
}

impl DemandServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl DemandService for DemandServiceImpl {
    async fn create_from_order(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<Vec<i64>> {
        // 查询订单的履行计划行（有缺口的行）
        let fp_lines: Vec<_> = FulfillmentPlanLineRepo::find_by_order_id(db, order_id)
            .await?
            .into_iter()
            .filter(|l| l.shortage_qty > Decimal::ZERO && l.status == FulfillmentLineStatus::Pending)
            .collect();

        if fp_lines.is_empty() {
            return Ok(Vec::new());
        }

        let mut demand_ids = Vec::with_capacity(fp_lines.len());

        for line in &fp_lines {
            let input = DemandInput {
                demand_type: 1,  // SalesOrder
                source_type: DocumentType::SalesOrder as i16,
                source_id: order_id,
                source_line_id: line.order_line_id,
                product_id: line.product_id,
                acquire_channel: line.acquire_channel.as_i16(),
                required_qty: line.shortage_qty,
                required_date: line.required_date,
                priority: 5,
                cascade_from_product_id: None,
                remark: String::new(),
                operator_id: ctx.operator_id,
            };

            let demand_id = DemandRepo::create(&mut *db, &input).await?;
            demand_ids.push(demand_id);

            // 发布 DemandCreated 事件（精简 payload）
            savepoint(db, &format!("sp_demand_evt_{demand_id}")).await.ok();
            if let Err(e) = new_domain_event_bus(self.pool.clone())
                .publish(ctx, db, EventPublishRequest {
                    event_type: DomainEventType::DemandCreated,
                    aggregate_type: "Demand".to_string(),
                    aggregate_id: demand_id,
                    payload: serde_json::json!({
                        "order_id": order_id,
                        "product_id": line.product_id,
                        "acquire_channel": line.acquire_channel.as_i16(),
                    }),
                    idempotency_key: None,
                })
                .await
            {
                tracing::warn!("DemandCreated event publish failed for demand {demand_id}: {e}");
                rollback_savepoint(db, &format!("sp_demand_evt_{demand_id}")).await.ok();
            } else {
                release_savepoint(db, &format!("sp_demand_evt_{demand_id}")).await.ok();
            }
        }

        // ════════════════════════════════════════════════════════════════════
        // BOM 级联展开：自制缺货行的原材料自动生成采购需求
        //
        // 参考 Odoo _run_manufacture 递归 + mts_else_mto 库存检查
        // 参考 ERPNext projected_qty 公式（actual + ordered + planned - reserved）
        // 参考 Odoo _make_mo_get_domain 去重
        // ════════════════════════════════════════════════════════════════════
        let mut cascade_demands: Vec<DemandInput> = Vec::new();

        for line in &fp_lines {
            if line.acquire_channel != AcquireChannel::SelfProduced
                && line.acquire_channel != AcquireChannel::Legacy
            {
                continue;
            }

            let product = new_product_service(self.pool.clone())
                .get(ctx, db, line.product_id).await?;
            let product_code = product.product_code.clone();

            let bom_reqs = new_bom_query_service(self.pool.clone())
                .explode_for_procurement(ctx, db, &product_code, line.shortage_qty)
                .await?;

            if bom_reqs.is_empty() {
                continue;
            }

            // 批量查询 projected_qty（消除 N+1）
            let raw_pids: Vec<i64> = bom_reqs.iter().map(|r| r.product_id).collect();
            let projected_map = new_stock_ledger_service(self.pool.clone())
                .query_projected_qty_batch(ctx, db, &raw_pids, None)
                .await?;

            // 批量查询已有级联需求（消除 N+1）
            let existing_pids = DemandRepo::find_cascade_existing_batch(
                db, order_id, line.order_line_id, line.product_id,
            ).await?;

            for req in &bom_reqs {
                // 已有同源级联需求 -> 跳过
                if existing_pids.contains(&req.product_id) {
                    continue;
                }

                let projected = projected_map.get(&req.product_id)
                    .map(|p| p.projected)
                    .unwrap_or(Decimal::ZERO);

                let net_shortage = (req.required_qty - projected).max(Decimal::ZERO);
                if net_shortage <= Decimal::ZERO {
                    continue;
                }

                cascade_demands.push(DemandInput {
                    demand_type: 2,
                    source_type: DocumentType::SalesOrder as i16,
                    source_id: order_id,
                    source_line_id: line.order_line_id,
                    product_id: req.product_id,
                    acquire_channel: AcquireChannel::Purchased.as_i16(),
                    required_qty: net_shortage,
                    required_date: line.required_date,
                    priority: 5,
                    cascade_from_product_id: Some(line.product_id),
                    remark: format!(
                        "BOM展开: 成品{} 层{} 总需{} 预计可用{} 净缺{}",
                        line.product_id, req.bom_level, req.required_qty, projected, net_shortage
                    ),
                    operator_id: ctx.operator_id,
                });
            }
        }

        for input in &cascade_demands {
            let demand_id = DemandRepo::create(&mut *db, input).await?;
            demand_ids.push(demand_id);

            savepoint(db, &format!("sp_cascade_evt_{demand_id}")).await.ok();
            if let Err(e) = new_domain_event_bus(self.pool.clone())
                .publish(ctx, db, EventPublishRequest {
                    event_type: DomainEventType::DemandCreated,
                    aggregate_type: "Demand".to_string(),
                    aggregate_id: demand_id,
                    payload: serde_json::json!({
                        "order_id": order_id,
                        "product_id": input.product_id,
                        "acquire_channel": input.acquire_channel,
                        "cascade_from": input.cascade_from_product_id,
                    }),
                    idempotency_key: None,
                })
                .await
            {
                tracing::warn!("Cascade DemandCreated event failed for demand {demand_id}: {e}");
                rollback_savepoint(db, &format!("sp_cascade_evt_{demand_id}")).await.ok();
            } else {
                release_savepoint(db, &format!("sp_cascade_evt_{demand_id}")).await.ok();
            }
        }

        if !cascade_demands.is_empty() {
            tracing::info!(
                "Created {} BOM cascade demands for order {order_id}",
                cascade_demands.len()
            );
        }

        Ok(demand_ids)
    }

    async fn find_by_id(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<Demand> {
        DemandRepo::find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Demand"))
    }

    async fn list(
        &self,
        _ctx: &ServiceContext, _db: PgExecutor<'_>,
        _query: DemandQuery,
        _page: PageParams,
    ) -> Result<PaginatedResult<Demand>> {
        // TODO: 实现动态查询
        Ok(PaginatedResult::empty(_page.page, _page.page_size))
    }

    async fn confirm(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: ConfirmDemandReq,
    ) -> Result<()> {
        let demand = DemandRepo::find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Demand"))?;

        if demand.status != DemandStatus::Pending {
            return Err(DomainError::business_rule(
                "Only Pending demands can be confirmed"
            ));
        }

        DemandRepo::update_status(db, id, DemandStatus::Confirmed).await?;
        DemandRepo::update_target_doc(db, id, req.target_doc_type, req.target_doc_id).await?;

        // 发布 DemandConfirmed 事件
        savepoint(db, "sp_demand_confirm_evt").await.ok();
        if let Err(e) = new_domain_event_bus(self.pool.clone())
            .publish(ctx, db, EventPublishRequest {
                event_type: DomainEventType::DemandConfirmed,
                aggregate_type: "Demand".to_string(),
                aggregate_id: id,
                payload: serde_json::json!({
                    "order_id": demand.source_id,
                    "order_line_id": demand.source_line_id,
                    "product_id": demand.product_id,
                    "acquire_channel": demand.acquire_channel,
                    "target_doc_type": req.target_doc_type,
                    "target_doc_id": req.target_doc_id,
                }),
                idempotency_key: None,
            })
            .await
        {
            tracing::warn!("DemandConfirmed event publish failed: {e}");
            rollback_savepoint(db, "sp_demand_confirm_evt").await.ok();
        } else {
            release_savepoint(db, "sp_demand_confirm_evt").await.ok();
        }

        Ok(())
    }

    async fn reject(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let demand = DemandRepo::find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Demand"))?;

        if demand.status != DemandStatus::Pending && demand.status != DemandStatus::Confirmed {
            return Err(DomainError::business_rule(
                "Only Pending or Confirmed demands can be rejected"
            ));
        }

        DemandRepo::update_status(db, id, DemandStatus::Rejected).await?;

        // 发布 DemandRejected 事件
        savepoint(db, "sp_demand_reject_evt").await.ok();
        if let Err(e) = new_domain_event_bus(self.pool.clone())
            .publish(ctx, db, EventPublishRequest {
                event_type: DomainEventType::DemandRejected,
                aggregate_type: "Demand".to_string(),
                aggregate_id: id,
                payload: serde_json::json!({
                    "order_id": demand.source_id,
                    "order_line_id": demand.source_line_id,
                    "product_id": demand.product_id,
                }),
                idempotency_key: None,
            })
            .await
        {
            tracing::warn!("DemandRejected event publish failed: {e}");
            rollback_savepoint(db, "sp_demand_reject_evt").await.ok();
        } else {
            release_savepoint(db, "sp_demand_reject_evt").await.ok();
        }

        Ok(())
    }

    async fn fulfill(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let demand = DemandRepo::find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Demand"))?;

        if demand.status != DemandStatus::Confirmed && demand.status != DemandStatus::InProgress {
            return Err(DomainError::business_rule(
                "Only Confirmed or InProgress demands can be fulfilled"
            ));
        }

        DemandRepo::update_status(db, id, DemandStatus::Fulfilled).await?;
        Ok(())
    }

    async fn cancel(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let demand = DemandRepo::find_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("Demand"))?;

        if demand.status == DemandStatus::Fulfilled {
            return Err(DomainError::business_rule("Cannot cancel a fulfilled demand"));
        }

        sqlx::query("UPDATE demands SET deleted_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(db)
            .await?;
        Ok(())
    }

    async fn find_mismatched(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<Vec<(i64, i64)>> {
        DemandRepo::find_mismatched(db, order_id).await
    }

    async fn update_target_doc(
        &self,
        db: PgExecutor<'_>,
        id: i64,
        target_doc_type: i16,
        target_doc_id: i64,
    ) -> Result<()> {
        DemandRepo::update_target_doc(db, id, target_doc_type, target_doc_id).await
    }

    async fn find_by_source(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        source_type: i16,
        source_id: i64,
    ) -> Result<Vec<Demand>> {
        DemandRepo::find_by_source(db, source_type, source_id).await
    }
}

// ---------------------------------------------------------------------------
// 事件处理器 — 下游模块注册消费
// ---------------------------------------------------------------------------

/// 处理 DemandConfirmed 事件 — 更新履行计划行状态
pub async fn handle_demand_confirmed(
    _pool: PgPool,
    _ctx: &ServiceContext,
    db: PgExecutor<'_>,
    event: &crate::shared::event_bus::model::DomainEvent,
) -> Result<()> {
    let payload = &event.payload;
    let order_line_id: i64 = payload["order_line_id"]
        .as_i64()
        .ok_or_else(|| DomainError::validation("Missing order_line_id in DemandConfirmed payload"))?;
    let acquire_channel: i16 = payload["acquire_channel"]
        .as_i64()
        .unwrap_or(9) as i16;
    let target_doc_type: i16 = payload["target_doc_type"]
        .as_i64()
        .unwrap_or(0) as i16;
    let target_doc_id: i64 = payload["target_doc_id"]
        .as_i64()
        .unwrap_or(0);

    // 查找履行计划行
    let fp_line = FulfillmentPlanLineRepo::find_by_order_line_id(db, order_line_id).await?
        .ok_or_else(|| DomainError::not_found("FulfillmentPlanLine"))?;

    // 根据 acquire_channel 决定新状态
    let new_status = match acquire_channel {
        1 => FulfillmentLineStatus::Producing,    // SelfProduced
        2 => FulfillmentLineStatus::Purchasing,   // Purchased
        3 => FulfillmentLineStatus::Producing,    // Outsourced → Producing
        _ => FulfillmentLineStatus::Pending,
    };

    // 更新履行计划行
    FulfillmentPlanLineRepo::update_status(db, fp_line.id, new_status, fp_line.version).await?;
    FulfillmentPlanLineRepo::update_source_doc(db, fp_line.id, target_doc_type, target_doc_id).await?;

    // 更新订单行状态
    let order_item_status = match acquire_channel {
        1 => SalesOrderLineStatus::Producing,
        2 => SalesOrderLineStatus::Purchasing,
        3 => SalesOrderLineStatus::Producing,
        _ => SalesOrderLineStatus::Pending,
    };

    let item_repo = SalesOrderItemRepo;
    item_repo.batch_update_line_status(
        db,
        &[(fp_line.order_line_id, order_item_status, 1)],
    ).await?;

    tracing::info!("DemandConfirmed handled: fp_line {} → {:?}", fp_line.id, new_status);

    Ok(())
}

/// 处理 DemandRejected 事件 — 将履行计划行回退到 Pending
pub async fn handle_demand_rejected(
    _pool: PgPool,
    _ctx: &ServiceContext,
    db: PgExecutor<'_>,
    event: &crate::shared::event_bus::model::DomainEvent,
) -> Result<()> {
    let payload = &event.payload;
    let order_line_id: i64 = payload["order_line_id"]
        .as_i64()
        .ok_or_else(|| DomainError::validation("Missing order_line_id in DemandRejected payload"))?;

    let fp_line = FulfillmentPlanLineRepo::find_by_order_line_id(db, order_line_id).await?
        .ok_or_else(|| DomainError::not_found("FulfillmentPlanLine"))?;

    // 回退到 Pending
    FulfillmentPlanLineRepo::update_status(
        db, fp_line.id, FulfillmentLineStatus::Pending, fp_line.version,
    ).await?;

    // 回退订单行状态
    let item_repo = SalesOrderItemRepo;
    item_repo.batch_update_line_status(
        db,
        &[(fp_line.order_line_id, SalesOrderLineStatus::Pending, 1)],
    ).await?;

    tracing::info!("DemandRejected handled: fp_line {} → Pending", fp_line.id);

    Ok(())
}
