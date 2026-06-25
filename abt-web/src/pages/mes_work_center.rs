//! MES 生产作业中心 — 需求池 / 订单排期 / 工单 三 card 聚合工作台。
//!
//! 架构（组件化单端点模式）：
//! - 首页内联渲染 3 个 card 外壳，每个 card 占位 div `hx-trigger="load"` 拉各自端点；
//! - 每个 card 一个 GET 端点，card 内 tab/筛选/分页走该端点 + `hx-select="#wc-xxx-card"` 局部刷新；
//! - 写操作（下达/分批/报工）POST 广播 `HX-Trigger: woChanged`，相关 card 声明
//!   `hx-trigger="woChanged from:body"` 自刷新；工序加载/编辑复用既有 mes_order 端点（广播 routingChanged）。

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use chrono::NaiveDate;
use maud::{html, Markup};
use rust_decimal::Decimal;
use serde::Deserialize;

use abt_core::mes::demand_handler::{
    DemandPoolQuery, DemandSummary, MaterialAggQuery, MaterialAggSummary, MesDemandService,
};
use abt_core::mes::enums::{ShiftType, WorkOrderStatus};
use abt_core::mes::production_batch::{
    ProductionBatch, ProductionBatchService, SplitReq, StepConfirmationReq, WorkOrderRouting,
};
use abt_core::mes::work_center::{MesWorkCenterService, MesWorkCenterSummary};
use abt_core::master_data::product::ProductService;
use abt_core::mes::work_order::{
    MaterialAvailabilityLevel, WorkOrder, WorkOrderFilter, WorkOrderService,
};
use abt_core::shared::types::{DomainError, PageParams};

use std::collections::HashMap;

use crate::components::icon;
use crate::components::material_badge::material_badge_mini;
use crate::components::pagination::pagination;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_order::{OrderRoutingApplyFromRoutingPath, OrderRoutingLoadRecentPath};
use crate::routes::mes_work_center::*;
use crate::utils::{empty_as_none, fmt_qty, RequestContext};
use abt_macros::require_permission;

// =============================================================================
// 首页
// =============================================================================

#[require_permission("WORK_ORDER", "read")]
pub async fn get_work_center(_path: WcPath, ctx: RequestContext) -> Result<Html<String>> {
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
        .mes_work_center_service()
        .summary(&service_ctx, &mut conn)
        .await
        .unwrap_or_default();

    let content = html! {
        div class="flex items-center justify-between mb-4 flex-wrap gap-4" {
            div {
                h1 class="text-xl font-bold text-fg tracking-tight" { "生产作业中心" }
                p class="text-sm text-muted mt-1" { "需求池 · 订单排期 · 工单 一屏处理，就地下达与报工" }
            }
        }
        (render_anchor_nav(&summary))
        (render_card_shell("wc-demand-card", WcDemandPath::PATH, "生产需求池"))
        (render_card_shell("wc-schedule-card", WcSchedulePath::PATH, "订单排期"))
        (render_card_shell("wc-orders-card", WcOrdersPath::PATH, "工单"))
        (render_drawer_overlay("release-overlay", "release-drawer", "release-drawer-body", "下达生产订单"))
        (render_drawer_overlay("report-overlay", "report-drawer", "report-drawer-body", "工序报工"))
    };

    Ok(Html(
        admin_page(
            is_htmx,
            "生产作业中心",
            &claims,
            "production",
            WcPath::PATH,
            "生产管理",
            Some("生产作业中心"),
            content,
            &nav_filter,
        )
        .into_string(),
    ))
}

// =============================================================================
// Card 端点（占位 — 后续 Edit 逐个填充真实数据渲染）
// =============================================================================

