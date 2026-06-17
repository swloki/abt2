use std::collections::HashMap;

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;

use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::model::AcquireChannel;
use abt_core::master_data::product::ProductService;
use abt_core::sales::sales_order::model::*;
use abt_core::sales::sales_order::{DemandService, SalesOrderService};
use abt_core::shared::enums::document_type::DocumentType;
use abt_core::shared::identity::UserService;
use abt_core::wms::stock_ledger::StockLedgerService;

const DECIMAL_100: Decimal = Decimal::from_parts(100, 0, 0, false, 0);

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::order::*;
use crate::utils::RequestContext;
use crate::utils::fmt_qty;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: SalesOrderStatus) -> (&'static str, &'static str) {
    match s {
        SalesOrderStatus::Draft => ("草稿", "status-draft"),
        SalesOrderStatus::Confirmed => ("已确认", "status-confirmed"),
        SalesOrderStatus::PartiallyShipped => ("部分发货", "status-progress"),
        SalesOrderStatus::Shipped => ("已发货", "status-shipped"),
        SalesOrderStatus::Completed => ("已完成", "status-completed"),
        SalesOrderStatus::Cancelled => ("已取消", "status-rejected"),
    }
}

fn line_status_pill(s: SalesOrderLineStatus) -> (&'static str, &'static str) {
    match s {
        SalesOrderLineStatus::Pending => ("待处理", "line-status-pending"),
        SalesOrderLineStatus::Allocated => ("已分配", "line-status-allocated"),
        SalesOrderLineStatus::Producing => ("生产中", "line-status-producing"),
        SalesOrderLineStatus::Purchasing => ("采购中", "line-status-purchasing"),
        SalesOrderLineStatus::Shipped => ("已发货", "line-status-shipped"),
        SalesOrderLineStatus::Cancelled => ("已取消", "line-status-cancelled"),
    }
}

fn fulfill_status_pill(s: FulfillmentLineStatus) -> (&'static str, &'static str) {
    match s {
        FulfillmentLineStatus::Pending => ("待处理", "line-status-pending"),
        FulfillmentLineStatus::Allocated => ("已分配", "line-status-allocated"),
        FulfillmentLineStatus::Producing => ("生产中", "line-status-producing"),
        FulfillmentLineStatus::Purchasing => ("采购中", "line-status-purchasing"),
        FulfillmentLineStatus::Fulfilled => ("已履约", "line-status-shipped"),
    }
}

fn acquire_tag(ch: AcquireChannel) -> (&'static str, &'static str) {
    match ch {
        AcquireChannel::SelfProduced | AcquireChannel::Legacy => ("自制", "self"),
        AcquireChannel::Purchased => ("外购", "purchase"),
        AcquireChannel::Outsourced => ("委外", "outsource"),
        AcquireChannel::NonInventory => ("非库存", "non-inventory"),
    }
}

struct ContactInfo {
    name: String,
    phone: Option<String>,
}

// ── Handlers ──

