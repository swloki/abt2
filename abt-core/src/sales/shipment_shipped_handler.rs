//! 销售发货完成 Handler — 监听 `ShipmentShipped` 事件，立 AR 台账 + COGS。
//!
//! 业务背景：销售发货 `ShippingRequest` 在 `ship()` 时发布 `ShipmentShipped` 事件
//!（`ship()` 只做仓库职责：扣库存 + 释放预留 + 回写订单 + 发事件）。
//! 本 handler 消费该事件，完成财务职责：立正向 AR 台账（`Debit`）+ 结转 COGS。
//!
//! 全程经 Service trait（ShippingRequestService / SalesOrderService / CustomerService /
//! ArApService / CostEntryService），**禁止跨域 repo 直访**（对齐 Issue #93 职责归属重构）。
//! 与 `SalesReturnReceivedHandler`（#86，反向冲减）对称。

use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use tracing::{info, warn};

use crate::fms::ar_ap::enums::LedgerDirection;
use crate::fms::ar_ap::model::PostLedgerEntryReq;
use crate::fms::ar_ap::{new_ar_ap_service, service::ArApService};
use crate::fms::enums::CounterpartyType;
use crate::master_data::customer::{new_customer_service, service::CustomerService};
use crate::sales::sales_order::{new_sales_order_service, service::SalesOrderService};
use crate::wms::outbound::{new_shipping_request_service, service::ShippingRequestService};
use crate::shared::cost_entry::model::EntryRequest;
use crate::shared::cost_entry::{new_cost_entry_service, service::CostEntryService};
use crate::shared::enums::DocumentType;
use crate::shared::enums::cost::{CostEntityType, CostType};
use crate::shared::event_bus::model::DomainEvent;
use crate::shared::event_bus::registry::EventHandler;
use crate::shared::types::{DomainError, Result, ServiceContext};

/// 销售发货完成 Handler
///
/// 监听 `ShipmentShipped` 事件：发货出库后立 AR 台账（Debit）+ COGS。
/// 数据全部经 trait 取得，立账经 `ArApService::post_entry`（幂等）。
pub struct ShipmentShippedHandler {
    pool: PgPool,
}

impl ShipmentShippedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventHandler for ShipmentShippedHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        let id = event.aggregate_id; // shipping_request_id

        // 事件 payload 携带的关联信息（由 ship() 发布时填入）
        let order_id = event
            .payload
            .get("order_id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| DomainError::Validation("ShipmentShipped 事件缺少 order_id".into()))?;
        let customer_id = event
            .payload
            .get("customer_id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| DomainError::Validation("ShipmentShipped 事件缺少 customer_id".into()))?;
        let doc_number = event
            .payload
            .get("doc_number")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let ctx = ServiceContext::system();
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 经 trait 取发货明细 / 订单明细 / 客户（禁止跨域 repo 直访）
        let shipping_items = new_shipping_request_service(self.pool.clone())
            .list_items(&ctx, &mut conn, id)
            .await?;
        let order_items = new_sales_order_service(self.pool.clone())
            .list_items(&ctx, &mut conn, order_id)
            .await?;
        let customer = new_customer_service(self.pool.clone())
            .get(&ctx, &mut conn, customer_id)
            .await?;

        let period = chrono::Utc::now().format("%Y-%m").to_string();
        let today = chrono::Local::now().date_naive();

        // COGS = Σ 发货量 × 订单行 unit_cost（经 shared.cost_entry，Atomic 双层记账）
        let mut cost_entries = Vec::with_capacity(shipping_items.len());
        for ship_item in &shipping_items {
            let unit_cost = order_items
                .iter()
                .find(|oi| oi.id == ship_item.order_item_id)
                .map(|oi| oi.unit_cost)
                .unwrap_or(Decimal::ZERO);
            let cogs = ship_item.requested_qty * unit_cost;
            if cogs > Decimal::ZERO {
                cost_entries.push(EntryRequest {
                    entity_type: CostEntityType::SalesOrder,
                    entity_id: order_id,
                    cost_type: CostType::Material,
                    debit_amount: cogs,
                    credit_amount: Decimal::ZERO,
                    cost_center: None,
                    profit_center: None,
                    period: period.clone(),
                    source_type: DocumentType::ShippingRequest,
                    source_id: id,
                });
            }
        }
        if !cost_entries.is_empty() {
            new_cost_entry_service(self.pool.clone())
                .create_entries(&ctx, &mut conn, cost_entries)
                .await?;
        }

        // AR 台账 = Σ 发货量 × 订单行 unit_price（经 ArApService::post_entry，幂等 ON CONFLICT）
        let ar_amount: Decimal = shipping_items
            .iter()
            .filter_map(|si| {
                order_items
                    .iter()
                    .find(|oi| oi.id == si.order_item_id)
                    .map(|oi| si.requested_qty * oi.unit_price)
            })
            .sum();

        if ar_amount > Decimal::ZERO {
            let due_days = crate::fms::ar_ap::payment_terms::parse_payment_terms_days(
                customer.payment_terms.as_deref(),
            );
            let due_date = today + chrono::Duration::days(due_days);
            let currency = customer
                .currency
                .as_deref()
                .filter(|c| !c.is_empty())
                .unwrap_or("CNY")
                .to_string();
            let description = format!("销售发货 {doc_number}");

            let inserted = new_ar_ap_service(self.pool.clone())
                .post_entry(
                    &ctx,
                    &mut conn,
                    PostLedgerEntryReq {
                        party_type: CounterpartyType::Customer,
                        party_id: customer_id,
                        source_type: DocumentType::ShippingRequest,
                        source_id: id,
                        source_doc_no: doc_number.clone(),
                        direction: LedgerDirection::Debit,
                        amount: ar_amount,
                        currency,
                        exchange_rate: Decimal::ONE,
                        transaction_date: today,
                        due_date: Some(due_date),
                        period: period.clone(),
                        description,
                    },
                )
                .await?;

            if inserted.is_some() {
                info!(
                    shipping_request_id = id,
                    customer_id, amount = %ar_amount, "AR ledger Debit (sales shipment) inserted"
                );
            } else {
                info!(shipping_request_id = id, "Shipment already has AR ledger entry, skipping");
            }
        } else {
            warn!(shipping_request_id = id, "Shipment AR amount <= 0, skipping AR ledger");
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "shipment_shipped"
    }
}
