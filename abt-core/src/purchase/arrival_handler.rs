//! 来料检验通过 Handler — 监听 ArrivalInspected 事件，回写 PO received_qty + 状态

use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use tracing::{info, warn};

use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, model::RecordAuditLogReq};
use crate::shared::enums::audit::AuditAction;
use crate::shared::document_link::{new_document_link_service, service::DocumentLinkService};
use crate::shared::enums::DocumentType;
use crate::shared::event_bus::model::DomainEvent;
use crate::shared::event_bus::registry::EventHandler;
use crate::shared::types::{DomainError, Result, ServiceContext};

use crate::purchase::enums::PurchaseOrderStatus;
use crate::purchase::order::repo::{PurchaseOrderItemRepo, PurchaseOrderRepo};
use crate::purchase::settings::repo::PurchaseSettingsRepo;
use crate::purchase::settings::model::PurchaseSettings;
use crate::fms::ar_ap::enums::LedgerDirection;
use crate::fms::ar_ap::repo::{ArApLedgerInsert, ArApLedgerRepo};
use crate::fms::enums::CounterpartyType;

/// 来料检验通过 Handler
///
/// 监听 `ArrivalInspected` 事件，回写关联 PO 的 received_qty 和状态：
/// 1. 重算每个 PO item 的 received_qty（SUM 所有关联来料通知的 accepted_qty）
/// 2. 判定 PO 状态：Confirmed → PartiallyReceived 或 Received
/// 3. 记录审计日志
pub struct ArrivalAcceptedHandler {
    pool: PgPool,
}