#[derive(Debug, Deserialize, Clone, Default)]
pub struct DemandCardParams {
    /// "material" | "detail"（默认 material）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub view: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub date_filter: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

/// 需求池 card（单端点）：物料汇总 / 订单行明细 两 tab，搜索 + 日期过滤 + 分页。
#[require_permission("WORK_ORDER", "read")]
pub async fn get_demand_card(
    _path: WcDemandPath,
    ctx: RequestContext,
    Query(p): Query<DemandCardParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.mes_demand_service();
    let view = p.view.as_deref().unwrap_or("material");
    let page = p.page.unwrap_or(1);
    let (date_start, date_end) = parse_date_filter(p.date_filter.as_deref());

    let body = if view == "detail" {
        let result = svc
            .list_pending_demands(
                &service_ctx,
                &mut conn,
                DemandPoolQuery {
                    keyword: p.keyword.clone(),
                    required_date_start: date_start,
                    required_date_end: date_end,
                    ..Default::default()
                },
                PageParams::new(page, 10),
            )
            .await?;
        render_demand_detail(&result, &p)
    } else {
        let result = svc
            .list_material_aggregated(
                &service_ctx,
                &mut conn,
                MaterialAggQuery {
                    keyword: p.keyword.clone(),
                    required_date_start: date_start,
                    required_date_end: date_end,
                    ..Default::default()
                },
                PageParams::new(page, 10),
            )
            .await?;
        render_demand_material(&result, &p)
    };

    Ok(Html(
        html! {
            div id="wc-demand-card" {
                (demand_filter_bar(view, &p))
                (body)
            }
        }
        .into_string(),
    ))
}

// ── 需求池渲染 ──

fn parse_date_filter(df: Option<&str>) -> (Option<NaiveDate>, Option<NaiveDate>) {
    let today = chrono::Local::now().date_naive();
    match df {
        Some("7days") => (None, Some(today + chrono::TimeDelta::days(7))),
        Some("30days") => (None, Some(today + chrono::TimeDelta::days(30))),
        Some("overdue") => (None, Some(today)),
        _ => (None, None),
    }
}

/// tab 切换 + 搜索 + 日期过滤（统一 hx-get WcDemandPath + hx-select #wc-demand-card）。
fn demand_filter_bar(view: &str, p: &DemandCardParams) -> Markup {
    let is_mat = view == "material";
    let kw = p.keyword.as_deref().unwrap_or("");
    let df = p.date_filter.as_deref().unwrap_or("");
    html! {
        div class="flex items-center justify-between flex-wrap gap-3 px-5 py-3 border-b border-border-soft" {
            div class="inline-flex bg-surface border border-border-soft rounded-md p-[3px] gap-0.5" {
                button class=(toggle_cls(is_mat)) type="button"
                    hx-get=(WcDemandPath::PATH)
                    hx-vals="{\"view\":\"material\"}"
                    hx-target="#wc-demand-card" hx-select="#wc-demand-card" hx-swap="outerHTML"
                    hx-push-url="true" hx-include="#wc-demand-filter-form"
                    { "物料汇总" }
                button class=(toggle_cls(!is_mat)) type="button"
                    hx-get=(WcDemandPath::PATH)
                    hx-vals="{\"view\":\"detail\"}"
                    hx-target="#wc-demand-card" hx-select="#wc-demand-card" hx-swap="outerHTML"
                    hx-push-url="true" hx-include="#wc-demand-filter-form"
                    { "订单行明细" }
            }
            form class="flex items-center gap-2"
                hx-get=(WcDemandPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.wc-search-input"
                hx-target="#wc-demand-card" hx-select="#wc-demand-card" hx-swap="outerHTML"
                hx-push-url="true" {
                input type="hidden" name="view" value=(view);
                div class="relative" {
                    (icon::search_icon("w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"));
                    input class="wc-search-input w-[180px] pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                        type="text" name="keyword" placeholder="搜索物料/订单"
                        value=(kw);
                }
                select class="px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg cursor-pointer"
                    name="date_filter" {
                    option value="" selected[df.is_empty()] { "全部日期" }
                    option value="7days" selected[df == "7days"] { "近7天到期" }
                    option value="30days" selected[df == "30days"] { "近30天到期" }
                    option value="overdue" selected[df == "overdue"] { "已逾期" }
                }
            }
            // 隐藏表单：tab 切换时携带 keyword/date_filter
            form id="wc-demand-filter-form" class="hidden" {
                input type="hidden" name="keyword" value=(kw);
                input type="hidden" name="date_filter" value=(df);
            }
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

fn render_demand_material(
    result: &abt_core::shared::types::PaginatedResult<MaterialAggSummary>,
    p: &DemandCardParams,
) -> Markup {
    let qs = demand_query_string("material", p.keyword.as_deref(), p.date_filter.as_deref());
    html! {
        div class="overflow-x-auto" {
            table class="w-full text-sm" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="text-left font-semibold py-2 px-5 uppercase tracking-wide" { "物料" }
                        th class="text-right font-semibold py-2 px-3 uppercase tracking-wide" { "总需求量" }
                        th class="text-center font-semibold py-2 px-3 uppercase tracking-wide" { "订单数" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "需求日期" }
                    }
                }
                tbody {
                    @if result.items.is_empty() {
                        tr { td colspan="4" class="text-center text-muted py-8" { "暂无待处理需求" } }
                    }
                    @for item in &result.items {
                        (demand_material_row(item))
                    }
                }
            }
        }
        (pagination(WcDemandPath::PATH, &qs, result.total, result.page, result.total_pages))
    }
}

fn demand_material_row(item: &MaterialAggSummary) -> Markup {
    let earliest = item
        .earliest_required_date
        .map(|d| d.format("%m/%d").to_string())
        .unwrap_or_else(|| "—".into());
    let latest = item
        .latest_required_date
        .map(|d| d.format("%m/%d").to_string())
        .unwrap_or_else(|| "—".into());
    html! {
        tr class="border-b border-border-soft hover:bg-accent-bg" {
            td class="py-2.5 px-5" {
                div class="font-medium text-fg" { (item.product_name) }
                div class="text-xs text-muted font-mono" { (item.product_code) }
            }
            td class="py-2.5 px-3 text-right font-mono tabular-nums font-semibold text-fg" {
                (fmt_qty(item.total_demand_qty))
            }
            td class="py-2.5 px-3 text-center font-mono tabular-nums text-accent" { (item.demand_count) }
            td class="py-2.5 px-3 text-fg-2 font-mono" { (earliest) " → " (latest) }
        }
    }
}

fn render_demand_detail(
    result: &abt_core::shared::types::PaginatedResult<DemandSummary>,
    p: &DemandCardParams,
) -> Markup {
    let qs = demand_query_string("detail", p.keyword.as_deref(), p.date_filter.as_deref());
    html! {
        div class="overflow-x-auto" {
            table class="w-full text-sm" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="text-left font-semibold py-2 px-5 uppercase tracking-wide" { "产品" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "来源订单" }
                        th class="text-right font-semibold py-2 px-3 uppercase tracking-wide" { "数量" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "需求日期" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "优先级" }
                    }
                }
                tbody {
                    @if result.items.is_empty() {
                        tr { td colspan="5" class="text-center text-muted py-8" { "暂无需求记录" } }
                    }
                    @for item in &result.items {
                        (demand_detail_row(item))
                    }
                }
            }
        }
        (pagination(WcDemandPath::PATH, &qs, result.total, result.page, result.total_pages))
    }
}

