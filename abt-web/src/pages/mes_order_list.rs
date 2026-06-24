use std::collections::HashMap;

use abt_core::mes::work_order::{WorkOrderFilter, WorkOrderService};
use abt_core::master_data::product::ProductService;
use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::components::icon;
use crate::components::material_badge::material_badge_mini;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_order::{OrderCreatePath, OrderDetailPath, OrderListPath, OrderRowDetailPath};
use crate::utils::{empty_as_none, fmt_qty, RequestContext};
use abt_macros::require_permission;

use abt_core::mes::enums::WorkOrderStatus;
use abt_core::mes::work_order::MaterialAvailabilityLevel;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct OrderQueryParams {
    #[serde(default, deserialize_with = "empty_as_none")]
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

/// 状态 pill：(标签, 语义色 token) — 颜色用 UnoCSS 语义 token，禁 hex。
fn wo_status_meta(s: &WorkOrderStatus) -> (&'static str, &'static str) {
    use WorkOrderStatus::*;
    match s {
        Draft => ("待计划", "muted"),
        Planned => ("已计划", "accent"),
        Released => ("已下达", "success"),
        InProduction => ("生产中", "warn"),
        Closed => ("已关闭", "purple"),
        Cancelled => ("已取消", "danger"),
    }
}

fn parse_wo_status(s: &str) -> Option<WorkOrderStatus> {
    use WorkOrderStatus::*;
    match s {
        "Draft" => Some(Draft),
        "Planned" => Some(Planned),
        "Released" => Some(Released),
        "InProduction" => Some(InProduction),
        "Closed" => Some(Closed),
        "Cancelled" => Some(Cancelled),
        _ => None,
    }
}

#[require_permission("WORK_ORDER", "read")]
pub async fn get_order_list(
    _path: OrderListPath,
    ctx: RequestContext,
    Query(params): Query<OrderQueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_create = ctx.has_permission("WORK_ORDER", "create").await;
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;
    let svc = state.work_order_service();
    let product_svc = state.product_service();
    let filter = WorkOrderFilter {
        status: params.status.as_deref().and_then(parse_wo_status),
        product_id: None,
        keyword: params.keyword.clone(),
        date_from: None,
        date_to: None,
        product_code: None,
    };
    let result = svc
        .list(&service_ctx, &mut conn, filter, params.page.unwrap_or(1), 20)
        .await?;

    let product_names: HashMap<i64, String> = {
        let pids: Vec<i64> = result.items.iter().map(|i| i.product_id).collect();
        product_svc
            .get_by_ids(&service_ctx, &mut conn, pids)
            .await
            .map(|ps| ps.iter().map(|p| (p.product_id, p.pdt_name.clone())).collect())
            .unwrap_or_default()
    };

    // 批量物料可用性（列表降级 2 级）— 已关闭/取消工单返回 Available+None
    let wo_ids: Vec<i64> = result.items.iter().map(|i| i.id).collect();
    let availability = svc
        .compute_availability_batch(&service_ctx, &mut conn, &wo_ids)
        .await?;

    let content = order_list_page(&result, &product_names, &availability, &params, can_create);
    Ok(Html(
        admin_page(
            is_htmx,
            "工单管理",
            &claims,
            "production",
            OrderListPath::PATH,
            "生产管理",
            None,
            content,
            &nav_filter,
        )
        .into_string(),
    ))
}

fn order_list_page(
    result: &abt_core::shared::types::PaginatedResult<abt_core::mes::work_order::WorkOrder>,
    product_names: &HashMap<i64, String>,
    availability: &HashMap<i64, (MaterialAvailabilityLevel, Option<String>)>,
    params: &OrderQueryParams,
    can_create: bool,
) -> Markup {
    html! {
        div {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "工单管理" }
                div class="flex gap-3" {
                    @if can_create {
                        a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                            href=(OrderCreatePath::PATH)
                        { (icon::plus_icon("w-4 h-4")) "新建工单" }
                    }
                }
            }
            (order_table_fragment(result, product_names, availability, params))
        }
    }
}

