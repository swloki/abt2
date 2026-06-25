//! 采购作业中心 — 需求 / 订单 / 对账付款 / 退货 四 card 聚合工作台。
//!
//! 架构（组件化单端点模式，同 mes_work_center）：
//! - 首页内联渲染 4 个 card 外壳，每个 card 占位 div `hx-trigger="load"` 拉各自端点；
//! - 每个 card 一个 GET 端点，card 内 tab/筛选走该端点 + `hx-select="#pc-xxx-card"` 局部刷新；
//! - 写操作（审批/驳回/对账确认/付款审批）POST 广播 `HX-Trigger`（poChanged / reconChanged），
//!   相关 card 声明 `hx-trigger="poChanged from:body"` 自刷新；首页锚点条监听三者重算 summary。
//! - drawer 就地操作：订单审批 / 付款审批。

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::demand_handler::{
    DemandPoolQuery, DemandSummary, MaterialAggQuery, MaterialAggSummary, PurchaseDemandService,
};
use abt_core::purchase::enums::{
    PaymentStatus, PurchaseOrderStatus, PurchaseReconStatus, PurchaseReturnStatus,
};
use abt_core::purchase::order::model::{PurchaseOrder, PurchaseOrderQuery};
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::purchase::payment::model::{PaymentRequest, PaymentRequestQuery};
use abt_core::purchase::payment::PaymentRequestService;
use abt_core::purchase::reconciliation::model::{PurchaseReconciliation, PurchaseReconciliationQuery};
use abt_core::purchase::reconciliation::PurchaseReconciliationService;
use abt_core::purchase::return_order::model::{PurchaseReturn, PurchaseReturnQuery};
use abt_core::purchase::return_order::PurchaseReturnService;
use abt_core::purchase::work_center::{PurchaseWorkCenterService, PurchaseWorkCenterSummary};
use abt_core::shared::types::{DomainError, PageParams};

use std::collections::HashMap;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_work_center::*;
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// =============================================================================
// 首页
// =============================================================================

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_work_center(_path: PurchaseWorkCenterPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;
    let summary = state
        .purchase_work_center_service()
        .summary(&service_ctx, &mut conn)
        .await
        .unwrap_or_default();

    let content = html! {
        // detail-header：标题 + meta + 内嵌锚点条
        div class="bg-bg border border-border-soft rounded-lg p-6 mb-4 shadow-[var(--shadow-card)]" {
            div class="flex items-center justify-between flex-wrap gap-4" {
                div {
                    h1 class="text-xl font-bold text-fg tracking-tight" { "采购作业中心" }
                    p class="text-sm text-muted mt-1" { "需求 · 订单 · 对账 · 退货 一屏处理，就地操作" }
                }
            }
            (render_anchor_nav(&summary))
        }
        (render_card_shell("pc-demand-card", PcDemandPath::PATH, "采购需求"))
        (render_card_shell("pc-orders-card", PcOrdersPath::PATH, "采购订单"))
        (render_card_shell("pc-settlement-card", PcSettlementPath::PATH, "对账付款"))
        (render_card_shell("pc-returns-card", PcReturnsPath::PATH, "采购退货"))
        (render_drawer_overlay("approve-overlay", "approve-drawer", "approve-drawer-body", "审批采购订单"))
        (render_drawer_overlay("pay-overlay", "pay-drawer", "pay-drawer-body", "审批付款"))
    };

    Ok(Html(
        admin_page(
            is_htmx,
            "采购作业中心",
            &claims,
            "purchase",
            PurchaseWorkCenterPath::PATH,
            "采购管理",
            Some("采购作业中心"),
            content,
            &nav_filter,
        )
        .into_string(),
    ))
}

// =============================================================================
// Card 端点
// =============================================================================

// ── ① 采购需求 ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct DemandCardParams {
    /// "material" | "detail"（默认 material）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub view: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub keyword: Option<String>,
}

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_demand_card(
    _path: PcDemandPath,
    ctx: RequestContext,
    Query(p): Query<DemandCardParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.purchase_demand_service();
    let view = p.view.as_deref().unwrap_or("material");

    let body = if view == "detail" {
        let result = svc
            .list_pending_demands(
                &service_ctx,
                &mut conn,
                DemandPoolQuery {
                    keyword: p.keyword.clone(),
                    ..Default::default()
                },
                PageParams::new(1, 10),
            )
            .await?;
        demand_detail_table(&result)
    } else {
        let result = svc
            .list_material_aggregated(
                &service_ctx,
                &mut conn,
                MaterialAggQuery {
                    keyword: p.keyword.clone(),
                    ..Default::default()
                },
                PageParams::new(1, 10),
            )
            .await?;
        demand_material_table(&result)
    };

    Ok(Html(
        html! {
            div id="pc-demand-card" {
                (demand_filter_bar(view, &p))
                (body)
                div class="px-5 py-3 border-t border-border-soft text-center" {
                    a class="text-sm text-accent font-semibold no-underline" href="/admin/purchase/demand-pool" { "查看全部需求 →" }
                }
            }
        }
        .into_string(),
    ))
}