#[require_permission("SALES_ORDER", "read")]
pub async fn get_order_detail(
    path: OrderDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
    let svc = state.sales_order_service();
    let customer_svc = state.customer_service();
    let product_svc = state.product_service();
    let user_svc = state.user_service();

    let order = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
    let items = svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();
    // 履行计划
    let plan_lines = svc.list_fulfillment_plan(
        &service_ctx, &mut conn,
        FulfillmentPlanQuery { order_id: Some(path.id), status: None },
    ).await.unwrap_or_default();

    // 查询各产品当前可用库存（ATP），用于实时计算满足率
    let stock_svc = state.stock_ledger_service();
    let mut atp_map: HashMap<i64, Decimal> = HashMap::new();
    for pl in &plan_lines {
        if !atp_map.contains_key(&pl.product_id) {
            if let Ok(atp) = stock_svc.query_available(&service_ctx, &mut conn, pl.product_id, None).await {
                atp_map.insert(pl.product_id, atp);
            }
        }
    }

    // 查询该订单关联的需求池（demand）真实状态，用于「需求状态」列
    // 无 demand → 已满足（库存已锁定，无需补货）；有 demand → 按 demand.status 显示
    let demand_svc = state.sales_demand_service();
    let demands = demand_svc
        .find_by_source(&service_ctx, &mut conn, DocumentType::SalesOrder as i16, path.id)
        .await.unwrap_or_default();

    // Smart Button 统计（参考 Odoo oe_button_box）
    let producing_count = demands.iter().filter(|d| d.acquire_channel == 1).count();
    let purchasing_count = demands.iter().filter(|d| d.acquire_channel == 2).count();
    let cascade_count = demands.iter().filter(|d| d.demand_type == 2).count();

    let demand_map: HashMap<i64, DemandStatus> = demands
        .into_iter()
        .map(|d| (d.source_line_id, d.status))
        .collect();

    let customer_name = customer_svc
        .get(&service_ctx, &mut conn, order.customer_id)
        .await
        .map(|c| c.name)
        .unwrap_or_else(|_| "未知客户".into());

    let contact = {
        let contacts = customer_svc.list_contacts(&service_ctx, &mut conn, order.customer_id).await.unwrap_or_default();
        contacts.into_iter().find(|c| c.id == order.contact_id).map(|c| ContactInfo {
            name: c.name,
            phone: c.phone,
        })
    };

    let sales_rep = user_svc
        .get_user(&service_ctx, &mut conn, order.sales_rep_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());

    // 产品信息
    let (product_names, product_codes) = {
        let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
        if product_ids.is_empty() {
            (HashMap::new(), HashMap::new())
        } else {
            let products = product_svc.get_by_ids(&service_ctx, &mut conn, product_ids).await.unwrap_or_default();
            let names: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.pdt_name.clone())).collect();
            let codes: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.product_code.clone())).collect();
            (names, codes)
        }
    };

    let content = order_detail_page(
        &order, &items, &plan_lines,
        &customer_name, &contact, &sales_rep,
        &product_names, &product_codes, &atp_map, &demand_map,
        producing_count, purchasing_count, cascade_count, path.id,
    );
    let page_html = admin_page(
        is_htmx, "订单详情", &claims, "sales",
        &format!("{}/{}", OrderListPath::PATH, path.id),
        "销售管理", Some("订单详情"), content, &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn confirm_order(
    path: ConfirmOrderPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.sales_order_service();

    svc.confirm(&service_ctx, &mut conn, path.id).await?;

    let redirect = OrderDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn complete_order(
    path: CompleteOrderPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.sales_order_service();

    svc.complete(&service_ctx, &mut conn, path.id).await?;

    let redirect = OrderDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("SALES_ORDER", "update")]
pub async fn cancel_order(
    path: CancelOrderPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.sales_order_service();

    svc.cancel(&service_ctx, &mut conn, path.id).await?;

    let redirect = OrderDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Workflow Steps ──

fn workflow_steps(current: SalesOrderStatus) -> Markup {
    let steps: &[(&str, SalesOrderStatus)] = &[
        ("草稿", SalesOrderStatus::Draft),
        ("已确认", SalesOrderStatus::Confirmed),
        ("部分发货", SalesOrderStatus::PartiallyShipped),
        ("已发货", SalesOrderStatus::Shipped),
        ("已完成", SalesOrderStatus::Completed),
    ];
    let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);
    let is_cancelled = current == SalesOrderStatus::Cancelled;

    html! {
        div class="flex items-center" {
            @for (i, (label, _)) in steps.iter().enumerate() {
                @if i > 0 {
                    @let line_class = if i <= current_idx && !is_cancelled { "wf-line completed" } else { "wf-line" };
                    div class=(line_class) {}
                }
                @let step_class = if is_cancelled {
                    "wf-step"
                } else if i < current_idx {
                    "wf-step completed"
                } else if i == current_idx {
                    "wf-step current"
                } else {
                    "wf-step"
                };
                div class=(step_class) {
                    span class="w-[10px] h-[10px] rounded-full bg-border" {}
                    (label)
                }
            }
            @if is_cancelled {
                div class="w-[48px] h-[2px] bg-border" {}
                div class="flex items-center gap-2 text-xs text-text-muted danger" {
                    span class="w-[10px] h-[10px] rounded-full bg-border" {}
                    "已取消"
                }
            }
        }
    }
}

// ── Fulfillment Progress Bar ──

fn fulfillment_progress(items: &[SalesOrderItem], plan_lines: &[FulfillmentPlanLine]) -> Markup {
    // 聚合统计
    let total_ordered: Decimal = items.iter().map(|i| i.quantity).sum();
    let total_shipped: Decimal = items.iter().map(|i| i.shipped_qty).sum();
    let total_cancelled: Decimal = items.iter().map(|i| i.cancelled_qty).sum();

    // 从履行计划行聚合补货状态
    let mut total_allocated = Decimal::ZERO;
    let mut total_producing = Decimal::ZERO;
    let mut total_purchasing = Decimal::ZERO;

    for pl in plan_lines {
        match pl.status {
            FulfillmentLineStatus::Allocated => total_allocated += pl.required_qty - pl.reserved_qty,
            FulfillmentLineStatus::Producing => total_producing += pl.shortage_qty,
            FulfillmentLineStatus::Purchasing => total_purchasing += pl.shortage_qty,
            _ => {}
        }
    }

    let total_open = total_ordered - total_shipped - total_cancelled;
    let active_total = total_ordered - total_cancelled;

    // 百分比（避免除以零）
    let pct = |v: Decimal| -> String {
        if active_total > Decimal::ZERO {
            let p = (v / active_total * DECIMAL_100)
                .round_dp_with_strategy(1, rust_decimal::RoundingStrategy::MidpointAwayFromZero);
            format!("{}%", p)
        } else {
            "0%".into()
        }
    };

    let pct_shipped = pct(total_shipped);
    let pct_allocated = pct(total_allocated);
    let pct_producing = pct(total_producing);
    let pct_purchasing = pct(total_purchasing);
    let pct_pending = pct(total_open - total_allocated - total_producing - total_purchasing);

    // 只有确认后且有关联数据才显示
    let show_bar = total_ordered > Decimal::ZERO;

    html! {
        @if show_bar {
        div class="bg-bg border border-border-soft rounded p-5" {
            div class="bg-bg border border-border-soft rounded p-5-header" {
                div class="bg-bg border border-border-soft rounded p-5-title" {
                    (icon::chart_bar_icon("w-4 h-4"))
                    "履约进度"
                }
                div class="flex gap-[24px]" {
                    div class="text-center" {
                        div class="text-[10px] text-muted mt-1-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg green" { (fmt_qty(total_shipped)) }
                        div class="text-[10px] text-muted mt-1-sm text-text-muted mt-1" { "已发货" }
                    }
                    div class="text-center" {
                        div class="text-[10px] text-muted mt-1-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg blue" { (fmt_qty(total_allocated)) }
                        div class="text-[10px] text-muted mt-1-sm text-text-muted mt-1" { "已分配" }
                    }
                    div class="text-center" {
                        div class="text-[10px] text-muted mt-1-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg orange" { (fmt_qty(total_producing + total_purchasing)) }
                        div class="text-[10px] text-muted mt-1-sm text-text-muted mt-1" { "补货中" }
                    }
                    div class="text-center" {
                        div class="text-[10px] text-muted mt-1-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (fmt_qty(total_open)) }
                        div class="text-[10px] text-muted mt-1-sm text-text-muted mt-1" { "未交量" }
                    }
                }
            }
            div class="h-[8px] bg-border-soft overflow-hidden flex" {
                div class="h-1.5 bg-[rgba(0,0,0,0.06)] rounded-full overflow-hidden-shipped" style=(format!("width:{}", pct_shipped)) {}
                div class="bg-accent transition-all duration-600" style=(format!("width:{}", pct_allocated)) {}
                div class="bg-warn transition-all duration-600" style=(format!("width:{}", pct_producing)) {}
                div class="bg-[#8b5cf6] transition-all duration-600" style=(format!("width:{}", pct_purchasing)) {}
                div class="h-1.5 bg-[rgba(0,0,0,0.06)] rounded-full overflow-hidden-pending" style=(format!("width:{}", pct_pending)) {}
            }
            div class="flex gap-5 flex-wrap" {
                span class="flex gap-5 flex-wrap-item" {
                    span class="progress-w-[10px] h-[3px]" style="background:var(--success)" {}
                    "已发货 " (pct_shipped)
                }
                span class="flex gap-5 flex-wrap-item" {
                    span class="progress-w-[10px] h-[3px]" style="background:var(--accent)" {}
                    "已分配 " (pct_allocated)
                }
                span class="flex gap-5 flex-wrap-item" {
                    span class="progress-w-[10px] h-[3px]" style="background:var(--warn)" {}
                    "生产中 " (pct_producing)
                }
                span class="flex gap-5 flex-wrap-item" {
                    span class="progress-w-[10px] h-[3px]" style="background:#8b5cf6" {}
                    "采购中 " (pct_purchasing)
                }
                span class="flex gap-5 flex-wrap-item" {
                    span class="progress-w-[10px] h-[3px]" style="background:var(--border)" {}
                    "待处理 " (pct_pending)
                }
            }
        }
        }
    }
}

// ── Fulfillment Workbench ──
fn fulfillment_workbench(
    plan_lines: &[FulfillmentPlanLine],
    product_names: &HashMap<i64, String>,
    product_codes: &HashMap<i64, String>,
    atp_map: &HashMap<i64, Decimal>,
    demand_map: &HashMap<i64, DemandStatus>,
) -> Markup {
    if plan_lines.is_empty() {
        return html! {};
    }

    // 需求流转统计
    let mut demand_open = 0usize;
    let mut demand_processing = 0usize;
    let mut demand_done = 0usize;
    for pl in plan_lines {
        match pl.status {
            FulfillmentLineStatus::Pending => demand_open += 1,
            FulfillmentLineStatus::Allocated | FulfillmentLineStatus::Producing | FulfillmentLineStatus::Purchasing => demand_processing += 1,
            FulfillmentLineStatus::Fulfilled => demand_done += 1,
        }
    }
    let demand_total = plan_lines.len();

    html! {
        div class="bg-bg border border-border-soft rounded overflow-hidden" {
            div class="flex items-center justify-between p-4 border-b bg-surface-raised" {
                div class="flex items-center justify-between p-4 border-b bg-surface-raised-left" {
                    span class="text-sm font-semibold text-fg" { "履约工作台" }
                    span class="bg-[#dbeafe] text-[#2563eb] rounded-full text-[11px] font-medium" { (format!("{} 行", demand_total)) }
                }
                div class="flex gap-2" {
                    button class="fulfill-inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative" {
                        (icon::refresh_icon("w-3.5 h-3.5"))
                        "刷新状态"
                    }
                    a class="fulfill-inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative" href="/admin/mes/demand-pool" title="生产需求池" {
                        (icon::grid_icon("w-3.5 h-3.5"))
                        "生产需求池"
                    }
                    a class="fulfill-inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative" href="/admin/purchase/demand-pool" title="采购需求池" {
                        (icon::clipboard_document_icon("w-3.5 h-3.5"))
                        "采购需求池"
                    }
                    button class="fulfill-inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative primary" {
                        (icon::truck_icon("w-3.5 h-3.5"))
                        "创建发货单"
                    }
                }
            }

            // ── 需求流转状态卡片 ──
            div style="display:grid;grid-template-columns:repeat(4,1fr);gap:var(--space-3);margin-bottom:var(--space-4);" {
                div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4" {
                    div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-icon" style="background:#dbeafe;color:var(--accent);" {
                        (icon::clipboard_list_icon("w-4 h-4"))
                    }
                    div {
                        div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-value" { (demand_total) }
                        div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-label" { "总需求行" }
                    }
                }
                div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4" {
                    div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-icon" style="background:#fef3c7;color:var(--warn);" {
                        (icon::clock_icon("w-4 h-4"))
                    }
                    div {
                        div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-value" { (demand_open) }
                        div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-label" { "待处理" }
                    }
                }
                div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4" {
                    div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-icon" style="background:#ede9fe;color:#7c3aed;" {
                        (icon::refresh_icon("w-4 h-4"))
                    }
                    div {
                        div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-value" { (demand_processing) }
                        div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-label" { "处理中" }
                    }
                }
                div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4" {
                    div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-icon" style="background:#dcfce7;color:var(--success);" {
                        (icon::check_circle_icon("w-4 h-4"))
                    }
                    div {
                        div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-value" { (demand_done) }
                        div class="flex items-center gap-3 text-center bg-bg border border-border-soft rounded-lg p-4-label" { "已完成" }
                    }
                }
            }

            table class="w-full border-collapse" {
                thead {
                    tr {
                        th { "产品" }
                        th { "获取途径" }
                        th class="text-right text-[13px]" { "需求量" }
                        th class="text-right text-[13px]" { "可满足量" }
                        th class="text-right text-[13px]" { "缺口" }
                        th { "库存满足率" }
                        th { "需求状态" }
                        th { "履约状态" }
                        th { "下游单据" }
                    }
                }
                tbody {
                    @for pl in plan_lines {
                        (fulfill_plan_row(pl, product_names, product_codes, atp_map, demand_map))
                    }
                }
            }
        }
    }
}

