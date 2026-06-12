use chrono::Local;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;

use crate::master_data::customer::{new_customer_service, service::CustomerService};
use crate::master_data::product::{new_product_service, service::ProductService};
use crate::master_data::product::model::AcquireChannel;
use crate::sales::quotation::{new_quotation_service, service::QuotationService};
use crate::sales::sales_order::model::*;
use crate::sales::sales_order::repo::{FulfillmentPlanLineRepo, SalesOrderItemRepo, SalesOrderRepo, savepoint, release_savepoint, rollback_savepoint};
use crate::sales::sales_order::service::SalesOrderService;
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

    if all_settled && any_shipped {
        SalesOrderStatus::Shipped
    } else if any_shipped && any_open {
        SalesOrderStatus::PartiallyShipped
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
                        warehouse_id: 1,
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

        // 5. 执行预留
        savepoint(db, "sp_reserve").await.ok();
        let mut succeeded_reservations: std::collections::HashSet<i64> = std::collections::HashSet::new();
        match new_inventory_reservation_service(self.pool.clone())
            .reserve(ctx, db, reserve_requests.clone())
            .await
        {
            Ok(batch) => {
                let failed_indices: std::collections::HashSet<i32> = batch.failed_items.iter().map(|f| f.index).collect();
                for (idx, req) in reserve_requests.iter().enumerate() {
                    if !failed_indices.contains(&(idx as i32)) {
                        if let Some(line_id) = req.source_line_id {
                            succeeded_reservations.insert(line_id);
                        }
                    }
                }
                release_savepoint(db, "sp_reserve").await.ok();
            }
            Err(e) => {
                tracing::warn!("inventory reserve error: {e}");
                rollback_savepoint(db, "sp_reserve").await.ok();
            }
        }

        // 6. 为库存类行生成履行计划（根据预留结果决定状态）
        for item in &items {
            let ac = product_map.get(&item.product_id)
                .copied()
                .unwrap_or(AcquireChannel::Legacy);
            if ac == AcquireChannel::NonInventory {
                continue;
            }

            let fully_reserved = succeeded_reservations.contains(&item.id);
            let (fp_status, line_status, reserved_qty, shortage_qty) = if fully_reserved {
                (FulfillmentLineStatus::Allocated, SalesOrderLineStatus::Allocated, item.quantity, Decimal::ZERO)
            } else {
                (FulfillmentLineStatus::Pending, SalesOrderLineStatus::Pending, Decimal::ZERO, item.quantity)
            };

            fp_inputs.push(FulfillmentPlanLineInput {
                order_id: id,
                order_line_id: item.id,
                product_id: item.product_id,
                acquire_channel: ac,
                required_qty: item.quantity,
                reserved_qty,
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
            && existing.status != SalesOrderStatus::PartiallyShipped
        {
            return Err(DomainError::business_rule(
                "Only Draft, Confirmed or PartiallyShipped orders can be cancelled",
            ));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "SalesOrderStatus", id, "Cancelled", None)
            .await?;

        // 释放所有预留（Confirmed/PartiallyShipped 状态下才可能有预留）
        if existing.status == SalesOrderStatus::Confirmed
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
                &format!("Cancelled qty {} exceeds open qty {}", req.cancelled_qty, item.open_qty())
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

    async fn reconcile_fulfillment_status(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<u32> {
        // P1 简化实现：查询订单的履行计划行，检查是否有状态异常
        // 完整实现对账在 P2（demands 表就绪后）
        let _lines = FulfillmentPlanLineRepo::find_by_order_id(db, order_id).await?;
        // P2 会实现：JOIN demands 表检查不一致
        Ok(0)
    }
}
