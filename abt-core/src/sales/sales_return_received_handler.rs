//! 销售退货完成 Handler — 监听 `SalesReturnReceived` 事件，写反向 AR 台账冲减应收。
//!
//! 业务背景：销售退货 `SalesReturn` 在 `complete()` 时（`sales/sales_return/implt.rs`）
//! 发布 `SalesReturnReceived` 事件。本 handler 消费该事件，写一笔反向 AR 台账
//!（`Credit`，应收减少），冲减发货时由 `ShippingRequest::ship()` 立的 `Debit`。
//! 与采购侧 `PurchaseReturnSettledHandler`（#85）完全对称。

use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use tracing::{info, warn};

use crate::fms::ar_ap::enums::LedgerDirection;
use crate::fms::ar_ap::repo::ArApLedgerRepo;
use crate::fms::enums::CounterpartyType;
use crate::sales::sales_return::repo::{SalesReturnItemRepo, SalesReturnRepo};
use crate::shared::enums::DocumentType;
use crate::shared::event_bus::model::DomainEvent;
use crate::shared::event_bus::registry::EventHandler;
use crate::shared::types::{DomainError, Result, ServiceContext};

/// 销售退货完成 Handler
///
/// 监听 `SalesReturnReceived` 事件：销售退货完成（`Completed`）后，写反向 AR 台账（`Credit`）
/// 冲减应收。幂等 + 往来方币种由 `ArApLedgerRepo::insert_reversal_if_absent` 统一处理。
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

        // 1. 取退货单主表 + 明细（域特定）
        let repo = SalesReturnRepo;
        let ret = repo
            .find_by_id(&mut *conn, return_id)
            .await?
            .ok_or_else(|| DomainError::not_found(format!("SalesReturn #{return_id}")))?;
        let items = SalesReturnItemRepo.find_by_return_id(&mut *conn, return_id).await?;

        // 退货冲减金额 = Σ 明细 amount
        let refund_amount: Decimal = items.iter().map(|i| i.amount).sum();
        if refund_amount <= Decimal::ZERO {
            warn!(return_id, "SalesReturn refund amount <= 0, skipping AR reversal");
            return Ok(());
        }

        // 2. 公共：幂等 + 按往来方币种 insert 反向 AR 台账（Credit 冲减 Debit）
        let desc = format!("销售退货冲减应收 {}", ret.doc_number);
        let inserted = ArApLedgerRepo::insert_reversal_if_absent(
            &mut *conn,
            CounterpartyType::Customer,
            ret.customer_id,
            DocumentType::SalesReturn,
            return_id,
            &ret.doc_number,
            LedgerDirection::Credit,
            refund_amount,
            &desc,
            ctx.operator_id,
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