fn fulfill_plan_row(
    pl: &FulfillmentPlanLine,
    names: &HashMap<i64, String>,
    codes: &HashMap<i64, String>,
    atp_map: &HashMap<i64, Decimal>,
    demand_map: &HashMap<i64, DemandStatus>,
) -> Markup {
    let p_name = names.get(&pl.product_id).map(|s| s.as_str()).unwrap_or("—");
    let p_code = codes.get(&pl.product_id).map(|s| s.as_str()).unwrap_or("—");
    let (ch_label, ch_class) = acquire_tag(pl.acquire_channel);
    let (st_label, st_class) = fulfill_status_pill(pl.status);

    // 需求状态 — 来自 demand 表的真实需求池状态（不再复用 fulfillment status）
    // 无 demand = 库存已满足（shortage=0，无需补货）；有 demand 则按 demand.status 显示
    let (demand_label, demand_style) = match demand_map.get(&pl.order_line_id) {
        None => ("✓ 已满足", "background:#d1fae5;color:#065f46;"),
        Some(DemandStatus::Pending) => ("⚠ 待补货", "background:#e5e7eb;color:#374151;"),
        Some(DemandStatus::Confirmed) => ("● 已确认", "background:#dbeafe;color:#1e40af;"),
        Some(DemandStatus::InProgress) => ("◐ 补货中", "background:#fef3c7;color:#92400e;"),
        Some(DemandStatus::Fulfilled) => ("✓ 补货完成", "background:#d1fae5;color:#065f46;"),
        Some(DemandStatus::Rejected) => ("✗ 已驳回", "background:#fee2e2;color:#991b1b;"),
    };

    // 满足率（含当前可用库存 ATP，实时反映入库后的库存变化）
    let current_atp = atp_map.get(&pl.product_id).copied().unwrap_or(Decimal::ZERO);
    let effective_qty = (pl.reserved_qty + current_atp).min(pl.required_qty);
    let effective_shortage = (pl.required_qty - effective_qty).max(Decimal::ZERO);
    let fill_pct_val = if pl.required_qty > Decimal::ZERO {
        (effective_qty / pl.required_qty * DECIMAL_100)
            .round_dp_with_strategy(0, rust_decimal::RoundingStrategy::MidpointAwayFromZero)
    } else {
        Decimal::ZERO
    };
    let fill_bar_pct = format!("width:{}%", fill_pct_val);
    let fill_pct_str = format!("{}%", fill_pct_val);
    let fill_color = if effective_qty >= pl.required_qty {
        "#10b981"
    } else if effective_qty > Decimal::ZERO {
        "#f59e0b"
    } else {
        "#ef4444"
    };

    // 下游单据链接
    let downstream_doc = match (pl.source_doc_type, pl.source_doc_id) {
        (Some(12), Some(doc_id)) => {
            // ProductionPlan
            Some(html! {
                a href=(format!("/admin/mes/plans/{}", doc_id)) class="text-accent font-medium cursor-pointer font-mono tabular-nums" style="font-size:12px;" {
                    (format!("PP-{}", doc_id))
                }
            })
        }
        (Some(7), Some(doc_id)) => {
            // PurchaseOrder
            Some(html! {
                a href=(format!("/admin/purchase/orders/{}", doc_id)) class="text-accent font-medium cursor-pointer font-mono tabular-nums" style="font-size:12px;" {
                    (format!("PO-{}", doc_id))
                }
            })
        }
        (Some(10), Some(doc_id)) => {
            // WorkOrder
            Some(html! {
                a href=(format!("/admin/mes/orders/{}", doc_id)) class="text-accent font-medium cursor-pointer font-mono tabular-nums" style="font-size:12px;" {
                    (format!("WO-{}", doc_id))
                }
            })
        }
        (Some(11), Some(doc_id)) => {
            // OutsourcingOrder
            Some(html! {
                a href=(format!("/admin/om/outsourcing/{}", doc_id)) class="text-accent font-medium cursor-pointer font-mono tabular-nums" style="font-size:12px;" {
                    (format!("OM-{}", doc_id))
                }
            })
        }
        _ => None,
    };

    html! {
        tr class=(if effective_shortage > Decimal::ZERO { "text-danger" } else if pl.reserved_qty > Decimal::ZERO { "text-warning" } else { "" }) {
            td {
                div class="product-cell" {
                    span class="product-name" { (p_name) }
                    span class="product-code" { (p_code) }
                }
            }
            td {
                span class=(format!("acquire-tag {}", ch_class)) { (ch_label) }
            }
            td class="text-right text-[13px]" { (fmt_qty(pl.required_qty)) }
            td class="text-right text-[13px]" { (fmt_qty(effective_qty)) }
            td class="text-right text-[13px]" {
                @if effective_shortage > Decimal::ZERO {
                    span class="text-danger" { (fmt_qty(effective_shortage)) }
                } @else {
                    span style="color:var(--success);" { "0" }
                }
            }
            td {
                div style="display:flex;align-items:center;gap:8px;" {
                    div style="flex:1;background:#e5e7eb;height:6px;border-radius:3px;overflow:hidden;" {
                        div style=(format!("width:{};background:{};height:100%;", fill_bar_pct, fill_color)) {}
                    }
                    span style="font-size:12px;color:var(--muted);" { (fill_pct_str) }
                }
            }
            td {
                span style=(format!("padding:2px 8px;border-radius:12px;font-size:12px;{}", demand_style)) { (demand_label) }
            }
            td {
                span class=(format!("line-status {}", st_class)) { (st_label) }
            }
            td {
                @if let Some(doc) = downstream_doc {
                    (doc)
                } @else {
                    span class="text-text-muted" { "—" }
                }
            }
        }
    }
}

