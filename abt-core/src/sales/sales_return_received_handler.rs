//! 销售退货完成 Handler — 监听 `SalesReturnReceived` 事件，写反向 AR 台账冲减应收。
//!
//! 业务背景：销售退货 `SalesReturn` 在 `complete()` 时（`sales/sales_return/implt.rs`）
//! 发布 `SalesReturnReceived` 事件。本 handler 消费该事件，写一笔反向 AR 台账
//!（`Credit`，应收减少），冲减发货时由 `ShipmentShippedHandler` 立的 `Debit`。
//! 与采购侧 `PurchaseReturnSettledHandler`（#85）完全对称。
//!
//! 全程经 Service trait（`CustomerService` / `ArApService`），**禁止跨域 repo 直访**
//!（Issue #93：原 `ArApLedgerRepo::insert_reversal_if_absent` 直访 + 内部 `fetch_party_currency`
//! 直访 customers，改为经 `ArApService::post_entry` + `CustomerService`）。

use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use tracing::{info, warn};

use crate::fms::ar_ap::enums::LedgerDirection;
use crate::fms::ar_ap::model::PostLedgerEntryReq;
use crate::fms::ar_ap::{new_ar_ap_service, service::ArApService};
use crate::fms::enums::CounterpartyType;
use crate::master_data::customer::{new_customer_service, service::CustomerService};
use crate::sales::sales_return::repo::{SalesReturnItemRepo, SalesReturnRepo};
use crate::shared::enums::DocumentType;
use crate::shared::event_bus::model::DomainEvent;
use crate::shared::event_bus::registry::EventHandler;
use crate::shared::types::{DomainError, Result, ServiceContext};

/// 销售退货完成 Handler
///
/// 监听 `SalesReturnReceived` 事件：销售退货完成（`Completed`）后，写反向 AR 台账（`Credit`）
/// 冲减应收。经 `ArApService::post_entry`（幂等）立账，客户币种经 `CustomerService` 取得。
pub struct SalesReturnReceivedHandler {
    pool: PgPool,
}

impl SalesReturnReceivedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventHandler for SalesReturnReceivedHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        let return_id = event.aggregate_id;

        let ctx = ServiceContext::system();
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 1. 取退货单主表 + 明细（sales 同域 repo）
        let ret = SalesReturnRepo
            .find_by_id(&mut conn, return_id)
            .await?
            .ok_or_else(|| DomainError::not_found(format!("SalesReturn #{return_id}")))?;
        let items = SalesReturnItemRepo
            .find_by_return_id(&mut conn, return_id)
            .await?;

        // 退货冲减金额 = Σ 明细 amount
        let refund_amount: Decimal = items.iter().map(|i| i.amount).sum();
        if refund_amount <= Decimal::ZERO {
            warn!(return_id, "SalesReturn refund amount <= 0, skipping AR reversal");
            return Ok(());
        }

        // 2. 经 trait 取客户币种（替代原 insert_reversal_if_absent 内部的 fetch_party_currency 直访）
        let customer = new_customer_service(self.pool.clone())
            .get(&ctx, &mut conn, ret.customer_id)
            .await?;
        let currency = customer
            .currency
            .as_deref()
            .filter(|c| !c.is_empty())
            .unwrap_or("CNY")
            .to_string();
        let period = chrono::Utc::now().format("%Y-%m").to_string();
        let today = chrono::Local::now().date_naive();
        let desc = format!("销售退货冲减应收 {}", ret.doc_number);

        // 3. 经 ArApService::post_entry 立反向 AR 台账（Credit 冲减 Debit，幂等）
        let inserted = new_ar_ap_service(self.pool.clone())
            .post_entry(
                &ctx,
                &mut conn,
                PostLedgerEntryReq {
                    party_type: CounterpartyType::Customer,
                    party_id: ret.customer_id,
                    source_type: DocumentType::SalesReturn,
                    source_id: return_id,
                    source_doc_no: ret.doc_number.clone(),
                    direction: LedgerDirection::Credit,
                    amount: refund_amount,
                    currency,
                    exchange_rate: Decimal::ONE,
                    transaction_date: today,
                    due_date: None,
                    period,
                    description: desc,
                },
            )
            .await?;

        if inserted.is_some() {
            info!(
                return_id,
                customer_id = ret.customer_id,
                amount = %refund_amount,
                "AR ledger Credit (sales return reversal) inserted"
            );
        } else {
            info!(return_id, "SalesReturn already has AR ledger entry, skipping");
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "sales_return_received"
    }
}