// ── ② 采购订单 ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct OrdersCardParams {
    /// "approval" | "confirmed" | "partial"（默认 approval）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub tab: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub keyword: Option<String>,
}

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_orders_card(
    _path: PcOrdersPath,
    ctx: RequestContext,
    Query(p): Query<OrdersCardParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.purchase_order_service();
    let tab = p.tab.as_deref().unwrap_or("approval");
    let status = match tab {
        "confirmed" => PurchaseOrderStatus::Confirmed,
        "partial" => PurchaseOrderStatus::PartiallyReceived,
        _ => PurchaseOrderStatus::PendingApproval,
    };
    let result = svc
        .list(
            &service_ctx,
            &mut conn,
            PurchaseOrderQuery {
                status: Some(status),
                doc_number: p.keyword.clone(),
                ..Default::default()
            },
            PageParams::new(1, 10),
        )
        .await?;
    let ids: Vec<i64> = result.items.iter().map(|o| o.supplier_id).collect();
    let names = supplier_names(&state, &service_ctx, &mut conn, &ids).await;

    Ok(Html(
        html! {
            div id="pc-orders-card" {
                (orders_filter_bar(tab, &p))
                (orders_table(&result.items, &names, tab))
                div class="px-5 py-3 border-t border-border-soft text-center" {
                    a class="text-sm text-accent font-semibold no-underline" href="/admin/purchase/orders" { "查看全部订单 →" }
                }
            }
        }
        .into_string(),
    ))
}

// ── ③ 对账付款 ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct SettlementCardParams {
    /// "recon" | "payment"（默认 recon — 草稿对账单）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub tab: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub keyword: Option<String>,
}

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_settlement_card(
    _path: PcSettlementPath,
    ctx: RequestContext,
    Query(p): Query<SettlementCardParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let tab = p.tab.as_deref().unwrap_or("recon");

    let body = if tab == "payment" {
        let svc = state.payment_request_service();
        let result = svc
            .list(
                &service_ctx,
                &mut conn,
                PaymentRequestQuery {
                    status: Some(PaymentStatus::Draft),
                    keyword: p.keyword.clone(),
                    ..Default::default()
                },
                PageParams::new(1, 10),
            )
            .await?;
        let ids: Vec<i64> = result.items.iter().map(|x| x.supplier_id).collect();
        let names = supplier_names(&state, &service_ctx, &mut conn, &ids).await;
        payment_table(&result.items, &names)
    } else {
        let svc = state.purchase_reconciliation_service();
        let result = svc
            .list(
                &service_ctx,
                &mut conn,
                PurchaseReconciliationQuery {
                    status: Some(PurchaseReconStatus::Draft),
                    ..Default::default()
                },
                PageParams::new(1, 10),
            )
            .await?;
        let ids: Vec<i64> = result.items.iter().map(|x| x.supplier_id).collect();
        let names = supplier_names(&state, &service_ctx, &mut conn, &ids).await;
        recon_table(&result.items, &names)
    };

    Ok(Html(
        html! {
            div id="pc-settlement-card" {
                (settlement_filter_bar(tab, &p))
                (body)
                div class="px-5 py-3 border-t border-border-soft text-center" {
                    a class="text-sm text-accent font-semibold no-underline" href="/admin/purchase/reconciliations" { "查看全部对账 →" }
                }
            }
        }
        .into_string(),
    ))
}

// ── ④ 采购退货 ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ReturnsCardParams {
    /// "confirmed" | "shipped"（默认 confirmed — 待发货）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub tab: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub keyword: Option<String>,
}

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_returns_card(
    _path: PcReturnsPath,
    ctx: RequestContext,
    Query(p): Query<ReturnsCardParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.purchase_return_service();
    let tab = p.tab.as_deref().unwrap_or("confirmed");
    let status = match tab {
        "shipped" => PurchaseReturnStatus::Shipped,
        _ => PurchaseReturnStatus::Confirmed,
    };
    let result = svc
        .list(
            &service_ctx,
            &mut conn,
            PurchaseReturnQuery {
                status: Some(status),
                ..Default::default()
            },
            PageParams::new(1, 10),
        )
        .await?;
    let ids: Vec<i64> = result.items.iter().map(|x| x.supplier_id).collect();
    let names = supplier_names(&state, &service_ctx, &mut conn, &ids).await;

    Ok(Html(
        html! {
            div id="pc-returns-card" {
                (returns_filter_bar(tab, &p))
                (returns_table(&result.items, &names, tab))
                div class="px-5 py-3 border-t border-border-soft text-center" {
                    a class="text-sm text-accent font-semibold no-underline" href="/admin/purchase/returns" { "查看全部退货 →" }
                }
            }
        }
        .into_string(),
    ))
}