fn demand_detail_row(item: &DemandSummary) -> Markup {
    html! {
        tr class="border-b border-border-soft hover:bg-accent-bg" {
            td class="py-2.5 px-5" {
                div class="font-medium text-fg" { (item.product_name) }
                div class="text-xs text-muted font-mono" { (item.product_code) }
            }
            td class="py-2.5 px-3" {
                a class="text-accent font-medium cursor-pointer"
                    href=(format!("/admin/orders/{}", item.order_id))
                    { (item.order_no.as_deref().unwrap_or("—")) }
            }
            td class="py-2.5 px-3 text-right font-mono tabular-nums" { (fmt_qty(item.quantity)) }
            td class="py-2.5 px-3 text-fg-2" { (format_date(item.required_date)) }
            td class="py-2.5 px-3" { (priority_pill(item.priority)) }
        }
    }
}

fn demand_query_string(view: &str, keyword: Option<&str>, date_filter: Option<&str>) -> String {
    let mut q = vec![format!("view={view}")];
    if let Some(k) = keyword
        && !k.is_empty()
    {
        q.push(format!("keyword={k}"));
    }
    if let Some(d) = date_filter
        && !d.is_empty()
    {
        q.push(format!("date_filter={d}"));
    }
    q.join("&")
}

fn format_date(d: Option<NaiveDate>) -> Markup {
    match d {
        Some(date) => html! { (date.format("%Y-%m-%d").to_string()) },
        None => html! { span class="text-muted" { "—" } },
    }
}

fn priority_pill(priority: i32) -> Markup {
    let (label, cls) = match priority {
        p if p >= 4 => ("紧急", "bg-danger-bg text-danger"),
        3 => ("高", "bg-warn-bg text-warn"),
        2 => ("中", "bg-accent-bg text-accent"),
        _ => ("低", "bg-slate-50 text-slate-400"),
    };
    html! {
        span class=(format!("inline-flex items-center text-[11px] px-2 py-0.5 rounded-full font-medium {cls}")) {
            (label)
        }
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ScheduleCardParams {
    #[serde(default, deserialize_with = "empty_as_none")]
    pub keyword: Option<String>,
}

/// 订单排期 card（单端点）：待下达工单（Draft + Planned）+ 行内「下达」入口。
///
/// Phase 1 不分页（合并两状态 list 各 50 条，订单排期通常量小）；搜索按 keyword。
#[require_permission("WORK_ORDER", "read")]
pub async fn get_schedule_card(
    _path: WcSchedulePath,
    ctx: RequestContext,
    Query(p): Query<ScheduleCardParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.work_order_service();
    let product_svc = state.product_service();

    let mk_filter = |status: WorkOrderStatus| WorkOrderFilter {
        status: Some(status),
        keyword: p.keyword.clone(),
        ..Default::default()
    };
    let mut orders = svc
        .list(&service_ctx, &mut conn, mk_filter(WorkOrderStatus::Draft), 1, 50)
        .await
        .map(|r| r.items)
        .unwrap_or_default();
    let mut planned = svc
        .list(
            &service_ctx,
            &mut conn,
            mk_filter(WorkOrderStatus::Planned),
            1,
            50,
        )
        .await
        .map(|r| r.items)
        .unwrap_or_default();
    orders.append(&mut planned);

    let product_names = resolve_product_names(&product_svc, &service_ctx, &mut conn, &orders).await;

    Ok(Html(
        html! {
            div id="wc-schedule-card" {
                (schedule_search_bar(&p))
                (render_schedule_table(&orders, &product_names))
            }
        }
        .into_string(),
    ))
}

// ── 订单排期渲染 ──

fn schedule_search_bar(p: &ScheduleCardParams) -> Markup {
    let kw = p.keyword.as_deref().unwrap_or("");
    html! {
        div class="px-5 py-3 border-b border-border-soft" {
            form hx-get=(WcSchedulePath::PATH)
                hx-trigger="keyup changed delay:300ms from:.wc-sched-search"
                hx-target="#wc-schedule-card" hx-select="#wc-schedule-card" hx-swap="outerHTML"
                hx-push-url="true" {
                div class="relative" {
                    (icon::search_icon("w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"));
                    input class="wc-sched-search w-[200px] pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                        type="text" name="keyword" placeholder="搜索工单号/产品"
                        value=(kw);
                }
            }
        }
    }
}

fn render_schedule_table(orders: &[WorkOrder], product_names: &HashMap<i64, String>) -> Markup {
    html! {
        @if orders.is_empty() {
            div class="p-8 text-center text-sm text-muted" { "暂无待下达工单" }
        } @else {
            div class="overflow-x-auto" {
                table class="w-full text-sm" {
                    thead {
                        tr class="bg-surface-raised text-xs text-muted" {
                            th class="text-left font-semibold py-2 px-5 uppercase tracking-wide" { "工单号" }
                            th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "产品" }
                            th class="text-right font-semibold py-2 px-3 uppercase tracking-wide" { "数量" }
                            th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "计划日期" }
                            th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "状态" }
                            th class="text-right font-semibold py-2 px-5 uppercase tracking-wide" { "操作" }
                        }
                    }
                    tbody {
                        @for w in orders {
                            (schedule_row(w, product_names))
                        }
                    }
                }
            }
        }
    }
}