fn order_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<abt_core::mes::work_order::WorkOrder>,
    product_names: &HashMap<i64, String>,
    availability: &HashMap<i64, (MaterialAvailabilityLevel, Option<String>)>,
    params: &OrderQueryParams,
) -> Markup {
    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(result.total) },
        TabItem { value: "Draft".into(), label: "待计划", count: None },
        TabItem { value: "Planned".into(), label: "已计划", count: None },
        TabItem { value: "Released".into(), label: "已下达", count: None },
        TabItem { value: "InProduction".into(), label: "生产中", count: None },
        TabItem { value: "Closed".into(), label: "已关闭", count: None },
    ];
    let sel = params.status.as_deref().unwrap_or("");

    html! {
        div {
            ({
                status_tabs_with_param(
                    OrderListPath::PATH,
                    "#order-data-card",
                    "#filter-form",
                    tabs,
                    sel,
                    "status",
                )
            })
            form
                id="filter-form"
                class="flex items-center gap-3 mb-5 flex-wrap filter-form"
                hx-get=(OrderListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#order-data-card"
                hx-select="#order-data-card"
                hx-swap="outerHTML"
                hx-include="#filter-form"
                hx-push-url="true"
            {
                div class="relative flex-1 max-w-xs icon:absolute icon:left-3 icon:top-1/2 icon:-translate-y-1/2 icon:w-4 icon:h-4 icon:text-muted"
                {
                    (icon::search_icon(""))
                    input
                        class="w-[180px] pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input"
                        type="text"
                        name="keyword"
                        placeholder="搜索工单编号…"
                        value=(params.keyword.as_deref().unwrap_or(""));
                }
            }
            (order_data_card(result, product_names, availability, params))
        }
    }
}

fn order_data_card(
    result: &abt_core::shared::types::PaginatedResult<abt_core::mes::work_order::WorkOrder>,
    product_names: &HashMap<i64, String>,
    availability: &HashMap<i64, (MaterialAvailabilityLevel, Option<String>)>,
    params: &OrderQueryParams,
) -> Markup {
    let mut qs = vec![];
    if let Some(ref k) = params.keyword {
        qs.push(format!("keyword={k}"));
    }
    if let Some(ref s) = params.status {
        qs.push(format!("status={s}"));
    }
    let query = qs.join("&");
    html! {
        div class="data-card" id="order-data-card" {
            div class="overflow-x-auto" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "工单编号" }
                            th { "产品" }
                            th { "物料" }
                            th { "生产进度" }
                            th { "状态" }
                            th class="!text-right" { "操作" }
                        }
                    }
                    tbody {
                        @for item in &result.items {
                            (order_data_row(item, product_names, availability))
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="6" class="text-center text-muted py-8" { "暂无工单" }
                            }
                        }
                    }
                }
            }
            ({
                pagination(
                    OrderListPath::PATH,
                    &query,
                    result.total,
                    result.page,
                    result.total_pages,
                )
            })
        }
    }
}

