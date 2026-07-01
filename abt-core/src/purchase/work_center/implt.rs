use async_trait::async_trait;
use sqlx::PgPool;

use super::model::{
    DraftReconAggregate, PendingPaymentAggregate, PoApSummary, PoHubSummary, PoProgress,
    PoSourceChain, PurchaseWorkCenterSummary, ReturnHubSummary, SettlementHubSummary,
    SettlementReconType, ThreeWayMatchSummary,
};
use super::repo::PurchaseWorkCenterRepo;
use super::service::PurchaseWorkCenterService;
use crate::purchase::demand_handler::{new_purchase_demand_service, MaterialAggQuery, PurchaseDemandService};
use crate::purchase::enums::{
    MiscRequestStatus, PaymentMethod, PaymentStatus, PurchaseOrderStatus, PurchaseReconStatus,
    PurchaseReturnStatus,
};
use crate::purchase::misc_request::model::MiscRequestQuery;
use crate::purchase::misc_request::{new_misc_request_service, MiscellaneousRequestService};
use crate::purchase::order::model::{PurchaseOrder, PurchaseOrderItem, PurchaseOrderQuery};
use crate::purchase::order::{new_purchase_order_service, PurchaseOrderService};
use crate::purchase::payment::model::PaymentRequestQuery;
use crate::purchase::payment::{new_payment_request_service, PaymentRequestService};
use crate::purchase::reconciliation::model::PurchaseReconciliationQuery;
use crate::purchase::reconciliation::{new_purchase_reconciliation_service, PurchaseReconciliationService};
use crate::purchase::return_order::model::PurchaseReturnQuery;
use crate::purchase::return_order::{new_purchase_return_service, PurchaseReturnService};
use crate::shared::types::context::ServiceContext;
use crate::shared::types::pagination::PaginatedResult;
use crate::shared::types::{PageParams, PgExecutor, Result};
use rust_decimal::Decimal;

use crate::fms::ar_ap::{new_ar_ap_service, ArApLedgerFilter, ArApService};
use crate::fms::enums::CounterpartyType;
use crate::master_data::supplier::{new_supplier_service, SupplierService};
use crate::shared::document_link::{new_document_link_service, DocumentLinkService};
use crate::shared::enums::DocumentType;

/// 临期窗口（天）。
const SOON_WINDOW_DAYS: i64 = 7;

pub struct PurchaseWorkCenterServiceImpl {
    pool: PgPool,
}

