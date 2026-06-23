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
use crate::fms::ar_ap::repo::{ArApLedgerInsert, ArApLedgerRepo};
use crate::fms::enums::CounterpartyType;
use crate::sales::sales_return::repo::{SalesReturnItemRepo, SalesReturnRepo};
use crate::shared::enums::DocumentType;
use crate::shared::event_bus::model::DomainEvent;
use crate::shared::event_bus::registry::EventHandler;
use crate::shared::types::{DomainError, Result, ServiceContext};

/// 销售退货完成 Handler
///
/// 监听 `SalesReturnReceived` 事件：销售退货完成（`Completed`）后，写反向 AR 台账（`Credit`）
/// 冲减应收。幂等：同一退货单（`source_type = SalesReturn, source_id = return_id`）不重复立账。
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

        // 1. 幂等：同一退货单不重复写台账（事件重放 / 重复 complete 保护）
        let dup: Option<i64> = sqlx::query_scalar::<sqlx::Postgres, i64>(
            "SELECT id FROM ar_ap_ledger WHERE source_type = $1 AND source_id = $2 LIMIT 1",
        )
        .bind(DocumentType::SalesReturn)
        .bind(return_id)
        .fetch_optional(&mut *conn)
        .await?;

        if dup.is_some() {
            info!(return_id, "SalesReturn already has AR ledger entry, skipping");
            return Ok(());
        }

        // 2. 取退货单主表 + 明细
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

        // 3. 写反向 AR 台账（Credit 冲减发货时立的 Debit）
        let period = chrono::Utc::now().format("%Y-%m").to_string();
        let today = chrono::Local::now().date_naive();
        let desc = format!("销售退货冲减应收 {}", ret.doc_number);

        ArApLedgerRepo::insert(
            &mut *conn,
            &ArApLedgerInsert {
                party_type: CounterpartyType::Customer,
                party_id: ret.customer_id,
                source_type: DocumentType::SalesReturn,
                source_id: return_id,
                source_doc_no: &ret.doc_number,
                against_type: None,
                against_id: None,
                direction: LedgerDirection::Credit,
                amount: refund_amount,
                currency: "CNY",
                exchange_rate: Decimal::ONE,
                transaction_date: today,
                due_date: None,
                period: &period,
                description: &desc,
                operator_id: ctx.operator_id,
            },
        )
        .await?;

        info!(
            return_id,
            customer_id = ret.customer_id,
            amount = %refund_amount,
            "AR ledger Credit (sales return reversal) inserted"
        );

        Ok(())
    }

    fn name(&self) -> &str {
        "sales_return_received"
    }
}