/// 单行（data-row）：6 列。工单号链接 onclick stopPropagation 跳详情；
/// 展开按钮 hx-get 懒加载行详情 tr（afterend）+ Hyperscript toggle .open。
fn order_data_row(
    item: &abt_core::mes::work_order::WorkOrder,
    product_names: &HashMap<i64, String>,
    availability: &HashMap<i64, (MaterialAvailabilityLevel, Option<String>)>,
) -> Markup {
    let (slabel, stoken) = wo_status_meta(&item.status);
    let pn = product_names
        .get(&item.product_id)
        .map(|s| s.as_str())
        .unwrap_or("\u{2014}");
    let detail_url = OrderDetailPath { id: item.id }.to_string();
    let total = item.total_steps.unwrap_or(0);
    let done = item.completed_steps.unwrap_or(0);

    // 物料徽章：已关闭/取消工单 availability 为 Available+None → 显示空占位（—）
    let is_terminal = matches!(item.status, WorkOrderStatus::Closed | WorkOrderStatus::Cancelled);
    let avail_cell = if is_terminal {
        html! { span class="text-muted text-sm" { "—" } }
    } else {
        let (level, headline) = availability
            .get(&item.id)
            .cloned()
            .unwrap_or((MaterialAvailabilityLevel::Available, None));
        material_badge_mini(level, headline.as_deref())
    };

    html! {
        tr class="data-row cursor-pointer hover:bg-accent-bg"
            onclick=(format!("location.href='{}'", detail_url))
        {
            // ① 工单编号（链接，onclick stopPropagation）
            td  class="link-cell text-accent font-medium cursor-pointer font-mono tabular-nums"
                onclick=(format!("event.stopPropagation(); location.href='{}'", detail_url))
            { (item.doc_number) }

            // ② 产品 + 数量（cell-stack）
            td {
                div class="flex flex-col gap-[2px]" {
                    span class="font-medium" { (pn) }
                    span class="sub text-xs text-muted" {
                        (fmt_qty(item.planned_qty)) " 件"
                        @if let Some(wc) = item.work_center_id {
                            span class="text-muted" { " · " (wc) }
                        }
                    }
                }
            }

            // ③ 物料徽章
            td { (avail_cell) }

            // ④ 生产进度（wo-progress 条 + 文本）
            td { (progress_cell(item, total, done)) }

            // ⑤ 状态 pill
            td {
                span class=({ format!("status-pill inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium bg-{}-bg text-{}", stoken, stoken) }) {
                    span class=({ format!("inline-block w-1.5 h-1.5 rounded-full bg-{}", stoken) }) {}
                    (slabel)
                }
            }

            // ⑥ 操作：展开按钮 + 进入工作台箭头
            td class="!text-right whitespace-nowrap" {
                button
                    class="expand-btn inline-flex items-center justify-center w-[26px] h-[26px] border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-bg hover:text-fg align-middle transition-all duration-150"
                    title="展开详情"
                    hx-get=(OrderRowDetailPath { order_id: item.id }.to_string())
                    hx-target="this"
                    hx-swap="afterend"
                    _="on click toggle .open on closest <tr/>"
                    onclick="event.stopPropagation()"
                {
                    (icon::chevron_right_icon("w-[15px] h-[15px] transition-transform duration-150"))
                }
                a
                    class="row-action-btn inline-flex items-center justify-center align-middle text-accent hover:text-accent-hover ml-1"
                    href=(detail_url)
                    title="进入工作台"
                    onclick="event.stopPropagation()"
                {
                    (icon::arrow_right_icon("w-4 h-4"))
                }
            }
        }
    }
}

