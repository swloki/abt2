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
    CreateWorkOrdersFromDemandsReq, DemandPoolQuery, DemandSummary, MaterialAggQuery,
    MaterialAggSummary, MesDemandService,
};
use abt_core::mes::enums::{BatchStatus, ShiftType, WorkOrderStatus};
use abt_core::mes::production_batch::{
    BatchListFilter, BatchListItem, ProductionBatch, ProductionBatchService, SplitReq,
    StepConfirmationReq, WorkOrderRouting,
};
use abt_core::mes::production_receipt::{CreateReceiptReq, ProductionReceiptService};
use abt_core::wms::material_requisition::MaterialRequisitionService;
use abt_core::mes::work_center::{MesWorkCenterService, MesWorkCenterSummary};
use abt_core::sales::sales_order::{SalesOrder, SalesOrderItem, SalesOrderService, SalesOrderStatus};
use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::master_data::work_center::{new_work_center_service, WorkCenterService};
use abt_core::mes::work_order::{
    MaterialAvailabilityLevel, WorkOrder, WorkOrderFilter, WorkOrderService,
};
use abt_core::shared::types::{DomainError, PageParams};

use std::collections::HashMap;

use crate::components::alert;
use crate::components::icon;
use crate::components::material_badge::material_badge_mini;
use crate::components::overlay::{drawer_shell, modal_shell};
use crate::components::product_picker;
use crate::components::routing_picker;
use crate::components::pagination::pagination;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_demand_pool::{MesDemandPoolCreatePath, MesDemandRowsPath};
use crate::routes::mes_order::{OrderRoutingApplyFromRoutingPath, OrderRoutingDeletePath, OrderRoutingLoadRecentPath};
use crate::pages::mes_order_detail::RoutingEditForm;
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

    let content = work_center_content(&summary);

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

/// work-center 首页 content：标题 + 锚点条 + 需求池/工单 card shell + drawer + 批量栏。
///
/// 首页 `get_work_center` 与各 card 端点（直接访问时）共用——card 端点直接访问也返回
/// 完整 work-center 页面（走 `admin_page(is_htmx)` 自动判断），而非裸片段。
fn work_center_content(summary: &MesWorkCenterSummary) -> Markup {
    html! {
        div class="flex items-center justify-between mb-4 flex-wrap gap-4" {
            div {
                h1 class="text-xl font-bold text-fg tracking-tight" { "生产作业中心" }
                p class="text-sm text-muted mt-1" { "需求池 · 订单排期 · 工单 一屏处理，就地下达与报工" }
            }
        }
        (render_anchor_nav(summary))
        (render_card_shell("wc-demand-card", WcDemandPath::PATH, "生产需求池", icon::globe_icon("w-[15px] h-[15px]"), Some((summary.pending_release, "danger")),
            Some(html! { (summary.pending_release) " 张待下达 · 销售订单驱动 · 就地「转化为工单」" })))
        (render_drawer_overlay("release-overlay", "release-drawer", "release-drawer-body", "下达工单", "w-[640px]"))
        (render_drawer_overlay("create-plan-overlay", "create-plan-drawer", "create-plan-drawer-body", "创建工单", "w-[680px]"))
        (render_drawer_overlay("report-overlay", "report-drawer", "report-drawer-body", "工序报工", "w-[480px]"))
        (render_drawer_overlay("batch-overlay", "batch-drawer", "batch-drawer-body", "批次处理", "w-[640px]"))
        (render_drawer_overlay("batch-req-overlay", "batch-req-drawer", "batch-req-drawer-body", "批次领料", "w-[640px]"))
        (render_drawer_overlay("batch-receipt-overlay", "batch-receipt-drawer", "batch-receipt-drawer-body", "完工入库", "w-[480px]"))
        // 创建计划完成事件桥接：planCreated → 切到订单排期 tab 重新加载需求池 card + 展开 card（看新建 Draft 工单）
        div hx-get=(format!("{}?view=schedule", WcDemandPath::PATH))
            hx-trigger="planCreated from:body"
            hx-target="#wc-demand-card" hx-select="#wc-demand-card" hx-swap="outerHTML" {}
        (routing_picker::routing_picker_modal("routing-picker-modal", "wc-routing-id", "wc-routing-name"))
        // 订单详情 modal 容器（slot）：GET 返回完整 modal_shell，innerHTML 进此 slot；afterSettle 打开子 modal
        div id="wc-order-detail-slot"
            _="on 'htmx:afterSettle'[#wc-order-detail-modal] add .is-open to #wc-order-detail-modal\non keydown[event.key is 'Escape' and #wc-order-detail-modal] from body remove .is-open from #wc-order-detail-modal" {}
        // 工序编辑 modal 容器（slot）：GET edit 返回完整 modal_shell（外壳+body），innerHTML 进此容器；
        // afterSettle 在 slot 触发（slot 静态、listener 预注册）→ 打开子 #routing-edit-modal。
        div id="routing-edit-slot"
            _="on 'htmx:afterSettle'[#routing-edit-modal] add .is-open to #routing-edit-modal\non keydown[event.key is 'Escape' and #routing-edit-modal] from body remove .is-open from #routing-edit-modal" {}
        // 产出品 picker（ID 与 mes_order_detail::routing_edit_form 写死的约定一致）
        (product_picker::product_picker_modal("routing-product-modal", "routing-product-id", "routing-product-display"))
    }
}

// =============================================================================
// Card 端点（占位 — 后续 Edit 逐个填充真实数据渲染）
// =============================================================================

#[derive(Debug, Deserialize, Clone, Default)]
pub struct DemandCardParams {
    /// "material" | "detail" | "schedule"（默认 material）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub view: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub date_filter: Option<String>,
    /// 排期视图：状态筛选（Draft/Planned）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub sched_status: Option<String>,
    /// 工单 tab：工单状态（InProduction/Released/Closed）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub wo_status: Option<String>,
    /// 批次 tab：批次状态（Pending/InProgress/Suspended/PendingReceipt/Completed/Cancelled）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub batch_status: Option<String>,
    /// 工单 tab：物料可用性（Available/Expected/Late/Unavailable）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub availability: Option<String>,
    /// 物料汇总/明细排序（urgency/qty/earliest/demand_count）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub sort: Option<String>,
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
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;
    // 直接访问 card 端点 → 返回完整 work-center 首页（card shell 懒加载，无需查 card 数据）
    if !is_htmx {
        let summary = state
            .mes_work_center_service()
            .summary(&service_ctx, &mut conn)
            .await
            .unwrap_or_default();
        return Ok(Html(
            admin_page(
                false,
                "生产作业中心",
                &claims,
                "production",
                WcPath::PATH,
                "生产管理",
                Some("生产作业中心"),
                work_center_content(&summary),
                &nav_filter,
            )
            .into_string(),
        ));
    }
    let view = p.view.as_deref().unwrap_or("material");

    let body = if view == "orders" {
        // 工单 tab：status（生产中/已下达/已完工）是 DB 字段，直接读不实时算；分页 list
        let wo_svc = state.work_order_service();
        let product_svc = state.product_service();
        let status = p.wo_status.as_deref().and_then(parse_wo_status);
        let today = chrono::Local::now().date_naive();
        let (date_from, date_to) = parse_wo_date_filter(p.date_filter.as_deref(), today);
        let page = p.page.unwrap_or(1);
        let result = wo_svc
            .list(
                &service_ctx,
                &mut conn,
                WorkOrderFilter {
                    status,
                    keyword: p.keyword.clone(),
                    date_from,
                    date_to,
                    ..Default::default()
                },
                page,
                10,
            )
            .await?;
        let product_names =
            resolve_product_names(&product_svc, &service_ctx, &mut conn, &result.items).await;
        html! { (orders_table(
            &result, &product_names,
            WcDemandPath::PATH, "#wc-demand-card", "#wc-demand-filter-form"
        )) }
    } else if view == "batches" {
        // 批次 tab：跨工单列出所有生产批次（工单下达 + 拆批后产生）
        let batch_svc = state.production_batch_service();
        let status = p.batch_status.as_deref().and_then(parse_batch_status);
        let page = p.page.unwrap_or(1);
        let result = batch_svc
            .list_batches(
                &service_ctx,
                &mut conn,
                BatchListFilter {
                    status,
                    keyword: p.keyword.clone(),
                },
                page,
                10,
            )
            .await?;
        html! { (batches_table(
            &result,
            WcDemandPath::PATH, "#wc-demand-card", "#wc-demand-filter-form"
        )) }
    } else {
        let svc = state.mes_demand_service();
        let page = p.page.unwrap_or(1);
        let (date_start, date_end) = parse_date_filter(p.date_filter.as_deref());

        if view == "detail" {
            let result = svc
                .list_pending_demands(
                    &service_ctx,
                    &mut conn,
                    DemandPoolQuery {
                        keyword: p.keyword.clone(),
                        required_date_start: date_start,
                        required_date_end: date_end,
                        sort: p.sort.clone(),
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
                        sort: p.sort.clone(),
                        ..Default::default()
                    },
                    PageParams::new(page, 10),
                )
                .await?;
            render_demand_material(&result, &p)
        }
    };