impl PurchaseWorkCenterServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// PO 上游销售订单链（经 DocumentLink 反查 PO→SO；查询失败返回空，best-effort）。
    async fn source_chain(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
    ) -> PoSourceChain {
        let svc = new_document_link_service(self.pool.clone());
        let Ok(linked) = svc
            .find_linked(ctx, db, DocumentType::PurchaseOrder, order_id, 1, 20)
            .await
        else {
            return PoSourceChain::default();
        };
        let docs: Vec<String> = linked
            .items
            .into_iter()
            .filter(|l| l.target_type == DocumentType::SalesOrder)
            .map(|l| {
                if l.path.is_empty() {
                    format!("SO #{}", l.target_id)
                } else {
                    l.path
                }
            })
            .collect();
        PoSourceChain {
            sales_order_docs: docs,
        }
    }

    /// PO 应付台账立账摘要（经 ArApService 按 supplier + doc_number 反查，post-filter 精确到本 PO；best-effort）。
    async fn ap_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order: &PurchaseOrder,
    ) -> PoApSummary {
        let svc = new_ar_ap_service(self.pool.clone());
        let filter = ArApLedgerFilter {
            party_type: Some(CounterpartyType::Supplier),
            party_id: Some(order.supplier_id),
            doc_no: Some(order.doc_number.clone()),
            ..Default::default()
        };
        let rows = svc
            .list_ledger(ctx, db, filter, PageParams::new(1, 50))
            .await
            .map(|r| r.items)
            .unwrap_or_default();
        let (ap, paid) = rows
            .into_iter()
            .filter(|r| r.source_type == DocumentType::PurchaseOrder && r.source_id == order.id)
            .fold((Decimal::ZERO, Decimal::ZERO), |(ap, paid), r| {
                (ap + r.amount, paid + r.amount_applied)
            });
        PoApSummary {
            ap_amount: ap,
            paid_amount: paid,
        }
    }

    /// 入库侧三单匹配：对账明细 received_qty ≤ PO 收货量，且金额 = 净量×单价（容差 0.5%）。
    /// 任一不匹配追加 difference 并返回 false；查不到对账明细则放行（best-effort）。
    async fn receipt_match(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        reconciliation_id: i64,
        differences: &mut Vec<String>,
    ) -> bool {
        let recon_svc = new_purchase_reconciliation_service(self.pool.clone());
        let po_svc = new_purchase_order_service(self.pool.clone());
        let recon_items = match recon_svc.list_items(ctx, db, reconciliation_id).await {
            Ok(v) => v,
            Err(_) => return true,
        };
        if recon_items.is_empty() {
            return true;
        }

        // 按 order_id 预载 PO 明细，避免逐行 list_items 产生 N+1
        use std::collections::{HashMap, HashSet};
        let order_ids: HashSet<i64> = recon_items.iter().map(|i| i.order_id).collect();
        let mut po_item_map: HashMap<i64, PurchaseOrderItem> = HashMap::new();
        for oid in order_ids {
            if let Ok(items) = po_svc.list_items(ctx, db, oid).await {
                for p in items {
                    po_item_map.insert(p.id, p);
                }
            }
        }

        let mut ok = true;
        for item in &recon_items {
            let Some(po_item) = po_item_map.get(&item.order_item_id) else {
                differences.push(format!("订单行 {} 不存在", item.order_item_id));
                ok = false;
                continue;
            };
            if item.received_qty > po_item.received_qty {
                differences.push(format!(
                    "对账数量 {} 超过收货数量 {}",
                    item.received_qty, po_item.received_qty
                ));
                ok = false;
                continue;
            }
            let net_qty = item.received_qty - item.returned_qty;
            let expected = net_qty * item.unit_price;
            let tolerance = expected * TOLERANCE_RATE;
            if (item.amount - expected).abs() > tolerance {
                differences.push(format!(
                    "对账金额 {} 与净量×单价 {} 不匹配（容差 0.5%）",
                    item.amount, expected
                ));
                ok = false;
            }
        }
        ok
    }

    /// 供应商名（best-effort：缺失则空串，前端可兜底）。
    async fn supplier_name(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        supplier_id: i64,
    ) -> String {
        new_supplier_service(self.pool.clone())
            .get(ctx, db, supplier_id)
            .await
            .map(|s| s.name)
            .unwrap_or_default()
    }

    /// 供应商当前应付未清余额（ArApService::get_party_balance.total_ap；best-effort）。
    async fn ap_outstanding(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        supplier_id: i64,
    ) -> Decimal {
        new_ar_ap_service(self.pool.clone())
            .get_party_balance(ctx, db, CounterpartyType::Supplier, supplier_id)
            .await
            .map(|b| b.total_ap)
            .unwrap_or(Decimal::ZERO)
    }

    /// 该供应商待结算（Shipped）退货的笔数 + 金额合计（best-effort）。
    async fn pending_returns(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        supplier_id: i64,
    ) -> (u64, Decimal) {
        // SQL COUNT+SUM，替代拉 200 条算金额（>200 会遗漏）
        match PurchaseWorkCenterRepo::return_stats(
            db,
            supplier_id,
            PurchaseReturnStatus::Shipped.as_i16(),
        )
        .await
        {
            Ok((cnt, amt)) => (cnt, amt),
            Err(_) => (0, Decimal::ZERO),
        }
    }
}

/// 三单匹配金额容差 ±0.5%（与 purchase::payment 模块同口径）。
const TOLERANCE_RATE: Decimal = Decimal::from_parts(5, 0, 0, false, 3); // 0.005

/// 金额是否在容差范围内（±TOLERANCE_RATE）。零基准则要求被比较数也为零。
fn within_tolerance(a: Decimal, b: Decimal) -> bool {
    if b == Decimal::ZERO {
        return a == Decimal::ZERO;
    }
    let diff = (a - b).abs();
    let threshold = b * TOLERANCE_RATE;
    diff <= threshold
}