impl ArrivalAcceptedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventHandler for ArrivalAcceptedHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        let arrival_notice_id = event.payload["arrival_notice_id"]
            .as_i64()
            .ok_or_else(|| DomainError::Validation("arrival_notice_id missing in payload".into()))?;

        let ctx = ServiceContext::system();
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 1. 查 DocumentLink 找关联 PO
        let link_svc = new_document_link_service(self.pool.clone());
        let links = link_svc
            .find_linked(
                &ctx, &mut conn,
                DocumentType::ArrivalNotice,
                arrival_notice_id,
                1,
                20,
            )
            .await?;

        // 找到 Fulfills → PurchaseOrder 的链接
        let po_id = links.items.iter().find_map(|l| {
            if l.target_type == DocumentType::PurchaseOrder {
                Some(l.target_id)
            } else {
                None
            }
        });

        let po_id = match po_id {
            Some(id) => id,
            None => {
                // 手工来料通知，无关联 PO — 跳过
                info!(arrival_notice_id, "No linked PO, skipping PO update");
                return Ok(());
            }
        };

        // 2. 读 PO 当前状态
        let po = PurchaseOrderRepo::get_by_id(&mut conn, po_id)
            .await?
            .ok_or_else(|| DomainError::not_found(format!("PurchaseOrder #{po_id}")))?;

        // 3. 重算所有 PO items 的 received_qty（幂等：全量 SUM）
        let recompute = PurchaseOrderItemRepo::recompute_received_qty(&mut conn, po_id).await?;

        // 4. 批量更新 received_qty
        PurchaseOrderItemRepo::batch_update_received_qty(&mut conn, &recompute).await?;

        // 4.5 校验超收容差（设置行读取失败时回退到零容差默认值）
        let settings = PurchaseSettingsRepo::get(&mut conn).await
            .unwrap_or_else(|_| PurchaseSettings::default());

        let po_items_check = PurchaseOrderItemRepo::list_by_order_id(&mut conn, po_id).await?;
        for item in &po_items_check {
            let max_qty = item.quantity
                * (Decimal::ONE + settings.over_delivery_allowance_pct / Decimal::from(100));
            if item.received_qty > max_qty {
                return Err(DomainError::Validation(format!(
                    "订单行 {} 收货数量 {} 超过允许上限 {}（含 {}% 容差）",
                    item.line_no, item.received_qty, max_qty,
                    settings.over_delivery_allowance_pct
                )));
            }
        }

        // 5. 判定目标状态
        let po_items = PurchaseOrderItemRepo::list_by_order_id(&mut conn, po_id).await?;
        let all_received = po_items
            .iter()
            .all(|item| item.received_qty >= item.quantity);
        let any_received = po_items
            .iter()
            .any(|item| item.received_qty > Decimal::ZERO);

        let target_status = if all_received {
            PurchaseOrderStatus::Received
        } else if any_received {
            PurchaseOrderStatus::PartiallyReceived
        } else {
            // 不应该发生（检验通过至少有 accepted_qty > 0），防御性处理
            warn!(po_id, "ArrivalInspected handler: no items received, skipping status change");
            return Ok(());
        };

        // 6. 防重入：PO 已是 Received 则只更新 received_qty，不再转换状态
        if po.status == PurchaseOrderStatus::Received {
            info!(po_id, "PO already Received, skipping status transition");
        } else if po.status != target_status {
            // 状态转换（乐观锁）
            let affected = PurchaseOrderRepo::update_status(
                &mut conn,
                po_id,
                target_status,
                &po.updated_at,
            )
            .await?;

            if affected == 0 {
                warn!(po_id, "Optimistic lock conflict on PO status update, will retry");
                return Err(DomainError::ConcurrentConflict);
            }

            // 7. 审计日志
            let audit_svc = new_audit_log_service(self.pool.clone());
            audit_svc
                .record(
                    &ctx,
                    &mut conn,
                    RecordAuditLogReq {
                        entity_type: "PurchaseOrder",
                        entity_id: po_id,
                        action: AuditAction::Transition,
                        changes: Some(serde_json::json!({
                            "from": format!("{:?}", po.status),
                            "to": format!("{:?}", target_status),
                            "trigger": "ArrivalInspected",
                            "arrival_notice_id": arrival_notice_id,
                        })),
                        context: None,
                    },
                )
                .await?;

            info!(
                po_id,
                arrival_notice_id,
                from = ?po.status,
                to = ?target_status,
                "PO status updated by ArrivalInspected handler"
            );
        }

        // 业财一体：入库即立 AP 台账（直接 insert，不经发票实体）
        // 幂等：同一来料通知不重复立账
        let dup_ledger: Option<i64> = sqlx::query_scalar::<sqlx::Postgres, i64>(
            "SELECT id FROM ar_ap_ledger WHERE source_type = $1 AND source_id = $2 LIMIT 1",
        )
        .bind(DocumentType::ArrivalNotice)
        .bind(arrival_notice_id)
        .fetch_optional(&mut *conn)
        .await?;

        if dup_ledger.is_none() {
            // 来料明细（accepted_qty）
            let arrival_items: Vec<(i64, Decimal)> = sqlx::query_as::<sqlx::Postgres, (i64, Decimal)>(
                "SELECT product_id, accepted_qty FROM arrival_notice_items WHERE notice_id = $1 AND accepted_qty > 0",
            )
            .bind(arrival_notice_id)
            .fetch_all(&mut *conn)
            .await?;

            // 应付金额 = Σ accepted_qty × PO unit_price（po_items 已在上方查询）
            let ap_amount: Decimal = arrival_items
                .iter()
                .filter_map(|(pid, qty)| {
                    po_items
                        .iter()
                        .find(|p| p.product_id == *pid)
                        .map(|p| *qty * p.unit_price)
                })
                .sum();

            if ap_amount > Decimal::ZERO {
                let period = chrono::Utc::now().format("%Y-%m").to_string();
                let today = chrono::Local::now().date_naive();
                let doc_no = format!("AN-{}", arrival_notice_id);
                let desc = format!("采购入库 {}", doc_no);

                let _ = ArApLedgerRepo::insert(
                    &mut *conn,
                    &ArApLedgerInsert {
                        party_type: CounterpartyType::Supplier,
                        party_id: po.supplier_id,
                        source_type: DocumentType::ArrivalNotice,
                        source_id: arrival_notice_id,
                        source_doc_no: &doc_no,
                        against_type: None,
                        against_id: None,
                        direction: LedgerDirection::Credit,
                        amount: ap_amount,
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
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "arrival_accepted"
    }
}