    Ok(Html(
        html! {
            div id="wc-demand-card"
                hx-get=(WcDemandPath::PATH)
                hx-trigger="batchChanged from:body"
                hx-vals=(serde_json::json!({ "view": view }).to_string())
                hx-include="#wc-demand-filter-form"
                hx-select="#wc-demand-card" hx-swap="outerHTML" {
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

/// 平铺 tab 栏 + 筛选表单（统一 hx-get WcDemandPath + hx-select #wc-demand-card）。
/// - 第一行：底部下划线式平铺 tab（物料汇总[可合并]/订单行明细/订单排期）+ mat/det 右侧「完整需求池」链接
/// - 第二行：筛选表单（mat/det：搜索+日期+排序；schedule：搜索+工作中心+状态+时间）
fn demand_filter_bar(
    view: &str,
    p: &DemandCardParams,
) -> Markup {
    let kw = p.keyword.as_deref().unwrap_or("");
    let df = p.date_filter.as_deref().unwrap_or("");
    let ss = p.sched_status.as_deref().unwrap_or("");
    let wos = p.wo_status.as_deref().unwrap_or("");
    let bs = p.batch_status.as_deref().unwrap_or("");
    let avail = p.availability.as_deref().unwrap_or("");
    let sort = p.sort.as_deref().unwrap_or("");
    let placeholder = if view == "schedule" || view == "orders" {
        "搜索工单号/产品"
    } else if view == "batches" {
        "搜索流转卡/批次号"
    } else {
        "搜索物料/订单"
    };
    html! {
        // 第一行：平铺 tab 栏（底部下划线）+ mat/det 右侧「完整需求池」
        div class="flex items-center gap-1 flex-wrap px-5 pt-3 border-b border-border-soft" {
            button class=(toggle_cls(view == "material")) type="button"
                hx-get=(WcDemandPath::PATH)
                hx-vals="{\"view\":\"material\"}"
                hx-target="#wc-demand-card" hx-select="#wc-demand-card" hx-swap="outerHTML"
                hx-include="#wc-demand-filter-form"
                { (icon::grid_4_icon("w-4 h-4")) "物料汇总"
                  span class="text-[10px] text-muted font-medium ml-0.5" { "可合并" } }
            button class=(toggle_cls(view == "detail")) type="button"
                hx-get=(WcDemandPath::PATH)
                hx-vals="{\"view\":\"detail\"}"
                hx-target="#wc-demand-card" hx-select="#wc-demand-card" hx-swap="outerHTML"
                hx-include="#wc-demand-filter-form"
                { (icon::rows_icon("w-4 h-4")) "订单行明细" }
            button class=(toggle_cls(view == "orders")) type="button"
                hx-get=(WcDemandPath::PATH) hx-vals="{\"view\":\"orders\"}"
                hx-target="#wc-demand-card" hx-select="#wc-demand-card" hx-swap="outerHTML"
                hx-include="#wc-demand-filter-form"
                { (icon::package_icon("w-4 h-4")) "工单" }
            button class=(toggle_cls(view == "batches")) type="button"
                hx-get=(WcDemandPath::PATH) hx-vals="{\"view\":\"batches\"}"
                hx-target="#wc-demand-card" hx-select="#wc-demand-card" hx-swap="outerHTML"
                hx-include="#wc-demand-filter-form"
                { (icon::box_icon("w-4 h-4")) "批次" }
            @if view == "material" || view == "detail" {
                a class="ml-auto text-xs text-accent font-semibold no-underline"
                    href="/admin/mes/demand-pool" { "完整需求池 →" }
            }
        }
        // 第二行：筛选表单（change / keyup 触发刷新）
        form class="flex items-center gap-2 flex-wrap px-5 py-3 border-b border-border-soft"
            hx-get=(WcDemandPath::PATH)
            hx-trigger="change, keyup changed delay:300ms from:.wc-search-input"
            hx-target="#wc-demand-card" hx-select="#wc-demand-card" hx-swap="outerHTML"
            {
            input type="hidden" name="view" value=(view);
            div class="relative" {
                (icon::search_icon("w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"));
                input class="wc-search-input w-[180px] pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                    type="text" name="keyword" placeholder=(placeholder)
                    value=(kw);
            }
            @if view == "orders" {
                // 工单筛选：状态（含待下达/已排期）/ 物料可用性 / 时间
                select class="px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg cursor-pointer"
                    name="wo_status" {
                    option value="" selected[wos.is_empty()] { "全部状态" }
                    option value="Draft" selected[wos == "Draft"] { "待下达" }
                    option value="Planned" selected[wos == "Planned"] { "已排期" }
                    option value="Released" selected[wos == "Released"] { "已下达" }
                    option value="InProduction" selected[wos == "InProduction"] { "生产中" }
                    option value="Closed" selected[wos == "Closed"] { "已完工" }
                }
                select class="px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg cursor-pointer"
                    name="availability" {
                    option value="" selected[avail.is_empty()] { "全部物料" }
                    option value="Available" selected[avail == "Available"] { "齐套" }
                    option value="Expected" selected[avail == "Expected"] { "在途" }
                    option value="Late" selected[avail == "Late"] { "在途迟" }
                    option value="Unavailable" selected[avail == "Unavailable"] { "缺料" }
                }
                select class="px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg cursor-pointer"
                    name="date_filter" {
                    option value="" selected[df.is_empty()] { "全部时间" }
                    option value="overdue" selected[df == "overdue"] { "已逾期" }
                    option value="this_week" selected[df == "this_week"] { "本周开工" }
                }
            } @else if view == "batches" {
                // 批次筛选：状态
                select class="px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg cursor-pointer"
                    name="batch_status" {
                    option value="" selected[bs.is_empty()] { "全部状态" }
                    option value="Pending" selected[bs == "Pending"] { "待开工" }
                    option value="InProgress" selected[bs == "InProgress"] { "进行中" }
                    option value="Suspended" selected[bs == "Suspended"] { "已暂停" }
                    option value="PendingReceipt" selected[bs == "PendingReceipt"] { "待入库" }
                    option value="Completed" selected[bs == "Completed"] { "已完工" }
                    option value="Cancelled" selected[bs == "Cancelled"] { "已取消" }
                }
            } @else {
                select class="px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg cursor-pointer"
                    name="date_filter" {
                    option value="" selected[df.is_empty()] { "全部日期" }
                    option value="7days" selected[df == "7days"] { "近7天到期" }
                    option value="30days" selected[df == "30days"] { "近30天到期" }
                    option value="overdue" selected[df == "overdue"] { "已逾期" }
                }
                span class="text-sm text-muted font-medium ml-1" { "排序" }
                select class="px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg cursor-pointer"
                    name="sort" {
                    option value="urgency" selected[sort == "urgency"] { "按紧急度" }
                    option value="qty" selected[sort == "qty"] { "按总需求量" }
                    option value="earliest" selected[sort == "earliest"] { "按最早交期" }
                    option value="demand_count" selected[sort == "demand_count"] { "按涉及订单数" }
                }
            }
        }
        // 隐藏表单：tab 切换时携带所有筛选参数
        form id="wc-demand-filter-form" class="hidden" {
            input type="hidden" name="keyword" value=(kw);
            input type="hidden" name="date_filter" value=(df);
            input type="hidden" name="sched_status" value=(ss);
            input type="hidden" name="wo_status" value=(wos);
            input type="hidden" name="batch_status" value=(bs);
            input type="hidden" name="availability" value=(avail);
            input type="hidden" name="sort" value=(sort);
        }
    }
}

fn toggle_cls(active: bool) -> &'static str {
    if active {
        "inline-flex items-center gap-1 px-3.5 py-1.5 text-sm text-accent font-semibold cursor-pointer bg-accent-bg rounded-sm border-none transition-colors"
    } else {
        "inline-flex items-center gap-1 px-3.5 py-1.5 text-sm text-muted font-medium cursor-pointer bg-transparent border-none rounded-sm hover:text-fg hover:bg-surface transition-colors"
    }
}

fn render_demand_material(
    result: &abt_core::shared::types::PaginatedResult<MaterialAggSummary>,
    _p: &DemandCardParams,
) -> Markup {
    html! {
        div class="data-card" {
            // 列头
            div class="grid grid-cols-[1fr_auto_auto_auto_auto] items-center gap-6 px-6 py-3 bg-surface-raised text-xs font-semibold uppercase tracking-wide text-muted border-b border-border-soft"
            {
                div { "物料信息" }
                div class="text-center w-[100px]" { "总需求量" }
                div class="text-center w-[80px]" { "涉及订单" }
                div class="text-center w-[160px]" { "需求日期范围" }
                div class="text-center w-[120px]" { "操作" }
            }
            @if result.items.is_empty() {
                div class="text-center p-6 text-muted text-sm" { "暂无待处理需求" }
            }
            @for item in &result.items {
                (wc_demand_material_row(item))
            }
            (pagination(WcDemandPath::PATH, "#wc-demand-card", "#wc-demand-filter-form", result.total, result.page, result.total_pages))
        }
    }
}

/// 物料汇总行：点击物料信息区展开（懒加载该物料需求明细），行尾「创建工单」。
fn wc_demand_material_row(item: &MaterialAggSummary) -> Markup {
    let pid = item.product_id;
    let hint = urgency_hint(item.earliest_required_date);
    let earliest_str = item
        .earliest_required_date
        .map(|d| d.format("%m/%d").to_string())
        .unwrap_or_else(|| "—".into());
    let latest_str = item
        .latest_required_date
        .map(|d| d.format("%m/%d").to_string())
        .unwrap_or_else(|| "—".into());
    let date_range = format!("{earliest_str} → {latest_str}");
    let qty_cls = demand_qty_class(item.total_demand_qty, item.earliest_required_date);
    let (icon_cls, mat_icon) = material_icon(item.earliest_required_date, item.total_demand_qty);
    let rows_url = format!("{}?product_id={}", MesDemandRowsPath::PATH, pid);

    html! {
        div class="grid grid-cols-[1fr_auto_auto_auto_auto] items-center gap-6 p-4 px-6 border-b border-border-soft"
        {
            // 物料信息（点击展开：懒加载该物料需求明细，复用 demand-pool 的 demand-rows 端点）
            div class="flex items-center gap-4 cursor-pointer"
                hx-get=(rows_url)
                hx-target=(format!("#wc-expand-tbody-{pid}"))
                hx-swap="innerHTML"
                hx-trigger="click once"
                _=(format!("on click toggle .expanded on #wc-expand-mat-{pid}"))
            {
                div class=(format!("w-[40px] h-[40px] rounded-md grid place-items-center shrink-0 {icon_cls}"))
                { (mat_icon) }
                div {
                    div class="font-semibold text-fg text-sm" { (item.product_name) }
                    div class="text-xs text-muted font-mono" { (item.product_code) }
                }
            }
            // 总需求量
            div class="flex flex-col" {
                div class=(format!("text-lg font-bold font-mono tabular-nums {qty_cls}")) {
                    (fmt_qty(item.total_demand_qty))
                }
                div class="text-sm text-muted mt-1" { "总需求量" }
            }
            // 涉及订单数
            div class="flex flex-col" {
                div class="text-2xl font-bold font-mono tabular-nums text-accent" { (item.demand_count) }
                div class="text-sm text-muted mt-1" { "涉及订单" }
            }
            // 需求日期范围 + 紧急度
            div class="flex flex-col" {
                div class="text-[13px] font-semibold text-fg" { (date_range) }
                @if let Some((hint_text, hint_cls)) = &hint {
                    div class=(format!("text-xs font-medium {hint_cls}")) { (hint_text) }
                }
            }
            // 操作
            div class="flex gap-2" {
                button class="inline-flex items-center gap-1.5 py-[5px] px-3 text-[13px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover font-medium cursor-pointer transition-all duration-150"
                    hx-get=(WcCreatePlanDrawerPath { product_id: pid }.to_string())
                    hx-target="#create-plan-drawer-body" hx-swap="innerHTML"
                    _="on click halt the event" {
                    (icon::plus_icon("w-3.5 h-3.5"))
                    "创建工单"
                }
            }
        }
        // 展开区：首次点击懒加载 demand-rows 片段（含 demand-cb checkbox + 全选），列对齐 demand_expand_row（7 列）
        div class="hidden expanded:block bg-surface-raised border-b border-border-soft batch-scope"
            id=(format!("wc-expand-mat-{pid}"))
        {
            div class="p-4" {
                table class="w-full text-sm" {
                    thead {
                        tr class="text-xs text-muted border-b border-border-soft" {
                            th class="w-10 py-1.5 px-2" {
                                input type="checkbox" title="全选"
                                    _="on change call toggleAllDemands(me, closest <table/>)";
                            }
                            th class="text-left py-1.5 px-2 font-semibold" { "需求ID" }
                            th class="text-left py-1.5 px-2 font-semibold" { "来源订单" }
                            th class="text-right py-1.5 px-2 font-semibold" { "数量" }
                            th class="text-left py-1.5 px-2 font-semibold" { "需求日期" }
                            th class="text-left py-1.5 px-2 font-semibold" { "优先级" }
                            th class="text-left py-1.5 px-2 font-semibold" { "状态" }
                        }
                    }
                    tbody id=(format!("wc-expand-tbody-{pid}")) {
                        tr { td colspan="7" class="text-center text-muted p-6" { "点击物料加载需求明细…" } }
                    }
                }
                (detail_batch_bar())
            }
        }
    }
}

fn render_demand_detail(
    result: &abt_core::shared::types::PaginatedResult<DemandSummary>,
    _p: &DemandCardParams,
) -> Markup {
    html! {
        div class="data-card batch-scope" {
            div class="overflow-x-auto" {
                table class="w-full text-sm" {
                    thead {
                        tr class="bg-surface-raised text-xs text-muted" {
                            th class="w-10 py-2 px-2" {
                                input type="checkbox" title="全选"
                                    _="on change call toggleAllDemands(me, closest <table/>)";
                            }
                            th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "产品" }
                            th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "来源订单" }
                            th class="text-right font-semibold py-2 px-3 uppercase tracking-wide" { "数量" }
                            th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "需求日期" }
                            th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "优先级" }
                            th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "状态" }
                        }
                    }
                    tbody {
                        @if result.items.is_empty() {
                            tr { td colspan="7" class="text-center text-muted py-8" { "暂无需求记录" } }
                        }
                        @for item in &result.items {
                            (wc_demand_detail_row(item))
                        }
                    }
                }
            }
            (detail_batch_bar())
            (pagination(WcDemandPath::PATH, "#wc-demand-card", "#wc-demand-filter-form", result.total, result.page, result.total_pages))
        }
    }
}

/// 订单明细批量栏（文档流内嵌，对齐原型 .batch-bar）。勾选 demand-cb 后由 app.js 显示。
fn detail_batch_bar() -> Markup {
    html! {
        div class="batch-bar hidden show:flex items-center gap-4 mt-3 px-4 py-3 rounded-md bg-fg text-white text-sm"
            data-create-path=(MesDemandPoolCreatePath::PATH)
        {
            span {
                "已选 "
                span class="batch-count inline-block px-2 rounded-full bg-white/15 font-mono font-bold" { "0" }
                " 条需求 · 可创建工单"
            }
            a class="batch-create-btn ml-auto inline-flex items-center gap-2 py-[5px] px-3 text-[13px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover font-medium cursor-pointer transition-all duration-150 no-underline"
                href=(MesDemandPoolCreatePath::PATH)
                hx-target="#create-plan-drawer-body" hx-swap="innerHTML" { "创建工单" }
            button class="batch-clear-btn inline-flex items-center gap-2 py-[5px] px-3 text-[13px] rounded-sm border border-[rgba(255,255,255,0.15)] text-[rgba(255,255,255,0.7)] hover:text-white hover:bg-[rgba(255,255,255,0.1)] bg-transparent font-medium cursor-pointer transition-all duration-150"
                type="button" { "清除选择" }
        }
    }
}