/// 聚合 PO 明细的收货进度（纯计算）。
fn po_progress(items: &[PurchaseOrderItem]) -> PoProgress {
    let ordered = items.iter().map(|i| i.quantity).sum::<Decimal>();
    let received = items.iter().map(|i| i.received_qty).sum::<Decimal>();
    let returned = items.iter().map(|i| i.returned_qty).sum::<Decimal>();
    let inspected = items.iter().map(|i| i.inspected_qty).sum::<Decimal>();
    let received_pct = if ordered > Decimal::ZERO {
        (received / ordered * Decimal::from(100)).min(Decimal::from(100))
    } else {
        Decimal::ZERO
    };
    PoProgress {
        ordered_qty: ordered,
        received_qty: received,
        returned_qty: returned,
        inspected_qty: inspected,
        received_pct,
        item_count: items.len(),
    }
}

/// 付款方式中文标签。
fn payment_method_label(m: PaymentMethod) -> &'static str {
    match m {
        PaymentMethod::BankTransfer => "银行转账",
        PaymentMethod::Cash => "现金",
        PaymentMethod::Note => "票据",
    }
}

/// 采购订单状态中文标签。
fn po_status_label(s: PurchaseOrderStatus) -> &'static str {
    match s {
        PurchaseOrderStatus::Draft => "草稿",
        PurchaseOrderStatus::PendingApproval => "待审批",
        PurchaseOrderStatus::Confirmed => "待收货",
        PurchaseOrderStatus::PartiallyReceived => "部分收货",
        PurchaseOrderStatus::Received => "已收货",
        PurchaseOrderStatus::Closed => "已关闭",
        PurchaseOrderStatus::Cancelled => "已取消",
    }
}

/// 采购退货结算状态文案（基于状态推导）。
fn return_settlement_hint(s: PurchaseReturnStatus) -> &'static str {
    match s {
        PurchaseReturnStatus::Draft => "草稿，确认后发出",
        PurchaseReturnStatus::Confirmed => "待发货，发货后转 Shipped",
        PurchaseReturnStatus::Shipped => "已发出，待供应商确认；对账时自动 Settled 并冲减应付",
        PurchaseReturnStatus::Settled => "已结算（对账已冲减应付）",
        PurchaseReturnStatus::Cancelled => "已取消",
    }
}

/// 单状态计数：查询失败（如依赖表未建）不连累整个 summary，log warn 后记 0。
/// 作业中心是聚合看板，容错保证部分状态可用时仍展示其余（同 MES work_center）。
async fn cnt<T>(label: &'static str, f: impl std::future::Future<Output = Result<PaginatedResult<T>>>) -> u64 {
    match f.await {
        Ok(r) => r.total,
        Err(e) => {
            tracing::warn!(label, error = %e, "purchase work_center count failed, recorded as 0");
            0
        }
    }
}