fn order_detail_page(
    o: &SalesOrder,
    items: &[SalesOrderItem],
    plan_lines: &[FulfillmentPlanLine],
    customer_name: &str,
    contact: &Option<ContactInfo>,
    sales_rep: &str,
    product_names: &HashMap<i64, String>,
    product_codes: &HashMap<i64, String>,
    atp_map: &HashMap<i64, Decimal>,
    demand_map: &HashMap<i64, DemandStatus>,
    producing_count: usize,
    purchasing_count: usize,
    cascade_count: usize,
    order_id: i64,
) -> Markup {
    let (status_text, status_class) = status_label(o.status);
    let contact_name = contact.as_ref().map(|c| c.name.as_str()).unwrap_or("—");
    let contact_phone = contact.as_ref().and_then(|c| c.phone.as_deref()).unwrap_or("—");
    html! {
        div {
            // ── Back Link ──
            a class="inline-flex items-center gap-2 text-sm text-text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", OrderListPath::PATH)) {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回销售订单列表"
            }

            // ── Detail Header ──
            div class="block bg-bg border border-border-soft rounded-lg p-6" {
                div {
                    div class="flex items-center justify-between" {
                        h1 class="text-2xl font-extrabold font-font-mono tabular-nums" { (o.doc_number) }
                        span class=(format!("status-pill {status_class}")) { (status_text) }
                    }
                }
                div class="flex gap-3" {
                    button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" {
                        (icon::printer_icon("w-4 h-4"))
                        "打印"
                    }
                    @if matches!(o.status, SalesOrderStatus::Confirmed | SalesOrderStatus::PartiallyShipped) {
                        a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href="#" {
                            (icon::truck_icon("w-4 h-4"))
                            "创建发货申请"
                        }
                    }
                    @if o.status == SalesOrderStatus::Draft {
                        button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                            hx-post=(ConfirmOrderPath { id: o.id }.to_string())
                            hx-confirm="确认审核此订单？" { "确认订单" }
                    }
                    @if matches!(o.status, SalesOrderStatus::Draft | SalesOrderStatus::Confirmed) {
                        button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90"
                            hx-post=(CancelOrderPath { id: o.id }.to_string())
                            hx-confirm="确认取消此订单？取消后不可恢复。" { "取消订单" }
                    }
                }
            }


            // ── Smart Buttons（参考 Odoo oe_button_box）──
            @if producing_count > 0 || purchasing_count > 0 || cascade_count > 0 {
                div class="flex gap-3 mb-4" {
                    @if producing_count > 0 {
                        a class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)] flex items-center gap-2 px-4 py-2 hover:shadow-md transition-shadow"
                          href=(format!("/admin/mes/demand-pool?order_id={}", order_id))
                        {
                            span class="text-2xl font-bold text-blue-600" { (producing_count) }
                            span class="text-sm text-gray-500" { "自制需求" }
                        }
                    }
                    @if purchasing_count > 0 {
                        a class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)] flex items-center gap-2 px-4 py-2 hover:shadow-md transition-shadow"
                          href=(format!("/admin/purchase/demand-pool?order_id={}", order_id))
                        {
                            span class="text-2xl font-bold text-orange-600" { (purchasing_count) }
                            span class="text-sm text-gray-500" { "采购需求" }
                        }
                    }
                    @if cascade_count > 0 {
                        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)] flex items-center gap-2 px-4 py-2" {
                            span class="text-2xl font-bold text-purple-600" { (cascade_count) }
                            span class="text-sm text-gray-500" { "BOM展开需求" }
                        }
                    }
                }
            }
            // ── Workflow Steps ──
            (workflow_steps(o.status))

            // ── Fulfillment Progress ──
            (fulfillment_progress(items, plan_lines))

            // ── Order Info ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]-title" { "订单信息" }
                div class="grid gap-4" {
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "客户名称" }
                        span class="text-sm text-fg font-medium" { (customer_name) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "联系人" }
                        span class="text-sm text-fg font-medium" { (contact_name) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "联系电话" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" { (contact_phone) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "业务员" }
                        span class="text-sm text-fg font-medium" { (sales_rep) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "交货日期" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" { (o.order_date.format("%Y-%m-%d")) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "付款条款" }
                        span class="text-sm text-fg font-medium" { (o.payment_terms.as_str()) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "交货条款" }
                        span class="text-sm text-fg font-medium" { (o.delivery_terms.as_str()) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "交货地址" }
                        span class="text-sm text-fg font-medium" { (o.delivery_address.as_str()) }
                    }
                }
            }

            // ── Items Table (四量模型) ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" {
                div class="overflow-x-auto" {
                    table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:text-left [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" {
                        thead {
                            tr {
                                th { "行号" }
                                th { "产品编码" }
                                th { "产品名称" }
                                th { "单位" }
                                th class="text-right text-[13px]" { "订单量" }
                                th class="text-right text-[13px]" { "已发货" }
                                th class="text-right text-[13px]" { "已取消" }
                                th class="text-right text-[13px]" { "未交量" }
                                th class="text-right text-[13px]" { "单价" }
                                th class="text-right text-[13px]" { "小计" }
                                th { "行状态" }
                                th { "交货日期" }
                            }
                        }
                        tbody {
                            @for item in items {
                                (item_row(item, product_names, product_codes))
                            }
                            @if items.is_empty() {
                                tr {
                                    td colspan="12" class="text-center p-8 text-text-muted" {
                                        "暂无明细"
                                    }
                                }
                            }
                        }
                    }
                }
                div class="flex justify-end gap-8 p-5 border-t bg-surface-raised" {
                    div class="flex gap-3" {
                        span class="text-[11px] text-text-muted font-medium uppercase" { "成本合计" }
                        span class="text-[20px] font-bold text-fg" { (crate::utils::fmt_amount(o.total_cost)) }
                    }
                    div class="flex gap-3" {
                        span class="text-[11px] text-text-muted font-medium uppercase" { "订单总额" }
                        span class="text-[20px] font-bold text-fg accent" { (crate::utils::fmt_amount(o.total_amount)) }
                    }
                }
            }

            // ── Fulfillment Workbench ──
            (fulfillment_workbench(plan_lines, product_names, product_codes, atp_map, demand_map))

            // ── Remarks ──
            @if !o.remark.is_empty() {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" style="margin-top:var(--space-6)" {
                    div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]-title" { "备注" }
                    p class="text-text-muted" { (o.remark.as_str()) }
                }
            }
        }
    }
}