/// 订单行明细行：checkbox（demand-cb，pending 可选）+ 产品/来源/数量/日期/优先级/状态。
fn wc_demand_detail_row(item: &DemandSummary) -> Markup {
    let is_pending = item.demand_status == 1;
    html! {
        tr class="border-b border-border-soft hover:bg-accent-bg" {
            td class="py-2.5 px-2" {
                @if is_pending {
                    input type="checkbox" class="demand-cb" value=(item.id)
                        data-product-id=(item.product_id)
                        data-product-name=(item.product_name)
                        data-product-code=(item.product_code);
                } @else {
                    input type="checkbox" class="demand-cb" disabled;
                }
            }
            td class="py-2.5 px-3" {
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
            td class="py-2.5 px-3" { (demand_status_label(item.demand_status)) }
        }
    }
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

/// 需求紧急度档位（物料图标底色 / 总需求量字色共用），对齐原型 mat-ic 按紧急度着色。
enum DemandTier {
    Danger,
    Warn,
    Accent,
}

fn demand_tier(earliest: Option<NaiveDate>, total: Decimal) -> DemandTier {
    if let Some(d) = earliest {
        let today = chrono::Local::now().date_naive();
        let diff = (d - today).num_days();
        if diff <= 3 {
            return DemandTier::Danger;
        }
        if diff <= 7 {
            return DemandTier::Warn;
        }
    }
    if total > Decimal::from(100) {
        return DemandTier::Warn;
    }
    DemandTier::Accent
}

/// 物料图标（按紧急度着色，与右侧总需求量同档），对齐原型 mat-ic：danger→wrench / warn→box / accent→check。
fn material_icon(earliest: Option<NaiveDate>, total: Decimal) -> (&'static str, Markup) {
    match demand_tier(earliest, total) {
        DemandTier::Danger => (
            "bg-danger-bg text-danger",
            icon::tool_icon("w-[20px] h-[20px]"),
        ),
        DemandTier::Warn => (
            "bg-warn-bg text-warn",
            icon::cube_icon("w-[20px] h-[20px]"),
        ),
        DemandTier::Accent => (
            "bg-accent-bg text-accent",
            icon::check_circle_icon("w-[20px] h-[20px]"),
        ),
    }
}

/// 总需求量着色：紧急度优先，其次量大用 warn，默认 accent。
fn demand_qty_class(total: Decimal, earliest: Option<NaiveDate>) -> &'static str {
    match demand_tier(earliest, total) {
        DemandTier::Danger => "text-danger",
        DemandTier::Warn => "text-warn",
        DemandTier::Accent => "text-accent",
    }
}

/// 最早需求日期的紧急度提示文案 + 颜色 class。
fn urgency_hint(earliest: Option<NaiveDate>) -> Option<(String, &'static str)> {
    earliest.and_then(|d| {
        let today = chrono::Local::now().date_naive();
        let diff = (d - today).num_days();
        if diff < 0 {
            Some((format!("⚠ 已逾期{}天", diff.abs()), "text-danger"))
        } else if diff == 0 {
            Some(("⚠ 今天到期".into(), "text-danger"))
        } else if diff <= 3 {
            Some((format!("⚠ {}天后到期", diff), "text-danger"))
        } else if diff <= 7 {
            Some((format!("{}天后到期", diff), "text-warn"))
        } else if diff <= 30 {
            Some((format!("{}天后到期", diff), "text-muted"))
        } else {
            None
        }
    })
}

/// 需求状态标签（对齐 demand-pool demand_status_label）。
fn demand_status_label(status: i16) -> Markup {
    let (label, cls) = match status {
        1 => ("待处理", "bg-surface text-muted"),
        2 => ("已确认", "bg-accent-bg text-accent"),
        3 => ("已创建计划", "bg-warn-bg text-warn"),
        4 => ("已完成", "bg-success-bg text-success"),
        5 => ("已拒绝", "bg-danger-bg text-danger"),
        _ => ("未知", "bg-surface text-muted"),
    };
    html! {
        span class=(format!(
            "inline-flex items-center text-[11px] px-2 py-0.5 rounded-full font-medium whitespace-nowrap {cls}"
        )) { (label) }
    }
}

/// 工单状态 → (标签, 语义色 token)。工单 card 共用。
fn wo_status_meta(s: &WorkOrderStatus) -> (&'static str, &'static str) {
    use WorkOrderStatus::*;
    match s {
        Draft => ("待下达", "muted"),
        Planned => ("已排期", "accent"),
        Released => ("已下达", "success"),
        InProduction => ("生产中", "warn"),
        Closed => ("已关闭", "purple"),
        Cancelled => ("已取消", "danger"),
    }
}

/// 批次状态 → (标签, 语义色 token)。批次 tab 共用。
fn batch_status_meta(s: &BatchStatus) -> (&'static str, &'static str) {
    use BatchStatus::*;
    match s {
        Pending => ("待开工", "muted"),
        InProgress => ("进行中", "accent"),
        Suspended => ("已暂停", "warn"),
        PendingReceipt => ("待入库", "purple"),
        Completed => ("已完工", "success"),
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

// ── 工单 tab 渲染（工单合并到需求池第 4 tab，orders_table/orders_row 复用）──

fn parse_wo_status(s: &str) -> Option<WorkOrderStatus> {
    use WorkOrderStatus::*;
    match s {
        "Released" => Some(Released),
        "InProduction" => Some(InProduction),
        "Closed" => Some(Closed),
        _ => None,
    }
}

fn parse_batch_status(s: &str) -> Option<BatchStatus> {
    use BatchStatus::*;
    match s {
        "Pending" => Some(Pending),
        "InProgress" => Some(InProgress),
        "Suspended" => Some(Suspended),
        "PendingReceipt" => Some(PendingReceipt),
        "Completed" => Some(Completed),
        "Cancelled" => Some(Cancelled),
        _ => None,
    }
}

/// 工单 tab 时间过滤 → (date_from, date_to)，作用于工单 scheduled_start / scheduled_end。
fn parse_wo_date_filter(df: Option<&str>, today: NaiveDate) -> (Option<NaiveDate>, Option<NaiveDate>) {
    match df {
        Some("this_week") => (Some(today), Some(today + chrono::TimeDelta::days(7))),
        Some("overdue") => (None, Some(today - chrono::TimeDelta::days(1))),
        _ => (None, None),
    }
}

fn orders_table(
    result: &abt_core::shared::types::PaginatedResult<WorkOrder>,
    product_names: &HashMap<i64, String>,
    list_path: &str,
    card_sel: &str,
    form_sel: &str,
) -> Markup {
    html! {
        div class="overflow-x-auto" {
            table class="w-full text-sm" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="text-left font-semibold py-2 px-5 uppercase tracking-wide" { "工单号" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "产品" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "进度" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "状态" }
                        th class="text-right font-semibold py-2 px-5 uppercase tracking-wide" { "操作" }
                    }
                }
                tbody {
                    @if result.items.is_empty() {
                        tr { td colspan="5" class="text-center text-muted py-8" { "暂无工单" } }
                    }
                    @for w in &result.items {
                        (orders_row(w, product_names))
                    }
                }
            }
        }
        (pagination(
            list_path,
            card_sel,
            form_sel,
            result.total,
            result.page,
            result.total_pages,
        ))
    }
}

fn orders_row(
    w: &WorkOrder,
    product_names: &HashMap<i64, String>,
) -> Markup {
    let pn = product_names
        .get(&w.product_id)
        .map(|s| s.as_str())
        .unwrap_or("—");
    let (slabel, stoken) = wo_status_meta(&w.status);
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
            td class="py-2.5 px-3" { (wo_progress(w)) }
            td class="py-2.5 px-3" {
                span class=(format!(
                    "inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium whitespace-nowrap bg-{stoken}-bg text-{stoken}"
                )) {
                    span class=(format!("inline-block w-1.5 h-1.5 rounded-full bg-{stoken}")) {}
                    (slabel)
                }
            }
            td class="py-2.5 px-5 text-right whitespace-nowrap" {
                @if matches!(w.status, WorkOrderStatus::Draft | WorkOrderStatus::Planned) {
                    button class="inline-flex items-center gap-1 px-2.5 py-1 rounded-sm border border-border text-xs font-medium text-fg cursor-pointer hover:bg-accent-bg hover:border-accent hover:text-accent transition-all"
                        hx-get=(WcReleaseDrawerPath { order_id: w.id }.to_string())
                        hx-target="#release-drawer-body" hx-swap="innerHTML"
                        _="on click halt the event" {
                        "下达"
                    }
                }
                button class="inline-flex items-center justify-center w-[26px] h-[26px] border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg align-middle transition-all"
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

// ── 批次 tab 渲染（跨工单列出所有生产批次）──

fn batches_table(
    result: &abt_core::shared::types::PaginatedResult<BatchListItem>,
    list_path: &str,
    card_sel: &str,
    form_sel: &str,
) -> Markup {
    html! {
        div class="overflow-x-auto" {
            table class="w-full text-sm" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="text-left font-semibold py-2 px-5 uppercase tracking-wide" { "流转卡/批次" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "产品" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "工单" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "进度" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "状态" }
                        th class="text-right font-semibold py-2 px-5 uppercase tracking-wide" { "操作" }
                    }
                }
                tbody {
                    @if result.items.is_empty() {
                        tr { td colspan="6" class="text-center text-muted py-8" { "暂无批次（工单下达并拆批后在此显示）" } }
                    }
                    @for b in &result.items {
                        (batch_row(b))
                    }
                }
            }
        }
        (pagination(list_path, card_sel, form_sel, result.total, result.page, result.total_pages))
    }
}