fn schedule_row(w: &WorkOrder, product_names: &HashMap<i64, String>) -> Markup {
    let pn = product_names
        .get(&w.product_id)
        .map(|s| s.as_str())
        .unwrap_or("—");
    let (slabel, stoken) = wo_status_meta(&w.status);
    html! {
        tr class="border-b border-border-soft hover:bg-accent-bg" {
            td class="py-2.5 px-5 font-mono tabular-nums text-accent font-medium" { (w.doc_number) }
            td class="py-2.5 px-3 text-fg" { (pn) }
            td class="py-2.5 px-3 text-right font-mono tabular-nums" { (fmt_qty(w.planned_qty)) }
            td class="py-2.5 px-3 text-fg-2 font-mono" {
                (w.scheduled_start.format("%m-%d")) " → " (w.scheduled_end.format("%m-%d"))
            }
            td class="py-2.5 px-3" {
                span class=(format!(
                    "inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium bg-{stoken}-bg text-{stoken}"
                )) {
                    span class=(format!("inline-block w-1.5 h-1.5 rounded-full bg-{stoken}")) {}
                    (slabel)
                }
            }
            td class="py-2.5 px-5 text-right" {
                button class="inline-flex items-center gap-1 px-3 py-1 rounded-sm bg-accent text-white text-xs font-semibold cursor-pointer border-none hover:opacity-90"
                    hx-get=(WcReleaseDrawerPath { order_id: w.id }.to_string())
                    hx-target="#release-drawer-body" hx-swap="innerHTML"
                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #release-overlay" {
                    "下达"
                }
            }
        }
    }
}

/// 工单状态 → (标签, 语义色 token)。工单 card / 订单排期共用。
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