#[async_trait]
impl PurchaseWorkCenterService for PurchaseWorkCenterServiceImpl {
    async fn summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<PurchaseWorkCenterSummary> {
        let demand_svc = new_purchase_demand_service(self.pool.clone());
        let misc_svc = new_misc_request_service(self.pool.clone());
        let po_svc = new_purchase_order_service(self.pool.clone());
        let recon_svc = new_purchase_reconciliation_service(self.pool.clone());
        let pay_svc = new_payment_request_service(self.pool.clone());
        let ret_svc = new_purchase_return_service(self.pool.clone());

        let one = PageParams::new(1, 1);

        let pending_demand = cnt(
            "demand",
            demand_svc.list_material_aggregated(
                ctx,
                db,
                MaterialAggQuery::default(),
                one.clone(),
            ),
        )
        .await;

        let pending_misc = cnt(
            "misc",
            misc_svc.list(
                ctx,
                db,
                MiscRequestQuery {
                    status: Some(MiscRequestStatus::Draft),
                    ..Default::default()
                },
                one.clone(),
            ),
        )
        .await;

        let po_pending_approval = cnt(
            "po_approval",
            po_svc.list(
                ctx,
                db,
                PurchaseOrderQuery {
                    status: Some(PurchaseOrderStatus::PendingApproval),
                    ..Default::default()
                },
                one.clone(),
            ),
        )
        .await;

        let po_pending_receive = cnt(
            "po_confirmed",
            po_svc.list(
                ctx,
                db,
                PurchaseOrderQuery {
                    status: Some(PurchaseOrderStatus::Confirmed),
                    ..Default::default()
                },
                one.clone(),
            ),
        )
        .await;

        let po_partial = cnt(
            "po_partial",
            po_svc.list(
                ctx,
                db,
                PurchaseOrderQuery {
                    status: Some(PurchaseOrderStatus::PartiallyReceived),
                    ..Default::default()
                },
                one.clone(),
            ),
        )
        .await;

        let recon_draft = cnt(
            "recon_draft",
            recon_svc.list(
                ctx,
                db,
                PurchaseReconciliationQuery {
                    status: Some(PurchaseReconStatus::Draft),
                    ..Default::default()
                },
                one.clone(),
            ),
        )
        .await;

        let payment_pending_approval = cnt(
            "payment_draft",
            pay_svc.list(
                ctx,
                db,
                PaymentRequestQuery {
                    status: Some(PaymentStatus::Draft),
                    ..Default::default()
                },
                one.clone(),
            ),
        )
        .await;

        let return_pending_ship = cnt(
            "return_confirmed",
            ret_svc.list(
                ctx,
                db,
                PurchaseReturnQuery {
                    status: Some(PurchaseReturnStatus::Confirmed),
                    ..Default::default()
                },
                one.clone(),
            ),
        )
        .await;

        let return_shipped = cnt(
            "return_shipped",
            ret_svc.list(
                ctx,
                db,
                PurchaseReturnQuery {
                    status: Some(PurchaseReturnStatus::Shipped),
                    ..Default::default()
                },
                one.clone(),
            ),
        )
        .await;

        // 逾期 / 临期：SQL COUNT FILTER（待收货 PO = Confirmed + PartiallyReceived），
        // 替代原「拉 RECEIVING_SCAN_SIZE × 2 状态到内存按日期 filter」。查询失败按 0 容错。
        let today = chrono::Utc::now().date_naive();
        let soon_limit = today
            .checked_add_days(chrono::Days::new(SOON_WINDOW_DAYS as u64))
            .unwrap_or(today);
        let (overdue_count, soon_count) =
            PurchaseWorkCenterRepo::count_po_overdue_soon(db, today, soon_limit)
                .await
                .unwrap_or((0, 0));

        Ok(PurchaseWorkCenterSummary {
            pending_demand,
            pending_misc,
            po_pending_approval,
            po_pending_receive,
            po_partial,
            recon_draft,
            payment_pending_approval,
            return_pending_ship,
            return_shipped,
            overdue_count,
            soon_count,
        })
    }

    async fn get_po_hub_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<PoHubSummary> {
        let po_svc = new_purchase_order_service(self.pool.clone());
        let order = po_svc.get(ctx, db, order_id).await?;

        // 供应商名（best-effort：缺失则空串，前端可兜底）
        let supplier_name = new_supplier_service(self.pool.clone())
            .get(ctx, db, order.supplier_id)
            .await
            .map(|s| s.name)
            .unwrap_or_default();

        // 收货进度（聚合明细）
        let items = po_svc.list_items(ctx, db, order_id).await?;
        let progress = po_progress(&items);

        // 来源链 + 应付台账（均 best-effort，子查询失败不连累整行）
        let source_chain = self.source_chain(ctx, db, order_id).await;
        let ap_summary = self.ap_summary(ctx, db, &order).await;

        Ok(PoHubSummary {
            order,
            supplier_name,
            progress,
            source_chain,
            ap_summary,
        })
    }

    async fn check_three_way_match(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        payment_id: i64,
    ) -> Result<ThreeWayMatchSummary> {
        let pay_svc = new_payment_request_service(self.pool.clone());
        let payment = pay_svc.get(ctx, db, payment_id).await?;
        let mut differences = Vec::new();

        // PO / 对账侧：付款金额 vs 对账确认金额；入库侧：recon_items vs po_items
        let (po_matched, receipt_matched) = if let Some(recon_id) = payment.reconciliation_id {
            let recon_svc = new_purchase_reconciliation_service(self.pool.clone());
            let po_m = match recon_svc.get(ctx, db, recon_id).await {
                Ok(recon) => {
                    let m = within_tolerance(payment.amount, recon.confirmed_amount);
                    if !m {
                        differences.push(format!(
                            "付款金额 {} 与对账确认金额 {} 偏差超过容差 ±0.5%",
                            payment.amount, recon.confirmed_amount
                        ));
                    }
                    m
                }
                Err(_) => {
                    differences.push("关联对账单不存在".into());
                    false
                }
            };
            let receipt_m = self.receipt_match(ctx, db, recon_id, &mut differences).await;
            (po_m, receipt_m)
        } else {
            // 无对账单：PO/入库侧不校验（同 payment::create / approve 口径，放行）
            (true, true)
        };

        // 发票侧：发票金额 vs 付款金额（无发票放行，同 create 口径）
        let invoice_matched = if let Some(inv) = payment.invoice_amount {
            let m = within_tolerance(payment.amount, inv);
            if !m {
                differences.push(format!(
                    "付款金额 {} 与发票金额 {} 偏差超过容差 ±0.5%",
                    payment.amount, inv
                ));
            }
            m
        } else {
            true
        };

        let can_pay = po_matched && receipt_matched && invoice_matched;
        Ok(ThreeWayMatchSummary {
            po_matched,
            receipt_matched,
            invoice_matched,
            can_pay,
            differences,
        })
    }