fn batch_row(b: &BatchListItem) -> Markup {
    let (slabel, stoken) = batch_status_meta(&b.status);
    let pct = if b.batch_qty > Decimal::ZERO {
        let p = b.completed_qty / b.batch_qty * Decimal::from(100);
        if p > Decimal::from(100) { Decimal::from(100) } else { p }
    } else {
        Decimal::ZERO
    };
    let pct_str = fmt_qty(pct);
    html! {
        tr class="border-b border-border-soft hover:bg-accent-bg" {
            td class="py-2.5 px-5" {
                div class="font-mono font-medium text-fg" { (b.card_sn.as_str()) }
                div class="text-xs text-muted font-mono" { (b.batch_no.as_str()) }
            }
            td class="py-2.5 px-3" {
                div class="font-medium text-fg" { (b.product_name.as_deref().unwrap_or("—")) }
            }
            td class="py-2.5 px-3 font-mono text-xs text-accent" {
                (b.wo_doc_number.as_deref().unwrap_or("—"))
            }
            td class="py-2.5 px-3" {
                div class="flex flex-col gap-[3px]" {
                    div class="w-[80px] h-[6px] bg-border-soft rounded-[3px] overflow-hidden" {
                        div class="h-full rounded-[3px] bg-accent transition-all duration-150"
                            style=(format!("width:{}%", pct_str)) {}
                    }
                    div class="text-[11px] text-muted font-mono tabular-nums" {
                        (fmt_qty(b.completed_qty)) "/" (fmt_qty(b.batch_qty))
                        @if let Some(cur) = b.current_step_name.as_deref() {
                            " · " (cur)
                        }
                    }
                }
            }
            td class="py-2.5 px-3" {
                span class=(format!(
                    "inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium whitespace-nowrap bg-{stoken}-bg text-{stoken}"
                )) {
                    span class=(format!("inline-block w-1.5 h-1.5 rounded-full bg-{stoken}")) {}
                    (slabel)
                }
            }
            td class="py-2.5 px-5 text-right whitespace-nowrap" {
                button class="inline-flex items-center gap-1 px-2.5 py-1 rounded-sm border border-border text-xs font-medium text-fg cursor-pointer hover:bg-accent-bg hover:border-accent hover:text-accent transition-all"
                    hx-get=(WcBatchDrawerPath { batch_id: b.id }.to_string())
                    hx-target="#batch-drawer-body" hx-swap="innerHTML"
                    _="on click halt the event" {
                    "处理"
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
    let bar_color = if pct < Decimal::from(30) {
        "bg-muted"
    } else if pct <= Decimal::from(70) {
        "bg-accent"
    } else {
        "bg-success"
    };
    html! {
        div class="flex flex-col gap-[3px]" {
            div class="w-[96px] h-[6px] bg-border-soft rounded-[3px] overflow-hidden" {
                div class=(format!("h-full rounded-[3px] {bar_color} transition-all duration-150"))
                    style=(format!("width:{}%", pct_str)) {}
            }
            div class="text-[11px] text-muted font-mono tabular-nums" {
                (pct_str) "% · " (fmt_qty(w.completed_qty)) "/" (fmt_qty(w.planned_qty))
            }
        }
    }
}

// =============================================================================
// Drawer body 端点（占位 — 后续 Edit 填充完整表单）
// =============================================================================

/// 销售订单详情 modal（drawer 内订单号点击查看）：订单头 + 行项目，不跳转。
#[require_permission("WORK_ORDER", "read")]
pub async fn get_order_detail_modal(
    path: WcOrderDetailModalPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.sales_order_service();
    let order = svc
        .find_by_id(&service_ctx, &mut conn, path.order_id)
        .await?;
    let items = svc
        .list_items(&service_ctx, &mut conn, path.order_id)
        .await?;
    let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
    let prod_map: HashMap<i64, String> = state
        .product_service()
        .get_by_ids(&service_ctx, &mut conn, product_ids)
        .await
        .unwrap_or_default()
        .iter()
        .map(|p| (p.product_id, p.pdt_name.clone()))
        .collect();
    let customer_name = state
        .customer_service()
        .get_by_ids(&service_ctx, &mut conn, &[order.customer_id])
        .await
        .unwrap_or_default()
        .into_iter()
        .next()
        .map(|c| c.name)
        .unwrap_or_else(|| format!("#{}", order.customer_id));
    Ok(Html(
        modal_shell(
            "wc-order-detail-modal",
            "z-[1100]",
            render_order_detail_panel(&order, &items, &prod_map, &customer_name),
        )
        .into_string(),
    ))
}

/// 订单详情 modal 面板：宽 920，卡片式订单头（客户/日期/状态/总额/付款条件）+
/// 行项目表（行号/产品/数量/单价/金额/已发货/交期）+ 备注。
fn render_order_detail_panel(
    order: &SalesOrder,
    items: &[SalesOrderItem],
    prod_map: &HashMap<i64, String>,
    customer_name: &str,
) -> Markup {
    let (status_label, status_cls) = order_status_meta(&order.status);
    html! {
        div class="bg-bg rounded-xl w-[920px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl" {
            div class="flex items-center justify-between px-7 py-5 border-b border-border-soft shrink-0" {
                div {
                    h2 class="text-lg font-bold text-fg m-0" { "销售订单详情" }
                    div class="text-sm text-muted mt-1" {
                        span class="font-mono text-accent" { (order.doc_number) }
                        " · " (customer_name)
                    }
                }
                button type="button"
                    class="text-2xl text-muted hover:text-fg cursor-pointer bg-transparent border-none p-1 leading-none"
                    _="on click remove .is-open from #wc-order-detail-modal" { "×" }
            }
            div class="overflow-y-auto flex-1 min-h-0 p-7" {
                // 订单头卡片
                div class="grid grid-cols-4 gap-5 mb-6 p-5 bg-surface-raised rounded-md" {
                    div {
                        div class="text-xs text-muted mb-1.5" { "订单日期" }
                        div class="text-sm text-fg font-mono" { (order.order_date.format("%Y-%m-%d")) }
                    }
                    div {
                        div class="text-xs text-muted mb-1.5" { "状态" }
                        span class=(format!(
                            "inline-flex items-center text-xs px-2 py-0.5 rounded-full font-medium {status_cls}"
                        )) { (status_label) }
                    }
                    div {
                        div class="text-xs text-muted mb-1.5" { "订单总额" }
                        div class="text-sm text-fg font-mono font-semibold" { "¥ " (fmt_qty(order.total_amount)) }
                    }
                    div {
                        div class="text-xs text-muted mb-1.5" { "付款条件" }
                        div class="text-sm text-fg truncate" { (order.payment_terms) }
                    }
                }
                // 行项目
                div class="text-xs font-semibold text-fg mb-2" { "行项目 · " (items.len()) " 条" }
                div class="border border-border-soft rounded-md overflow-hidden" {
                    table class="w-full text-xs" {
                        thead {
                            tr class="bg-surface text-muted" {
                                th class="text-center font-medium py-2 px-2 w-10" { "#" }
                                th class="text-left font-medium py-2 px-3" { "产品" }
                                th class="text-right font-medium py-2 px-3" { "数量" }
                                th class="text-right font-medium py-2 px-2" { "单价" }
                                th class="text-right font-medium py-2 px-2" { "金额" }
                                th class="text-right font-medium py-2 px-2" { "已发货" }
                                th class="text-left font-medium py-2 px-3" { "交期" }
                            }
                        }
                        tbody {
                            @for item in items {
                                tr class="border-t border-border-soft" {
                                    td class="py-2 px-2 text-center text-muted font-mono" { (item.line_no) }
                                    td class="py-2 px-3" {
                                        div class="text-fg font-medium" {
                                            (prod_map.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—"))
                                        }
                                        @if !item.description.is_empty() {
                                            div class="text-[11px] text-muted mt-0.5" { (item.description) }
                                        }
                                    }
                                    td class="py-2 px-3 text-right font-mono whitespace-nowrap" {
                                        (fmt_qty(item.quantity)) " " (item.unit)
                                    }
                                    td class="py-2 px-2 text-right font-mono text-fg-2" { (fmt_qty(item.unit_price)) }
                                    td class="py-2 px-2 text-right font-mono font-medium" { (fmt_qty(item.amount)) }
                                    td class="py-2 px-2 text-right font-mono text-fg-2" { (fmt_qty(item.shipped_qty)) }
                                    td class="py-2 px-3 font-mono text-fg-2" {
                                        (item.delivery_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into()))
                                    }
                                }
                            }
                        }
                    }
                }
                @if !order.remark.is_empty() {
                    div class="mt-4 p-3 bg-surface-raised rounded-sm text-xs text-fg-2" {
                        span class="text-muted" { "备注：" } (order.remark)
                    }
                }
            }
        }
    }
}

/// 销售订单状态 → (文案, 语义色 class)。
fn order_status_meta(s: &SalesOrderStatus) -> (&'static str, &'static str) {
    use SalesOrderStatus::*;
    match s {
        Draft => ("草稿", "bg-surface text-muted"),
        Confirmed => ("已确认", "bg-accent-bg text-accent"),
        ReadyToShip => ("待发货", "bg-accent-bg text-accent"),
        PartiallyShipped => ("部分发货", "bg-warn-bg text-warn"),
        Shipped => ("已发货", "bg-success-bg text-success"),
        Completed => ("已完成", "bg-success-bg text-success"),
        Cancelled => ("已取消", "bg-danger-bg text-danger"),
        ShippingRequested => ("待拣货", "bg-warn-bg text-warn"),
    }
}

/// 创建工单 drawer body：加载该物料 pending 需求 + 精简表单（就地创建，不跳转）。
#[derive(Debug, Default, Deserialize)]
pub struct DemandIdsQuery {
    #[serde(default, deserialize_with = "empty_as_none")]
    pub demand_ids: Option<String>,
}

#[require_permission("WORK_ORDER", "read")]
pub async fn get_create_plan_drawer(
    path: WcCreatePlanDrawerPath,
    ctx: RequestContext,
    Query(q): Query<DemandIdsQuery>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let mut demands = state
        .mes_demand_service()
        .list_pending_demands(
            &service_ctx,
            &mut conn,
            DemandPoolQuery {
                status: Some(1),
                product_id: Some(path.product_id),
                ..Default::default()
            },
            PageParams::new(1, 100),
        )
        .await?
        .items;
    // 精确加载勾选的需求（来自批量栏 batch-create-btn 的 demand_ids query）
    if let Some(ids_str) = q.demand_ids.as_deref() {
        let ids: std::collections::HashSet<i64> = ids_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        demands.retain(|d| ids.contains(&d.id));
    }
    let product_name = demands
        .first()
        .map(|d| d.product_name.as_str())
        .unwrap_or("—");
    let product_code = demands
        .first()
        .map(|d| d.product_code.as_str())
        .unwrap_or("—");
    Ok(Html(
        render_create_plan_drawer_body(path.product_id, product_name, product_code, &demands)
            .into_string(),
    ))
}

/// 创建工单 drawer 表单（精简版）：物料标题 + 只读需求列表（全部纳入）+
/// 开工/完工日期 + 取消/创建。
fn render_create_plan_drawer_body(
    product_id: i64,
    product_name: &str,
    product_code: &str,
    demands: &[DemandSummary],
) -> Markup {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let default_end = chrono::Local::now()
        .checked_add_days(chrono::Days::new(10))
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    let total_qty: Decimal = demands.iter().map(|d| d.quantity).sum();
    let demand_ids_str = demands
        .iter()
        .map(|d| d.id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    html! {
        // 物料信息
        div class="mb-6 pb-5 border-b border-border-soft" {
            div class="text-xs text-muted mb-1" { "物料" }
            div class="font-semibold text-fg" { (product_name) }
            div class="text-xs text-muted font-mono mt-1" { (product_code) }
        }
        form hx-post=(WcCreatePlanPath { product_id }.to_string())
            hx-swap="none"
            _="on 'htmx:beforeRequest'[detail.elt is me] add .hidden to #create-plan-error
                on 'htmx:afterRequest'[detail.xhr.status < 400 and detail.elt is me] remove .open from #create-plan-overlay
                on 'htmx:afterRequest'[detail.xhr.status >= 400 and detail.elt is me] put detail.xhr.responseText into #create-plan-error then remove .hidden from #create-plan-error" {
            input type="hidden" name="demand_ids" value=(demand_ids_str);
            // 需求列表（只读，全部纳入计划）
            div class="mb-6" {
                div class="text-xs font-semibold text-fg mb-2" {
                    "需求明细 · " (demands.len()) " 条 · 总数量 " (fmt_qty(total_qty))
                }
                div class="max-h-[220px] overflow-y-auto border border-border-soft rounded-sm" {
                    table class="w-full text-xs" {
                        thead {
                            tr class="bg-surface text-muted" {
                                th class="text-left font-medium py-1.5 px-2" { "来源订单" }
                                th class="text-right font-medium py-1.5 px-2" { "数量" }
                                th class="text-left font-medium py-1.5 px-2" { "需求日期" }
                            }
                        }
                        tbody {
                            @for d in demands {
                                tr class="border-t border-border-soft" {
                                    td class="py-2 px-2 font-mono" {
                                        a class="text-accent cursor-pointer no-underline"
                                            hx-get=(WcOrderDetailModalPath { order_id: d.order_id }.to_string())
                                            hx-target="#wc-order-detail-slot" hx-swap="innerHTML"
                                            _="on click halt the event" {
                                            (d.order_no.as_deref().unwrap_or("—"))
                                        }
                                    }
                                    td class="py-2 px-2 text-right font-mono" { (fmt_qty(d.quantity)) }
                                    td class="py-2 px-2 font-mono text-fg-2" { (format_date(d.required_date)) }
                                }
                            }
                        }
                    }
                }
            }
            // 排程参数（扁平化：开工/完工日期，计划类型/计划日期已废弃）
            div class="grid grid-cols-2 gap-3 mb-6" {
                div {
                    label class="block text-xs text-fg-2 mb-1" { "开工日期" }
                    input type="date" name="default_scheduled_start" value=(today)
                        class="w-full px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent";
                }
                div {
                    label class="block text-xs text-fg-2 mb-1" { "完工日期" }
                    input type="date" name="default_scheduled_end" value=(default_end)
                        class="w-full px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent";
                }
            }
            // 错误区（创建失败时由 afterRequest 填入服务端错误信息）
            div id="create-plan-error" class="hidden mb-4 p-3 rounded-sm bg-danger-bg text-danger text-sm" {}
            // 操作
            div class="flex justify-end gap-2 pt-4 border-t border-border-soft" {
                button type="button"
                    class="px-3 py-2 rounded-sm bg-white text-fg-2 border border-border text-sm cursor-pointer hover:bg-surface"
                    _="on click remove .open from #create-plan-overlay" { "取消" }
                button type="submit"
                    class="inline-flex items-center gap-1.5 px-3.5 py-2 rounded-sm bg-accent text-accent-on border-none text-sm font-medium cursor-pointer hover:bg-accent-hover" {
                    (icon::check_circle_icon("w-3.5 h-3.5")) "创建"
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct WcCreatePlanForm {
    pub default_scheduled_start: Option<String>,
    pub default_scheduled_end: Option<String>,
    pub demand_ids: String,
}

/// 转化为工单提交：需求直达生成 Draft 工单（该物料所有需求合并到一个工单，扁平化：废弃 PP 层）。
/// 数量分批不在这一步做 —— 交给工单下达时生产批次（ProductionBatch）拆分
///（与 ERPNext/Odoo/OFBiz 一致：一个需求一个工单，批次是数量维度的执行拆分，见 docs/solutions/mes-wo-vs-batch-modeling.md）。
#[require_permission("WORK_ORDER", "create")]
pub async fn create_plan(
    _path: WcCreatePlanPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<WcCreatePlanForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let demand_ids: Vec<i64> = form
        .demand_ids
        .split(',')
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect();
    if demand_ids.is_empty() {
        return Err(DomainError::validation("请至少选择一条生产需求").into());
    }
    let default_scheduled_start = form
        .default_scheduled_start
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .transpose()
        .map_err(|e| DomainError::validation(format!("无效开工日期: {e}")))?;
    let default_scheduled_end = form
        .default_scheduled_end
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .transpose()
        .map_err(|e| DomainError::validation(format!("无效完工日期: {e}")))?;

    let create_req = CreateWorkOrdersFromDemandsReq {
        demand_ids,
        remark: None,
        items: None,
        default_scheduled_start,
        default_scheduled_end,
    };

    // 单事务：需求直达生成 Draft 工单（扁平化：废弃 PP 层，一个物料一个工单，不 release）
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let _result = state
        .mes_demand_service()
        .create_work_orders_from_demands(&service_ctx, &mut tx, create_req)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 广播 planCreated：drawer 关闭（form afterRequest）+ 需求池 card 切到工单 tab 重新加载（看新建 Draft 工单）
    Ok(([("HX-Trigger", "planCreated")], Html(String::new())))
}

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
    let product_svc = state.product_service();

    let order = wo_svc.find_by_id(&service_ctx, &mut conn, path.order_id).await?;
    let product_name = wo_svc
        .get_product_name(&mut conn, order.product_id)
        .await?
        .unwrap_or_else(|| format!("#{}", order.product_id));
    let routings = batch_svc
        .list_routings(&service_ctx, &mut conn, path.order_id)
        .await
        .unwrap_or_default();

    // 工作中心 id→name（工序表「工作中心」列）
    let wc_map: HashMap<i64, String> =
        new_work_center_service(state.pool.clone())
            .list_active(&service_ctx, &mut conn)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|wc| (wc.id, wc.name))
            .collect();

    // 产出品 id→name（工序表「产出品」列）；顺带取工单产品用于倒冲模式
    let mut pids: Vec<i64> = routings.iter().filter_map(|r| r.product_id).collect();
    if !pids.contains(&order.product_id) {
        pids.push(order.product_id);
    }
    let products = product_svc
        .get_by_ids(&service_ctx, &mut conn, pids)
        .await
        .unwrap_or_default();
    let prod_map: HashMap<i64, String> = products
        .iter()
        .map(|p| (p.product_id, p.pdt_name.clone()))
        .collect();

    // 物料齐套（③ 物料确认 badge）
    let avail = wo_svc
        .compute_availability_batch(&service_ctx, &mut conn, &[order.clone()])
        .await
        .unwrap_or_default();
    let (level, headline) = avail
        .get(&order.id)
        .cloned()
        .unwrap_or((MaterialAvailabilityLevel::Available, None));

    // 倒冲/领料模式
    let consumption_label = products
        .iter()
        .find(|p| p.product_id == order.product_id)
        .map(|p| match p.meta.material_consumption_mode {
            abt_core::master_data::product::model::MaterialConsumptionMode::Backflush => "倒冲",
            abt_core::master_data::product::model::MaterialConsumptionMode::Picking => "领料",
        })
        .unwrap_or("倒冲");

    Ok(Html(
        render_release_drawer_body(
            &order,
            &product_name,
            &routings,
            &wc_map,
            &prod_map,
            level,
            headline.as_deref(),
            consumption_label,
        )
        .into_string(),
    ))
}

// ── 下达 drawer 渲染 ──

fn render_release_drawer_body(
    order: &WorkOrder,
    product_name: &str,
    routings: &[WorkOrderRouting],
    wc_map: &HashMap<i64, String>,
    prod_map: &HashMap<i64, String>,
    level: MaterialAvailabilityLevel,
    headline: Option<&str>,
    consumption_label: &str,
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

        form hx-post=(WcReleasePath { order_id: order.id }.to_string())
            hx-swap="none"
            _="on woChanged from body remove .open from #release-overlay" {

            // ① 批次规划 · 生成生产批次（拆批）
            div class="mb-5" {
                div class="text-sm font-semibold text-fg mb-2" { "① 批次规划 · 生成生产批次（拆批）" }
                div id="wc-release-splits" data-splits="" {
                    (render_split_row(0, order.planned_qty))
                }
                button type="button"
                    class="text-xs px-2 py-1 rounded-sm border border-border text-fg-2 hover:bg-accent-bg hover:text-accent cursor-pointer transition-all mt-2"
                    _="on click call addSplitRow(me)" { "+ 添加生产批次" }
            }

            // ② 工序生成 · 来自工艺路线（首报工前可编辑）
            div class="mb-5" {
                div class="text-sm font-semibold text-fg mb-2" { "② 工序生成 · 来自工艺路线（首报工前可编辑）" }
                div class="flex gap-2 mb-2" {
                    @match order.routing_id {
                        Some(rid) => {
                            button type="button"
                                class="text-xs px-2 py-1 rounded-sm border border-border text-fg-2 hover:bg-accent-bg hover:text-accent cursor-pointer transition-all"
                                hx-post=(OrderRoutingApplyFromRoutingPath { order_id: order.id }.to_string())
                                hx-vals=(format!("{{\"routing_id\":{rid}}}"))
                                hx-swap="none"
                                { "从 Routing 加载" }
                        }
                        None => {
                            button type="button"
                                class="text-xs px-2 py-1 rounded-sm border border-border text-fg-2 hover:bg-accent-bg hover:text-accent cursor-pointer transition-all"
                                _="on click add .is-open to #routing-picker-modal"
                                { "从 Routing 加载" }
                        }
                    }
                    button type="button"
                        class="text-xs px-2 py-1 rounded-sm border border-border text-fg-2 hover:bg-accent-bg hover:text-accent cursor-pointer transition-all"
                        hx-post=(OrderRoutingLoadRecentPath { order_id: order.id }.to_string())
                        hx-swap="none"
                        { "从最近工单加载" }
                }
                // 未绑 Routing 时：picker 选 routing 后提交（routingSelected 触发 form）
                @if order.routing_id.is_none() {
                    form hx-post=(OrderRoutingApplyFromRoutingPath { order_id: order.id }.to_string())
                        hx-trigger="routingSelected from:body"
                        hx-swap="none" {
                        input type="hidden" name="routing_id" id="wc-routing-id" {};
                        // routing_picker click_hs 会 put 工艺名至此元素；缺失则 put 中断、
                        // routingSelected 不发 → 点「选择」无反应（弹窗不关、工序不加载）
                        span id="wc-routing-name" class="hidden" {};
                    }
                }
                div id="wc-release-routings"
                    hx-get=(WcReleaseDrawerPath { order_id: order.id }.to_string())
                    hx-target="this" hx-select="#wc-release-routings" hx-swap="outerHTML"
                    hx-trigger="routingChanged from:body" {
                    (render_release_routings(routings, order.id, wc_map, prod_map))
                }
            }

            // ③ 物料确认
            div class="mb-5" {
                div class="text-sm font-semibold text-fg mb-2" { "③ 物料确认" }
                div class="flex items-center gap-2 text-xs text-fg-2 bg-surface-raised rounded-sm px-3 py-2" {
                    (material_badge_mini(level, headline))
                    span { (consumption_label) "模式 · BOM 已快照" }
                }
            }

            // info-box
            div class="flex items-start gap-2 text-xs text-fg-2 bg-accent-bg rounded-sm px-3 py-2 mb-5" {
                (icon::info_icon("w-4 h-4 shrink-0 mt-0.5 text-accent"))
                span { "「确认下达」执行 release：工单 → Released + BOM 快照 + 工序初始化 + 按拆批创建生产批次 + 领料/倒冲。下达后从订单排期出列，进入「工单」card。" }
            }

            div class="flex justify-end gap-3 pt-4 border-t border-border-soft" {
                @if routings.is_empty() {
                    span class="text-xs text-danger mr-auto self-center" {
                        "⚠ 工序为空，请先在上方「② 工序生成」加载工序后再下达"
                    }
                }
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

fn render_release_routings(
    routings: &[WorkOrderRouting],
    order_id: i64,
    wc_map: &HashMap<i64, String>,
    prod_map: &HashMap<i64, String>,
) -> Markup {
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
                    th class="text-left py-1.5 px-2 font-semibold" { "产出品" }
                    th class="text-left py-1.5 px-2 font-semibold" { "工作中心" }
                    th class="text-right py-1.5 px-2 font-semibold" { "单价" }
                    th class="text-center py-1.5 px-2 font-semibold" { "委外" }
                    th class="text-center py-1.5 px-2 font-semibold" { "质检点" }
                    th class="text-right py-1.5 px-2 font-semibold" { "操作" }
                }
            }
            tbody {
                @for r in routings {
                    (render_release_routing_row(r, order_id, wc_map, prod_map))
                }
            }
        }
    }
}

/// 工序行：产出品/工作中心名称从 map 映射（无则 —）。
fn render_release_routing_row(
    r: &WorkOrderRouting,
    order_id: i64,
    wc_map: &HashMap<i64, String>,
    prod_map: &HashMap<i64, String>,
) -> Markup {
    let wc_name = r
        .work_center_id
        .and_then(|id| wc_map.get(&id))
        .map(String::as_str)
        .unwrap_or("—");
    let prod_name = r
        .product_id
        .and_then(|id| prod_map.get(&id))
        .map(String::as_str)
        .unwrap_or("—");
    html! {
        tr class="border-b border-border-soft last:border-b-0" {
            td class="py-1.5 px-2 text-muted font-mono" { (r.step_no) }
            td class="py-1.5 px-2 text-fg" { (r.process_name) }
            td class="py-1.5 px-2 text-fg-2" { (prod_name) }
            td class="py-1.5 px-2 text-fg-2" { (wc_name) }
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
            td class="py-1.5 px-2 text-right whitespace-nowrap" {
                button class="text-muted hover:text-accent cursor-pointer border-none bg-transparent p-1 align-middle"
                    title="编辑产出品/单价"
                    hx-get=(WcRoutingEditPath { order_id, routing_id: r.id }.to_string())
                    hx-target="#routing-edit-slot" hx-swap="innerHTML"
                    { (icon::edit_icon("w-4 h-4")) }
                button class="text-muted hover:text-danger cursor-pointer border-none bg-transparent p-1 ml-1 align-middle"
                    title="删除该工序"
                    hx-post=(OrderRoutingDeletePath { order_id, routing_id: r.id }.to_string())
                    hx-confirm="删除该工序并重排后续工序号？"
                    hx-swap="none"
                    hx-disabled-elt="this"
                    { (icon::trash_icon("w-4 h-4")) }
            }
        }
    }
}

// ── 工序编辑 modal（GET 返回完整 modal_shell swap 进 #routing-edit-slot；POST 保存）──
// 复用 abt-core update_routing service；与 mes_order_detail 的 drawer 编辑解耦。

#[require_permission("WORK_ORDER", "update")]
pub async fn get_wc_routing_edit(
    path: WcRoutingEditPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    use abt_core::master_data::work_center::WorkCenterService;
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_batch_service();
    let routings = svc.list_routings(&service_ctx, &mut conn, path.order_id).await?;
    let routing = routings
        .iter()
        .find(|r| r.id == path.routing_id)
        .ok_or_else(|| DomainError::not_found("WorkOrderRouting"))?;
    let pname =
        resolve_routing_product_name(&state, &service_ctx, &mut conn, routing.product_id).await;
    let work_centers = new_work_center_service(state.pool.clone())
        .list_active(&service_ctx, &mut conn)
        .await
        .unwrap_or_default();
    Ok(Html(
        modal_shell(
            "routing-edit-modal",
            "z-[1000]",
            wc_routing_edit_panel(path.order_id, path.routing_id, routing, &pname, &work_centers, None),
        )
        .into_string(),
    ))
}

/// POST：保存产出品/单价/工作中心/工时/委外 → 广播 routingChanged（#wc-release-routings 自刷新）；
/// 失败 OOB 回填完整面板 + alert 到 #routing-edit-modal（modal 保持开）。
#[require_permission("WORK_ORDER", "update")]
pub async fn post_wc_routing_edit(
    path: WcRoutingEditPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<RoutingEditForm>,
) -> Result<impl IntoResponse> {
    use abt_core::master_data::work_center::WorkCenterService;
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_batch_service();
    match svc
        .update_routing(
            &service_ctx,
            &mut conn,
            path.order_id,
            path.routing_id,
            form.product_id,
            form.unit_price,
            form.work_center_id,
            form.standard_time,
            form.is_outsourced,
        )
        .await
    {
        Ok(_updated) => Ok(([("HX-Trigger", "routingChanged")], Html(String::new()))),
        Err(e) => {
            let routings = svc.list_routings(&service_ctx, &mut conn, path.order_id).await?;
            let routing = routings
                .iter()
                .find(|r| r.id == path.routing_id)
                .ok_or_else(|| DomainError::not_found("WorkOrderRouting"))?;
            let pname = resolve_routing_product_name(&state, &service_ctx, &mut conn, routing.product_id).await;
            let work_centers = new_work_center_service(state.pool.clone())
                .list_active(&service_ctx, &mut conn)
                .await
                .unwrap_or_default();
            Ok((
                [("HX-Trigger", "")],
                Html(html! {
                    div hx-swap-oob="innerHTML:#routing-edit-modal" {
                        (wc_routing_edit_panel(path.order_id, path.routing_id, routing, &pname, &work_centers, Some(&format!("保存失败：{}", friendly_err_msg(&e)))))
                    }
                }.into_string()),
            ))
        }
    }
}

/// 解析单个产出品名（无则 #id 或空串）。
async fn resolve_routing_product_name(
    state: &crate::state::AppState,
    ctx: &abt_core::shared::types::context::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    pid: Option<i64>,
) -> String {
    match pid {
        Some(id) => state
            .product_service()
            .get_by_ids(ctx, db, vec![id])
            .await
            .ok()
            .and_then(|v| v.into_iter().next())
            .map(|p| p.pdt_name)
            .unwrap_or_else(|| format!("#{}", id)),
        None => String::new(),
    }
}

/// 剥离 DomainError Display 的类型前缀（Validation/Business rule/...），只留可读消息供 alert 展示。
fn friendly_err_msg(e: &DomainError) -> String {
    let s = e.to_string();
    for p in ["Validation: ", "Business rule: ", "Unauthorized: ", "Permission denied: "] {
        if let Some(rest) = s.strip_prefix(p) {
            return rest.to_string();
        }
    }
    s
}

/// 工序编辑完整 modal 面板（header + form[产出品 picker/工作中心/单价/工时/委外 + footer 取消/保存]）。
/// 关闭靠保存 form 自身的 afterRequest（成功空 body 判定）+ × / Esc。
fn wc_routing_edit_panel(
    work_order_id: i64,
    routing_id: i64,
    r: &WorkOrderRouting,
    product_name: &str,
    work_centers: &[abt_core::master_data::work_center::model::WorkCenter],
    error_msg: Option<&str>,
) -> Markup {
    html! {
        div class="bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl" {
            div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                h2 class="text-lg font-semibold m-0" { "编辑工序" }
                button type="button"
                    class="text-2xl text-muted hover:text-fg cursor-pointer bg-transparent border-none p-1 leading-none"
                    _="on click remove .is-open from #routing-edit-modal then empty #routing-edit-modal"
                    { "×" }
            }
            div class="overflow-y-auto flex-1 min-h-0 p-6" {
                @if let Some(msg) = error_msg {
                    div class="mb-4" { (alert::alert_error(msg)) }
                }
                form id="wc-routing-edit-form"
                    hx-post=(WcRoutingEditPath { order_id: work_order_id, routing_id }.to_string())
                    hx-swap="none"
                    // 成功（200 + 空 body）才关 modal；失败返回 OOB 面板（responseText 非空）保持打开显示 alert。
                    _="on 'htmx:afterRequest'[detail.xhr.responseText.length == 0] remove .is-open from #routing-edit-modal then empty #routing-edit-modal"
                {
                    input type="hidden" name="product_id" id="routing-product-id"
                        value=(r.product_id.map(|p| p.to_string()).unwrap_or_default()) {};
                    div class="mb-4" {
                        label class="block text-xs font-medium text-fg-2 mb-1.5" { "产出品" }
                        div class="flex gap-2" {
                            span id="routing-product-display"
                                class="flex-1 px-3 py-2 border border-border rounded-sm text-sm bg-surface inline-flex items-center min-h-[38px]"
                            {
                                @if product_name.is_empty() {
                                    span class="text-muted" { "点击右侧选择产出品…" }
                                } @else {
                                    (product_name)
                                }
                            }
                            button type="button"
                                class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg-2 cursor-pointer hover:bg-surface"
                                _="on click add .is-open to #routing-product-modal"
                                { "选择" }
                        }
                    }
                    div class="mb-4" {
                        label class="block text-xs font-medium text-fg-2 mb-1.5" { "工作中心" }
                        select name="work_center_id"
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent"
                        {
                            option value="" { "—" }
                            @for wc in work_centers {
                                option value=(wc.id) selected[r.work_center_id == Some(wc.id)] { (wc.name) }
                            }
                        }
                    }
                    div class="grid grid-cols-2 gap-3 mb-4" {
                        div {
                            label class="block text-xs font-medium text-fg-2 mb-1.5" { "计件单价（元/件）" }
                            input name="unit_price" type="number" step="any" required
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent font-mono"
                                value=(r.unit_price.map(|p| p.to_string()).unwrap_or_default());
                        }
                        div {
                            label class="block text-xs font-medium text-fg-2 mb-1.5" { "标准工时（小时）" }
                            input name="standard_time" type="number" step="any"
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent font-mono"
                                value=(r.standard_time.map(|t| t.to_string()).unwrap_or_default());
                        }
                    }
                    div class="mb-2" {
                        label class="flex items-center gap-2 cursor-pointer" {
                            input type="checkbox" name="is_outsourced" value="true"
                                class="w-4 h-4 accent-accent" checked[r.is_outsourced];
                            span class="text-sm text-fg-2 select-none" { "委外工序" }
                        }
                    }
                    div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3" {
                        button type="button"
                            class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150"
                            _="on click remove .is-open from #routing-edit-modal then empty #routing-edit-modal"
                            { "取消" }
                        button type="submit"
                            class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150"
                            { "保存" }
                    }
                }
            }
        }
    }
}

/// 单条生产批次行（数量 input + 删除按钮）。.split-row 供 addSplitRow 克隆；至少保留 1 行。
fn render_split_row(idx: usize, qty: Decimal) -> Markup {
    html! {
        div class="split-row flex items-center gap-2 mb-2" {
            span class="text-xs text-muted w-20 whitespace-nowrap split-label" { "生产批次" (idx + 1) }
            input class="split-qty w-24 px-2 py-1 border border-border rounded-sm text-sm font-mono text-right bg-white outline-none focus:border-accent"
                type="number" step="0.01 " min="0"
                name=(format!("splits[{idx}][batch_qty]")) value=(fmt_qty(qty));
            span class="text-xs text-muted" { "件" }
            button type="button" class="split-remove text-muted hover:text-danger cursor-pointer bg-transparent border-none px-1 text-base leading-none disabled:opacity-30 disabled:cursor-not-allowed"
                title="删除生产批次"
                disabled
                _="on click call removeSplitRow(me)" { "×" }
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
        // 工序非空校验：报工强依赖工序，无工序下达会形成无法报工的死状态。
        // 用户需先在 drawer「② 工序生成」手动从 Routing / 最近工单加载工序。
        let routings = batch_svc
            .list_routings(&service_ctx, &mut tx, path.order_id)
            .await?;
        if routings.is_empty() {
            return Err(DomainError::business_rule(
                "工单尚无工序，请先在「② 工序生成」从 Routing 加载工序后再下达",
            )
            .into());
        }
        // #124 加强：每道工序必须配置产出品 + 计件单价（工序级领料依赖产出品）
        for r in &routings {
            if r.product_id.is_none() {
                return Err(DomainError::BusinessRule(format!(
                    "工序 {}「{}」未配置产出品，无法下达（请先编辑工序配上产出品）",
                    r.step_no,
                    r.process_name
                ))
                .into());
            }
            if r.unit_price.is_none() || r.unit_price == Some(Decimal::ZERO) {
                return Err(DomainError::BusinessRule(format!(
                    "工序 {}「{}」未配置计件单价，无法下达",
                    r.step_no,
                    r.process_name
                ))
                .into());
            }
        }
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
// 批次 tab：drawer body + 各操作 handler（batch 维度，不跳转）
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct BatchReportForm {
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

#[derive(Debug, Default, Deserialize)]
pub struct BatchReasonForm {
    #[serde(default)]
    pub reason: String,
}

/// 批次处理 drawer body：批次信息 + 工序进度 + 报工表单 + 状态操作（按 BatchStatus 门控）。
#[require_permission("WORK_ORDER", "read")]
pub async fn get_batch_drawer(
    path: WcBatchDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let batch_svc = state.production_batch_service();
    let wo_svc = state.work_order_service();
    let batch = batch_svc.find_by_id(&service_ctx, &mut conn, path.batch_id).await?;
    let order = wo_svc.find_by_id(&service_ctx, &mut conn, batch.work_order_id).await?;
    let product_name = wo_svc
        .get_product_name(&mut conn, batch.product_id)
        .await?
        .unwrap_or_else(|| format!("#{}", batch.product_id));
    let routings = batch_svc
        .list_routings(&service_ctx, &mut conn, batch.work_order_id)
        .await
        .unwrap_or_default();
    // 每道工序产出品的物料齐套（#124 工序级齐套）
    let mut step_avails: HashMap<i64, abt_core::mes::work_order::MaterialAvailability> =
        HashMap::new();
    for r in &routings {
        if r.product_id.is_some()
            && let Ok(avail) = wo_svc
                .compute_step_availability(
                    &service_ctx,
                    &mut conn,
                    batch.work_order_id,
                    r.id,
                    Some(batch.id),
                )
                .await
        {
            step_avails.insert(r.id, avail);
        }
    }
    // 工作中心 id→name
    let wc_map: HashMap<i64, String> = new_work_center_service(state.pool.clone())
        .list_active(&service_ctx, &mut conn)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|wc| (wc.id, wc.name))
        .collect();
    Ok(Html(
        render_batch_drawer_body(&batch, &order, &product_name, &routings, &step_avails, &wc_map)
            .into_string(),
    ))
}

fn render_batch_drawer_body(
    batch: &ProductionBatch,
    order: &WorkOrder,
    product_name: &str,
    routings: &[WorkOrderRouting],
    step_avails: &HashMap<i64, abt_core::mes::work_order::MaterialAvailability>,
    wc_map: &HashMap<i64, String>,
) -> Markup {
    let (slabel, stoken) = batch_status_meta(&batch.status);
    let can_start = matches!(batch.status, BatchStatus::Pending);
    let can_suspend = matches!(batch.status, BatchStatus::InProgress);
    let can_resume = matches!(batch.status, BatchStatus::Suspended);
    let can_scrap = matches!(batch.status, BatchStatus::InProgress | BatchStatus::Suspended);
    let can_receipt = matches!(batch.status, BatchStatus::PendingReceipt);
    html! {
        // 批次信息头
        div class="mb-4 pb-3 border-b border-border-soft" {
            div class="flex items-center gap-2 mb-1" {
                span class="text-xs text-muted" { "流转卡" }
                span class="font-mono font-semibold text-fg" { (batch.card_sn.as_str()) }
                span class=(format!("ml-auto inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium bg-{stoken}-bg text-{stoken}")) {
                    span class=(format!("inline-block w-1.5 h-1.5 rounded-full bg-{stoken}")) {}
                    (slabel)
                }
            }
            div class="text-xs text-muted leading-relaxed" {
                (product_name) " · " (fmt_qty(batch.batch_qty)) " 件 · 工单 " (order.doc_number.as_str())
                br;
                "进度 " (fmt_qty(batch.completed_qty)) "/" (fmt_qty(batch.batch_qty)) " 件 · 工序 " (batch.current_step) "/" (routings.len() as i32)
            }
        }

        // 工序流转矩阵（#124 v1 矩阵）
        div class="overflow-x-auto mb-4" {
            table class="w-full text-sm" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="text-left font-semibold py-2 px-3" { "工序" }
                        th class="text-left font-semibold py-2 px-3" { "①齐套分析" }
                        th class="text-left font-semibold py-2 px-3" { "②领料" }
                        th class="text-left font-semibold py-2 px-3" { "③报工" }
                    }
                }
                tbody {
                    @if routings.is_empty() {
                        tr { td colspan="4" class="text-center text-muted py-6" { "该工单尚无工序" } }
                    }
                    @for r in routings {
                        (render_batch_matrix_row(batch, r, step_avails, wc_map))
                    }
                }
            }
        }

        // 底部状态操作 + 入库
        div class="pt-3 border-t border-border-soft flex flex-wrap items-center gap-2" {
            @if can_start {
                form hx-post=(WcBatchStartPath { batch_id: batch.id }.to_string()) hx-swap="none"
                    hx-on:htmx:after-request="if(event.detail.successful) document.querySelector('#batch-overlay').classList.remove('open')" {
                    button type="submit" class="px-3 py-1.5 rounded-sm bg-accent text-accent-on border-none text-xs font-medium cursor-pointer hover:bg-accent-hover"
                        hx-confirm="开工该批次？" { "开工" }
                }
            }
            @if can_suspend {
                form hx-post=(WcBatchSuspendPath { batch_id: batch.id }.to_string()) hx-swap="none"
                    hx-on:htmx:after-request="if(event.detail.successful) document.querySelector('#batch-overlay').classList.remove('open')" {
                    input type="hidden" name="reason" value="手动暂停";
                    button type="submit" class="px-3 py-1.5 rounded-sm border border-border text-xs font-medium text-fg-2 cursor-pointer hover:bg-warn-bg hover:text-warn"
                        hx-confirm="暂停？" { "暂停" }
                }
            }
            @if can_resume {
                form hx-post=(WcBatchResumePath { batch_id: batch.id }.to_string()) hx-swap="none"
                    hx-on:htmx:after-request="if(event.detail.successful) document.querySelector('#batch-overlay').classList.remove('open')" {
                    button type="submit" class="px-3 py-1.5 rounded-sm border border-border text-xs font-medium text-fg-2 cursor-pointer hover:bg-accent-bg hover:text-accent" { "恢复" }
                }
            }
            @if can_scrap {
                form hx-post=(WcBatchScrapPath { batch_id: batch.id }.to_string()) hx-swap="none"
                    hx-on:htmx:after-request="if(event.detail.successful) document.querySelector('#batch-overlay').classList.remove('open')" {
                    input type="hidden" name="reason" value="手动报废";
                    button type="submit" class="px-3 py-1.5 rounded-sm border border-border text-xs font-medium text-fg-2 cursor-pointer hover:bg-danger-bg hover:text-danger"
                        hx-confirm="报废？不可撤销" { "报废" }
                }
            }
            div class="ml-auto flex items-center gap-2" {
                @if can_receipt {
                    button class="px-3 py-1.5 rounded-sm bg-success text-white border-none text-xs font-medium cursor-pointer hover:opacity-90"
                        hx-get=(WcBatchReceiptDrawerPath { batch_id: batch.id }.to_string())
                        hx-target="#batch-receipt-drawer-body" hx-swap="innerHTML"
                        _="on click halt the event" { "入库" }
                } @else {
                    span class="text-xs text-muted" { "工序全部报工后可入库" }
                }
            }
        }
    }
}

/// 物料可用性四级 → (标签, 语义色 token)。
fn avail_meta(level: &abt_core::mes::work_order::MaterialAvailabilityLevel) -> (&'static str, &'static str) {
    use abt_core::mes::work_order::MaterialAvailabilityLevel::*;
    match level {
        Available => ("✅ 齐套", "success"),
        Expected => ("🟡 在途", "warning"),
        Late => ("🟠 迟料", "warn"),
        Unavailable => ("🔴 欠缺", "danger"),
    }
}

/// 矩阵行：工序 | 齐套徽章 | 领料 | 报工（#124）。
fn render_batch_matrix_row(
    batch: &ProductionBatch,
    r: &WorkOrderRouting,
    step_avails: &HashMap<i64, abt_core::mes::work_order::MaterialAvailability>,
    wc_map: &HashMap<i64, String>,
) -> Markup {
    use abt_core::mes::work_order::MaterialAvailabilityLevel;
    let wc_name = r
        .work_center_id
        .and_then(|id| wc_map.get(&id))
        .map(String::as_str)
        .unwrap_or("—");
    let (alabel, atoken, shortage_n) = match step_avails.get(&r.id) {
        Some(a) => {
            let n = a
                .lines
                .iter()
                .filter(|l| matches!(l.level, MaterialAvailabilityLevel::Unavailable | MaterialAvailabilityLevel::Late))
                .count();
            let (lb, tk) = avail_meta(&a.level);
            (lb, tk, n)
        }
        None => ("散料", "muted", 0),
    };
    let report_state = if r.step_no < batch.current_step {
        "✅ 已完成"
    } else if r.step_no == batch.current_step && matches!(batch.status, BatchStatus::InProgress) {
        "🔵 进行中"
    } else {
        "—"
    };
    let can_act = matches!(batch.status, BatchStatus::Pending | BatchStatus::InProgress);
    html! {
        tr class="border-b border-border-soft align-top" {
            td class="py-2.5 px-3" {
                div class="font-medium text-fg" { (r.step_no) ". " (r.process_name.as_str()) }
                div class="text-xs text-muted mt-0.5" { (wc_name) }
            }
            td class="py-2.5 px-3" {
                @if shortage_n > 0 {
                    button class=(format!("inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium whitespace-nowrap bg-{atoken}-bg text-{atoken} cursor-pointer hover:opacity-80 transition-opacity"))
                        hx-get=(WcBatchShortagePath { batch_id: batch.id, routing_id: r.id }.to_string())
                        hx-target=(format!("#batch-shortage-{}-{}", batch.id, r.id))
                        hx-swap="innerHTML"
                        _="on click halt the event" {
                        (alabel) " 欠" (shortage_n) "项 ▾"
                    }
                    div id=(format!("batch-shortage-{}-{}", batch.id, r.id)) class="mt-2" {}
                } @else {
                    span class=(format!("inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium whitespace-nowrap bg-{atoken}-bg text-{atoken}")) { (alabel) }
                }
            }
            td class="py-2.5 px-3" {
                @if can_act {
                    button class="text-xs px-2 py-1 rounded-sm border border-border text-fg-2 hover:bg-accent-bg hover:text-accent cursor-pointer transition-all"
                        hx-get=(WcBatchReqDrawerPath { batch_id: batch.id }.to_string())
                        hx-target="#batch-req-drawer-body" hx-swap="innerHTML"
                        _="on click halt the event" {
                        "领料"
                    }
                } @else {
                    span class="text-xs text-muted" { "—" }
                }
            }
            td class="py-2.5 px-3 text-xs text-fg-2" { (report_state) }
        }
    }
}

/// 批次工序缺料明细（#124）：展开该工序产出品子 BOM 的物料可用性明细。
#[require_permission("WORK_ORDER", "read")]
pub async fn get_batch_shortage(
    path: WcBatchShortagePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let batch_svc = state.production_batch_service();
    let batch = batch_svc.find_by_id(&service_ctx, &mut conn, path.batch_id).await?;
    let avail = state
        .work_order_service()
        .compute_step_availability(
            &service_ctx,
            &mut conn,
            batch.work_order_id,
            path.routing_id,
            Some(path.batch_id),
        )
        .await?;
    Ok(Html(render_shortage_detail(&avail).into_string()))
}

/// 缺料明细表（物料×需求×库存×在途×状态）。
fn render_shortage_detail(avail: &abt_core::mes::work_order::MaterialAvailability) -> Markup {
    html! {
        div class="rounded-sm border border-border-soft overflow-hidden" {
            table class="w-full text-xs" {
                thead {
                    tr class="bg-surface text-muted" {
                        th class="text-left font-medium py-1.5 px-2" { "物料" }
                        th class="text-right font-medium py-1.5 px-2" { "需求" }
                        th class="text-right font-medium py-1.5 px-2" { "库存" }
                        th class="text-right font-medium py-1.5 px-2" { "在途" }
                        th class="text-right font-medium py-1.5 px-2" { "状态" }
                    }
                }
                tbody {
                    @for line in &avail.lines {
                        @let (lb, tk) = avail_meta(&line.level);
                        tr class="border-t border-border-soft" {
                            td class="py-1.5 px-2" {
                                div class="text-fg" { (line.product_name.as_str()) }
                                div class="text-muted font-mono" { (line.product_code.as_str()) }
                            }
                            td class="py-1.5 px-2 text-right font-mono" { (fmt_qty(line.required_qty)) }
                            td class="py-1.5 px-2 text-right font-mono" { (fmt_qty(line.atp)) }
                            td class="py-1.5 px-2 text-right font-mono" { (fmt_qty(line.projected)) }
                            td class="py-1.5 px-2 text-right" {
                                span class=(format!("inline-flex items-center px-1.5 py-0.5 rounded-full text-[10px] font-medium whitespace-nowrap bg-{tk}-bg text-{tk}")) { (lb) }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 批次报工：confirm_routing_step（batch_id + step_no），广播 batchChanged。
#[require_permission("WORK_ORDER", "update")]
pub async fn batch_report(
    path: WcBatchReportPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<BatchReportForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let completed_qty = form
        .completed_qty
        .parse::<Decimal>()
        .map_err(|_| DomainError::validation("完成量格式错误"))?;
    let defect_qty = form.defect_qty.parse::<Decimal>().unwrap_or(Decimal::ZERO);
    let work_hours = form.work_hours.parse::<Decimal>().unwrap_or(Decimal::ZERO);
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
        .confirm_routing_step(&service_ctx, &mut tx, path.batch_id, form.step_no, req)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(([("HX-Trigger", "batchChanged")], Html(String::new())))
}

/// 批次暂停（InProgress → Suspended），广播 batchChanged。
#[require_permission("WORK_ORDER", "update")]
pub async fn batch_suspend(
    path: WcBatchSuspendPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<BatchReasonForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let reason = if form.reason.trim().is_empty() {
        "手动暂停".to_string()
    } else {
        form.reason
    };
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .production_batch_service()
        .suspend(&service_ctx, &mut tx, path.batch_id, reason)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(([("HX-Trigger", "batchChanged")], Html(String::new())))
}

/// 批次恢复（Suspended → InProgress），广播 batchChanged。
#[require_permission("WORK_ORDER", "update")]
pub async fn batch_resume(
    path: WcBatchResumePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .production_batch_service()
        .resume(&service_ctx, &mut tx, path.batch_id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(([("HX-Trigger", "batchChanged")], Html(String::new())))
}

/// 批次报废（InProgress/Suspended → Cancelled），广播 batchChanged。
#[require_permission("WORK_ORDER", "update")]
pub async fn batch_scrap(
    path: WcBatchScrapPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<BatchReasonForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let reason = if form.reason.trim().is_empty() {
        "手动报废".to_string()
    } else {
        form.reason
    };
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .production_batch_service()
        .scrap(&service_ctx, &mut tx, path.batch_id, reason)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(([("HX-Trigger", "batchChanged")], Html(String::new())))
}

/// 批次推进入库（InProgress/Suspended → PendingReceipt），广播 batchChanged。
#[require_permission("WORK_ORDER", "update")]
pub async fn batch_advance(
    path: WcBatchAdvancePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .production_batch_service()
        .advance_to_receipt(&service_ctx, &mut tx, path.batch_id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(([("HX-Trigger", "batchChanged")], Html(String::new())))
}

/// 批次开工：Pending → InProgress（start_batch），广播 batchChanged。
#[require_permission("WORK_ORDER", "update")]
pub async fn batch_start(
    path: WcBatchStartPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .production_batch_service()
        .start_batch(&service_ctx, &mut tx, path.batch_id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(([("HX-Trigger", "batchChanged")], Html(String::new())))
}

#[derive(Debug, Deserialize)]
pub struct BatchReqForm {
    pub routing_id: i64,
}

/// 批次领料 drawer body：列工单工序（有产出品可领料）。
#[require_permission("WORK_ORDER", "read")]
pub async fn get_batch_req_drawer(
    path: WcBatchReqDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let batch_svc = state.production_batch_service();
    let wo_svc = state.work_order_service();
    let batch = batch_svc.find_by_id(&service_ctx, &mut conn, path.batch_id).await?;
    let order = wo_svc.find_by_id(&service_ctx, &mut conn, batch.work_order_id).await?;
    let routings = batch_svc
        .list_routings(&service_ctx, &mut conn, batch.work_order_id)
        .await
        .unwrap_or_default();
    Ok(Html(render_batch_req_drawer_body(&batch, &order, &routings).into_string()))
}

fn render_batch_req_drawer_body(
    batch: &ProductionBatch,
    order: &WorkOrder,
    routings: &[WorkOrderRouting],
) -> Markup {
    html! {
        // 批次信息头
        div class="mb-5 pb-4 border-b border-border-soft" {
            div class="text-xs text-muted" { "流转卡" }
            div class="font-mono font-semibold text-fg" { (batch.card_sn.as_str()) }
            div class="text-xs text-muted font-mono mt-1" {
                "工单 " (order.doc_number.as_str()) " · 本批 " (fmt_qty(batch.batch_qty)) " 件"
            }
        }
        // 说明：按工序产出品领料
        div class="flex items-start gap-2 text-xs text-fg-2 bg-accent-bg rounded-sm px-3 py-2 mb-4" {
            (icon::info_icon("w-4 h-4 shrink-0 mt-0.5 text-accent"))
            span { "按工序产出品领料：自动展开该工序产出品的子 BOM 原料，数量按本批 "
                (fmt_qty(batch.batch_qty)) " 件计算。产出品无 BOM 的工序（散料）走完工倒冲。"
            }
        }
        // 工序列表
        div class="space-y-2" {
            @if routings.is_empty() {
                div class="text-xs text-muted text-center py-4" { "该工单尚无工序" }
            }
            @for r in routings {
                div class="flex items-center justify-between p-3 border border-border-soft rounded-sm" {
                    div {
                        div class="text-sm font-medium text-fg" {
                            (r.step_no) ". " (r.process_name.as_str())
                        }
                        div class="text-xs text-muted mt-0.5" {
                            @match r.product_id {
                                Some(pid) => { "产出品 #" (pid) " · 按子 BOM 领料" }
                                None => { "未配置产出品 · 散料（走倒冲）" }
                            }
                        }
                    }
                    @if r.product_id.is_some() {
                        form hx-post=(WcBatchReqPath { batch_id: batch.id }.to_string()) hx-swap="none"
                            hx-on:htmx:after-request="if(event.detail.successful) document.querySelector('#batch-req-overlay').classList.remove('open')" {
                            input type="hidden" name="routing_id" value=(r.id);
                            button type="submit"
                                class="px-3 py-1.5 rounded-sm bg-accent text-accent-on border-none text-xs font-medium cursor-pointer hover:bg-accent-hover transition-all" {
                                "领料"
                            }
                        }
                    } @else {
                        span class="text-xs text-muted" { "—" }
                    }
                }
            }
        }
    }
}

/// 批次领料提交：create_for_routing_step（产出品→子BOM→operation_id），广播 batchChanged。
#[require_permission("WORK_ORDER", "update")]
pub async fn batch_requisition(
    path: WcBatchReqPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<BatchReqForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let batch_svc = state.production_batch_service();
    let batch = batch_svc.find_by_id(&service_ctx, &mut tx, path.batch_id).await?;
    state
        .material_requisition_service()
        .create_for_routing_step(
            &service_ctx,
            &mut tx,
            batch.work_order_id,
            form.routing_id,
            Some(path.batch_id),
        )
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(([("HX-Trigger", "batchChanged")], Html(String::new())))
}

#[derive(Debug, Deserialize)]
pub struct BatchReceiptForm {
    pub warehouse_id: i64,
    pub received_qty: String,
    pub receipt_date: chrono::NaiveDate,
    #[serde(default)]
    pub remark: Option<String>,
}

/// 批次入库 drawer body。
#[require_permission("WORK_ORDER", "read")]
pub async fn get_batch_receipt_drawer(
    path: WcBatchReceiptDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let batch_svc = state.production_batch_service();
    let wo_svc = state.work_order_service();
    let batch = batch_svc.find_by_id(&service_ctx, &mut conn, path.batch_id).await?;
    let order = wo_svc.find_by_id(&service_ctx, &mut conn, batch.work_order_id).await?;
    let product_name = wo_svc
        .get_product_name(&mut conn, order.product_id)
        .await?
        .unwrap_or_else(|| format!("#{}", order.product_id));
    Ok(Html(render_batch_receipt_drawer_body(&batch, &order, &product_name).into_string()))
}

fn render_batch_receipt_drawer_body(
    batch: &ProductionBatch,
    order: &WorkOrder,
    product_name: &str,
) -> Markup {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    html! {
        div class="mb-5 pb-4 border-b border-border-soft" {
            div class="text-xs text-muted" { "流转卡" }
            div class="font-mono font-semibold text-fg" { (batch.card_sn.as_str()) }
            div class="text-sm text-fg-2 mt-1" { (product_name) }
            div class="text-xs text-muted font-mono mt-0.5" { "工单 " (order.doc_number.as_str()) }
        }
        form hx-post=(WcBatchReceiptPath { batch_id: batch.id }.to_string())
            hx-swap="none"
            hx-on:htmx:after-request="if(event.detail.successful) document.querySelector('#batch-receipt-overlay').classList.remove('open')" {
            div class="grid grid-cols-2 gap-3 mb-3" {
                div {
                    label class="block text-xs text-fg-2 mb-1" { "入库数量" }
                    input type="number" step="0.01" min="0" name="received_qty" value=(fmt_qty(batch.batch_qty))
                        class="w-full px-2 py-1.5 border border-border rounded-sm text-sm font-mono text-right bg-white outline-none focus:border-accent";
                }
                div {
                    label class="block text-xs text-fg-2 mb-1" { "目标仓库 ID" }
                    input type="number" name="warehouse_id" required
                        class="w-full px-2 py-1.5 border border-border rounded-sm text-sm font-mono bg-white outline-none focus:border-accent";
                }
            }
            div class="mb-3" {
                label class="block text-xs text-fg-2 mb-1" { "入库日期" }
                input type="date" name="receipt_date" value=(today)
                    class="w-full px-2 py-1.5 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent";
            }
            div class="mb-3" {
                label class="block text-xs text-fg-2 mb-1" { "备注" }
                input type="text" name="remark"
                    class="w-full px-2 py-1.5 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent";
            }
            div class="flex items-start gap-2 text-xs text-fg-2 bg-accent-bg rounded-sm px-3 py-2 mb-3" {
                (icon::info_icon("w-4 h-4 shrink-0 mt-0.5 text-accent"))
                span { "入库触发倒冲（按 BOM 扣原材料）+ 成本归集 + FQC 门控。" }
            }
            div class="flex justify-end" {
                button type="submit"
                    class="px-3.5 py-2 rounded-sm bg-accent text-accent-on border-none text-sm font-medium cursor-pointer hover:bg-accent-hover" {
                    "确认入库"
                }
            }
        }
    }
}

/// 批次入库：ProductionReceipt.create+confirm（倒冲 + 成本 + FQC），广播 batchChanged。
#[require_permission("WORK_ORDER", "update")]
pub async fn batch_receipt(
    path: WcBatchReceiptPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<BatchReceiptForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let received_qty = form
        .received_qty
        .parse::<Decimal>()
        .map_err(|_| DomainError::validation("入库数量格式错误"))?;
    if received_qty <= Decimal::ZERO {
        return Err(DomainError::validation("入库数量必须大于 0").into());
    }
    let batch_svc = state.production_batch_service();
    let batch = batch_svc.find_by_id(&service_ctx, &mut tx, path.batch_id).await?;
    let order = state
        .work_order_service()
        .find_by_id(&service_ctx, &mut tx, batch.work_order_id)
        .await?;
    let rcpt_svc = state.production_receipt_service();
    let req = CreateReceiptReq {
        work_order_id: batch.work_order_id,
        batch_id: Some(path.batch_id),
        product_id: order.product_id,
        received_qty,
        warehouse_id: form.warehouse_id,
        zone_id: None,
        bin_id: None,
        receipt_date: form.receipt_date,
        remark: form.remark,
    };
    let receipt_id = rcpt_svc.create(&service_ctx, &mut tx, req).await?;
    rcpt_svc.confirm(&service_ctx, &mut tx, receipt_id).await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(([("HX-Trigger", "batchChanged")], Html(String::new())))
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
        }
    }
}

/// Card 外壳：标题栏 + 占位 div（`hx-trigger="load"` 拉 card 端点内容替换）。
///
/// 占位 div 的 `id` 与 card 端点返回的最外层 div 一致，懒加载与 card 内交互
/// 都用 `hx-swap="outerHTML"` + `hx-select="#wc-xxx-card"` 替换，保证 card 自洽。
fn render_card_shell(
    card_id: &str,
    src: &str,
    title: &str,
    icon: Markup,
    dot: Option<(u64, &'static str)>,
    meta: Option<Markup>,
) -> Markup {
    html! {
        section class="bg-bg border border-border-soft rounded-lg mb-4 shadow-[var(--shadow-card)] overflow-hidden" {
            div class="flex items-center gap-3 px-5 py-3 border-b border-border-soft" {
                div class="relative w-7 h-7 rounded-md grid place-items-center bg-surface text-fg-2 shrink-0" {
                    (icon)
                    @if let Some((count, token)) = dot {
                        @if count > 0 {
                            span class=(format!("absolute -top-0.5 -right-0.5 w-2 h-2 rounded-full bg-{token} ring-2 ring-bg")) {}
                        }
                    }
                }
                span class="font-semibold text-fg shrink-0" { (title) }
                @if let Some(m) = meta {
                    span class="text-xs text-muted font-mono flex-1 min-w-0 truncate" { (m) }
                } @else {
                    span class="flex-1" {}
                }
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
/// 开关：overlay 用 `.drawer-overlay` class，hyperscript toggle `.open`（preflight CSS 驱动显隐+平移，同 components/drawer.rs）。
fn render_drawer_overlay(overlay_id: &str, _drawer_id: &str, body_id: &str, title: &str, width_class: &str) -> Markup {
    drawer_shell(overlay_id, width_class, html! {
        div class="flex items-center justify-between px-6 py-5 border-b border-border-soft" {
            div class="font-bold text-base text-fg" { (title) }
            button type="button"
                class="w-8 h-8 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg flex items-center justify-center"
                _=(format!("on click remove .open from #{}", overlay_id)) {
                (icon::x_icon("w-4 h-4"))
            }
        }
        div id=(body_id) class="flex-1 overflow-y-auto px-6 py-5"
            _=(format!("on 'htmx:afterSettle' add .open to #{}", overlay_id)) {}
    })
}