/// 批量解析工单产品名（失败容错返回空 map）。
async fn resolve_product_names(
    product_svc: &impl ProductService,
    ctx: &abt_core::shared::types::context::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    orders: &[WorkOrder],
) -> HashMap<i64, String> {
    let pids: Vec<i64> = orders.iter().map(|w| w.product_id).collect();
    match product_svc.get_by_ids(ctx, db, pids).await {
        Ok(ps) => ps.iter().map(|p| (p.product_id, p.pdt_name.clone())).collect(),
        Err(_) => HashMap::new(),
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct OrdersCardParams {
    #[serde(default, deserialize_with = "empty_as_none")]
    pub keyword: Option<String>,
    /// "InProduction" | "Released"（默认 InProduction）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

/// 工单 card（单端点）：生产中 / 已下达 状态 tab + 搜索 + 物料徽章 + 进度 + 行内展开 + 报工入口。
#[require_permission("WORK_ORDER", "read")]
pub async fn get_orders_card(
    _path: WcOrdersPath,
    ctx: RequestContext,
    Query(p): Query<OrdersCardParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.work_order_service();
    let product_svc = state.product_service();
    let page = p.page.unwrap_or(1);
    let status = p
        .status
        .as_deref()
        .and_then(parse_wo_status)
        .unwrap_or(WorkOrderStatus::InProduction);

    let result = svc
        .list(
            &service_ctx,
            &mut conn,
            WorkOrderFilter {
                status: Some(status),
                keyword: p.keyword.clone(),
                ..Default::default()
            },
            page,
            10,
        )
        .await?;

    let product_names = resolve_product_names(&product_svc, &service_ctx, &mut conn, &result.items).await;
    let wo_ids: Vec<i64> = result.items.iter().map(|w| w.id).collect();
    let availability = svc
        .compute_availability_batch(&service_ctx, &mut conn, &wo_ids)
        .await
        .unwrap_or_default();

    Ok(Html(
        html! {
            div id="wc-orders-card" {
                (orders_tabs_and_filter(&p))
                (orders_table(&result, &product_names, &availability, &p))
            }
        }
        .into_string(),
    ))
}

// ── 工单 card 渲染 ──

fn parse_wo_status(s: &str) -> Option<WorkOrderStatus> {
    use WorkOrderStatus::*;
    match s {
        "Released" => Some(Released),
        "InProduction" => Some(InProduction),
        _ => None,
    }
}

fn orders_tabs_and_filter(p: &OrdersCardParams) -> Markup {
    let tabs = [("InProduction", "生产中"), ("Released", "已下达")];
    let sel = p.status.as_deref().unwrap_or("InProduction");
    let kw = p.keyword.as_deref().unwrap_or("");
    html! {
        div class="flex items-center justify-between flex-wrap gap-3 px-5 py-3 border-b border-border-soft" {
            div class="inline-flex bg-surface border border-border-soft rounded-md p-[3px] gap-0.5" {
                @for (v, label) in &tabs {
                    button class=(toggle_cls(sel == *v)) type="button"
                        hx-get=(WcOrdersPath::PATH)
                        hx-vals=(format!("{{\"status\":\"{v}\"}}"))
                        hx-target="#wc-orders-card" hx-select="#wc-orders-card" hx-swap="outerHTML"
                        hx-push-url="true" hx-include="#wc-orders-filter-form"
                        { (label) }
                }
            }
            form hx-get=(WcOrdersPath::PATH)
                hx-trigger="keyup changed delay:300ms from:.wc-orders-search"
                hx-target="#wc-orders-card" hx-select="#wc-orders-card" hx-swap="outerHTML"
                hx-push-url="true" {
                input type="hidden" name="status" value=(sel);
                div class="relative" {
                    (icon::search_icon("w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"));
                    input class="wc-orders-search w-[180px] pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                        type="text" name="keyword" placeholder="搜索工单号"
                        value=(kw);
                }
            }
            form id="wc-orders-filter-form" class="hidden" {
                input type="hidden" name="keyword" value=(kw);
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn orders_table(
    result: &abt_core::shared::types::PaginatedResult<WorkOrder>,
    product_names: &HashMap<i64, String>,
    availability: &HashMap<i64, (MaterialAvailabilityLevel, Option<String>)>,
    p: &OrdersCardParams,
) -> Markup {
    let qs = orders_query_string(p.status.as_deref(), p.keyword.as_deref());
    html! {
        div class="overflow-x-auto" {
            table class="w-full text-sm" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="text-left font-semibold py-2 px-5 uppercase tracking-wide" { "工单号" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "产品" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "物料" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "进度" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "状态" }
                        th class="text-right font-semibold py-2 px-5 uppercase tracking-wide" { "操作" }
                    }
                }
                tbody {
                    @if result.items.is_empty() {
                        tr { td colspan="6" class="text-center text-muted py-8" { "暂无工单" } }
                    }
                    @for w in &result.items {
                        (orders_row(w, product_names, availability))
                    }
                }
            }
        }
        (pagination(
            WcOrdersPath::PATH,
            &qs,
            result.total,
            result.page,
            result.total_pages,
        ))
    }
}

#[allow(clippy::type_complexity)]
fn orders_row(
    w: &WorkOrder,
    product_names: &HashMap<i64, String>,
    availability: &HashMap<i64, (MaterialAvailabilityLevel, Option<String>)>,
) -> Markup {
    let pn = product_names
        .get(&w.product_id)
        .map(|s| s.as_str())
        .unwrap_or("—");
    let (slabel, stoken) = wo_status_meta(&w.status);
    let (level, headline) = availability
        .get(&w.id)
        .cloned()
        .unwrap_or((MaterialAvailabilityLevel::Available, None));
    let detail_url = format!("/admin/mes/orders/{}", w.id);
    html! {
        tr class="border-b border-border-soft hover:bg-accent-bg" {
            td class="py-2.5 px-5 font-mono tabular-nums" {
                a class="text-accent font-medium" href=(detail_url) { (w.doc_number) }
            }
            td class="py-2.5 px-3" {
                div class="font-medium text-fg" { (pn) }
                div class="text-xs text-muted" { (fmt_qty(w.planned_qty)) " 件" }
            }
            td class="py-2.5 px-3" { (material_badge_mini(level, headline.as_deref())) }
            td class="py-2.5 px-3" { (wo_progress(w)) }
            td class="py-2.5 px-3" {
                span class=(format!(
                    "inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium bg-{stoken}-bg text-{stoken}"
                )) {
                    span class=(format!("inline-block w-1.5 h-1.5 rounded-full bg-{stoken}")) {}
                    (slabel)
                }
            }
            td class="py-2.5 px-5 text-right whitespace-nowrap" {
                button class="inline-flex items-center gap-1 px-2.5 py-1 rounded-sm border border-border text-xs font-medium text-fg cursor-pointer hover:bg-accent-bg hover:border-accent hover:text-accent transition-all"
                    hx-get=(WcReportDrawerPath { order_id: w.id }.to_string())
                    hx-target="#report-drawer-body" hx-swap="innerHTML"
                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #report-overlay" {
                    "报工"
                }
                button class="inline-flex items-center justify-center w-[26px] h-[26px] border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-bg hover:text-fg align-middle transition-all"
                    title="展开详情"
                    hx-get=(format!("/admin/mes/orders/{}/row-detail", w.id))
                    hx-target="this" hx-swap="afterend"
                    _="on click toggle .open on closest <tr/>" {
                    (icon::chevron_right_icon("w-[15px] h-[15px]"))
                }
            }
        }
    }
}

/// 工单进度条（动态宽度用 style，同 mes_order_list 既定做法）。
fn wo_progress(w: &WorkOrder) -> Markup {
    let pct = if w.planned_qty > Decimal::ZERO {
        let p = w.completed_qty / w.planned_qty * Decimal::from(100);
        if p > Decimal::from(100) {
            Decimal::from(100)
        } else {
            p
        }
    } else {
        Decimal::ZERO
    };
    let pct_str = fmt_qty(pct);
    html! {
        div class="flex flex-col gap-[3px]" {
            div class="w-[84px] h-[6px] bg-border-soft rounded-[3px] overflow-hidden" {
                div class="h-full rounded-[3px] bg-accent transition-all duration-150"
                    style=(format!("width:{}%", pct_str)) {}
            }
            div class="text-[11px] text-muted font-mono tabular-nums" {
                (pct_str) "% · " (fmt_qty(w.completed_qty)) "/" (fmt_qty(w.planned_qty))
            }
        }
    }
}

fn orders_query_string(status: Option<&str>, keyword: Option<&str>) -> String {
    let mut q = vec![];
    if let Some(s) = status
        && !s.is_empty()
    {
        q.push(format!("status={s}"));
    }
    if let Some(k) = keyword
        && !k.is_empty()
    {
        q.push(format!("keyword={k}"));
    }
    q.join("&")
}

// =============================================================================
// Drawer body 端点（占位 — 后续 Edit 填充完整表单）
// =============================================================================

/// 下达 drawer body：工单信息 + 工序区（加载/查看）+ 分批规划 + 确认下达 form。
#[require_permission("WORK_ORDER", "read")]
pub async fn get_release_drawer(
    path: WcReleaseDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let wo_svc = state.work_order_service();
    let batch_svc = state.production_batch_service();

    let order = wo_svc.find_by_id(&service_ctx, &mut conn, path.order_id).await?;
    let product_name = wo_svc
        .get_product_name(&mut conn, order.product_id)
        .await?
        .unwrap_or_else(|| format!("#{}", order.product_id));
    let routings = batch_svc
        .list_routings(&service_ctx, &mut conn, path.order_id)
        .await
        .unwrap_or_default();

    Ok(Html(
        render_release_drawer_body(&order, &product_name, &routings).into_string(),
    ))
}

// ── 下达 drawer 渲染 ──

fn render_release_drawer_body(
    order: &WorkOrder,
    product_name: &str,
    routings: &[WorkOrderRouting],
) -> Markup {
    html! {
        // 工单信息
        div class="mb-5 pb-4 border-b border-border-soft" {
            div class="text-xs text-muted mb-0.5" { "工单" }
            div class="font-mono font-semibold text-fg" { (order.doc_number) }
            div class="text-sm text-fg-2 mt-1" {
                (product_name) " · " (fmt_qty(order.planned_qty)) " 件"
            }
        }

        // 工序区：加载按钮（复用 mes_order 端点，广播 routingChanged）+ 工序表（监听自刷新）
        div class="mb-5" {
            div class="flex items-center justify-between mb-2" {
                span class="text-sm font-semibold text-fg" { "工序" }
                div class="flex gap-2" {
                    @if let Some(rid) = order.routing_id {
                        button type="button"
                            class="text-xs px-2 py-1 rounded-sm border border-border text-fg-2 hover:bg-accent-bg hover:text-accent cursor-pointer transition-all"
                            hx-post=(OrderRoutingApplyFromRoutingPath { order_id: order.id }.to_string())
                            hx-vals=(format!("{{\"routing_id\":{rid}}}"))
                            { "从 Routing 加载" }
                    }
                    button type="button"
                        class="text-xs px-2 py-1 rounded-sm border border-border text-fg-2 hover:bg-accent-bg hover:text-accent cursor-pointer transition-all"
                        hx-post=(OrderRoutingLoadRecentPath { order_id: order.id }.to_string())
                        { "从最近工单加载" }
                }
            }
            div id="wc-release-routings"
                hx-get=(WcReleaseDrawerPath { order_id: order.id }.to_string())
                hx-target="this" hx-select="#wc-release-routings" hx-swap="outerHTML"
                hx-trigger="routingChanged from:body" {
                (render_release_routings(routings))
            }
        }

        // 分批规划 + 确认下达（复合 release_order：release + split 单事务）
        form hx-post=(WcReleasePath { order_id: order.id }.to_string())
            hx-swap="none"
            _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #release-overlay" {
            div class="mb-5" {
                div class="text-sm font-semibold text-fg mb-2" { "分批规划" }
                (render_split_row(0, order.planned_qty))
                p class="text-xs text-muted mt-1.5" { "默认整批一个流转卡；修改数量即分批，多批规划待 Phase 2" }
            }
            div class="flex justify-end gap-3 pt-4 border-t border-border-soft" {
                button type="button"
                    class="px-4 py-2 rounded-sm bg-white text-fg-2 border border-border text-sm cursor-pointer hover:bg-surface"
                    _="on click remove .open from #release-overlay" { "取消" }
                button type="submit"
                    class="px-4 py-2 rounded-sm bg-accent text-white text-sm font-medium cursor-pointer border-none hover:opacity-90" {
                    "确认下达"
                }
            }
        }
    }
}

fn render_release_routings(routings: &[WorkOrderRouting]) -> Markup {
    if routings.is_empty() {
        return html! {
            div class="text-xs text-muted p-3 text-center bg-surface rounded-sm" {
                "尚无工序，点上方按钮从 Routing 加载"
            }
        };
    }
    html! {
        table class="w-full text-xs" {
            thead {
                tr class="text-muted border-b border-border-soft" {
                    th class="text-left py-1.5 px-2 font-semibold" { "#" }
                    th class="text-left py-1.5 px-2 font-semibold" { "工序" }
                    th class="text-right py-1.5 px-2 font-semibold" { "单价" }
                    th class="text-center py-1.5 px-2 font-semibold" { "委外" }
                    th class="text-center py-1.5 px-2 font-semibold" { "质检点" }
                }
            }
            tbody {
                @for r in routings {
                    tr class="border-b border-border-soft last:border-b-0" {
                        td class="py-1.5 px-2 text-muted font-mono" { (r.step_no) }
                        td class="py-1.5 px-2 text-fg" { (r.process_name) }
                        td class="py-1.5 px-2 text-right font-mono text-fg-2" {
                            (r.unit_price.map(fmt_qty).unwrap_or_else(|| "—".into()))
                        }
                        td class="py-1.5 px-2 text-center" {
                            @if r.is_outsourced {
                                span class="text-accent" { "✓" }
                            } @else {
                                span class="text-muted" { "—" }
                            }
                        }
                        td class="py-1.5 px-2 text-center" {
                            @if r.is_inspection_point {
                                span class="text-accent" { "✓" }
                            } @else {
                                span class="text-muted" { "—" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 单条分批行（流转卡数量输入）。Phase 1 单批；多批克隆待 Phase 2。
fn render_split_row(idx: usize, qty: Decimal) -> Markup {
    html! {
        div class="flex items-center gap-2" {
            span class="text-xs text-muted w-14" { "流转卡" (idx + 1) }
            input class="w-28 px-2 py-1 border border-border rounded-sm text-sm font-mono text-right bg-white outline-none focus:border-accent"
                type="number" step="0.01 " min="0"
                name=(format!("splits[{idx}][batch_qty]")) value=(fmt_qty(qty));
            span class="text-xs text-muted" { "件" }
        }
    }
}

/// 报工 drawer body：批次/工序选择 + 完成量/不良量/报工人/班次/工时/日期 + 确认报工。
#[require_permission("WORK_ORDER", "read")]
pub async fn get_report_drawer(
    path: WcReportDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let wo_svc = state.work_order_service();
    let batch_svc = state.production_batch_service();

    let order = wo_svc.find_by_id(&service_ctx, &mut conn, path.order_id).await?;
    let product_name = wo_svc
        .get_product_name(&mut conn, order.product_id)
        .await?
        .unwrap_or_else(|| format!("#{}", order.product_id));
    let batches = batch_svc
        .list_by_work_order(&service_ctx, &mut conn, path.order_id)
        .await
        .unwrap_or_default();
    let routings = batch_svc
        .list_routings(&service_ctx, &mut conn, path.order_id)
        .await
        .unwrap_or_default();
    let today = chrono::Local::now().date_naive();

    Ok(Html(
        render_report_drawer_body(&order, &product_name, &batches, &routings, today).into_string(),
    ))
}

// ── 报工 drawer 渲染 ──

fn render_report_drawer_body(
    order: &WorkOrder,
    product_name: &str,
    batches: &[ProductionBatch],
    routings: &[WorkOrderRouting],
    today: NaiveDate,
) -> Markup {
    let today_str = today.format("%Y-%m-%d").to_string();
    html! {
        div class="mb-5 pb-4 border-b border-border-soft" {
            div class="text-xs text-muted mb-0.5" { "工单" }
            div class="font-mono font-semibold text-fg" { (order.doc_number) }
            div class="text-sm text-fg-2 mt-1" { (product_name) }
        }
        form hx-post=(WcReportPath { order_id: order.id }.to_string())
            hx-swap="none"
            _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #report-overlay" {
            div class="mb-3" {
                label class="block text-xs text-fg-2 mb-1" { "批次" }
                select name="batch_id" class="w-full px-2 py-1.5 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent" {
                    @if batches.is_empty() {
                        option value="" { "暂无批次（请先下达并分批）" }
                    }
                    @for b in batches {
                        option value=(b.id) { (b.batch_no) " · " (fmt_qty(b.batch_qty)) " 件" }
                    }
                }
            }
            div class="mb-3" {
                label class="block text-xs text-fg-2 mb-1" { "工序" }
                select name="step_no" class="w-full px-2 py-1.5 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent" {
                    @for r in routings {
                        option value=(r.step_no) { (r.step_no) ". " (r.process_name) }
                    }
                }
            }
            div class="grid grid-cols-2 gap-3 mb-3" {
                div {
                    label class="block text-xs text-fg-2 mb-1" { "本次完成量" }
                    input type="number" step="0.01 " min="0" name="completed_qty"
                        class="w-full px-2 py-1.5 border border-border rounded-sm text-sm font-mono text-right bg-white outline-none focus:border-accent" {};
                }
                div {
                    label class="block text-xs text-fg-2 mb-1" { "不良量" }
                    input type="number" step="0.01 " min="0" name="defect_qty" value="0"
                        class="w-full px-2 py-1.5 border border-border rounded-sm text-sm font-mono text-right bg-white outline-none focus:border-accent" {};
                }
            }
            div class="grid grid-cols-3 gap-3 mb-3" {
                div {
                    label class="block text-xs text-fg-2 mb-1" { "报工人 ID" }
                    input type="number" name="worker_id"
                        class="w-full px-2 py-1.5 border border-border rounded-sm text-sm font-mono bg-white outline-none focus:border-accent" {};
                }
                div {
                    label class="block text-xs text-fg-2 mb-1" { "班次" }
                    select name="shift" class="w-full px-2 py-1.5 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent" {
                        option value="1" selected { "白班" }
                        option value="2" { "夜班" }
                    }
                }
                div {
                    label class="block text-xs text-fg-2 mb-1" { "工时(h)" }
                    input type="number" step="0.1 " min="0" name="work_hours" value="8"
                        class="w-full px-2 py-1.5 border border-border rounded-sm text-sm font-mono text-right bg-white outline-none focus:border-accent" {};
                }
            }
            div class="mb-4" {
                label class="block text-xs text-fg-2 mb-1" { "报工日期" }
                input type="date" name="report_date" value=(today_str)
                    class="w-full px-2 py-1.5 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent" {};
            }
            div class="flex justify-end gap-3 pt-4 border-t border-border-soft" {
                button type="button"
                    class="px-4 py-2 rounded-sm bg-white text-fg-2 border border-border text-sm cursor-pointer hover:bg-surface"
                    _="on click remove .open from #report-overlay" { "取消" }
                button type="submit" disabled[batches.is_empty()]
                    class="px-4 py-2 rounded-sm bg-accent text-white text-sm font-medium cursor-pointer border-none hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed" {
                    "确认报工"
                }
            }
        }
    }
}

// =============================================================================
// 写 handler（完整 — 复用底层 service，事务包裹，HX-Trigger 广播）
// =============================================================================

/// 下达（复合单事务：release + 分批）：Draft/Planned → Released，随后按分批数据 split。
///
/// `split_work_order` 要求工单已 Released，单事务内 `release → split` 顺序天然满足，
/// 比双端点 HTMX 串联更干净且原子。分批数据为空则仅 release。广播 `woChanged`。
#[require_permission("WORK_ORDER", "update")]
pub async fn release_order(
    path: WcReleasePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<SplitMultiForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let wo_svc = state.work_order_service();
    let batch_svc = state.production_batch_service();
    let order = wo_svc.find_by_id(&service_ctx, &mut tx, path.order_id).await?;

    // ① 下达（幂等：已 Released/InProduction 跳过状态转换）
    if order.status != WorkOrderStatus::Released && order.status != WorkOrderStatus::InProduction {
        wo_svc
            .release(&service_ctx, &mut tx, path.order_id, order.version)
            .await?;
    }

    // ② 分批（release 后状态为 Released，满足 split_work_order 前置条件）
    let splits: Vec<SplitReq> = form
        .splits
        .into_iter()
        .filter_map(|s| {
            let q = s.batch_qty.parse::<Decimal>().ok()?;
            if q <= Decimal::ZERO {
                return None;
            }
            Some(SplitReq {
                batch_qty: q,
                team_id: s.team_id,
            })
        })
        .collect();
    if !splits.is_empty() {
        batch_svc
            .split_work_order(&service_ctx, &mut tx, path.order_id, splits)
            .await?;
    }

    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(([("HX-Trigger", "woChanged")], Html(String::new())))
}

/// 分批：一次事务创建多批（`Vec<SplitReq>`），广播 `woChanged`。
///
/// 既有 `mes_order_detail::split_order` 只建 1 批，工作中心下达 drawer 需一次规划多批，故新建此端点。
#[derive(Debug, Deserialize)]
pub struct SplitLineForm {
    pub batch_qty: String,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub team_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SplitMultiForm {
    #[serde(default)]
    pub splits: Vec<SplitLineForm>,
}

#[require_permission("WORK_ORDER", "update")]
pub async fn split_multi(
    path: WcSplitMultiPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<SplitMultiForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let splits: Vec<SplitReq> = form
        .splits
        .into_iter()
        .filter_map(|s| {
            let q = s.batch_qty.parse::<Decimal>().ok()?;
            if q <= Decimal::ZERO {
                return None;
            }
            Some(SplitReq {
                batch_qty: q,
                team_id: s.team_id,
            })
        })
        .collect();
    if splits.is_empty() {
        return Err(DomainError::validation("至少需要一条有效分批").into());
    }
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .production_batch_service()
        .split_work_order(&service_ctx, &mut tx, path.order_id, splits)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(([("HX-Trigger", "woChanged")], Html(String::new())))
}

/// 报工：`confirm_routing_step`（事务包裹），广播 `woChanged`。
#[derive(Debug, Deserialize)]
pub struct ReportStepForm {
    /// 报工目标批次（drawer 内选择，path 用 order_id）
    pub batch_id: i64,
    pub step_no: i32,
    pub worker_id: i64,
    pub shift: ShiftType,
    pub completed_qty: String,
    #[serde(default)]
    pub defect_qty: String,
    #[serde(default)]
    pub work_hours: String,
    pub report_date: chrono::NaiveDate,
    #[serde(default)]
    pub remark: Option<String>,
}

#[require_permission("WORK_ORDER", "update")]
pub async fn report_step(
    _path: WcReportPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ReportStepForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let completed_qty = form
        .completed_qty
        .parse::<Decimal>()
        .map_err(|_| DomainError::validation("完成量格式错误"))?;
    let defect_qty = form.defect_qty.parse::<Decimal>().unwrap_or(Decimal::ZERO);
    let work_hours = form
        .work_hours
        .parse::<Decimal>()
        .unwrap_or(Decimal::ZERO);

    let req = StepConfirmationReq {
        step_no: form.step_no,
        worker_id: form.worker_id,
        shift: form.shift,
        completed_qty,
        defect_qty,
        defect_reason: None,
        work_hours,
        report_date: form.report_date,
        remark: form.remark,
    };

    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .production_batch_service()
        .confirm_routing_step(&service_ctx, &mut tx, form.batch_id, form.step_no, req)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(([("HX-Trigger", "woChanged")], Html(String::new())))
}

// =============================================================================
// 渲染辅助
// =============================================================================

/// 锚点条：待办总数 + 待下达 / 生产中 chip（点击锚定对应 card）。
fn render_anchor_nav(summary: &MesWorkCenterSummary) -> Markup {
    let total = summary.total();
    html! {
        div class="sticky top-0 z-20 flex items-center gap-4 p-3 mb-4 rounded-lg border border-border-soft bg-bg shadow-xs flex-wrap" {
            div class="flex flex-col items-center pr-4 border-r border-border-soft shrink-0" {
                span class="text-xl font-bold font-mono tabular-nums text-accent leading-tight" { (total) }
                span class="text-xs text-muted font-medium" { "待办" }
            }
            div class="flex items-center gap-2 flex-wrap" {
                (nav_chip("#wc-schedule-card", "待下达", summary.pending_release))
                (nav_chip("#wc-orders-card", "生产中", summary.in_production))
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

/// Card 外壳：标题栏 + 占位 div（`hx-trigger="load"` 拉 card 端点内容替换）。
///
/// 占位 div 的 `id` 与 card 端点返回的最外层 div 一致，懒加载与 card 内交互
/// 都用 `hx-swap="outerHTML"` + `hx-select="#wc-xxx-card"` 替换，保证 card 自洽。
fn render_card_shell(card_id: &str, src: &str, title: &str) -> Markup {
    html! {
        section class="bg-bg border border-border-soft rounded-lg mb-4 shadow-[var(--shadow-card)] overflow-hidden" {
            div class="px-5 py-3 border-b border-border-soft" {
                span class="font-semibold text-fg" { (title) }
            }
            div id=(card_id)
                class="p-5 text-sm text-muted"
                hx-get=(src)
                hx-trigger="load"
                hx-swap="outerHTML" {
                "加载中…"
            }
        }
    }
}

/// Drawer overlay 壳：背景点击/关闭按钮收起，body 由 `hx-get` 填充。
///
/// 开关：给 overlay 加 `.open` class（`open:` 前缀的 UnoCSS 变体驱动显隐 + 平移）。
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