fn item_row(
    item: &SalesOrderItem,
    names: &HashMap<i64, String>,
    codes: &HashMap<i64, String>,
) -> Markup {
    let product_name = names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let product_code = codes.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let delivery = item.delivery_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into());
    let open_qty = item.open_qty();
    let (ls_label, ls_class) = line_status_pill(item.line_status);

    html! {
        tr {
            td class="font-mono tabular-nums" { (item.line_no) }
            td class="font-mono tabular-nums" { (product_code) }
            td { (product_name) }
            td { (item.unit.as_str()) }
            td class="text-right text-[13px]" { (fmt_qty(item.quantity)) }
            td class="text-right text-[13px]" { (fmt_qty(item.shipped_qty)) }
            td class="text-right text-[13px]" { (fmt_qty(item.cancelled_qty)) }
            td class="text-right text-[13px]" {
                @if open_qty > Decimal::ZERO {
                    span class="text-danger" { (fmt_qty(open_qty)) }
                } @else {
                    (fmt_qty(open_qty))
                }
            }
            td class="text-right text-[13px]" { (crate::utils::fmt_amount(item.unit_price)) }
            td class="text-right text-[13px]" { (crate::utils::fmt_amount(item.amount)) }
            td {
                span class=(format!("line-status {}", ls_class)) { (ls_label) }
            }
            td class="font-mono tabular-nums" { (delivery) }
        }
    }
}