    async fn get_settlement_hub_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        recon_type: SettlementReconType,
        ref_id: i64,
    ) -> Result<SettlementHubSummary> {
        match recon_type {
            SettlementReconType::DraftRecon => {
                let recon_svc = new_purchase_reconciliation_service(self.pool.clone());
                let recon = recon_svc.get(ctx, db, ref_id).await?;
                let supplier_name = self.supplier_name(ctx, db, recon.supplier_id).await;
                let item_count = recon_svc
                    .list_items(ctx, db, ref_id)
                    .await
                    .map(|v| v.len())
                    .unwrap_or(0);
                let pending = self.pending_returns(ctx, db, recon.supplier_id).await;
                let ap = self.ap_outstanding(ctx, db, recon.supplier_id).await;
                Ok(SettlementHubSummary {
                    recon_type,
                    supplier_name,
                    draft_recon: Some(DraftReconAggregate {
                        doc_number: recon.doc_number,
                        period: recon.period,
                        total_amount: recon.total_amount,
                        confirmed_amount: recon.confirmed_amount,
                        difference: recon.difference,
                        item_count,
                        pending_returns_count: pending.0,
                        pending_returns_amount: pending.1,
                        ap_outstanding: ap,
                    }),
                    pending_payment: None,
                })
            }
            SettlementReconType::PendingPayment => {
                let pay_svc = new_payment_request_service(self.pool.clone());
                let pay = pay_svc.get(ctx, db, ref_id).await?;
                let supplier_name = self.supplier_name(ctx, db, pay.supplier_id).await;
                let source_recon_doc = if let Some(rid) = pay.reconciliation_id {
                    new_purchase_reconciliation_service(self.pool.clone())
                        .get(ctx, db, rid)
                        .await
                        .ok()
                        .map(|r| r.doc_number)
                } else {
                    None
                };
                let three_way_match = self
                    .check_three_way_match(ctx, db, ref_id)
                    .await
                    .unwrap_or_default();
                let ap = self.ap_outstanding(ctx, db, pay.supplier_id).await;
                Ok(SettlementHubSummary {
                    recon_type,
                    supplier_name,
                    draft_recon: None,
                    pending_payment: Some(PendingPaymentAggregate {
                        payment_id: pay.id,
                        doc_number: pay.doc_number,
                        amount: pay.amount,
                        payment_method: payment_method_label(pay.payment_method).into(),
                        invoice_number: pay.invoice_number,
                        invoice_amount: pay.invoice_amount,
                        source_recon_doc,
                        three_way_match,
                        ap_outstanding: ap,
                    }),
                })
            }
        }
    }

    async fn get_return_hub_summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        return_id: i64,
    ) -> Result<ReturnHubSummary> {
        let ret_svc = new_purchase_return_service(self.pool.clone());
        let po_svc = new_purchase_order_service(self.pool.clone());
        let return_order = ret_svc.get(ctx, db, return_id).await?;
        let supplier_name = self.supplier_name(ctx, db, return_order.supplier_id).await;
        let (source_po_doc, source_po_status) = match po_svc.get(ctx, db, return_order.order_id).await {
            Ok(po) => (po.doc_number, po_status_label(po.status).into()),
            Err(_) => (format!("#{}", return_order.order_id), "已删除".into()),
        };
        let items = ret_svc.list_items(ctx, db, return_id).await.unwrap_or_default();
        let total_qty = items.iter().map(|i| i.returned_qty).sum::<Decimal>();
        let settlement_hint = return_settlement_hint(return_order.status).into();
        Ok(ReturnHubSummary {
            return_order,
            supplier_name,
            source_po_doc,
            source_po_status,
            item_count: items.len(),
            total_qty,
            settlement_hint,
        })
    }
}