// =============================================================================
// Drawer GET（就地操作表单）
// =============================================================================

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_order_approve_drawer(
    path: PcOrderApproveDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let order = state
        .purchase_order_service()
        .get(&service_ctx, &mut conn, path.id)
        .await?;
    let supplier = supplier_names(&state, &service_ctx, &mut conn, &[order.supplier_id])
        .await
        .get(&order.supplier_id)
        .cloned()
        .unwrap_or_default();
    Ok(Html(render_order_approve_body(&order, &supplier).into_string()))
}

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_payment_approve_drawer(
    path: PcPaymentApproveDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.payment_request_service();
    // PaymentRequestService 无 get-by-id 时用 list 反查
    let list = svc
        .list(
            &service_ctx,
            &mut conn,
            PaymentRequestQuery::default(),
            PageParams::new(1, 500),
        )
        .await?;
    let pay = list
        .items
        .into_iter()
        .find(|x| x.id == path.id)
        .ok_or_else(|| DomainError::NotFound("付款申请不存在".into()))?;
    let supplier = supplier_names(&state, &service_ctx, &mut conn, &[pay.supplier_id])
        .await
        .get(&pay.supplier_id)
        .cloned()
        .unwrap_or_default();
    Ok(Html(render_payment_approve_body(&pay, &supplier).into_string()))
}

// =============================================================================
// 写 handler（事务包裹，HX-Trigger 广播）
// =============================================================================

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn approve_order(path: PcOrderApprovePath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .purchase_order_service()
        .approve_po(&service_ctx, &mut tx, path.id, None)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(([("HX-Trigger", "poChanged")], Html(String::new())))
}

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn reject_order(path: PcOrderRejectPath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .purchase_order_service()
        .reject(&service_ctx, &mut tx, path.id, String::new(), None)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(([("HX-Trigger", "poChanged")], Html(String::new())))
}

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn confirm_recon(path: PcReconConfirmPath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .purchase_reconciliation_service()
        .confirm(&service_ctx, &mut tx, path.id, None)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(([("HX-Trigger", "reconChanged")], Html(String::new())))
}

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn approve_payment(path: PcPaymentApprovePath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .payment_request_service()
        .approve(&service_ctx, &mut tx, path.id, None)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(([("HX-Trigger", "reconChanged")], Html(String::new())))
}

// =============================================================================
// 渲染辅助
// =============================================================================

/// 锚点条：待办总数 + 4 业务 chip + 逾期/临期告警。
fn render_anchor_nav(s: &PurchaseWorkCenterSummary) -> Markup {
    let total = s.total();
    let demand_cnt = s.pending_demand + s.pending_misc;
    let order_cnt = s.po_pending_approval + s.po_pending_receive + s.po_partial;
    let settle_cnt = s.recon_draft + s.payment_pending_approval;
    let return_cnt = s.return_pending_ship + s.return_shipped;
    html! {
        div class="sticky top-0 z-20 flex items-center gap-4 p-3 mt-4 rounded-lg border border-border-soft bg-bg shadow-xs flex-wrap" {
            div class="flex flex-col items-center pr-4 border-r border-border-soft shrink-0" {
                span class="text-xl font-bold font-mono tabular-nums text-accent leading-tight" { (total) }
                span class="text-xs text-muted font-medium" { "待办" }
            }
            div class="flex items-center gap-2 flex-wrap" {
                (nav_chip("#pc-demand-card", "采购需求", demand_cnt))
                (nav_chip("#pc-orders-card", "采购订单", order_cnt))
                (nav_chip("#pc-settlement-card", "对账付款", settle_cnt))
                (nav_chip("#pc-returns-card", "采购退货", return_cnt))
            }
            div class="ml-auto flex items-center gap-2" {
                @if s.overdue_count > 0 {
                    span class="inline-flex items-center gap-1 px-2.5 py-1 rounded-full bg-danger-bg text-danger text-[11px] font-semibold" {
                        (s.overdue_count) " 逾期"
                    }
                }
                @if s.soon_count > 0 {
                    span class="inline-flex items-center gap-1 px-2.5 py-1 rounded-full bg-warn-bg text-warn text-[11px] font-semibold" {
                        (s.soon_count) " 临期"
                    }
                }
            }
        }
    }
}

fn nav_chip(href: &str, label: &str, count: u64) -> Markup {
    if count == 0 {
        return html! {};
    }
    html! {
        a class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-surface border border-border-soft text-sm font-semibold text-fg-2 no-underline cursor-pointer hover:bg-accent-bg hover:border-accent hover:text-accent transition-all"
            href=(href)
            _=(format!("on click halt the event then call document.querySelector('{href}')?.scrollIntoView({{behavior:'smooth',block:'center'}})")) {
            (label)
            span class="font-mono font-bold text-accent" { (count) }
        }
    }
}

/// Card 外壳：标题栏 + 占位 div（`hx-trigger="load"` 拉 card 端点）。
/// 监听 `poChanged`/`reconChanged`/`returnChanged` 自刷新（写操作后）。
fn render_card_shell(card_id: &str, src: &str, title: &str) -> Markup {
    let trigger = format!("load, poChanged from:body, reconChanged from:body, returnChanged from:body");
    html! {
        section class="bg-bg border border-border-soft rounded-lg mb-4 shadow-[var(--shadow-card)] overflow-hidden" {
            div class="px-5 py-3 border-b border-border-soft" {
                span class="font-semibold text-fg" { (title) }
            }
            div id=(card_id)
                class="p-5 text-sm text-muted"
                hx-get=(src)
                hx-trigger=(trigger)
                hx-target="this"
                hx-swap="outerHTML" {
                "加载中…"
            }
        }
    }
}