/// 进度单元格：进度条（low/mid/high 三色）+ 文本。
fn progress_cell(item: &abt_core::mes::work_order::WorkOrder, total: i32, done: i32) -> Markup {
    // 完工百分比（按 received/completed qty vs planned）
    let pct = if item.planned_qty > Decimal::ZERO {
        let p = item.completed_qty / item.planned_qty * Decimal::from(100);
        if p > Decimal::from(100) {
            Decimal::from(100)
        } else {
            p
        }
    } else {
        Decimal::ZERO
    };
    let not_started = total == 0 && item.completed_qty == Decimal::ZERO;

    if not_started {
        return html! {
            div class="flex flex-col gap-[3px]" {
                div class="wo-progress w-[84px] h-[6px] bg-border-soft rounded-[3px] overflow-hidden" {
                    div class="wo-progress-bar low h-full rounded-[3px]" style="width:0%" {}
                }
                div class="wo-progress-text text-[11px] text-muted font-mono tabular-nums" { "尚未开始" }
            }
        };
    }

    // 条颜色：low(<30)/mid(<85)/high(≥85)
    let bar_class = if pct >= Decimal::from(85) {
        "high"
    } else if pct >= Decimal::from(30) {
        "mid"
    } else {
        "low"
    };
    let pct_str = fmt_qty(pct);
    let text = if item.completed_qty > Decimal::ZERO {
        format!("{}% · {}/{}", pct_str, fmt_qty(item.completed_qty), fmt_qty(item.planned_qty))
    } else if total > 0 {
        if done >= total {
            "工序完成".to_string()
        } else {
            format!("工序 {}/{}", done, total)
        }
    } else {
        format!("{}%", pct_str)
    };

    html! {
        div class="flex flex-col gap-[3px]" {
            div class="wo-progress w-[84px] h-[6px] bg-border-soft rounded-[3px] overflow-hidden" {
                div class=({ format!("wo-progress-bar {} h-full rounded-[3px] transition-all duration-150", bar_class) })
                    style=({ format!("width:{}%", pct_str) }) {}
            }
            div class="wo-progress-text text-[11px] text-muted font-mono tabular-nums" { (text) }
        }
    }
}

// =============================================================================
// C3: 行内展开（懒加载）— hx-get OrderRowDetailPath 返回单个 <tr class="row-detail">
// =============================================================================

#[require_permission("WORK_ORDER", "read")]
pub async fn get_order_row_detail(
    path: OrderRowDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.work_order_service();
    let summary = svc
        .get_hub_summary(&service_ctx, &mut conn, path.order_id)
        .await?;

    Ok(Html(row_detail_tr(&summary).into_string()))
}