/// Drawer overlay 壳（同 mes_work_center）：背景点击/关闭按钮收起，body 由 `hx-get` 填充。
fn render_drawer_overlay(overlay_id: &str, drawer_id: &str, body_id: &str, title: &str) -> Markup {
    html! {
        div id=(overlay_id)
            class="fixed inset-0 bg-slate-900/40 opacity-0 invisible pointer-events-none transition-opacity duration-200 z-[90] open:opacity-100 open:visible open:pointer-events-auto"
            _=(format!("on click[me is event.target] remove .open from #{}", overlay_id)) {
            div id=(drawer_id)
                class="fixed top-0 right-0 h-full w-[480px] max-w-[92vw] bg-bg shadow-lg translate-x-full transition-transform duration-300 flex flex-col z-[91] open:translate-x-0"
                _="on click halt the event" {
                div class="flex items-center justify-between px-6 py-5 border-b border-border-soft" {
                    div class="font-bold text-base text-fg" { (title) }
                    button type="button"
                        class="w-8 h-8 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg flex items-center justify-center"
                        _=(format!("on click remove .open from #{}", overlay_id)) {
                        (icon::x_icon("w-4 h-4"))
                    }
                }
                div id=(body_id) class="flex-1 overflow-y-auto px-6 py-5" {}
            }
        }
    }
}

/// 供应商名称映射：按 id 批量反查（拉首页 500 条过滤）。
async fn supplier_names(
    state: &crate::state::AppState,
    ctx: &abt_core::shared::types::context::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    ids: &[i64],
) -> HashMap<i64, String> {
    if ids.is_empty() {
        return HashMap::new();
    }
    let svc = state.supplier_service();
    match svc
        .list(ctx, db, SupplierQuery::default(), PageParams::new(1, 500))
        .await
    {
        Ok(r) => r
            .items
            .into_iter()
            .filter(|s| ids.contains(&s.id))
            .map(|s| (s.id, s.name))
            .collect(),
        Err(_) => HashMap::new(),
    }
}

fn pill(key: &str, label: &str) -> Markup {
    html! {
        span class=(format!("status-pill {} text-[11px]", crate::utils::status_color(key))) {
            (label)
        }
    }
}

fn toggle_cls(active: bool) -> &'static str {
    if active {
        "inline-flex items-center gap-1 px-3 py-1 text-sm text-accent font-semibold cursor-pointer bg-bg shadow-xs rounded-sm"
    } else {
        "inline-flex items-center gap-1 px-3 py-1 text-sm text-muted cursor-pointer bg-transparent border-none rounded-sm hover:text-fg transition-colors"
    }
}

// ── 需求 card 渲染 ──

fn demand_filter_bar(view: &str, p: &DemandCardParams) -> Markup {
    let is_mat = view == "material";
    let kw = p.keyword.as_deref().unwrap_or("");
    html! {
        div class="flex items-center justify-between flex-wrap gap-3 px-5 py-3 border-b border-border-soft" {
            div class="inline-flex bg-surface border border-border-soft rounded-md p-[3px] gap-0.5" {
                button class=(toggle_cls(is_mat)) type="button"
                    hx-get=(PcDemandPath::PATH)
                    hx-vals="{\"view\":\"material\"}"
                    hx-target="#pc-demand-card" hx-select="#pc-demand-card" hx-swap="outerHTML"
                    hx-push-url="true" hx-include="#pc-demand-filter-form"
                    { "物料汇总" }
                button class=(toggle_cls(!is_mat)) type="button"
                    hx-get=(PcDemandPath::PATH)
                    hx-vals="{\"view\":\"detail\"}"
                    hx-target="#pc-demand-card" hx-select="#pc-demand-card" hx-swap="outerHTML"
                    hx-push-url="true" hx-include="#pc-demand-filter-form"
                    { "请购明细" }
            }
            form class="flex items-center gap-2"
                hx-get=(PcDemandPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.pc-demand-search"
                hx-target="#pc-demand-card" hx-select="#pc-demand-card" hx-swap="outerHTML"
                hx-push-url="true" {
                input type="hidden" name="view" value=(view);
                div class="relative" {
                    (icon::search_icon("w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"));
                    input class="pc-demand-search w-[180px] pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                        type="text" name="keyword" placeholder="搜索物料/订单"
                        value=(kw);
                }
            }
            form id="pc-demand-filter-form" class="hidden" {
                input type="hidden" name="keyword" value=(kw);
            }
        }
    }
}

fn demand_material_table(result: &abt_core::shared::types::PaginatedResult<MaterialAggSummary>) -> Markup {
    html! {
        div class="overflow-x-auto" {
            table class="w-full text-sm" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="text-left font-semibold py-2 px-5 uppercase tracking-wide" { "物料" }
                        th class="text-right font-semibold py-2 px-3 uppercase tracking-wide" { "总需求量" }
                        th class="text-center font-semibold py-2 px-3 uppercase tracking-wide" { "来源数" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "需求日期" }
                        th class="text-right font-semibold py-2 px-5 uppercase tracking-wide" { "操作" }
                    }
                }
                tbody {
                    @if result.items.is_empty() {
                        tr { td colspan="5" class="text-center text-muted py-8" { "暂无待处理需求" } }
                    }
                    @for item in &result.items {
                        tr class="border-b border-border-soft hover:bg-accent-bg" {
                            td class="py-2.5 px-5" {
                                div class="font-medium text-fg" { (item.product_name) }
                                div class="text-xs text-muted font-mono" { (item.product_code) }
                            }
                            td class="text-right font-mono py-2.5 px-3" { (fmt_decimal(item.total_demand_qty)) }
                            td class="text-center font-mono py-2.5 px-3 text-accent" { (item.demand_count) }
                            td class="py-2.5 px-3 text-muted" {
                                (fmt_date(item.earliest_required_date))
                                @if let Some(latest) = item.latest_required_date {
                                    span class="text-xs" { " → " (fmt_date(Some(latest))) }
                                }
                            }
                            td class="text-right py-2.5 px-5" {
                                a class="inline-flex items-center px-3 py-1 rounded-sm bg-accent text-white text-xs font-semibold no-underline hover:opacity-90"
                                    href="/admin/purchase/demand-pool" { "转采购单" }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn demand_detail_table(result: &abt_core::shared::types::PaginatedResult<DemandSummary>) -> Markup {
    html! {
        div class="overflow-x-auto" {
            table class="w-full text-sm" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="text-left font-semibold py-2 px-5 uppercase tracking-wide" { "物料" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "来源订单" }
                        th class="text-right font-semibold py-2 px-3 uppercase tracking-wide" { "数量" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "需求日期" }
                        th class="text-right font-semibold py-2 px-5 uppercase tracking-wide" { "操作" }
                    }
                }
                tbody {
                    @if result.items.is_empty() {
                        tr { td colspan="5" class="text-center text-muted py-8" { "暂无待处理需求" } }
                    }
                    @for item in &result.items {
                        tr class="border-b border-border-soft hover:bg-accent-bg" {
                            td class="py-2.5 px-5" {
                                div class="font-medium text-fg" { (item.product_name) }
                                div class="text-xs text-muted font-mono" { (item.product_code) }
                            }
                            td class="py-2.5 px-3 font-mono text-accent" { (item.order_no.as_deref().unwrap_or("—")) }
                            td class="text-right font-mono py-2.5 px-3" { (fmt_decimal(item.quantity)) }
                            td class="py-2.5 px-3 text-muted" { (fmt_date(item.required_date)) }
                            td class="text-right py-2.5 px-5" {
                                a class="inline-flex items-center px-3 py-1 rounded-sm bg-accent text-white text-xs font-semibold no-underline hover:opacity-90"
                                    href="/admin/purchase/demand-pool" { "转采购单" }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── 订单 card 渲染 ──

fn orders_filter_bar(tab: &str, p: &OrdersCardParams) -> Markup {
    let kw = p.keyword.as_deref().unwrap_or("");
    let tab_btn = |val: &str, label: &str, active: bool| -> Markup {
        html! {
            button class=(toggle_cls(active)) type="button"
                hx-get=(PcOrdersPath::PATH)
                hx-vals=(format!("{{\"tab\":\"{}\"}}", val))
                hx-target="#pc-orders-card" hx-select="#pc-orders-card" hx-swap="outerHTML"
                hx-push-url="true" hx-include="#pc-orders-filter-form"
                { (label) }
        }
    };
    html! {
        div class="flex items-center justify-between flex-wrap gap-3 px-5 py-3 border-b border-border-soft" {
            div class="inline-flex bg-surface border border-border-soft rounded-md p-[3px] gap-0.5" {
                (tab_btn("approval", "待审批", tab == "approval"));
                (tab_btn("confirmed", "待收货", tab == "confirmed"));
                (tab_btn("partial", "部分收货", tab == "partial"));
            }
            form class="flex items-center gap-2"
                hx-get=(PcOrdersPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.pc-orders-search"
                hx-target="#pc-orders-card" hx-select="#pc-orders-card" hx-swap="outerHTML"
                hx-push-url="true" {
                input type="hidden" name="tab" value=(tab);
                div class="relative" {
                    (icon::search_icon("w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"));
                    input class="pc-orders-search w-[180px] pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                        type="text" name="keyword" placeholder="搜索 PO 号"
                        value=(kw);
                }
            }
            form id="pc-orders-filter-form" class="hidden" {
                input type="hidden" name="keyword" value=(kw);
            }
        }
    }
}

fn orders_table(items: &[PurchaseOrder], names: &HashMap<i64, String>, tab: &str) -> Markup {
    html! {
        div class="overflow-x-auto" {
            table class="w-full text-sm" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="text-left font-semibold py-2 px-5 uppercase tracking-wide" { "订单号" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "供应商" }
                        th class="text-right font-semibold py-2 px-3 uppercase tracking-wide" { "金额" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "交期" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "状态" }
                        th class="text-right font-semibold py-2 px-5 uppercase tracking-wide" { "操作" }
                    }
                }
                tbody {
                    @if items.is_empty() {
                        tr { td colspan="6" class="text-center text-muted py-8" { "暂无待处理订单" } }
                    }
                    @for o in items {
                        tr class="border-b border-border-soft hover:bg-accent-bg" {
                            td class="py-2.5 px-5 font-mono text-accent font-medium" { (o.doc_number) }
                            td class="py-2.5 px-3 text-fg" { (names.get(&o.supplier_id).map(|s| s.as_str()).unwrap_or("—")) }
                            td class="text-right font-mono py-2.5 px-3" { (fmt_decimal(o.total_amount)) }
                            td class="py-2.5 px-3 text-muted" { (fmt_date(o.expected_delivery_date)) }
                            td class="py-2.5 px-3" { (po_status_pill(o.status)) }
                            td class="text-right py-2.5 px-5 whitespace-nowrap" {
                                @if tab == "approval" {
                                    button class="inline-flex items-center px-3 py-1 rounded-sm bg-accent text-white text-xs font-semibold border-none cursor-pointer hover:opacity-90 mr-1"
                                        hx-get=(PcOrderApproveDrawerPath { id: o.id }.to_string())
                                        hx-target="#approve-drawer-body" hx-swap="innerHTML"
                                        _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #approve-overlay" { "审批" }
                                } @else {
                                    a class="inline-flex items-center px-3 py-1 rounded-sm bg-white text-fg-2 border border-border text-xs font-medium no-underline cursor-pointer hover:bg-surface mr-1"
                                        href=(format!("/admin/purchase/orders/{}", o.id)) { "登记收货" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn po_status_pill(status: PurchaseOrderStatus) -> Markup {
    use PurchaseOrderStatus::*;
    match status {
        PendingApproval => pill("progress", "待审批"),
        Confirmed => pill("info", "待收货"),
        PartiallyReceived => pill("partial", "部分收货"),
        Received => pill("completed", "已收货"),
        Closed => pill("completed", "已关闭"),
        Cancelled => pill("cancelled", "已取消"),
        Draft => pill("draft", "草稿"),
    }
}

// ── 对账付款 card 渲染 ──

fn settlement_filter_bar(tab: &str, p: &SettlementCardParams) -> Markup {
    let kw = p.keyword.as_deref().unwrap_or("");
    let tab_btn = |val: &str, label: &str, active: bool| -> Markup {
        html! {
            button class=(toggle_cls(active)) type="button"
                hx-get=(PcSettlementPath::PATH)
                hx-vals=(format!("{{\"tab\":\"{}\"}}", val))
                hx-target="#pc-settlement-card" hx-select="#pc-settlement-card" hx-swap="outerHTML"
                hx-push-url="true" hx-include="#pc-settlement-filter-form"
                { (label) }
        }
    };
    html! {
        div class="flex items-center justify-between flex-wrap gap-3 px-5 py-3 border-b border-border-soft" {
            div class="inline-flex bg-surface border border-border-soft rounded-md p-[3px] gap-0.5" {
                (tab_btn("recon", "草稿对账单", tab == "recon"));
                (tab_btn("payment", "待审批付款", tab == "payment"));
            }
            form class="flex items-center gap-2"
                hx-get=(PcSettlementPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.pc-settlement-search"
                hx-target="#pc-settlement-card" hx-select="#pc-settlement-card" hx-swap="outerHTML"
                hx-push-url="true" {
                input type="hidden" name="tab" value=(tab);
                div class="relative" {
                    (icon::search_icon("w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"));
                    input class="pc-settlement-search w-[180px] pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                        type="text" name="keyword" placeholder="搜索供应商/单号"
                        value=(kw);
                }
            }
            form id="pc-settlement-filter-form" class="hidden" {
                input type="hidden" name="keyword" value=(kw);
            }
        }
    }
}

fn recon_table(items: &[PurchaseReconciliation], names: &HashMap<i64, String>) -> Markup {
    html! {
        div class="overflow-x-auto" {
            table class="w-full text-sm" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="text-left font-semibold py-2 px-5 uppercase tracking-wide" { "对账单号" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "供应商" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "期间" }
                        th class="text-right font-semibold py-2 px-3 uppercase tracking-wide" { "金额" }
                        th class="text-right font-semibold py-2 px-5 uppercase tracking-wide" { "操作" }
                    }
                }
                tbody {
                    @if items.is_empty() {
                        tr { td colspan="5" class="text-center text-muted py-8" { "暂无草稿对账单" } }
                    }
                    @for r in items {
                        tr class="border-b border-border-soft hover:bg-accent-bg" {
                            td class="py-2.5 px-5 font-mono text-accent font-medium" { (r.doc_number) }
                            td class="py-2.5 px-3 text-fg" { (names.get(&r.supplier_id).map(|s| s.as_str()).unwrap_or("—")) }
                            td class="py-2.5 px-3 text-muted font-mono" { (r.period) }
                            td class="text-right font-mono py-2.5 px-3" { (fmt_decimal(r.total_amount)) }
                            td class="text-right py-2.5 px-5 whitespace-nowrap" {
                                button class="inline-flex items-center px-3 py-1 rounded-sm bg-accent text-white text-xs font-semibold border-none cursor-pointer hover:opacity-90 mr-1"
                                    hx-post=(PcReconConfirmPath { id: r.id }.to_string())
                                    hx-target="this" hx-swap="none"
                                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] call showToast('对账单已确认')"
                                    { "确认对账" }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn payment_table(items: &[PaymentRequest], names: &HashMap<i64, String>) -> Markup {
    html! {
        div class="overflow-x-auto" {
            table class="w-full text-sm" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="text-left font-semibold py-2 px-5 uppercase tracking-wide" { "付款单号" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "供应商" }
                        th class="text-right font-semibold py-2 px-3 uppercase tracking-wide" { "金额" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "付款日" }
                        th class="text-right font-semibold py-2 px-5 uppercase tracking-wide" { "操作" }
                    }
                }
                tbody {
                    @if items.is_empty() {
                        tr { td colspan="5" class="text-center text-muted py-8" { "暂无待审批付款" } }
                    }
                    @for pay in items {
                        tr class="border-b border-border-soft hover:bg-accent-bg" {
                            td class="py-2.5 px-5 font-mono text-accent font-medium" { (pay.doc_number) }
                            td class="py-2.5 px-3 text-fg" { (names.get(&pay.supplier_id).map(|s| s.as_str()).unwrap_or("—")) }
                            td class="text-right font-mono py-2.5 px-3" { (fmt_decimal(pay.amount)) }
                            td class="py-2.5 px-3 text-muted font-mono" { (fmt_date(Some(pay.payment_date))) }
                            td class="text-right py-2.5 px-5 whitespace-nowrap" {
                                button class="inline-flex items-center px-3 py-1 rounded-sm bg-accent text-white text-xs font-semibold border-none cursor-pointer hover:opacity-90 mr-1"
                                    hx-get=(PcPaymentApproveDrawerPath { id: pay.id }.to_string())
                                    hx-target="#pay-drawer-body" hx-swap="innerHTML"
                                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #pay-overlay" { "审批付款" }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── 退货 card 渲染 ──

fn returns_filter_bar(tab: &str, p: &ReturnsCardParams) -> Markup {
    let kw = p.keyword.as_deref().unwrap_or("");
    let tab_btn = |val: &str, label: &str, active: bool| -> Markup {
        html! {
            button class=(toggle_cls(active)) type="button"
                hx-get=(PcReturnsPath::PATH)
                hx-vals=(format!("{{\"tab\":\"{}\"}}", val))
                hx-target="#pc-returns-card" hx-select="#pc-returns-card" hx-swap="outerHTML"
                hx-push-url="true" hx-include="#pc-returns-filter-form"
                { (label) }
        }
    };
    html! {
        div class="flex items-center justify-between flex-wrap gap-3 px-5 py-3 border-b border-border-soft" {
            div class="inline-flex bg-surface border border-border-soft rounded-md p-[3px] gap-0.5" {
                (tab_btn("confirmed", "待发货", tab == "confirmed"));
                (tab_btn("shipped", "已发出", tab == "shipped"));
            }
            form class="flex items-center gap-2"
                hx-get=(PcReturnsPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.pc-returns-search"
                hx-target="#pc-returns-card" hx-select="#pc-returns-card" hx-swap="outerHTML"
                hx-push-url="true" {
                input type="hidden" name="tab" value=(tab);
                div class="relative" {
                    (icon::search_icon("w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"));
                    input class="pc-returns-search w-[180px] pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                        type="text" name="keyword" placeholder="搜索退货单号"
                        value=(kw);
                }
            }
            form id="pc-returns-filter-form" class="hidden" {
                input type="hidden" name="keyword" value=(kw);
            }
        }
    }
}

fn returns_table(items: &[PurchaseReturn], names: &HashMap<i64, String>, tab: &str) -> Markup {
    html! {
        div class="overflow-x-auto" {
            table class="w-full text-sm" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="text-left font-semibold py-2 px-5 uppercase tracking-wide" { "退货单号" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "供应商" }
                        th class="text-right font-semibold py-2 px-3 uppercase tracking-wide" { "金额" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "原因" }
                        th class="text-right font-semibold py-2 px-5 uppercase tracking-wide" { "操作" }
                    }
                }
                tbody {
                    @if items.is_empty() {
                        tr { td colspan="5" class="text-center text-muted py-8" { "暂无退货" } }
                    }
                    @for r in items {
                        tr class="border-b border-border-soft hover:bg-accent-bg" {
                            td class="py-2.5 px-5 font-mono text-accent font-medium" { (r.doc_number) }
                            td class="py-2.5 px-3 text-fg" { (names.get(&r.supplier_id).map(|s| s.as_str()).unwrap_or("—")) }
                            td class="text-right font-mono py-2.5 px-3" { (fmt_decimal(r.total_amount)) }
                            td class="py-2.5 px-3 text-muted" { @if r.return_reason.is_empty() { "—" } @else { (r.return_reason.as_str()) } }
                            td class="text-right py-2.5 px-5 whitespace-nowrap" {
                                @if tab == "confirmed" {
                                    a class="inline-flex items-center px-3 py-1 rounded-sm bg-accent text-white text-xs font-semibold no-underline cursor-pointer hover:opacity-90 mr-1"
                                        href=(format!("/admin/purchase/returns/{}", r.id)) { "发货" }
                                } @else {
                                    span class="text-xs text-muted" { "待供应商确认" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Drawer body ──

fn field_readonly(label: &str, value: &str) -> Markup {
    html! {
        div class="mb-4" {
            label class="block text-xs text-muted font-medium mb-1.5" { (label) }
            div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg-2 font-mono" { (value) }
        }
    }
}

fn render_order_approve_body(o: &PurchaseOrder, supplier: &str) -> Markup {
    html! {
        div class="grid grid-cols-2 gap-4" {
            (field_readonly("供应商", supplier));
            (field_readonly("订单日期", &fmt_date(Some(o.order_date))));
        }
        div class="grid grid-cols-2 gap-4 mt-2" {
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "期望到货" }
                div class=(format!("px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm font-mono {}", if is_overdue_or_soon(o.expected_delivery_date) { "text-warn" } else { "text-fg-2" })) {
                    (fmt_date(o.expected_delivery_date))
                }
            }
            (field_readonly("订单金额", &fmt_decimal(o.total_amount)));
        }
        div class="p-3 mt-3 bg-surface-raised border border-border-soft rounded-sm text-xs text-muted leading-relaxed" {
            "审批通过后订单转 Confirmed，进入「待收货」，可继续登记收货（触发来料通知 → PO 回写 + 立应付台账）；驳回则退回草稿。"
        }
        form class="mt-5"
            hx-post=(PcOrderApprovePath { id: o.id }.to_string())
            hx-target="this" hx-swap="none"
            _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #approve-overlay then call showToast('审批通过')" {
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "审批意见（可选）" }
                textarea class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent resize-y"
                    name="remark" rows="3" placeholder="填写审批意见" {}
            }
            div class="flex justify-between gap-2 pt-3 border-t border-border-soft" {
                button type="button" class="inline-flex items-center px-4 py-2 rounded-sm bg-white text-danger border border-border text-sm font-medium cursor-pointer hover:bg-surface"
                    hx-post=(PcOrderRejectPath { id: o.id }.to_string())
                    hx-swap="none"
                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #approve-overlay then call showToast('已驳回')" {
                    "驳回"
                }
                button type="submit" class="inline-flex items-center px-4 py-2 rounded-sm bg-accent text-white text-sm font-semibold border-none cursor-pointer hover:opacity-90" {
                    "通过审批"
                }
            }
        }
    }
}

fn render_payment_approve_body(pay: &PaymentRequest, supplier: &str) -> Markup {
    html! {
        div class="grid grid-cols-2 gap-4" {
            (field_readonly("供应商", supplier));
            (field_readonly("付款单号", &pay.doc_number));
        }
        div class="grid grid-cols-2 gap-4 mt-2" {
            (field_readonly("付款金额", &fmt_decimal(pay.amount)));
            (field_readonly("付款日", &fmt_date(Some(pay.payment_date))));
        }
        @if let Some(inv) = &pay.invoice_number {
            div class="mt-2" { (field_readonly("发票号", inv)) }
        }
        div class="p-3 mt-3 bg-surface-raised border border-border-soft rounded-sm text-xs text-muted leading-relaxed" {
            "审批通过后付款申请转 Approved，等待 FMS 实际付款；FMS 付款成功后自动回写 Paid 并结算应付台账。"
        }
        div class="flex justify-end gap-2 mt-5 pt-3 border-t border-border-soft" {
            form hx-post=(PcPaymentApprovePath { id: pay.id }.to_string()) hx-swap="none"
                _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #pay-overlay then call showToast('付款已审批')" {
                button type="submit" class="inline-flex items-center px-4 py-2 rounded-sm bg-accent text-white text-sm font-semibold border-none cursor-pointer hover:opacity-90" {
                    "通过付款"
                }
            }
        }
    }
}

// ── 格式化辅助 ──

fn fmt_decimal(d: rust_decimal::Decimal) -> String {
    let f: f64 = d.to_string().parse().unwrap_or(0.0);
    if f.abs() >= 1_000_000.0 {
        format!("¥{:.1}M", f / 1_000_000.0)
    } else if f.abs() >= 10_000.0 {
        format!("¥{:.1}K", f / 1_000.0)
    } else {
        format!("¥{:.0}", f)
    }
}

fn fmt_date(d: Option<chrono::NaiveDate>) -> String {
    match d {
        Some(d) => d.format("%Y-%m-%d").to_string(),
        None => "—".to_string(),
    }
}

fn is_overdue_or_soon(d: Option<chrono::NaiveDate>) -> bool {
    // 交期在 7 天内或已过期 → drawer 交期标 warn 色（逾期/临期提示）。
    let Some(d) = d else {
        return false;
    };
    let today = chrono::Utc::now().date_naive();
    let soon = today.checked_add_days(chrono::Days::new(7)).unwrap_or(today);
    d <= soon
}