/// 渲染单个 `<tr class="row-detail"><td colspan="6">...</td></tr>`（照原型 row-detail-inner）。
fn row_detail_tr(summary: &abt_core::mes::work_order::WorkOrderHubSummary) -> Markup {
    let order = &summary.order;
    let hub_url = OrderDetailPath { id: order.id }.to_string();

    // 来源链
    let source_cell = html! {
        @if summary.source_chain.sales_order_doc.is_some()
            || summary.source_chain.plan_doc.is_some()
        {
            div class="db-val text-sm text-fg leading-relaxed" {
                @if let Some(sodoc) = summary.source_chain.sales_order_doc.as_deref() {
                    @if let Some(soid) = order.sales_order_id {
                        a class="text-accent font-medium" href=(format!("/admin/orders/{}", soid)) { (sodoc) }
                    } @else {
                        span { (sodoc) }
                    }
                    span class="text-muted mx-1" { "→" }
                }
                @if let (Some(pid), Some(pdoc)) =
                    (order.source_plan_id, summary.source_chain.plan_doc.as_deref())
                {
                    a class="text-accent font-medium" href=(format!("/admin/mes/plans/{}", pid)) { (pdoc) }
                }
                @if let Some(cust) = summary.source_chain.customer_name.as_deref() {
                    br;
                    span class="text-[11px] text-muted" { "客户：" (cust) }
                }
            }
        } @else {
            span class="text-sm text-muted" { "手动创建（无上游单据）" }
        }
    };

    // 排程 / 班组
    let schedule_cell = html! {
        div class="db-val text-sm text-fg leading-relaxed" {
            span class="font-mono tabular-nums" {
                (order.scheduled_start.format("%m-%d")) " → " (order.scheduled_end.format("%m-%d"))
            }
            br;
            span class="text-muted" {
                (summary.info.routing_step_count) " 工序"
                @if let Some(wc) = summary.work_center_name.as_deref() {
                    " · " (wc)
                }
            }
        }
    };

    // 批次进度摘要
    let batch_count = summary.source_chain.batch_count;
    let batch_cell = if batch_count == 0 {
        html! {
            div class="db-val text-sm" {
                "尚未拆批"
                br;
                span class="text-[11px] text-muted" { "物料齐套，可开工" }
            }
        }
    } else {
        html! {
            div class="db-val text-sm leading-relaxed" {
                (batch_count) " 批次 · "
                span class="text-success font-semibold" {
                    (fmt_qty(summary.received_qty)) " 入库"
                }
                br;
                span class="text-[11px] text-muted" {
                    "进行中 " (fmt_qty(summary.in_progress_qty))
                }
            }
        }
    };

    // 报工 / 入库摘要
    let report_cell = html! {
        div class="db-val text-sm leading-relaxed" {
            "报工 " (summary.reports.total_count) " 次"
            @if summary.reports.total_defect > Decimal::ZERO {
                span class="text-danger font-semibold ml-1" {
                    "报废 " (fmt_qty(summary.reports.total_defect))
                }
            }
            br;
            @if summary.receipts.fqc_passed {
                span class="text-success" { "FQC 通过" }
            } @else if summary.receipts.total_received > Decimal::ZERO {
                span class="text-muted" { "FQC 进行中" }
            } @else {
                span class="text-muted" { "—" }
            }
            @if summary.receipts.backflush_done {
                span class="text-muted" { " · 倒冲完成" }
            }
        }
    };

    // 行内操作按钮（MVP 直接链工作台）
    let actions = match order.status {
        WorkOrderStatus::Closed | WorkOrderStatus::Cancelled => html! {
            a class="hub-link inline-flex items-center gap-1 text-sm text-accent font-semibold hover:underline"
              href=(hub_url)
            {
                "进入工作台 " (icon::arrow_right_icon("w-[14px] h-[14px]"))
            }
        },
        _ => html! {
            a class="inline-flex items-center gap-1 px-3 py-1.5 rounded-sm border border-border text-sm text-fg hover:bg-accent-bg hover:border-accent hover:text-accent transition-all duration-150"
              href=(hub_url)
            { "报工" }
            a class="inline-flex items-center gap-1 px-3 py-1.5 rounded-sm border border-border text-sm text-fg hover:bg-accent-bg hover:border-accent hover:text-accent transition-all duration-150"
              href=(hub_url)
            { "入库登记" }
            a class="inline-flex items-center gap-1 px-3 py-1.5 rounded-sm border border-border text-sm text-fg hover:bg-accent-bg hover:border-accent hover:text-accent transition-all duration-150"
              href=(hub_url)
            { "拆批" }
            a class="hub-link inline-flex items-center gap-1 text-sm text-accent font-semibold hover:underline ml-auto"
              href=(hub_url)
            {
                "进入工作台 " (icon::arrow_right_icon("w-[14px] h-[14px]"))
            }
        },
    };

    html! {
        tr class="row-detail" {
            td colspan="6" class="p-0 border-none bg-surface-raised" {
                div class="row-detail-inner p-5 border-t border-dashed border-border-soft border-b border-border-soft" {
                    div class="grid grid-cols-4 gap-5 mb-4" {
                        div class="detail-block" {
                            div class="db-label text-[11px] text-muted font-medium mb-[5px] uppercase tracking-wide" { "来源链" }
                            (source_cell)
                        }
                        div class="detail-block" {
                            div class="db-label text-[11px] text-muted font-medium mb-[5px] uppercase tracking-wide" { "排程 / 班组" }
                            (schedule_cell)
                        }
                        div class="detail-block" {
                            div class="db-label text-[11px] text-muted font-medium mb-[5px] uppercase tracking-wide" { "批次进度" }
                            (batch_cell)
                        }
                        div class="detail-block" {
                            div class="db-label text-[11px] text-muted font-medium mb-[5px] uppercase tracking-wide" { "报工 / 入库" }
                            (report_cell)
                        }
                    }
                    div class="detail-actions flex items-center gap-2 pt-3 border-t border-border-soft flex-wrap" {
                        (actions)
                    }
                }
            }
        }
    }
}
