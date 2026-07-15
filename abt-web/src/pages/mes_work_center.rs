//! MES 生产作业中心 — 单 card 聚合工作台（订单行明细 / 物料汇总 / 工单 / 批次 四 view tab）。
//!
//! 架构（列表页单端点模式）：
//! - 首页渲染 1 个 card 外壳（`#wc-demand-card`），占位 div `hx-trigger="load"` 拉 `WcDemandPath` 端点；
//! - 单 GET 端点 `get_demand_card` 按 `view` 参数（detail/material/orders/batches）渲染不同视图，
//!   tab/筛选/分页走同一端点 + `hx-select="#wc-demand-card"` 局部刷新（§2 列表页单端点）；
//! - 写操作（下达/分批/报工）POST 广播 `HX-Trigger: woChanged`，card 声明
//!   `hx-trigger="woChanged from:body"` 自刷新；工序由工单创建时从 BOM 自动加载，
//!   下达 drawer ② 工序区可「从 BOM 更新」同步最新工艺路线（计件单价从主数据 bom_step_prices 自动回填）。

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
use abt_core::wms::picking::{CreatePickingItemReq, CreatePickingReq, PickingService};
use abt_core::mes::work_center::{MesWorkCenterService, MesWorkCenterSummary};
use abt_core::sales::sales_order::{SalesOrder, SalesOrderItem, SalesOrderService, SalesOrderStatus};
use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::master_data::work_center::{new_work_center_service, WorkCenterService};
use abt_core::mes::work_order::{
    MaterialAvailabilityLevel, WorkOrder, WorkOrderFilter, WorkOrderService,
};
use abt_core::shared::types::{DomainError, PgExecutor, PageParams, ServiceContext};
use abt_core::shared::identity::model::UserWithRoles;
use abt_core::shared::identity::UserService;

use std::collections::HashMap;

use crate::components::alert;
use crate::components::icon;
use crate::components::material_badge::material_badge_mini;
use crate::components::overlay::drawer_shell;
use crate::components::worker_picker;
use crate::components::pagination::pagination;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_demand_pool::{MesDemandPoolCreatePath, MesDemandRowsPath};
use abt_core::master_data::bom::{new_bom_query_service, service::BomQueryService};
use abt_core::master_data::supplier::{Supplier, SupplierQuery, SupplierService};
use abt_core::om::enums::OutsourcingType;
use abt_core::om::outsourcing_order::{
    CreateOutsourcingOrderReq, OutsourcingMaterialItem, OutsourcingOrderService,
    ReceiveOutsourcingReq, SendOutsourcingReq,
};
use abt_core::wms::warehouse::{Warehouse, WarehouseFilter, WarehouseService};
use crate::routes::mes_work_center::*;
use crate::utils::{empty_as_none, fmt_qty, RequestContext};
use abt_macros::require_permission;

// =============================================================================
// 首页
// =============================================================================

#[derive(Debug, Deserialize, Default, Clone)]
pub struct WcHomeQuery {
    /// 按销售订单过滤 demand card（来自订单详情页按钮；仅 detail 视图生效）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub order_id: Option<i64>,
}

#[require_permission("WORK_ORDER", "read")]
pub async fn get_work_center(
    _path: WcPath,
    ctx: RequestContext,
    Query(q): Query<WcHomeQuery>,
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
    let summary = state
        .mes_work_center_service()
        .summary(&service_ctx, &mut conn)
        .await
        .unwrap_or_default();

    let content = work_center_content(&summary, q.order_id);

    Ok(Html(
        admin_page(
            is_htmx,
            "作业中心",
            &claims,
            "production",
            WcPath::PATH,
            "生产管理",
            Some("作业中心"),
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
fn work_center_content(summary: &MesWorkCenterSummary, order_id: Option<i64>) -> Markup {
    // order_id 有值（来自订单详情页按钮）：demand card 初始进明细视图 + 按订单过滤
    let demand_src = match order_id {
        Some(oid) => format!("{}?view=detail&order_id={}", WcDemandPath::PATH, oid),
        None => WcDemandPath::PATH.to_string(),
    };
    html! {
        div class="flex items-center justify-between mb-4 flex-wrap gap-4" {
            div {
                div class="flex items-center gap-2.5" {
                    h1 class="text-xl font-bold text-fg tracking-tight" { "作业中心" }
                    span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-accent-bg text-accent text-xs font-semibold" {
                        span class="font-mono tabular-nums font-bold" { (summary.total()) }
                        "待办"
                    }
                }
                p class="text-sm text-muted mt-1" { "需求池 · 订单排期 · 工单 一屏处理，就地下达与报工" }
            }
            a class="inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded-sm bg-accent text-accent-on text-sm font-medium no-underline cursor-pointer border-none hover:opacity-90 transition-all"
                href=(crate::routes::mes_order::OrderCreatePath::PATH) {
                (icon::plus_icon("w-3.5 h-3.5"))
                "手动创建工单"
            }
        }
        (render_card_shell("wc-demand-card", &demand_src, "生产需求池", icon::globe_icon("w-[15px] h-[15px]"), Some((summary.pending_release, "danger")),
            Some(html! { (summary.pending_release) " 张待下达 · 销售订单驱动 · 就地「转化为工单」" })))
        (render_drawer_overlay("release-overlay", "release-drawer", "release-drawer-body", "下达工单", "w-[640px]"))
        (render_drawer_overlay("create-plan-overlay", "create-plan-drawer", "create-plan-drawer-body", "创建工单", "w-[680px]"))
        (render_drawer_overlay("batch-overlay", "batch-drawer", "batch-drawer-body", "批次处理", "w-[640px]"))
        (render_drawer_overlay("order-overlay", "order-drawer", "order-drawer-body", "工单详情", "w-[720px]"))
        // 完工入库 drawer 容器（slot）：GET 返回 drawer_shell，innerHTML 进 slot；afterSettle 打开子 drawer
        div id="batch-receipt-modal-slot"
            _="on 'htmx:afterSettle'[#batch-receipt-drawer] add .open to #batch-receipt-drawer\non keydown[event.key is 'Escape' and #batch-receipt-drawer] from body remove .open from #batch-receipt-drawer" {}
        // 创建计划完成事件桥接：planCreated → 跳「工单」tab + keyword 定位刚创建的工单
        // wo_no 经 HX-Trigger JSON 传来，config-request 从 triggeringEvent.detail 取出注入 keyword
        div hx-get=(WcDemandPath::PATH)
            hx-trigger="planCreated from:body"
            hx-on:htmx:config-request="event.detail.parameters['view']='orders';var w=event.detail.triggeringEvent&&event.detail.triggeringEvent.detail&&event.detail.triggeringEvent.detail.wo_no;if(w)event.detail.parameters['keyword']=w"
            hx-target="#wc-demand-card" hx-select="#wc-demand-card" hx-swap="outerHTML" {}
        // 订单详情 drawer 容器（slot）：GET 返回完整 drawer_shell，innerHTML 进此 slot；afterSettle 打开子 drawer
        div id="wc-order-detail-slot"
            _="on 'htmx:afterSettle'[#wc-order-detail-drawer] add .open to #wc-order-detail-drawer\non keydown[event.key is 'Escape' and #wc-order-detail-drawer] from body remove .open from #wc-order-detail-drawer" {}
        // 报废 drawer 容器（slot）
        div id="batch-scrap-modal-slot"
            _="on 'htmx:afterSettle'[#batch-scrap-drawer] add .open to #batch-scrap-drawer\non keydown[event.key is 'Escape' and #batch-scrap-drawer] from body remove .open from #batch-scrap-drawer" {}
        // 报工 drawer 容器（slot）
        div id="batch-report-modal-slot"
            _="on 'htmx:afterSettle'[#batch-report-drawer] add .open to #batch-report-drawer\non keydown[event.key is 'Escape' and #batch-report-drawer] from body remove .open from #batch-report-drawer" {}
        // 报工人选择 picker（常驻 add-row；报工 modal「+ 添加工人」打开，选中加行到 #report-workers-tbody）
        (worker_picker::worker_picker_modal_with_search(
            "worker-picker-modal",
            WcWorkerRowPath::PATH,
            "report-workers-tbody",
        ))
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
    /// 批次 tab：工单号模糊搜索（wo.doc_number ILIKE）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub wo_no: Option<String>,
    /// 工单 tab：物料可用性（Available/Expected/Late/Unavailable）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub availability: Option<String>,
    /// 物料汇总/明细排序（urgency/qty/earliest/demand_count）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub sort: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
    /// 按销售订单过滤（来自订单详情页按钮；仅 detail 视图生效，material 物料汇总忽略）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub order_id: Option<i64>,
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
                "作业中心",
                &claims,
                "production",
                WcPath::PATH,
                "生产管理",
                Some("作业中心"),
                work_center_content(&summary, p.order_id),
                &nav_filter,
            )
            .into_string(),
        ));
    }
    let view = p.view.as_deref().unwrap_or("detail");
    // 各 tab 的待处理总数（固定口径，不随筛选变；COUNT 轻量，card 每次刷新都查以保证 badge 实时）
    let tab_counts = load_tab_counts(&state, &service_ctx, &mut conn).await;

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
                    work_order_no: p.wo_no.clone(),
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
                        order_id: p.order_id,
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
                hx-trigger="batchChanged from:body, woChanged from:body"
                hx-vals=(serde_json::json!({ "view": view }).to_string())
                hx-include="#wc-demand-filter-form"
                hx-select="#wc-demand-card" hx-swap="outerHTML" {
                (demand_filter_bar(view, &p, &tab_counts))
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

/// 各 tab 的待处理总数（固定口径，不随筛选变）。
struct TabCounts {
    detail: u64,   // 待处理需求（demand_status=1）
    material: u64, // 待处理物料聚合数
    orders: u64,   // 待下达工单（Draft + Planned）
    batches: u64,  // 待开工批次（Pending）
}

/// 查各 tab 的待处理总数（COUNT 轻量；card 每次刷新都查，保证 badge 实时）。
async fn load_tab_counts(
    state: &crate::state::AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
) -> TabCounts {
    use abt_core::mes::enums::BatchStatus;
    let demand_svc = state.mes_demand_service();
    let detail = demand_svc
        .list_pending_demands(ctx, db, DemandPoolQuery { status: Some(1), ..Default::default() }, PageParams::new(1, 1))
        .await.map(|r| r.total).unwrap_or(0);
    let material = demand_svc
        .list_material_aggregated(ctx, db, MaterialAggQuery::default(), PageParams::new(1, 1))
        .await.map(|r| r.total).unwrap_or(0);
    let orders = state
        .mes_work_center_service()
        .summary(ctx, db).await.map(|s| s.pending_release).unwrap_or(0);
    let batches = state
        .production_batch_service()
        .list_batches(ctx, db, BatchListFilter { status: Some(BatchStatus::Pending), ..Default::default() }, 1, 1)
        .await.map(|r| r.total).unwrap_or(0);
    TabCounts { detail, material, orders, batches }
}

/// tab 标题后的待处理计数 badge（>0 才显示）。
fn tab_badge(n: u64) -> Markup {
    if n > 0 {
        html! {
            span class="ml-1 inline-flex items-center justify-center min-w-[20px] h-5 px-1.5 rounded-full bg-accent text-accent-on text-[11px] font-bold font-mono tabular-nums leading-none" {
                (n)
            }
        }
    } else {
        html! {}
    }
}

/// 平铺 tab 栏 + 筛选表单（统一 hx-get WcDemandPath + hx-select #wc-demand-card）。
/// - 第一行：底部下划线式平铺 tab（物料汇总[可合并]/订单行明细/工单/批次）
/// - 第二行：筛选表单（mat/det：搜索+日期+排序；schedule：搜索+工作中心+状态+时间）
fn demand_filter_bar(
    view: &str,
    p: &DemandCardParams,
    counts: &TabCounts,
) -> Markup {
    let kw = p.keyword.as_deref().unwrap_or("");
    let df = p.date_filter.as_deref().unwrap_or("");
    let ss = p.sched_status.as_deref().unwrap_or("");
    let wos = p.wo_status.as_deref().unwrap_or("");
    let bs = p.batch_status.as_deref().unwrap_or("");
    let wono = p.wo_no.as_deref().unwrap_or("");
    let avail = p.availability.as_deref().unwrap_or("");
    let sort = p.sort.as_deref().unwrap_or("");
    let placeholder = if view == "schedule" || view == "orders" {
        "搜索工单号/产品/来源订单"
    } else if view == "batches" {
        "搜索流转卡/批次号"
    } else {
        "搜索物料/订单"
    };
    html! {
        @if let Some(oid) = p.order_id {
            div class="flex items-center justify-between px-5 py-2 border-b border-border-soft bg-accent-bg" {
                span class="text-xs text-accent font-medium flex items-center gap-1.5" {
                    (icon::clipboard_document_icon("w-3.5 h-3.5"))
                    "销售订单 #" (oid) " 的自制需求 · 全部状态"
                }
                button type="button" class="text-xs text-muted hover:text-fg underline cursor-pointer"
                    hx-get=(WcDemandPath::PATH)
                    hx-target="#wc-demand-card" hx-select="#wc-demand-card" hx-swap="outerHTML"
                { "清除筛选" }
            }
        }
        // 第一行：平铺 tab 栏（底部下划线：订单行明细/物料汇总[可合并]/工单/批次）
        div class="flex items-center gap-1 flex-wrap px-5 pt-3 border-b border-border-soft" {
            button class=(toggle_cls(view == "detail")) type="button"
                hx-get=(WcDemandPath::PATH)
                hx-vals="{\"view\":\"detail\"}"
                hx-target="#wc-demand-card" hx-select="#wc-demand-card" hx-swap="outerHTML"
                hx-include="#wc-demand-filter-form"
                { (icon::rows_icon("w-4 h-4")) "订单行明细" (tab_badge(counts.detail)) }
            button class=(toggle_cls(view == "material")) type="button"
                hx-get=(WcDemandPath::PATH)
                hx-vals="{\"view\":\"material\"}"
                hx-target="#wc-demand-card" hx-select="#wc-demand-card" hx-swap="outerHTML"
                hx-include="#wc-demand-filter-form"
                { (icon::grid_4_icon("w-4 h-4")) "物料汇总"
                  span class="text-[10px] text-muted font-medium ml-0.5" { "可合并" }
                  (tab_badge(counts.material)) }
            button class=(toggle_cls(view == "orders")) type="button"
                hx-get=(WcDemandPath::PATH) hx-vals="{\"view\":\"orders\"}"
                hx-target="#wc-demand-card" hx-select="#wc-demand-card" hx-swap="outerHTML"
                hx-include="#wc-demand-filter-form"
                { (icon::package_icon("w-4 h-4")) "工单" (tab_badge(counts.orders)) }
            button class=(toggle_cls(view == "batches")) type="button"
                hx-get=(WcDemandPath::PATH) hx-vals="{\"view\":\"batches\"}"
                hx-target="#wc-demand-card" hx-select="#wc-demand-card" hx-swap="outerHTML"
                hx-include="#wc-demand-filter-form"
                { (icon::box_icon("w-4 h-4")) "批次" (tab_badge(counts.batches)) }
        }
        // 第二行：筛选表单（change / keyup 触发刷新）
        form class="flex items-center gap-2 flex-wrap px-5 py-3 border-b border-border-soft"
            hx-get=(WcDemandPath::PATH)
            hx-trigger="change, keyup changed delay:300ms from:.wc-search-input"
            hx-target="#wc-demand-card" hx-select="#wc-demand-card" hx-swap="outerHTML"
            {
            input type="hidden" name="view" value=(view);
            @if let Some(oid) = p.order_id {
                input type="hidden" name="order_id" value=(oid);
            }
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
                // 批次筛选：工单号
                div class="relative" {
                    (icon::search_icon("w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"));
                    input class="wc-search-input w-[250px] pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                        type="text" name="wo_no" placeholder="搜索工单号"
                        value=(wono);
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
            // 重置：清所有筛选回默认（只传 view，不带 keyword/status/date_filter 等）
            button type="button" class="ml-auto inline-flex items-center gap-1 px-2.5 py-1.5 text-xs text-muted hover:text-fg hover:bg-surface rounded-sm border border-border cursor-pointer transition-all"
                hx-get=(WcDemandPath::PATH)
                hx-vals=(serde_json::json!({ "view": view }).to_string())
                hx-target="#wc-demand-card" hx-select="#wc-demand-card" hx-swap="outerHTML"
                _="on click halt the event" {
                (icon::refresh_icon("w-3.5 h-3.5"))
                "重置"
            }
        }
        // 隐藏表单：tab 切换时携带所有筛选参数
        form id="wc-demand-filter-form" class="hidden" {
            input type="hidden" name="keyword" value=(kw);
            input type="hidden" name="date_filter" value=(df);
            input type="hidden" name="sched_status" value=(ss);
            input type="hidden" name="wo_status" value=(wos);
            input type="hidden" name="batch_status" value=(bs);
            input type="hidden" name="wo_no" value=(wono);
            input type="hidden" name="availability" value=(avail);
            input type="hidden" name="sort" value=(sort);
            @if let Some(oid) = p.order_id {
                input type="hidden" name="order_id" value=(oid);
            }
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
        div class="p-5" {
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
        div class="p-5 batch-scope" {
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
        div class="batch-bar hidden show:flex items-center gap-4 fixed bottom-4 left-1/2 -translate-x-1/2 z-50 px-5 py-3 rounded-md bg-fg text-white text-sm shadow-lg"
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
                div class="font-medium text-fg" {
                    a class="text-fg hover:text-accent hover:underline cursor-pointer"
                        href=(crate::routes::product::ProductDetailPath { id: item.product_id }.to_string())
                        target="_blank" { (item.product_name) }
                }
                div class="text-xs text-muted font-mono" { (item.product_code) }
            }
            td class="py-2.5 px-3" {
                a class="text-accent font-medium cursor-pointer no-underline"
                    hx-get=(WcOrderDetailModalPath { order_id: item.order_id }.to_string())
                    hx-target="#wc-order-detail-slot" hx-swap="innerHTML"
                    _="on click halt the event"
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
        Closed => ("已完工", "purple"),
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
    use WorkOrderStatus::*;
    let pn = product_names
        .get(&w.product_id)
        .map(|s| s.as_str())
        .unwrap_or("—");
    let (slabel, stoken) = wo_status_meta(&w.status);
    html! {
        tr class="border-b border-border-soft hover:bg-accent-bg" {
            // 工单号（点击 → 工单详情 drawer）
            td class="py-2.5 px-5" {
                button class="font-mono tabular-nums text-accent font-medium cursor-pointer hover:underline bg-transparent border-none p-0"
                    hx-get=(WcOrderDrawerPath { order_id: w.id }.to_string())
                    hx-target="#order-drawer-body" hx-swap="innerHTML"
                    _="on click halt the event" { (w.doc_number.as_str()) }
            }
            td class="py-2.5 px-3" {
                div class="font-medium text-fg" {
                    a class="text-fg hover:text-accent hover:underline cursor-pointer"
                        href=(crate::routes::product::ProductDetailPath { id: w.product_id }.to_string())
                        target="_blank" { (pn) }
                }
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
                // 下达（Draft/Planned）— 打开下达 drawer
                @if matches!(w.status, Draft | Planned) {
                    button class="inline-flex items-center gap-1 px-2.5 py-1 rounded-sm border border-accent/50 text-xs font-medium text-accent cursor-pointer hover:bg-accent hover:text-accent-on hover:border-accent transition-all ml-1.5"
                        hx-get=(WcReleaseDrawerPath { order_id: w.id }.to_string())
                        hx-target="#release-drawer-body" hx-swap="innerHTML"
                        _="on click halt the event" {
                        (icon::rocket_icon("w-3.5 h-3.5"))
                        "下达"
                    }
                }
                // 批次（已下达）— 切到批次 tab 并按当前工单号筛选（钻取入口）
                @if matches!(w.status, Released | InProduction | Closed) {
                    button class="inline-flex items-center gap-1 px-2.5 py-1 rounded-sm border border-border text-xs font-medium text-fg cursor-pointer hover:bg-accent-bg hover:border-accent hover:text-accent transition-all ml-1.5"
                        hx-get=(WcDemandPath::PATH)
                        hx-vals=(serde_json::json!({ "view": "batches", "wo_no": w.doc_number }).to_string())
                        hx-target="#wc-demand-card" hx-select="#wc-demand-card" hx-swap="outerHTML"
                        _="on click halt the event" {
                        (icon::box_icon("w-3.5 h-3.5"))
                        "批次"
                    }
                }
                // 取消（非终态）— 作废：反向取消领料单 + 释放预留 + 需求回池；已完工不可取消
                @if matches!(w.status, Draft | Planned | Released | InProduction) {
                    button class="inline-flex items-center gap-1 px-2.5 py-1 rounded-sm border border-danger/40 text-xs font-medium text-danger cursor-pointer hover:bg-danger-bg hover:border-danger transition-all ml-1.5"
                        hx-post=(WcCancelPath { order_id: w.id }.to_string())
                        hx-confirm="确认取消此工单？\n• 作废工单，反向取消已领物料单并释放库存预留\n• 关联需求回池，可重新规划\n• 已有完工入库单的工单不可取消"
                        hx-disabled-elt="this" {
                        (icon::x_icon("w-3.5 h-3.5"))
                        "取消"
                    }
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
            td class="py-2.5 px-3" {
                @if let Some(wn) = b.wo_doc_number.as_deref() {
                    button class="text-accent text-xs font-mono cursor-pointer hover:underline bg-transparent border-none p-0"
                        hx-get=(WcDemandPath::PATH)
                        hx-vals=(serde_json::json!({ "view": "orders", "keyword": wn }).to_string())
                        hx-target="#wc-demand-card" hx-select="#wc-demand-card" hx-swap="outerHTML"
                        _="on click halt the event" { (wn) }
                } @else {
                    span class="text-muted text-xs font-mono" { "—" }
                }
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

/// 工单进度条（动态宽度用 style，同既有列表页做法）。
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

/// 销售订单详情 drawer（drawer 内订单号点击查看）：订单头 + 行项目，不跳转。
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
    // 销售员名（Odoo Responsible）— 用于订单头展示
    let sales_rep_name = state
        .user_service()
        .get_user(&service_ctx, &mut conn, order.sales_rep_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| format!("#{}", order.sales_rep_id));
    Ok(Html(
        drawer_shell(
            "wc-order-detail-drawer",
            "w-[920px]",
            render_order_detail_panel(&order, &items, &prod_map, &customer_name, &sales_rep_name),
        )
        .into_string(),
    ))
}

/// 订单详情 drawer 面板：统计带（总额/已发货额/行数）+ 客户信息（客户/销售员/收货地址）
/// + 订单信息（日期/状态/付款条件）+ 行项目表 + 备注。区块对齐 Odoo sale.order / WMS drawer 共识。
fn render_order_detail_panel(
    order: &SalesOrder,
    items: &[SalesOrderItem],
    prod_map: &HashMap<i64, String>,
    customer_name: &str,
    sales_rep_name: &str,
) -> Markup {
    let (status_label, status_cls) = order_status_meta(&order.status);
    let shipped_total: rust_decimal::Decimal =
        items.iter().map(|i| i.shipped_qty * i.unit_price).sum();
    let delivery_addr = if order.delivery_address.is_empty() {
        "—".to_string()
    } else {
        order.delivery_address.clone()
    };
    html! {
        // 头部
        div class="flex items-center justify-between px-7 py-5 border-b border-border-soft shrink-0" {
            div {
                h2 class="text-lg font-bold text-fg m-0" { "销售订单详情" }
                div class="text-sm text-muted mt-1 flex items-center gap-2" {
                    span class="font-mono text-accent font-semibold" { (order.doc_number) }
                    span class="text-muted/40" { "·" }
                    span { (customer_name) }
                }
            }
            button type="button"
                class="text-2xl text-muted hover:text-fg cursor-pointer bg-transparent border-none p-1 leading-none"
                _="on click remove .open from #wc-order-detail-drawer" { "×" }
        }
            div class="overflow-y-auto flex-1 min-h-0 p-7" {
                // ① 统计带
                div class="flex rounded-lg bg-surface border border-border-soft mb-5 overflow-hidden" {
                    div class="flex-1 px-4 py-3 flex flex-col items-center gap-0.5 border-r border-border-soft" {
                        span class="text-lg font-bold text-fg tabular-nums leading-none" { "¥ " (fmt_qty(order.total_amount)) }
                        span class="text-[11px] text-muted font-medium" { "订单总额" }
                    }
                    div class="flex-1 px-4 py-3 flex flex-col items-center gap-0.5 border-r border-border-soft" {
                        span class="text-lg font-bold text-success tabular-nums leading-none" { "¥ " (fmt_qty(shipped_total)) }
                        span class="text-[11px] text-muted font-medium" { "已发货额" }
                    }
                    div class="flex-1 px-4 py-3 flex flex-col items-center gap-0.5" {
                        span class="text-lg font-bold text-fg tabular-nums leading-none" { (items.len()) }
                        span class="text-[11px] text-muted font-medium" { "行项目" }
                    }
                }
                // ② 客户信息
                div class="mb-5" {
                    div class="flex items-center gap-2 mb-3" {
                        span class="w-1 h-3.5 rounded-full bg-accent shrink-0" {}
                        span class="text-xs font-semibold text-fg" { "客户信息" }
                    }
                    div class="grid grid-cols-2 gap-x-6 gap-y-3 pl-3" {
                        div {
                            div class="text-[11px] text-muted mb-0.5" { "客户" }
                            div class="text-sm text-fg font-medium" { (customer_name) }
                        }
                        div {
                            div class="text-[11px] text-muted mb-0.5" { "销售员" }
                            div class="text-sm text-fg-2" { (sales_rep_name) }
                        }
                        div class="col-span-2" {
                            div class="text-[11px] text-muted mb-0.5" { "收货地址" }
                            div class="text-sm text-fg-2 leading-relaxed" { (delivery_addr) }
                        }
                    }
                }
                // ③ 订单信息
                div class="mb-5" {
                    div class="flex items-center gap-2 mb-3" {
                        span class="w-1 h-3.5 rounded-full bg-purple shrink-0" {}
                        span class="text-xs font-semibold text-fg" { "订单信息" }
                    }
                    div class="grid grid-cols-3 gap-x-6 gap-y-3 pl-3" {
                        div {
                            div class="text-[11px] text-muted mb-0.5" { "订单日期" }
                            div class="text-sm font-mono text-fg-2" { (order.order_date.format("%Y-%m-%d")) }
                        }
                        div {
                            div class="text-[11px] text-muted mb-0.5" { "状态" }
                            span class=(format!("inline-flex items-center text-[11px] px-2 py-0.5 rounded-full font-semibold {status_cls}")) { (status_label) }
                        }
                        div {
                            div class="text-[11px] text-muted mb-0.5" { "付款条件" }
                            div class="text-sm text-fg-2 truncate" { (order.payment_terms) }
                        }
                    }
                }
                // ④ 行项目
                div {
                    div class="flex items-center gap-2 mb-3" {
                        span class="w-1 h-3.5 rounded-full bg-success shrink-0" {}
                        span class="text-xs font-semibold text-fg" { "行项目 · " (items.len()) " 条" }
                    }
                    div class="rounded-lg border border-border-soft overflow-hidden" {
                        table class="w-full text-xs" {
                            thead {
                                tr class="bg-surface/60 text-muted" {
                                    th class="text-center font-semibold py-2.5 px-2 w-10" { "#" }
                                    th class="text-left font-semibold py-2.5 px-3" { "产品" }
                                    th class="text-right font-semibold py-2.5 px-3" { "数量" }
                                    th class="text-right font-semibold py-2.5 px-2" { "单价" }
                                    th class="text-right font-semibold py-2.5 px-2" { "金额" }
                                    th class="text-right font-semibold py-2.5 px-2" { "已发货" }
                                    th class="text-left font-semibold py-2.5 px-3" { "交期" }
                                }
                            }
                            tbody {
                                @for item in items {
                                    tr class="border-t border-border-soft hover:bg-surface/50 transition-colors" {
                                        td class="py-2.5 px-2 text-center text-muted font-mono" { (item.line_no) }
                                        td class="py-2.5 px-3" {
                                            div class="text-fg font-medium" {
                                                (prod_map.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—"))
                                            }
                                            @if !item.description.is_empty() {
                                                div class="text-[11px] text-muted mt-0.5" { (item.description) }
                                            }
                                        }
                                        td class="py-2.5 px-3 text-right font-mono whitespace-nowrap tabular-nums" {
                                            (fmt_qty(item.quantity)) " " (item.unit)
                                        }
                                        td class="py-2.5 px-2 text-right font-mono text-fg-2 tabular-nums" { (fmt_qty(item.unit_price)) }
                                        td class="py-2.5 px-2 text-right font-mono font-medium tabular-nums" { (fmt_qty(item.amount)) }
                                        td class="py-2.5 px-2 text-right font-mono text-fg-2 tabular-nums" { (fmt_qty(item.shipped_qty)) }
                                        td class="py-2.5 px-3 font-mono text-fg-2" {
                                            (item.delivery_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into()))
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                // ⑤ 备注
                @if !order.remark.is_empty() {
                    div class="mt-4 p-3.5 rounded-lg bg-surface border border-border-soft" {
                        div class="flex items-start gap-2" {
                            span class="text-[11px] text-muted font-medium shrink-0 mt-px" { "备注" }
                            span class="text-xs text-fg-2 leading-relaxed" { (order.remark) }
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
        Cancelled => ("已取消", "bg-danger-bg text-danger"),
        ShippingRequested => ("待发货", "bg-warn-bg text-warn"),
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
    let demands = load_demands_for_create_drawer(
        &state,
        &service_ctx,
        &mut conn,
        path.product_id,
        q.demand_ids.as_deref(),
    )
    .await?;
    let product_name = demands
        .first()
        .map(|d| d.product_name.as_str())
        .unwrap_or("—");
    let product_code = demands
        .first()
        .map(|d| d.product_code.as_str())
        .unwrap_or("—");
    Ok(Html(
        render_create_plan_drawer_body(path.product_id, product_name, product_code, &demands, None, None, None)
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
    errors: Option<&std::collections::HashMap<&str, String>>,
    submitted_start: Option<&str>,
    submitted_end: Option<&str>,
) -> Markup {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let default_end = chrono::Local::now()
        .checked_add_days(chrono::Days::new(10))
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    // 回填用户提交值（失败重渲染不丢输入）；初次加载用 today / default_end
    let start_val = submitted_start.unwrap_or(&today);
    let end_val = submitted_end.unwrap_or(&default_end);
    let total_qty: Decimal = demands.iter().map(|d| d.quantity).sum();
    let demand_ids_str = demands
        .iter()
        .map(|d| d.id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    html! {
        form hx-post=(WcCreatePlanPath { product_id }.to_string())
            hx-target="this"
            hx-swap="outerHTML"
            // 复用 wc_routing_edit 范式：成功（空 body）才关 overlay；失败重渲染 form（非空）保持打开
            _="on 'htmx:afterRequest'[detail.xhr.responseText.length == 0 and detail.elt is me] remove .open from #create-plan-overlay" {
            // 物料信息
            div class="mb-6 pb-5 border-b border-border-soft" {
                div class="text-xs text-muted mb-1" { "物料" }
                div class="font-semibold text-fg" { (product_name) }
                div class="text-xs text-muted font-mono mt-1" { (product_code) }
            }
            // 顶部兜底 alert（非字段特定的通用错误，如未选需求）
            @if let Some(m) = errors.and_then(|e| e.get("__all__")) {
                div class="mb-4" { (alert::alert_error(m)) }
            }
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
                    input type="date" name="default_scheduled_start" value=(start_val)
                        class=(field_cls("default_scheduled_start", errors));
                    @if let Some(m) = errors.and_then(|e| e.get("default_scheduled_start")) {
                        p class="text-danger text-xs mt-1" { (m) }
                    }
                }
                div {
                    label class="block text-xs text-fg-2 mb-1" { "完工日期" }
                    input type="date" name="default_scheduled_end" value=(end_val)
                        class=(field_cls("default_scheduled_end", errors));
                    @if let Some(m) = errors.and_then(|e| e.get("default_scheduled_end")) {
                        p class="text-danger text-xs mt-1" { (m) }
                    }
                }
            }
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

/// 字段 input class：有错用 danger 边框，否则常规边框（创建工单 drawer 校验失败标红）。
fn field_cls(
    field: &str,
    errors: Option<&std::collections::HashMap<&str, String>>,
) -> &'static str {
    match errors.and_then(|e| e.get(field)) {
        Some(_) => "w-full px-2 py-1.5 border border-danger rounded-sm text-sm bg-white text-fg outline-none focus:border-danger",
        None => "w-full px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent",
    }
}

/// 按 product_id 取 pending 需求，再按 demand_ids 精确过滤。
/// drawer 初始加载与 create_plan 失败重渲染共用。
async fn load_demands_for_create_drawer(
    state: &crate::state::AppState,
    ctx: &abt_core::shared::types::context::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    product_id: i64,
    demand_ids_csv: Option<&str>,
) -> Result<Vec<DemandSummary>> {
    let mut demands = state
        .mes_demand_service()
        .list_pending_demands(
            ctx,
            db,
            DemandPoolQuery {
                status: Some(1),
                product_id: Some(product_id),
                ..Default::default()
            },
            PageParams::new(1, 100),
        )
        .await?
        .items;
    if let Some(ids_str) = demand_ids_csv.map(str::trim).filter(|s| !s.is_empty()) {
        let ids: std::collections::HashSet<i64> = ids_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        demands.retain(|d| ids.contains(&d.id));
    }
    Ok(demands)
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
    path: WcCreatePlanPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<WcCreatePlanForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    // 收集字段级校验错误（失败时不走 Err/全局 toast，而是 OOB 重渲染 form 标红）
    let mut errors: std::collections::HashMap<&str, String> = std::collections::HashMap::new();

    let demand_ids: Vec<i64> = form
        .demand_ids
        .split(',')
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect();
    if demand_ids.is_empty() {
        errors.insert("__all__", "请至少选择一条生产需求".into());
    }

    let default_scheduled_start = match form.default_scheduled_start.as_deref().filter(|s| !s.is_empty()) {
        Some(s) => match chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
            Ok(d) => Some(d),
            Err(e) => {
                errors.insert("default_scheduled_start", format!("无效开工日期: {e}"));
                None
            }
        },
        None => None,
    };
    let default_scheduled_end = match form.default_scheduled_end.as_deref().filter(|s| !s.is_empty()) {
        Some(s) => match chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
            Ok(d) => Some(d),
            Err(e) => {
                errors.insert("default_scheduled_end", format!("无效完工日期: {e}"));
                None
            }
        },
        None => None,
    };

    // 开工日期必须早于完工日期：仅当两日期都解析成功时 zip 才为 Some（隐含排除已塞错误的非法日期）
    if let Some((s, e)) = default_scheduled_start.zip(default_scheduled_end)
        && s >= e
    {
        errors.insert("default_scheduled_end", "完工日期必须晚于开工日期".into());
    }

    // 校验失败 → OOB 重渲染 form（带 errors），drawer 保持打开
    if !errors.is_empty() {
        let demands = load_demands_for_create_drawer(
            &state,
            &service_ctx,
            &mut conn,
            path.product_id,
            Some(form.demand_ids.as_str()),
        )
        .await?;
        let product_name = demands.first().map(|d| d.product_name.as_str()).unwrap_or("—");
        let product_code = demands.first().map(|d| d.product_code.as_str()).unwrap_or("—");
        let body = render_create_plan_drawer_body(
            path.product_id,
            product_name,
            product_code,
            &demands,
            Some(&errors),
            form.default_scheduled_start.as_deref(),
            form.default_scheduled_end.as_deref(),
        );
        return Ok(([("HX-Trigger", String::new())], Html(body.into_string())));
    }

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
    let result = state
        .mes_demand_service()
        .create_work_orders_from_demands(&service_ctx, &mut tx, create_req)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 取首个新工单号（create drawer 单产品→1 工单）作 keyword，
    // planCreated 桥接据此跳「工单」tab 并定位刚创建的工单
    let wo_no = match result.wo_ids.first() {
        Some(&id) => state
            .work_order_service()
            .find_by_id(&service_ctx, &mut conn, id)
            .await
            .ok()
            .map(|w| w.doc_number),
        None => None,
    };
    let trigger = match &wo_no {
        Some(no) => serde_json::json!({ "planCreated": { "wo_no": no } }).to_string(),
        None => "planCreated".to_string(),
    };
    // 成功：空 body + HX-Trigger（form afterRequest 判定空 → 关 overlay；planCreated → 跳工单 tab + 定位新工单）
    Ok(([("HX-Trigger", trigger)], Html(String::new())))
}

/// 下达 drawer 校验错误（工序行级）：工序整体为空 / 某道工序缺产出品 / 缺计件单价。
#[derive(Default)]
struct ReleaseErrors {
    empty_routings: bool,
    /// routing_id → 错误消息（含工序号/名）
    product_missing: HashMap<i64, String>,
    /// routing_id → 错误消息
    price_missing: HashMap<i64, String>,
}

impl ReleaseErrors {
    fn is_empty(&self) -> bool {
        !self.empty_routings && self.product_missing.is_empty() && self.price_missing.is_empty()
    }
    /// 顶部汇总 alert 文案（存在任一错误时）。
    fn summary(&self) -> Option<String> {
        if self.empty_routings {
            return Some("该产品 BOM 尚未配置工序，请到 BOM 编辑页配置（或从工艺路线拷贝）后直接重新下达，无需重建工单".into());
        }
        let msgs: Vec<&String> = self
            .product_missing
            .values()
            .chain(self.price_missing.values())
            .collect();
        if msgs.is_empty() {
            return None;
        }
        let mut combined = String::from("以下工序尚未定价，请在下方表格直接填写单价后点下达：");
        for m in msgs {
            combined.push_str("\n• ");
            combined.push_str(m);
        }
        Some(combined)
    }
}

/// 下达 drawer 全量数据（get_release_drawer 初始加载与 release_order 失败重渲染共用）。
struct ReleaseDrawerData {
    order: WorkOrder,
    product_name: String,
    routings: Vec<WorkOrderRouting>,
    wc_map: HashMap<i64, String>,
    prod_map: HashMap<i64, String>,
    level: MaterialAvailabilityLevel,
    headline: Option<String>,
    consumption_label: String,
    /// 产品已发布 BOM（id + 名称 + 编码），用于展示
    bom_id: Option<i64>,
    bom_name: Option<String>,
    bom_code: Option<String>,
}

/// 加载下达 drawer 全量数据（order / routings / 工作中心 / 产出品 / 物料齐套 / 倒冲模式）。
async fn load_release_drawer_data(
    state: &crate::state::AppState,
    ctx: &abt_core::shared::types::context::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    order_id: i64,
) -> Result<ReleaseDrawerData> {
    let wo_svc = state.work_order_service();
    let batch_svc = state.production_batch_service();
    let product_svc = state.product_service();
    let order = wo_svc.find_by_id(ctx, db, order_id).await?;
    let product_name = wo_svc
        .get_product_name(db, order.product_id)
        .await?
        .unwrap_or_else(|| format!("#{}", order.product_id));
    let routings = batch_svc
        .list_routings(ctx, db, order_id)
        .await
        .unwrap_or_default();
    let wc_map: HashMap<i64, String> = new_work_center_service(state.pool.clone())
        .list_active(ctx, db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|wc| (wc.id, wc.name))
        .collect();
    let mut pids: Vec<i64> = routings.iter().filter_map(|r| r.product_id).collect();
    if !pids.contains(&order.product_id) {
        pids.push(order.product_id);
    }
    let products = product_svc
        .get_by_ids(ctx, db, pids)
        .await
        .unwrap_or_default();
    let prod_map: HashMap<i64, String> = products
        .iter()
        .map(|p| (p.product_id, p.pdt_name.clone()))
        .collect();
    let avail = wo_svc
        .compute_availability_batch(ctx, db, std::slice::from_ref(&order))
        .await
        .unwrap_or_default();
    let (level, headline) = avail
        .get(&order.id)
        .cloned()
        .unwrap_or((MaterialAvailabilityLevel::Available, None));
    let consumption_label = products
        .iter()
        .find(|p| p.product_id == order.product_id)
        .map(|p| match p.meta.material_consumption_mode {
            abt_core::master_data::product::model::MaterialConsumptionMode::Backflush => "倒冲",
            abt_core::master_data::product::model::MaterialConsumptionMode::Picking => "领料",
        })
        .unwrap_or("倒冲");
    // BOM：drawer 展示 BOM 名称/编码
    let product_code = products
        .iter()
        .find(|p| p.product_id == order.product_id)
        .map(|p| p.product_code.clone());
    let bom_query = new_bom_query_service(state.pool.clone());
    let bom_id = match &product_code {
        Some(pc) => bom_query.find_published_bom_by_product_code(ctx, db, pc).await.unwrap_or(None),
        None => None,
    };
    let bom = match bom_id {
        Some(id) => bom_query.get(ctx, db, id).await.ok(),
        None => None,
    };
    let bom_name = bom.as_ref().map(|b| format!("{} v{}", b.bom_name, b.version));
    let bom_code = bom.as_ref().and_then(|b| b.product_code.clone());
    Ok(ReleaseDrawerData {
        order,
        product_name,
        routings,
        wc_map,
        prod_map,
        level,
        headline,
        consumption_label: consumption_label.to_string(),
        bom_id,
        bom_name,
        bom_code,
    })
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
    let data = load_release_drawer_data(&state, &service_ctx, &mut conn, path.order_id).await?;
    Ok(Html(render_release_drawer_body(&data, None).into_string()))
}

/// 工单详情 drawer body（只读）：复用 load_release_drawer_data 全量数据 + 来源销售订单。
#[require_permission("WORK_ORDER", "read")]
pub async fn get_order_drawer(
    path: WcOrderDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let data = load_release_drawer_data(&state, &service_ctx, &mut conn, path.order_id).await?;
    // 来源销售订单：sales_order_id 有值时取单号+客户名（工单从需求转化，需求来自 SO）
    let source_so: Option<(i64, String, String)> = match data.order.sales_order_id {
        Some(so_id) => match state
            .sales_order_service()
            .find_by_id(&service_ctx, &mut conn, so_id)
            .await
        {
            Ok(o) => {
                let customer_name = state
                    .customer_service()
                    .get_by_ids(&service_ctx, &mut conn, &[o.customer_id])
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .next()
                    .map(|c| c.name)
                    .unwrap_or_else(|| format!("#{}", o.customer_id));
                Some((so_id, o.doc_number, customer_name))
            }
            Err(_) => None,
        },
        None => None,
    };
    Ok(Html(render_order_drawer_body(&data, source_so).into_string()))
}

/// 工单详情 drawer 渲染（只读）：头部摘要 / 来源+排程 / 工艺路线 / 物料 / 备注。
/// 区块参考 ERPNext Work Order / Odoo MO 详情共识。
fn render_order_drawer_body(
    data: &ReleaseDrawerData,
    source_so: Option<(i64, String, String)>,
) -> Markup {
    let order = &data.order;
    let product_name = data.product_name.as_str();
    let routings = data.routings.as_slice();
    let wc_map = &data.wc_map;
    let prod_map = &data.prod_map;
    let level = data.level;
    let headline = data.headline.as_deref();
    let consumption_label = data.consumption_label.as_str();
    let bom_name = data.bom_name.as_deref();
    let bom_code = data.bom_code.as_deref();
    let bom_id = data.bom_id;
    let (slabel, stoken) = wo_status_meta(&order.status);
    html! {
        // ① 头部摘要
        div class="mb-5 pb-4 border-b border-border-soft" {
            div class="flex items-center gap-2 mb-1" {
                span class="text-xs text-muted" { "工单" }
                span class="font-mono font-semibold text-fg" { (order.doc_number.as_str()) }
                span class=(format!("inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium whitespace-nowrap bg-{stoken}-bg text-{stoken}")) {
                    span class=(format!("inline-block w-1.5 h-1.5 rounded-full bg-{stoken}")) {}
                    (slabel)
                }
            }
            div class="text-sm text-fg mb-3" { (product_name) }
            div class="grid grid-cols-3 gap-4 mb-3" {
                div {
                    div class="text-xs text-muted mb-1" { "计划量" }
                    div class="text-sm font-mono font-semibold text-fg" { (fmt_qty(order.planned_qty)) }
                }
                div {
                    div class="text-xs text-muted mb-1" { "已完成" }
                    div class="text-sm font-mono font-semibold text-success" { (fmt_qty(order.completed_qty)) }
                }
                div {
                    div class="text-xs text-muted mb-1" { "报废" }
                    div class="text-sm font-mono font-semibold text-danger" { (fmt_qty(order.scrap_qty)) }
                }
            }
            (wo_progress(order))
        }

        // ② 来源 + 排程
        div class="mb-5 grid grid-cols-2 gap-x-6 gap-y-4 p-4 bg-surface-raised rounded-md text-sm" {
            @if let Some((so_id, so_doc, customer)) = &source_so {
                div {
                    div class="text-xs text-muted mb-1" { "来源销售订单" }
                    div class="flex items-center gap-1.5" {
                        a class="text-accent font-medium cursor-pointer hover:underline"
                            hx-get=(WcOrderDetailModalPath { order_id: *so_id }.to_string())
                            hx-target="#wc-order-detail-slot" hx-swap="innerHTML"
                            _="on click halt the event" { (so_doc.as_str()) }
                        span class="text-muted text-xs" { (customer.as_str()) }
                    }
                }
            }
            div {
                div class="text-xs text-muted mb-1" { "BOM" }
                @match bom_id {
                    Some(_) => {
                        span class="text-fg-2" { (bom_name.unwrap_or("已发布")) }
                    }
                    None => {
                        span class="text-danger" { "未发布 BOM" }
                    }
                }
            }
            div {
                div class="text-xs text-muted mb-1" { "计划开工" }
                div class="text-fg-2 font-mono" { (order.scheduled_start.format("%Y-%m-%d")) }
            }
            div {
                div class="text-xs text-muted mb-1" { "计划完工" }
                div class="text-fg-2 font-mono" { (order.scheduled_end.format("%Y-%m-%d")) }
            }
        }

        // ③ 工艺路线（复用 release drawer 的只读工序表）
        div class="mb-5" {
            div class="text-sm font-semibold text-fg mb-2" { "工艺路线" }
            (render_release_routings(routings, wc_map, prod_map, None))
        }

        // ④ 物料（齐套徽章 + 倒冲/领料模式 + BOM 编码）
        div class="mb-5" {
            div class="text-sm font-semibold text-fg mb-2" { "物料" }
            div class="flex items-center gap-2 text-xs text-fg-2 bg-surface-raised rounded-sm px-3 py-2" {
                (material_badge_mini(level, headline))
                span { (consumption_label) "模式" }
                @if let Some(code) = bom_code {
                    span class="text-muted" { "· BOM 编码 " (code) }
                }
            }
        }

        // ⑤ 备注
        @if !order.remark.is_empty() {
            div class="p-3 bg-surface-raised rounded-sm text-xs text-fg-2" {
                span class="text-muted" { "备注：" } (order.remark.as_str())
            }
        }
    }
}

// ── 下达 drawer 渲染 ──

fn render_release_drawer_body(data: &ReleaseDrawerData, errors: Option<&ReleaseErrors>) -> Markup {
    let order = &data.order;
    let product_name = data.product_name.as_str();
    let routings = data.routings.as_slice();
    let wc_map = &data.wc_map;
    let prod_map = &data.prod_map;
    let level = data.level;
    let headline = data.headline.as_deref();
    let consumption_label = data.consumption_label.as_str();
    let bom_id = data.bom_id;
    let bom_name = data.bom_name.as_deref();
    let bom_code = data.bom_code.as_deref();
    html! {
        // 工单信息
        div class="mb-5 pb-4 border-b border-border-soft" {
            div class="text-xs text-muted mb-0.5" { "工单" }
            div class="font-mono font-semibold text-fg" { (order.doc_number) }
            div class="text-sm text-fg-2 mt-1" {
                (product_name) " · " (fmt_qty(order.planned_qty)) " 件"
            }
            // BOM（仅展示名称，BOM 不支持在线编辑）
            div class="text-sm mt-2 flex items-center gap-1.5" {
                span class="text-muted" { "BOM：" }
                @match bom_id {
                    Some(_) => {
                        span class="text-fg-2" { (bom_name.unwrap_or("已发布")) }
                    }
                    None => {
                        span class="text-danger" { "未发布 BOM" }
                    }
                }
            }
            // BOM 编码（根产品编码，仅已发布 BOM 时显示）
            @if bom_id.is_some() {
                div class="text-sm mt-1 flex items-center gap-1.5" {
                    span class="text-muted" { "BOM 编码：" }
                    span class="text-fg-2 font-mono" { (bom_code.unwrap_or("—")) }
                }
            }
        }

        form hx-post=(WcReleasePath { order_id: order.id }.to_string())
            hx-target="this"
            hx-swap="outerHTML"
            hx-on:htmx:config-request="event.detail.parameters['splits_json'] = window.collectReleaseSplits(event.detail.elt)"
            // 复用 create_plan 范式：成功（空 body）才关 overlay；失败重渲染 form（非空）保持打开
            _="on 'htmx:afterRequest'[detail.xhr.responseText.length == 0 and detail.elt is me] remove .open from #release-overlay" {
            // 顶部兜底 alert（工序为空 / 某工序缺产出品/单价）
            @if let Some(m) = errors.and_then(|e| e.summary()) {
                div class="mb-4" { (alert::alert_error(&m)) }
            }

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

            // ② 工序（来自 BOM 工艺路线）：标题旁「从 BOM 更新」入口；id 为 reload 端点替换边界
            (render_release_routings_block(routings, wc_map, prod_map, errors))

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
        // ② 工序「从 BOM 更新」确认弹窗（置于 release form 之外，避免 <form> 嵌套）
        ({
            crate::components::confirm_dialog::confirm_dialog(
                "wc-reload-dialog",
                "从 BOM 更新工序",
                "将用 BOM 最新工艺路线<strong>覆盖当前工序快照</strong>。<br/>✅ 已设计件单价会自动保留（单价是主数据 bom_step_prices）。<br/>⚠ 若 BOM 调整了工序顺序，单价可能错位，<strong>更新后请逐行核对</strong>。",
                "确认更新",
                "wc-reload-form",
                html! {
                    form id="wc-reload-form" class="hidden"
                        hx-post=(WoReloadRoutingsPath { order_id: order.id }.to_string())
                        hx-target="#wc-release-routings"
                        hx-select="#wc-release-routings"
                        hx-swap="outerHTML" {}
                },
            )
        })
    }
}

/// ② 工序区（标题 + 「从 BOM 更新」按钮 + 工序表）。
/// drawer 首渲与 reload-routings 端点共用；id="wc-release-routings" 为 reload 端点 hx-target/hx-select 边界。
fn render_release_routings_block(
    routings: &[WorkOrderRouting],
    wc_map: &HashMap<i64, String>,
    prod_map: &HashMap<i64, String>,
    errors: Option<&ReleaseErrors>,
) -> Markup {
    html! {
        div id="wc-release-routings" class="mb-5" {
            div class="flex items-center justify-between mb-2" {
                div class="text-sm font-semibold text-fg" { "② 工序（来自 BOM 工艺路线）" }
                button type="button"
                    class="inline-flex items-center gap-1 text-xs px-2 py-1 rounded-sm border border-border text-fg-2 hover:bg-accent-bg hover:text-accent cursor-pointer transition-all"
                    title="用 BOM 最新工艺路线重新加载工序（已设单价自动保留）"
                    _="on click show #wc-reload-dialog" {
                    (icon::refresh_icon("w-3.5 h-3.5"))
                    "从 BOM 更新"
                }
            }
            (render_release_routings(routings, wc_map, prod_map, errors))
        }
    }
}

fn render_release_routings(
    routings: &[WorkOrderRouting],
    wc_map: &HashMap<i64, String>,
    prod_map: &HashMap<i64, String>,
    errors: Option<&ReleaseErrors>,
) -> Markup {
    if routings.is_empty() {
        // BOM 尚未配置内联工序：提示去 BOM 配置后重新创建工单
        return html! {
            div class="text-xs text-fg-2 p-3 bg-warn-bg rounded-sm flex items-start gap-2" {
                (icon::info_icon("w-4 h-4 shrink-0 mt-0.5 text-warn"))
                span { "该产品 BOM 尚未配置工序，请先在 BOM 中配置工序后再创建工单。" }
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
                }
            }
            tbody {
                @for r in routings {
                    (render_release_routing_row(r, wc_map, prod_map, errors))
                }
            }
        }
    }
}

/// 工序行（只读）：产出品/工作中心名称从 map 映射（无则 —）。
fn render_release_routing_row(
    r: &WorkOrderRouting,
    wc_map: &HashMap<i64, String>,
    prod_map: &HashMap<i64, String>,
    errors: Option<&ReleaseErrors>,
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
    // 兜底标红（routing 模板已校验产出品/单价，正常不触发）
    let prod_err = errors.and_then(|e| e.product_missing.get(&r.id)).is_some();
    let price_err = errors.and_then(|e| e.price_missing.get(&r.id)).is_some();
    html! {
        tr class="border-b border-border-soft last:border-b-0" {
            td class="py-1.5 px-2 text-muted font-mono" { (r.step_no) }
            td class="py-1.5 px-2 text-fg" { (r.process_name) }
            td class=(if prod_err { "py-1.5 px-2 text-danger bg-danger-bg font-medium" } else { "py-1.5 px-2 text-fg-2" }) { (prod_name) }
            td class="py-1.5 px-2 text-fg-2" { (wc_name) }
            td class="py-1.5 px-2 text-right font-mono" {
                input type="number" step="0.000001" name="unit_price"
                    value=(r.unit_price.map(|d| d.to_string()).unwrap_or_default())
                    placeholder="未定价"
                    class=(format!("w-20 px-1.5 py-0.5 text-right border rounded-sm bg-white text-xs font-mono {}", if price_err { "border-danger" } else { "border-border-soft" }))
                    hx-post=(WoStepPricePath { order_id: r.work_order_id, step_no: r.step_no }.to_string())
                    hx-trigger="blur"
                    hx-target="closest tr" hx-swap="outerHTML"
                    // 此 input 嵌在 release <form> 内，htmx 对 POST 会无条件序列化最近 <form>
                    // （含每行工序各一个 unit_price），导致 set_step_price 报 duplicate field 'unit_price'。
                    // 故 config-request 时只回填本输入框自己的值（order_id/step_no 已在 URL 路径里）。
                    hx-on:htmx:config-request="event.detail.parameters = {unit_price: event.detail.elt.value}"
                    title="此单价保存后对该产品后续所有工单生效（主数据·本产品通用）";
            }
            td class="py-1.5 px-2 text-center" {
                @if r.is_outsourced {
                    span class="text-accent" { "✓" }
                } @else {
                    span class="text-muted" { "—" }
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct StepPriceForm {
    pub unit_price: String,
}

/// release drawer 内联填计件单价（per-step）：blur 触发，行自替换。
/// 权限 BOM_STEP_PRICE（R-13：定价影响全员工资，独立闸门）。
#[require_permission("BOM_STEP_PRICE", "update")]
pub async fn set_step_price(
    path: WoStepPricePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<StepPriceForm>,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let unit_price: rust_decimal::Decimal = form.unit_price.trim().parse()
        .map_err(|_| abt_core::shared::types::DomainError::business_rule("单价格式错误"))?;
    let mut tx = state.pool.begin().await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
    state.work_order_service()
        .set_work_order_step_price(&service_ctx, &mut tx, path.order_id, path.step_no, unit_price).await?;
    tx.commit().await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
    // 返回刷新的行（反映已填价）
    let svc = state.production_batch_service();
    let rs = svc.list_routings(&service_ctx, &mut conn, path.order_id).await?;
    let r = rs.iter().find(|x| x.step_no == path.step_no)
        .ok_or_else(|| abt_core::shared::types::DomainError::not_found("WorkOrderRouting"))?;
    let wc_map: HashMap<i64, String> = state.work_center_service()
        .list_active(&service_ctx, &mut conn).await.unwrap_or_default()
        .into_iter().map(|w| (w.id, w.name)).collect();
    let prod_ids: Vec<i64> = rs.iter().filter_map(|x| x.product_id).collect();
    let prod_map: HashMap<i64, String> = if prod_ids.is_empty() { HashMap::new() } else {
        state.product_service().get_by_ids(&service_ctx, &mut conn, prod_ids).await.unwrap_or_default()
            .into_iter().map(|p| (p.product_id, p.pdt_name.clone())).collect()
    };
    Ok(Html(render_release_routing_row(r, &wc_map, &prod_map, None).into_string()))
}

/// 下达 drawer：从 BOM 重新加载工序（覆盖当前快照；计件单价自动从主数据 bom_step_prices 回填）。
/// 权限 WORK_ORDER update（与 release 同级；仅刷新工序结构，不改价，故不走 BOM_STEP_PRICE）。
/// 守卫复用 load_operations_from_bom：整单零报工 + 状态 Draft/Planned/Released/InProduction。
/// 成功 → 重渲 ② 工序区（#wc-release-routings）返回；service Err（已报工/状态不符）→ errors.rs toast。
#[require_permission("WORK_ORDER", "update")]
pub async fn reload_routings(
    path: WoReloadRoutingsPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let wo_svc = state.work_order_service();
    let batch_svc = state.production_batch_service();

    // 取工单 + product_code（load_operations_from_bom 入参）
    let order = wo_svc.find_by_id(&service_ctx, &mut conn, path.order_id).await?;
    let product = state
        .product_service()
        .get(&service_ctx, &mut conn, order.product_id)
        .await?;

    // 事务包裹：load = DELETE + INSERT work_order_routings + 审计（多步写）
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
    batch_svc
        .load_operations_from_bom(&service_ctx, &mut tx, path.order_id, product.product_code.clone())
        .await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;

    // 重渲 ② 工序区（reload 后快照 + 最新主数据价）
    let routings = batch_svc
        .list_routings(&service_ctx, &mut conn, path.order_id)
        .await?;
    let wc_map: HashMap<i64, String> = new_work_center_service(state.pool.clone())
        .list_active(&service_ctx, &mut conn)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|w| (w.id, w.name))
        .collect();
    let prod_ids: Vec<i64> = routings.iter().filter_map(|x| x.product_id).collect();
    let prod_map: HashMap<i64, String> = if prod_ids.is_empty() {
        HashMap::new()
    } else {
        state.product_service()
            .get_by_ids(&service_ctx, &mut conn, prod_ids)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|p| (p.product_id, p.pdt_name.clone()))
            .collect()
    };
    Ok(Html(render_release_routings_block(&routings, &wc_map, &prod_map, None).into_string()))
}

/// 单条生产批次行（数量 input + 删除按钮）。.split-row 供 addSplitRow 克隆；至少保留 1 行。
fn render_split_row(idx: usize, qty: Decimal) -> Markup {
    html! {
        div class="split-row flex items-center gap-2 mb-2" {
            span class="text-xs text-muted w-20 whitespace-nowrap split-label" { "生产批次" (idx + 1) }
            input class="split-qty w-24 px-2 py-1 border border-border rounded-sm text-sm font-mono text-right bg-white outline-none focus:border-accent"
                type="number" step="0.01 " min="0"
                value=(fmt_qty(qty));
            span class="text-xs text-muted" { "件" }
            button type="button" class="split-remove text-muted hover:text-danger cursor-pointer bg-transparent border-none px-1 text-base leading-none disabled:opacity-30 disabled:cursor-not-allowed"
                title="删除生产批次"
                disabled
                _="on click call removeSplitRow(me)" { "×" }
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
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let wo_svc = state.work_order_service();
    let batch_svc = state.production_batch_service();

    // 事务外只读校验：失败重渲染 form 标红（不走 Err/全局 toast）
    let order = wo_svc.find_by_id(&service_ctx, &mut conn, path.order_id).await?;
    let mut errors = ReleaseErrors::default();
    if order.status != WorkOrderStatus::Released && order.status != WorkOrderStatus::InProduction {
        let routings = batch_svc
            .list_routings(&service_ctx, &mut conn, path.order_id)
            .await?;
        if routings.is_empty() {
            // 报工强依赖工序，无工序下达会形成无法报工的死状态。
            errors.empty_routings = true;
        } else {
            // 计件单价校验（报工 confirm_routing_step 硬依赖 unit_price）；
            // 产出品可选——检测工序无产出，不领料（output=None → 空领料自然跳过，学 Odoo）
            for r in &routings {
                if r.unit_price.is_none() || r.unit_price == Some(Decimal::ZERO) {
                    errors
                        .price_missing
                        .insert(r.id, format!("工序 {}「{}」未配置计件单价", r.step_no, r.process_name));
                }
            }
        }
    }

    // 校验失败 → 重渲染 release drawer form（工序行产出品/单价标红 + 顶部 alert），保持打开
    if !errors.is_empty() {
        let data = load_release_drawer_data(&state, &service_ctx, &mut conn, path.order_id).await?;
        let body = render_release_drawer_body(&data, Some(&errors));
        return Ok(([("HX-Trigger", "")], Html(body.into_string())));
    }

    // 校验通过 → 下达（幂等：已 Released/InProduction 跳过）+ 分批，单事务
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if order.status != WorkOrderStatus::Released && order.status != WorkOrderStatus::InProduction {
        wo_svc
            .release(&service_ctx, &mut tx, path.order_id, order.version)
            .await?;
    }
    let splits: Vec<SplitReq> = parse_splits_json(&form.splits_json);
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
/// 既有 split_order（已随工单详情页下线）只建 1 批，工作中心下达 drawer 需一次规划多批，故新建此端点。
/// 分批 form：JS 收集 split 行为 JSON（`[{batch_qty, team_id?}, ...]`）传 `splits_json`。
/// 用 JSON 桥接而非 `Vec<SplitLineForm>` —— serde_urlencoded 不支持 `Vec<Struct>` 解析，
/// 旧实现 splits 永远为空导致 split_work_order 被跳过（下达后无批次 bug）。
#[derive(Debug, Deserialize)]
pub struct SplitMultiForm {
    #[serde(default)]
    pub splits_json: String,
}

/// JSON 桥接中转行（batch_qty 用字符串，handler 再 parse 成 Decimal）。
#[derive(Debug, Deserialize)]
struct SplitJsonLine {
    pub batch_qty: String,
    #[serde(default)]
    pub team_id: Option<String>,
}

/// 解析 splits_json → SplitReq（过滤 qty<=0 的无效行；team_id 空串/非数字→None）。
fn parse_splits_json(json: &str) -> Vec<SplitReq> {
    serde_json::from_str::<Vec<SplitJsonLine>>(json)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|l| {
            let q = l.batch_qty.parse::<Decimal>().ok()?;
            if q <= Decimal::ZERO {
                return None;
            }
            let team_id = l.team_id.and_then(|t| t.trim().parse::<i64>().ok());
            Some(SplitReq { batch_qty: q, team_id })
        })
        .collect()
}

#[require_permission("WORK_ORDER", "update")]
pub async fn split_multi(
    path: WcSplitMultiPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<SplitMultiForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let splits: Vec<SplitReq> = parse_splits_json(&form.splits_json);
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

// =============================================================================
// 工单级操作（取消）— 就地操作 + 广播 woChanged
// 完工（Closed）由完工入库闭环自动派生（production_receipt confirm → 所有批次终态），
// 无人工关闭入口（对齐 Odoo：Done 由库存移动闭环触发，非按钮）。
// =============================================================================

/// 取消工单：→ Cancelled。幂等：已 Cancelled 直接成功。
#[require_permission("WORK_ORDER", "update")]
pub async fn cancel_order(
    path: WcCancelPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let svc = state.work_order_service();
    let order = svc.find_by_id(&service_ctx, &mut tx, path.order_id).await?;
    if order.status == WorkOrderStatus::Cancelled {
        tx.commit()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        return Ok(([("HX-Trigger", "woChanged")], Html(String::new())));
    }
    svc.cancel(&service_ctx, &mut tx, path.order_id, order.version)
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
    /// 多人报工 JSON：[{worker_id, completed_qty, defect_qty}]（hx-on:htmx:config-request 注入）。
    #[serde(default)]
    pub workers_json: String,
    pub shift: ShiftType,
    #[serde(default)]
    pub work_hours: String,
    pub report_date: chrono::NaiveDate,
    #[serde(default)]
    pub remark: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WorkerReportItem {
    pub worker_id: i64,
    pub completed_qty: String,
    #[serde(default)]
    pub defect_qty: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct BatchReasonForm {
    #[serde(default)]
    pub reason: String,
}

/// 加载并渲染批次 drawer body（get_batch_drawer + 领料/收料/报工成功后刷新复用）。
async fn load_batch_drawer_html(
    state: &crate::state::AppState,
    service_ctx: &ServiceContext,
    conn: PgExecutor<'_>,
    batch_id: i64,
) -> Result<String> {
    let batch_svc = state.production_batch_service();
    let wo_svc = state.work_order_service();
    let batch = batch_svc.find_by_id(service_ctx, conn, batch_id).await?;
    let order = wo_svc.find_by_id(service_ctx, conn, batch.work_order_id).await?;
    let product_name = wo_svc
        .get_product_name(conn, batch.product_id)
        .await?
        .unwrap_or_else(|| format!("#{}", batch.product_id));
    let routings = batch_svc
        .list_routings(service_ctx, conn, batch.work_order_id)
        .await
        .unwrap_or_default();
    // 每道工序产出品的物料齐套（#124 工序级齐套）
    let mut step_avails: HashMap<i64, abt_core::mes::work_order::MaterialAvailability> =
        HashMap::new();
    for r in &routings {
        if let Ok(avail) = wo_svc
            .compute_step_availability(service_ctx, conn, batch.work_order_id, r.id, Some(batch.id))
            .await
        {
            step_avails.insert(r.id, avail);
        }
    }
    let wc_map: HashMap<i64, String> = new_work_center_service(state.pool.clone())
        .list_active(service_ctx, conn)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|wc| (wc.id, wc.name))
        .collect();
    // 批次已领料的工序集合（Confirmed/Done；防重复领料 + InProgress 报工前置）
    let req_routing_ids: std::collections::HashSet<i64> = state
        .picking_service()
        .list_requisitioned_routing_ids(service_ctx, conn, batch.id)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();
    // 批次已发料完成的工序集合（Done；收料前置——仓库 issue 发齐才能收料开工）
    let issued_routing_ids: std::collections::HashSet<i64> = state
        .picking_service()
        .list_issued_routing_ids(service_ctx, conn, batch.id)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();
    // 需领料的工序集合（产出品含外购子级）；纯半成品/散料工序不需领料，动作位直接收料
    let needs_requisition: std::collections::HashSet<i64> = batch_svc
        .list_routings_needing_requisition(service_ctx, conn, batch.work_order_id)
        .await
        .unwrap_or_default();
    // 委外工序的活跃委外单（动作位判定：创建/发料/收货/已完成）
    let om_svc = state.outsourcing_order_service();
    let mut osa_map: HashMap<i64, abt_core::om::outsourcing_order::OutsourcingOrder> = HashMap::new();
    for r in &routings {
        if r.is_outsourced
            && let Ok(list) = om_svc
                .find_active_for_routing(service_ctx, conn, batch.work_order_id, r.id, Some(batch.id))
                .await
            && let Some(o) = list.into_iter().next()
        {
            osa_map.insert(r.id, o);
        }
    }
    Ok(render_batch_drawer_body(&batch, &order, &product_name, &routings, &step_avails, &wc_map, &req_routing_ids, &issued_routing_ids, &needs_requisition, &osa_map).into_string())
}

/// 批次处理 drawer body：批次信息 + 工序进度 + 报工表单 + 状态操作（按 BatchStatus 门控）。
#[require_permission("WORK_ORDER", "read")]
pub async fn get_batch_drawer(
    path: WcBatchDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let body = load_batch_drawer_html(&state, &service_ctx, &mut conn, path.batch_id).await?;
    Ok(Html(body))
}

fn render_batch_drawer_body(
    batch: &ProductionBatch,
    order: &WorkOrder,
    product_name: &str,
    routings: &[WorkOrderRouting],
    step_avails: &HashMap<i64, abt_core::mes::work_order::MaterialAvailability>,
    wc_map: &HashMap<i64, String>,
    req_routing_ids: &std::collections::HashSet<i64>,
    issued_routing_ids: &std::collections::HashSet<i64>,
    needs_requisition: &std::collections::HashSet<i64>,
    osa_map: &HashMap<i64, abt_core::om::outsourcing_order::OutsourcingOrder>,
) -> Markup {
    let (slabel, stoken) = batch_status_meta(&batch.status);
    let can_suspend = matches!(batch.status, BatchStatus::InProgress);
    let can_resume = matches!(batch.status, BatchStatus::Suspended);
    let can_scrap = matches!(batch.status, BatchStatus::InProgress | BatchStatus::Suspended);
    let can_receipt = matches!(batch.status, BatchStatus::PendingReceipt);
    html! {
        // 批次信息头 + 右上角工具栏
        div class="mb-4 pb-3 border-b border-border-soft" {
            div class="flex items-start justify-between gap-3" {
                // 左侧：批次信息
                div class="flex-1" {
                    div class="flex items-center gap-2 mb-1" {
                        span class="text-xs text-muted" { "流转卡" }
                        span class="font-mono font-semibold text-fg" { (batch.card_sn.as_str()) }
                        span class=(format!("inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium bg-{stoken}-bg text-{stoken}")) {
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
                // 右侧：操作按钮工具栏
                div class="flex items-center gap-2 shrink-0" {
                    @if can_suspend {
                        form hx-post=(WcBatchSuspendPath { batch_id: batch.id }.to_string()) hx-swap="none"
                            hx-on:htmx:after-request="if(event.detail.successful) document.querySelector('#batch-overlay').classList.remove('open')" {
                            input type="hidden" name="reason" value="手动暂停";
                            button type="submit" class="px-3 py-1.5 rounded-sm border border-border text-xs font-medium text-fg-2 cursor-pointer hover:bg-warn-bg hover:text-warn"
                                hx-confirm="暂停该批次？" { "暂停" }
                        }
                    }
                    @if can_resume {
                        form hx-post=(WcBatchResumePath { batch_id: batch.id }.to_string()) hx-swap="none"
                            hx-on:htmx:after-request="if(event.detail.successful) document.querySelector('#batch-overlay').classList.remove('open')" {
                            button type="submit" class="px-3 py-1.5 rounded-sm border border-border text-xs font-medium text-fg-2 cursor-pointer hover:bg-accent-bg hover:text-accent" { "恢复" }
                        }
                    }
                    @if can_scrap {
                        button type="button"
                            class="px-3 py-1.5 rounded-sm border border-border text-xs font-medium text-fg-2 cursor-pointer hover:bg-danger-bg hover:text-danger"
                            hx-get=(WcBatchScrapModalPath { batch_id: batch.id }.to_string())
                            hx-target="#batch-scrap-modal-slot" hx-swap="innerHTML"
                            _="on click halt the event" { "报废" }
                    }
                    @if can_receipt {
                        button class="px-3 py-1.5 rounded-sm bg-success text-white border-none text-xs font-medium cursor-pointer hover:opacity-90"
                            hx-get=(WcBatchReceiptModalPath { batch_id: batch.id }.to_string())
                            hx-target="#batch-receipt-modal-slot" hx-swap="innerHTML"
                            _="on click halt the event" { "完工入库" }
                    } @else if matches!(batch.status, BatchStatus::Pending) {
                        // 待开工：工具栏不空白，引导到下方工序区第①道「收料」（issue #260）
                        span class="text-xs text-muted" { "👇 请到下方工序区第①道「收料」" }
                    } @else if matches!(batch.status, BatchStatus::Completed) {
                        span class="text-xs text-success font-medium" { "✓ 已完工" }
                    } @else if matches!(batch.status, BatchStatus::Cancelled) {
                        span class="text-xs text-muted" { "已取消" }
                    }
                }
            }
        }

        // 工序流转矩阵（#130 改版：领料→收料/报工因果链）
        div class="overflow-x-auto" {
            table class="w-full text-sm" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="text-left font-semibold py-2 px-3" { "工序" }
                        th class="text-left font-semibold py-2 px-3" { "①齐套分析" }
                        th class="text-left font-semibold py-2 px-3" { "②动作（领料 / 收料 / 报工）" }
                    }
                }
                tbody {
                    @if routings.is_empty() {
                        tr { td colspan="3" class="text-center text-muted py-6" { "该工单尚无工序" } }
                    }
                    @for r in routings {
                        (render_batch_matrix_row(batch, r, step_avails, wc_map, req_routing_ids, issued_routing_ids, needs_requisition, osa_map))
                    }
                }
            }
        }
        // 委外工序就地 drawer 容器：GET 加载创建表单 innerHTML 进此 slot
        // （OsaCreate 动作位 hx-target="#osa-drawer-slot"；提交后 hx-target="#batch-drawer-body" 刷新整 body）
        div id="osa-drawer-slot" {}
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

/// 矩阵行：工序 | 齐套徽章 | 动作（领料→收料→报工 按状态推进，#131）。
fn render_batch_matrix_row(
    batch: &ProductionBatch,
    r: &WorkOrderRouting,
    step_avails: &HashMap<i64, abt_core::mes::work_order::MaterialAvailability>,
    wc_map: &HashMap<i64, String>,
    req_routing_ids: &std::collections::HashSet<i64>,
    issued_routing_ids: &std::collections::HashSet<i64>,
    needs_requisition: &std::collections::HashSet<i64>,
    osa_map: &HashMap<i64, abt_core::om::outsourcing_order::OutsourcingOrder>,
) -> Markup {
    use abt_core::mes::work_order::MaterialAvailabilityLevel;
    use abt_core::om::enums::OutsourcingStatus;
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
    // current_step = 最后一个已完成工序号（0=尚无完成，见 confirm_routing_step 推进语义）。
    // 原代码把它当成「当前进行工序号」造成 off-by-one：第一道完成后仍显示报工而非已完成。
    let batch_done = matches!(
        batch.status,
        BatchStatus::PendingReceipt | BatchStatus::Completed
    );
    let active_step = if batch_done {
        i32::MAX // 全完成：靠 is_completed 兜底全部 Done，无越界
    } else if matches!(batch.status, BatchStatus::Suspended) {
        batch.current_step // 待检工序（刚报工、质检中）保留为当前
    } else if batch.current_step == 0 {
        1 // 首道待开工
    } else {
        batch.current_step + 1 // 下一道待加工（核心修正）
    };
    let is_current = r.step_no == active_step;
    let is_completed = r.step_no < active_step;
    let has_req = req_routing_ids.contains(&r.id);
    let has_issued = issued_routing_ids.contains(&r.id);
    // 无产出工序（检测/检验）→ 无消耗物料 → 视同已领料，不显示领料按钮（学 OFBiz 按有无消耗决定按钮）
    let has_output = r.product_id.is_some();
    let ready = has_req || !has_output || !needs_requisition.contains(&r.id);
    // 收料前置：仓库发料完成（Done）或无需领料（无产出 / 纯半成品车间直转且齐套）才能收料开工
    let can_receive = has_issued || !has_output || (!needs_requisition.contains(&r.id) && shortage_n == 0);
    let is_pending = matches!(batch.status, BatchStatus::Pending);
    let is_inprogress = matches!(batch.status, BatchStatus::InProgress);
    let kitted = shortage_n == 0;
    // 动作位统一判定：批次状态 × 当前工序 × 领料/发料状态 → Action 枚举，模板只做 @match 渲染
    enum Action {
        Done,        // ✅ 已完成
        Receive,     // 收料（Pending→InProgress 开工）
        WaitIssue,   // ⏳ 待仓库发料（已领料、仓库未发齐）
        Requisition, // 领料（建领料单）
        Shortage,    // 欠料置灰
        Report,      // 报工
        Suspended,   // ⚠ 已暂停
        Dash,        // —
        OsaCreate,   // 🔧 委外：创建委外单
        OsaDraft,    // 委外单已建(Draft) → 发料
        OsaSent,     // 委外已发料(Sent) → 收货
        OsaDone,     // 委外已收货(Received)
    }
    // 委外工序走独立动作流（不参与车间领料/收料/报工）
    let action = if r.is_outsourced {
        match osa_map.get(&r.id) {
            None if is_current => Action::OsaCreate,
            Some(o) if o.status == OutsourcingStatus::Draft => Action::OsaDraft,
            Some(o) if o.status == OutsourcingStatus::Sent => Action::OsaSent,
            Some(o) if o.status == OutsourcingStatus::Received => Action::OsaDone,
            _ if is_completed => Action::Done,
            _ => Action::Dash,
        }
    } else if is_completed {
        Action::Done
    } else if is_current && is_pending {
        if can_receive {
            Action::Receive
        } else if has_req {
            Action::WaitIssue
        } else if has_output && kitted {
            Action::Requisition
        } else {
            Action::Shortage
        }
    } else if is_current && is_inprogress {
        if ready {
            Action::Report
        } else if has_output && kitted {
            Action::Requisition
        } else {
            Action::Dash
        }
    } else if matches!(batch.status, BatchStatus::Suspended) && is_current {
        Action::Suspended
    } else {
        Action::Dash
    };
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
            // 第3列：动作位（@match action 纯渲染，判定逻辑见上方 action 计算）
            td class="py-2.5 px-3 text-xs" {
                @match action {
                    Action::Done => {
                        span class="text-success font-medium" { "✅ 已完成" }
                    }
                    Action::Receive => {
                        // 已发料完成（或无产出工序）+ Pending → 收料开工
                        form hx-post=(WcBatchReceivePath { batch_id: batch.id }.to_string()) hx-target="#batch-drawer-body" hx-swap="innerHTML" {
                            button type="submit"
                                class="text-xs px-2 py-1 rounded-sm bg-accent text-accent-on border-none cursor-pointer hover:opacity-90 transition-all font-medium"
                                hx-confirm="确认收料开工？" { "收料" }
                        }
                    }
                    Action::WaitIssue => {
                        // 已领料但仓库未发齐 → 待仓库 issue 发料完成
                        span class="text-xs text-warn" title="已领料，等待仓库发料完成" { "⏳ 待仓库发料" }
                    }
                    Action::Requisition => {
                        // 有产出 + 未领料 + 齐套 → 领料
                        form hx-post=(WcBatchReqPath { batch_id: batch.id }.to_string()) hx-target="#batch-drawer-body" hx-swap="innerHTML" {
                            input type="hidden" name="routing_id" value=(r.id);
                            button type="submit"
                                class="text-xs px-2 py-1 rounded-sm border border-border text-fg-2 hover:bg-accent-bg hover:text-accent cursor-pointer transition-all font-medium"
                                hx-confirm="确认领料本工序物料？" { "领料" }
                        }
                    }
                    Action::Shortage => {
                        // 未领料 + 欠料 → 置灰
                        span class="text-xs text-muted cursor-not-allowed" title="欠料，无法领料" { "领料" }
                    }
                    Action::Report => {
                        // 已领料（或无产出工序）+ InProgress → 报工
                        button class="text-xs px-2 py-1 rounded-sm bg-accent text-accent-on border-none cursor-pointer hover:opacity-90 transition-all font-medium"
                            hx-get=(WcBatchReportModalPath { batch_id: batch.id, step_no: r.step_no }.to_string())
                            hx-target="#batch-report-modal-slot" hx-swap="innerHTML"
                            _="on click halt the event" {
                            "报工"
                        }
                    }
                    Action::Suspended => {
                        span class="text-warn" { "⚠ 已暂停" }
                    }
                    Action::OsaCreate => {
                        // 委外工序当前道 + 无委外单 → 创建委外单（就地 drawer）
                        button class="text-xs px-2 py-1 rounded-sm bg-purple text-white border-none cursor-pointer hover:opacity-90 transition-all font-medium"
                            hx-get=(WcBatchOsaCreateDrawerPath { batch_id: batch.id, routing_id: r.id }.to_string())
                            hx-target="#osa-drawer-slot" hx-swap="innerHTML"
                            _="on click halt the event" {
                            "创建委外单"
                        }
                    }
                    Action::OsaDraft => {
                        // 委外单已建(Draft) → 发料给供应商（om send）
                        form hx-post=(WcBatchOsaSendPath { batch_id: batch.id, routing_id: r.id }.to_string()) hx-target="#batch-drawer-body" hx-swap="innerHTML" {
                            button type="submit" class="text-xs px-2 py-1 rounded-sm bg-accent text-accent-on border-none cursor-pointer hover:opacity-90 transition-all font-medium"
                                hx-confirm="确认发料给供应商？" { "委外发料" }
                        }
                    }
                    Action::OsaSent => {
                        // 委外已发料(Sent) → 收货（om receive 入 WIP-SHOP）
                        form hx-post=(WcBatchOsaReceivePath { batch_id: batch.id, routing_id: r.id }.to_string()) hx-target="#batch-drawer-body" hx-swap="innerHTML" {
                            button type="submit" class="text-xs px-2 py-1 rounded-sm bg-success text-white border-none cursor-pointer hover:opacity-90 transition-all font-medium"
                                hx-confirm="确认委外收货？" { "委外收货" }
                        }
                    }
                    Action::OsaDone => {
                        span class="text-success font-medium" { "✅ 委外已完成" }
                    }
                    Action::Dash => {
                        span class="text-muted" { "—" }
                    }
                }
            }
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

/// 批次报工（多人）：解析 workers_json，事务内循环 confirm_routing_step（各人完成量累加）。
/// 共享不良量归首个工人；后端守卫不改（#131），同工序多人补报不被拦。广播 batchChanged。
#[require_permission("WORK_ORDER", "update")]
pub async fn batch_report(
    path: WcBatchReportPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<BatchReportForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let workers: Vec<WorkerReportItem> = if form.workers_json.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(&form.workers_json)
            .map_err(|_| DomainError::validation("报工人数据格式错误"))?
    };
    if workers.is_empty() {
        return Err(DomainError::validation("请至少选择一名报工人").into());
    }
    let work_hours = form.work_hours.parse::<Decimal>().unwrap_or(Decimal::ZERO);
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    for w in workers.iter() {
        let completed_qty = w
            .completed_qty
            .parse::<Decimal>()
            .map_err(|_| DomainError::validation("完成量格式错误"))?;
        let defect_qty = w.defect_qty.parse::<Decimal>().unwrap_or(Decimal::ZERO);
        let req = StepConfirmationReq {
            step_no: form.step_no,
            worker_id: w.worker_id,
            shift: form.shift,
            completed_qty,
            // 每工人各自的不良量（defect_reason=None 不计入个人工资）
            defect_qty,
            defect_reason: None,
            work_hours,
            report_date: form.report_date,
            remark: form.remark.clone(),
        };
        state
            .production_batch_service()
            .confirm_routing_step(&service_ctx, &mut tx, path.batch_id, form.step_no, req)
            .await?;
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 刷新 drawer body（进度/动作位更新）+ 批次状态变更刷新卡片 + 成功 toast
    crate::toast::add_toast(
        service_ctx.operator_id,
        format!("已提交 {} 人报工", workers.len()),
        crate::toast::ToastType::Success,
    );
    let mut conn = state.pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
    let body = load_batch_drawer_html(&state, &service_ctx, &mut conn, path.batch_id).await?;
    Ok(([("HX-Trigger", "batchChanged, showToast")], Html(body)))
}
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

/// 批次收料（开工）：Pending → InProgress，复用 start_batch，广播 batchChanged。
#[require_permission("WORK_ORDER", "update")]
pub async fn batch_receive(
    path: WcBatchReceivePath,
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
    // 刷新 drawer body（动作位变报工）+ 批次状态变更刷新卡片 + 成功 toast
    crate::toast::add_toast(
        service_ctx.operator_id,
        "已收料，批次开工",
        crate::toast::ToastType::Success,
    );
    let mut conn = state.pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
    let body = load_batch_drawer_html(&state, &service_ctx, &mut conn, path.batch_id).await?;
    Ok(([("HX-Trigger", "batchChanged, showToast")], Html(body)))
}

#[derive(Debug, Deserialize)]
pub struct BatchScrapSubmitForm {
    pub scrap_qty: String,
    pub reason: String,
    #[serde(default)]
    pub notes: Option<String>,
}

/// 批次部分报废提交：记录 scrap_qty，不取消批次，广播 batchChanged。
#[require_permission("WORK_ORDER", "update")]
pub async fn batch_scrap_submit(
    path: WcBatchScrapSubmitPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<BatchScrapSubmitForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let scrap_qty = form
        .scrap_qty
        .parse::<Decimal>()
        .map_err(|_| DomainError::validation("报废数量格式错误"))?;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .production_batch_service()
        .record_scrap(
            &service_ctx,
            &mut tx,
            path.batch_id,
            scrap_qty,
            form.reason,
            form.notes,
        )
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(([("HX-Trigger", "batchChanged")], Html(String::new())))
}

/// 报废 modal 表单（GET）。
#[require_permission("WORK_ORDER", "read")]
pub async fn get_batch_scrap_modal(
    path: WcBatchScrapModalPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let batch = state
        .production_batch_service()
        .find_by_id(&service_ctx, &mut conn, path.batch_id)
        .await?;
    let remaining = batch.batch_qty - batch.completed_qty - batch.scrap_qty;
    Ok(Html(
        drawer_shell(
            "batch-scrap-drawer",
            "w-[480px]",
            render_scrap_form(path.batch_id, &batch, remaining),
        )
        .into_string(),
    ))
}

/// 报废原因选项（标签对）。
fn scrap_reason_options() -> Vec<(&'static str, &'static str)> {
    vec![
        ("material_defect", "材料不良"),
        ("equipment_fault", "设备故障"),
        ("operator_error", "操作失误"),
        ("process_issue", "工艺问题"),
        ("other", "其他"),
    ]
}

fn render_scrap_form(
    batch_id: i64,
    batch: &ProductionBatch,
    remaining: rust_decimal::Decimal,
) -> Markup {
    html! {
        // Header
        div class="px-6 py-4 border-b border-border-soft flex items-center justify-between shrink-0" {
                h2 class="text-base font-bold text-fg m-0" { "批次报废" }
                button type="button"
                    class="w-7 h-7 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg flex items-center justify-center"
                    _="on click remove .open from closest .drawer-overlay" {
                    (icon::x_icon("w-4 h-4"))
                }
            }
            // Body
            div class="p-6 space-y-4" {
                div class="text-xs text-muted font-mono mb-2" {
                    "流转卡 " (batch.card_sn.as_str()) " · 批次量 " (fmt_qty(batch.batch_qty))
                    " · 已完成 " (fmt_qty(batch.completed_qty))
                }
                form hx-post=(WcBatchScrapSubmitPath { batch_id }.to_string())
                    hx-swap="none"
                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from closest .drawer-overlay then trigger batchChanged from:body" {
                    // Scrap quantity
                    div {
                        label class="block text-xs text-fg-2 mb-1" { "报废数量" }
                        input type="number" step="0.01" min="0.01" max=(fmt_qty(remaining))
                            name="scrap_qty" required
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm font-mono text-right bg-white outline-none focus:border-accent ";
                        div class="text-xs text-muted mt-1" { "可报废量 " (fmt_qty(remaining)) }
                    }
                    // Reason dropdown
                    div {
                        label class="block text-xs text-fg-2 mb-1" { "报废原因" }
                        select name="reason"
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent" {
                            @for (val, label) in scrap_reason_options() {
                                option value=(val) { (label) }
                            }
                        }
                    }
                    // Notes
                    div {
                        label class="block text-xs text-fg-2 mb-1" { "备注（可选）" }
                        textarea name="notes" rows="2"
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent resize-y" {}
                    }
                    // Footer buttons
                    div class="flex justify-end gap-3 pt-4 border-t border-border-soft mt-4" {
                        button type="button"
                            class="px-4 py-2 rounded-sm bg-white text-fg-2 border border-border text-sm cursor-pointer hover:bg-surface"
                            _="on click remove .open from closest .drawer-overlay" { "取消" }
                        button type="submit"
                            class="px-4 py-2 rounded-sm bg-danger text-white text-sm font-medium cursor-pointer border-none hover:opacity-90" {
                            "确认报废"
                        }
                    }
                }
            }
    }
}

/// 报工 modal 表单（GET：从批次抽屉矩阵行直接报工，预填工序号）。
#[require_permission("WORK_ORDER", "read")]
pub async fn get_batch_report_modal(
    path: WcBatchReportModalPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let batch = state
        .production_batch_service()
        .find_by_id(&service_ctx, &mut conn, path.batch_id)
        .await?;
    let routing = state
        .production_batch_service()
        .list_routings(&service_ctx, &mut conn, batch.work_order_id)
        .await
        .unwrap_or_default()
        .into_iter()
        .find(|r| r.step_no == path.step_no);
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    Ok(Html(
        drawer_shell(
            "batch-report-drawer",
            "w-[720px]",
            render_batch_report_modal_body(path.batch_id, &batch, routing.as_ref(), &today),
        )
        .into_string(),
    ))
}

fn render_batch_report_modal_body(
    batch_id: i64,
    batch: &ProductionBatch,
    routing: Option<&WorkOrderRouting>,
    today: &str,
) -> Markup {
    let step_no = routing.map(|r| r.step_no).unwrap_or(1);
    html! {
        div class="px-6 py-4 border-b border-border-soft flex items-center justify-between shrink-0" {
                h2 class="text-base font-bold text-fg m-0" { "工序报工" }
                button type="button"
                    class="w-7 h-7 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg flex items-center justify-center"
                    _="on click remove .open from closest .drawer-overlay" {
                    (icon::x_icon("w-4 h-4"))
                }
            }
            div class="p-6" {
                div class="text-xs text-muted font-mono mb-4" {
                    "流转卡 " (batch.card_sn.as_str())
                }
                form hx-post=(WcBatchReportPath { batch_id }.to_string())
                    hx-target="#batch-drawer-body" hx-swap="innerHTML"
                    hx-on:htmx:config-request="event.detail.parameters['workers_json'] = window.wcCollectWorkers(event.detail.elt)"
                    hx-on:htmx:after-request="if(event.detail.successful) document.getElementById('batch-report-drawer').classList.remove('open')" {
                    input type="hidden" name="step_no" value=(step_no);
                    // 工人表格（picker add-row：每行 工人 + 完成量 + 不良量；「添加工人」按钮在表格下方）
                    div class="mb-3" {
                        table class="w-full border border-border rounded-sm table-fixed" {
                            thead class="bg-surface" {
                                tr class="text-xs text-fg-2" {
                                    th class="px-2 py-1.5 text-left font-medium w-48" { "工人" }
                                    th class="px-2 py-1.5 text-right font-medium" { "完成量" }
                                    th class="px-2 py-1.5 text-right font-medium" { "不良量" }
                                    th class="px-2 py-1.5 text-center font-medium" { "操作" }
                                }
                            }
                            tbody id="report-workers-tbody" {
                            }
                        }
                        button type="button"
                            class="mt-2 w-full px-3 py-1.5 rounded-sm border border-dashed border-border text-xs text-accent hover:bg-accent-bg cursor-pointer bg-transparent"
                            _="on click add .is-open to #worker-picker-modal" { "+ 添加工人" }
                    }
                    div class="grid grid-cols-3 gap-3 mb-4" {
                        div {
                            label class="block text-xs text-fg-2 mb-1" { "班次" }
                            select name="shift" class="w-full px-2 py-1.5 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent" {
                                option value="1" selected { "白班" }
                                option value="2" { "夜班" }
                            }
                        }
                        div {
                            label class="block text-xs text-fg-2 mb-1" { "工时(h)" }
                            input type="number" step="0.1" min="0" name="work_hours" value="8"
                                class="w-full px-2 py-1.5 border border-border rounded-sm text-sm font-mono text-right bg-white outline-none focus:border-accent" {};
                        }
                        div {
                            label class="block text-xs text-fg-2 mb-1" { "报工日期" }
                            input type="date" name="report_date" value=(today)
                                class="w-full px-2 py-1.5 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent" {};
                        }
                    }
                    div class="flex justify-end gap-3 pt-4 border-t border-border-soft" {
                        button type="button"
                            class="px-4 py-2 rounded-sm bg-white text-fg-2 border border-border text-sm cursor-pointer hover:bg-surface"
                            _="on click remove .open from closest .drawer-overlay" { "取消" }
                        button type="submit"
                            class="px-4 py-2 rounded-sm bg-accent text-white text-sm font-medium cursor-pointer border-none hover:opacity-90" {
                            "确认报工"
                        }
                    }
                }
            }
    }
}

#[derive(Debug, Deserialize)]
pub struct WorkerRowParams {
    pub worker_id: i64,
}

/// 报工工人行（GET ?worker_id=X → 渲染一行进 #report-workers-tbody，worker_picker add-row）。
#[require_permission("WORK_ORDER", "read")]
pub async fn get_worker_row(
    ctx: RequestContext,
    Query(params): Query<WorkerRowParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let users = state
        .user_service()
        .list_users_by_departments(&service_ctx, &mut conn, &["SHENGCHAN"])
        .await
        .unwrap_or_default();
    let user = users
        .into_iter()
        .find(|u| u.user.user_id == params.worker_id)
        .ok_or_else(|| DomainError::not_found("生产部人员"))?;
    Ok(Html(render_worker_row(&user).into_string()))
}

/// 报工工人表格行：hidden worker_id + 完成量 input + 删除。data-k 字段由 wcCollectWorkers 收集。
fn render_worker_row(user: &UserWithRoles) -> Markup {
    let display_name = user.user.display_name.as_deref().unwrap_or(user.user.username.as_str());
    html! {
        tr data-worker-row class="border-b border-border-soft" {
            td class="px-2 py-1.5 text-sm text-fg" {
                (display_name)
                input type="hidden" data-k="worker_id" value=(user.user.user_id);
            }
            td class="px-2 py-1.5" {
                input type="number" step="0.01" min="0" data-k="completed_qty" required placeholder="完成量"
                    class="w-full px-2 py-1 border border-border rounded-sm text-sm font-mono text-right bg-white outline-none focus:border-accent" {};
            }
            td class="px-2 py-1.5" {
                input type="number" step="0.01" min="0" data-k="defect_qty" value="0" placeholder="不良量"
                    class="w-full px-2 py-1 border border-border rounded-sm text-sm font-mono text-right bg-white outline-none focus:border-accent" {};
            }
            td class="px-2 py-1.5 text-center" {
                button type="button"
                    class="text-xs text-danger hover:underline cursor-pointer border-none bg-transparent"
                    _="on click remove closest <tr/>" { "删除" }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct BatchReqForm {
    pub routing_id: i64,
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
        .picking_service()
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
    // 刷新 drawer body（动作位由「领料」变「收料」）+ 成功 toast
    crate::toast::add_toast(
        service_ctx.operator_id,
        "领料单已生成，已通知仓库",
        crate::toast::ToastType::Success,
    );
    let mut conn = state.pool.acquire().await.map_err(|e| DomainError::Internal(e.into()))?;
    let body = load_batch_drawer_html(&state, &service_ctx, &mut conn, path.batch_id).await?;
    Ok(([("HX-Trigger", "showToast")], Html(body)))
}

#[derive(Debug, Deserialize)]
pub struct BatchReceiptForm {
    pub received_qty: String,
    pub receipt_date: chrono::NaiveDate,
    #[serde(default)]
    pub remark: Option<String>,
}

/// 批次入库 modal 表单（GET：加载入库弹窗，预填批次量/今日）。
#[require_permission("WORK_ORDER", "read")]
pub async fn get_batch_receipt_modal(
    path: WcBatchReceiptModalPath,
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
    Ok(Html(
        drawer_shell(
            "batch-receipt-drawer",
            "w-[480px]",
            render_batch_receipt_modal_body(&batch, &product_name),
        )
        .into_string(),
    ))
}

fn render_batch_receipt_modal_body(
    batch: &ProductionBatch,
    product_name: &str,
) -> Markup {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    html! {
        div class="px-6 py-4 border-b border-border-soft flex items-center justify-between shrink-0" {
                h2 class="text-base font-bold text-fg m-0" { "完工入库" }
                button type="button"
                    class="w-7 h-7 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg flex items-center justify-center"
                    _="on click remove .open from closest .drawer-overlay" {
                    (icon::x_icon("w-4 h-4"))
                }
            }
            div class="p-6" {
                div class="text-xs text-muted font-mono mb-3" {
                    "流转卡 " (batch.card_sn.as_str()) " · " (product_name)
                }
                form hx-post=(WcBatchReceiptPath { batch_id: batch.id }.to_string())
                    hx-swap="none"
                    hx-on:htmx:after-request="if(event.detail.successful) document.getElementById('batch-receipt-drawer').classList.remove('open')" {
                    div class="grid grid-cols-2 gap-3 mb-3" {
                        div {
                            label class="block text-xs text-fg-2 mb-1" { "入库数量" }
                            input type="number" step="0.01" min="0" name="received_qty" value=(fmt_qty(batch.batch_qty))
                                class="w-full px-2 py-1.5 border border-border rounded-sm text-sm font-mono text-right bg-white outline-none focus:border-accent";
                        }
                        div {
                            label class="block text-xs text-fg-2 mb-1" { "入库日期" }
                            input type="date" name="receipt_date" value=(today)
                                class="w-full px-2 py-1.5 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent";
                        }
                    }
                    div class="mb-3" {
                        label class="block text-xs text-fg-2 mb-1" { "备注" }
                        input type="text" name="remark"
                            class="w-full px-2 py-1.5 border border-border rounded-sm text-sm bg-white outline-none focus:border-accent";
                    }
                    div class="flex items-start gap-2 text-xs text-fg-2 bg-accent-bg rounded-sm px-3 py-2 mb-4" {
                        (icon::info_icon("w-4 h-4 shrink-0 mt-0.5 text-accent"))
                        span { "提交后生成入库申请单（草稿），由仓库在「完工入库」确认后触发倒冲（按 BOM 扣原材料）+ 成本归集 + FQC 门控。" }
                    }
                    div class="flex justify-end gap-3 pt-4 border-t border-border-soft" {
                        button type="button"
                            class="px-4 py-2 rounded-sm bg-white text-fg-2 border border-border text-sm cursor-pointer hover:bg-surface"
                            _="on click remove .open from closest .drawer-overlay" { "取消" }
                        button type="submit"
                            class="px-4 py-2 rounded-sm bg-success text-white text-sm font-medium cursor-pointer border-none hover:opacity-90" {
                            "提交入库申请"
                        }
                    }
                }
            }
    }
}

/// 批次入库申请：ProductionReceipt.create 建 Draft（不填仓库，不 confirm），广播 batchChanged + toast。
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
    let pick_svc = state.picking_service();
    let req = CreatePickingReq {
        picking_type: abt_core::wms::enums::PickingType::IncomingWorkOrder,
        source_type: Some("work_order".to_string()),
        source_id: Some(batch.work_order_id),
        partner_id: None,
        from_warehouse_id: None,
        from_zone_id: None,
        from_bin_id: None,
        to_warehouse_id: None,
        to_zone_id: None,
        to_bin_id: None,
        scheduled_date: Some(form.receipt_date),
        work_order_id: Some(batch.work_order_id),
        remark: form.remark,
        shipping_requirements: None,
        items: vec![CreatePickingItemReq {
            product_id: order.product_id,
            batch_no: None,
            qty_requested: received_qty,
            from_bin_id: None,
            to_bin_id: None,
            operation_id: None,
            batch_id: Some(path.batch_id),
            source_item_id: None,
            remark: None,
        }],
    };
    // 两步流程：生产侧只创建 Draft 入库 picking（不填仓库），由仓库在「完工入库」receive_production
    let _picking_id = pick_svc.create(&service_ctx, &mut tx, req).await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    crate::toast::add_toast(
        service_ctx.operator_id,
        "入库申请已提交，待仓库确认",
        crate::toast::ToastType::Success,
    );
    Ok(([("HX-Trigger", "batchChanged, showToast")], Html(String::new())))
}

// =============================================================================
// 批次工序委外（OSA）：就地创建 / 发料 / 收货
// =============================================================================
// 设计：委外工序在 batch drawer 矩阵中走独立动作流（OsaCreate→OsaDraft→OsaSent→OsaDone）。
// - get_osa_create_drawer：渲染创建表单 innerHTML 进 #osa-drawer-slot（矩阵下方）；
// - osa_create/osa_send/osa_receive：POST 后均刷新整个 #batch-drawer-body（动作位推进）+ 广播 woChanged。
//   返回 fresh body（非空 body + 事件）——与 batch_requisition/batch_receive 同范式：矩阵表单
//   hx-target="#batch-drawer-body"，空 body 会清空 drawer，故必须回传重渲染的 body。

#[derive(Debug, Deserialize)]
pub struct OsaCreateForm {
    pub supplier_id: i64,
    pub unit_price: String,
    #[serde(default)]
    pub virtual_warehouse_id: i64,
    #[serde(default)]
    pub source_warehouse_id: i64,
}

/// 批次工序委外：就地创建委外单 drawer 表单（GET，预填本道半成品 + 计划量）。
///
/// 渲染创建表单片段，innerHTML 进 `#osa-drawer-slot`（batch drawer body 矩阵下方）。
/// 提交走 `osa_create`（form hx-target="#batch-drawer-body"），成功后刷新整个 drawer body。
#[require_permission("WORK_ORDER", "read")]
pub async fn get_osa_create_drawer(
    path: WcBatchOsaCreateDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let batch_svc = state.production_batch_service();
    let wo_svc = state.work_order_service();
    let batch = batch_svc
        .find_by_id(&service_ctx, &mut conn, path.batch_id)
        .await?;
    let routings = batch_svc
        .list_routings(&service_ctx, &mut conn, batch.work_order_id)
        .await
        .unwrap_or_default();
    let routing = routings
        .iter()
        .find(|r| r.id == path.routing_id)
        .ok_or_else(|| DomainError::not_found("WorkOrderRouting"))?;
    if !routing.is_outsourced {
        return Err(DomainError::business_rule("该工序非委外工序").into());
    }
    let semi_pid = routing
        .product_id
        .ok_or_else(|| DomainError::business_rule("委外工序未设置产出品"))?;
    let semi_name = wo_svc
        .get_product_name(&mut conn, semi_pid)
        .await?
        .unwrap_or_else(|| format!("#{}", semi_pid));
    // 供应商下拉
    let suppliers = state
        .supplier_service()
        .list(
            &service_ctx,
            &mut conn,
            SupplierQuery {
                name: None,
                status: None,
                category: None,
            },
            PageParams::new(1, 200),
        )
        .await?;
    // 仓库下拉（virtual=供应商虚拟仓 / source=原料仓）
    let warehouses = state
        .warehouse_service()
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
        .await?;
    Ok(Html(
        render_osa_create_form(
            &batch,
            path.routing_id,
            routing.process_name.as_str(),
            &semi_name,
            &suppliers.items,
            &warehouses.items,
        )
        .into_string(),
    ))
}

/// 渲染创建委外单表单片段（进 #osa-drawer-slot）。
fn render_osa_create_form(
    batch: &ProductionBatch,
    routing_id: i64,
    process_name: &str,
    semi_name: &str,
    suppliers: &[Supplier],
    warehouses: &[Warehouse],
) -> Markup {
    html! {
        div class="mt-4 border border-purple/40 rounded-md p-4 bg-purple-bg" {
            div class="flex items-center gap-2 mb-3" {
                span class="inline-block w-1.5 h-1.5 rounded-full bg-purple" {}
                span class="text-sm font-semibold text-fg" { "创建工序委外单" }
            }
            // 只读信息：工序 / 本道半成品 / 计划量
            div class="grid grid-cols-3 gap-3 mb-4 text-xs" {
                div {
                    div class="text-muted mb-0.5" { "工序" }
                    div class="text-fg font-medium" { (process_name) }
                }
                div {
                    div class="text-muted mb-0.5" { "本道半成品" }
                    div class="text-fg font-medium" { (semi_name) }
                }
                div {
                    div class="text-muted mb-0.5" { "计划量" }
                    div class="text-fg font-medium font-mono" { (fmt_qty(batch.batch_qty)) " 件" }
                }
            }
            form hx-post=(WcBatchOsaCreatePath { batch_id: batch.id, routing_id }.to_string())
                hx-target="#batch-drawer-body" hx-swap="innerHTML" {
                div class="grid grid-cols-2 gap-3 mb-3" {
                    div {
                        label class="block text-xs text-fg-2 mb-1 whitespace-nowrap" { "供应商 " span class="text-danger" { "*" } }
                        select name="supplier_id" required
                            class="w-full px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent" {
                            option value="" { "请选择供应商" }
                            @for s in suppliers {
                                option value=(s.id) { (s.name) }
                            }
                        }
                    }
                    div {
                        label class="block text-xs text-fg-2 mb-1 whitespace-nowrap" { "加工单价 " span class="text-danger" { "*" } }
                        input type="number" step="any" min="0" name="unit_price" required placeholder="0.00"
                            class="w-full px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent font-mono";
                    }
                    div {
                        label class="block text-xs text-fg-2 mb-1 whitespace-nowrap" { "供应商虚拟仓" }
                        select name="virtual_warehouse_id"
                            class="w-full px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent" {
                            option value="0" { "（不指定）" }
                            @for w in warehouses {
                                @if w.is_virtual {
                                    option value=(w.id) { (w.name) "（虚拟）" }
                                }
                            }
                        }
                    }
                    div {
                        label class="block text-xs text-fg-2 mb-1 whitespace-nowrap" { "原料仓" }
                        select name="source_warehouse_id"
                            class="w-full px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent" {
                            option value="0" { "（不指定）" }
                            @for w in warehouses {
                                @if !w.is_virtual {
                                    option value=(w.id) { (w.name) }
                                }
                            }
                        }
                    }
                }
                div class="text-xs text-muted bg-surface rounded-sm px-3 py-2 mb-3" {
                    "提交后创建委外单（草稿），发料明细按本道半成品 BOM 自动展开。"
                }
                div class="flex justify-end gap-2" {
                    button type="button"
                        class="px-3 py-1.5 rounded-sm bg-white text-fg-2 border border-border text-xs cursor-pointer hover:bg-surface"
                        _="on click empty #osa-drawer-slot" { "取消" }
                    button type="submit"
                        class="px-3 py-1.5 rounded-sm bg-purple text-white border-none text-xs font-medium cursor-pointer hover:opacity-90" {
                        "创建委外单"
                    }
                }
            }
        }
    }
}

/// 批次工序委外：创建委外单（POST）。
///
/// 防重复（同批次同工序已有活跃委外单）；物料明细从本道半成品 BOM 直接子级自动展开
/// （child.quantity × batch.batch_qty）。成功后刷新 batch drawer body（动作位 OsaCreate→OsaDraft）
/// + 广播 woChanged（刷新 demand card）。
#[require_permission("WORK_ORDER", "update")]
pub async fn osa_create(
    path: WcBatchOsaCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<OsaCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let batch_svc = state.production_batch_service();
    let wo_svc = state.work_order_service();
    let om_svc = state.outsourcing_order_service();
    let batch = batch_svc
        .find_by_id(&service_ctx, &mut tx, path.batch_id)
        .await?;
    let routings = batch_svc
        .list_routings(&service_ctx, &mut tx, batch.work_order_id)
        .await
        .unwrap_or_default();
    let routing = routings
        .iter()
        .find(|r| r.id == path.routing_id)
        .ok_or_else(|| DomainError::not_found("WorkOrderRouting"))?;
    // 防重复：同批次同工序已有活跃委外单
    let existing = om_svc
        .find_active_for_routing(
            &service_ctx,
            &mut tx,
            batch.work_order_id,
            path.routing_id,
            Some(batch.id),
        )
        .await?;
    if !existing.is_empty() {
        return Err(DomainError::business_rule("该工序已有活跃委外单").into());
    }
    let semi_pid = routing
        .product_id
        .ok_or_else(|| DomainError::business_rule("委外工序未设置产出品"))?;
    let unit_price = form
        .unit_price
        .parse::<Decimal>()
        .map_err(|_| DomainError::validation("加工单价格式错误"))?;
    if unit_price <= Decimal::ZERO {
        return Err(DomainError::validation("加工单价必须大于 0").into());
    }
    // 物料明细：本道半成品的 BOM 直接子级展开（用量 × 批次量）
    let order = wo_svc
        .find_by_id(&service_ctx, &mut tx, batch.work_order_id)
        .await?;
    let finished = state
        .product_service()
        .get(&service_ctx, &mut tx, order.product_id)
        .await?;
    let bom_query = new_bom_query_service(state.pool.clone());
    let materials: Vec<OutsourcingMaterialItem> = match bom_query
        .find_published_bom_by_product_code(&service_ctx, &mut tx, &finished.product_code)
        .await?
    {
        Some(bom_id) => bom_query
            .get_direct_children_by_product(&service_ctx, &mut tx, bom_id, semi_pid)
            .await?
            .into_iter()
            .map(|child| OutsourcingMaterialItem {
                product_id: child.product_id,
                planned_qty: child.quantity * batch.batch_qty,
                unit_cost: None,
            })
            .collect(),
        None => Vec::new(),
    };
    let process_name = routing.process_name.clone();
    let req = CreateOutsourcingOrderReq {
        work_order_id: Some(batch.work_order_id),
        routing_id: Some(path.routing_id),
        process_name: Some(process_name),
        supplier_id: form.supplier_id,
        product_id: semi_pid,
        outsourcing_type: OutsourcingType::Process,
        planned_qty: batch.batch_qty,
        unit_price,
        scheduled_date: None,
        virtual_warehouse_id: form.virtual_warehouse_id,
        source_warehouse_id: form.source_warehouse_id,
        batch_id: Some(batch.id),
        remark: Some("工序委外".to_string()),
        materials,
    };
    om_svc.create(&service_ctx, &mut tx, req, None).await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    crate::toast::add_toast(
        service_ctx.operator_id,
        "委外单已创建",
        crate::toast::ToastType::Success,
    );
    let mut conn = state
        .pool
        .acquire()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let body = load_batch_drawer_html(&state, &service_ctx, &mut conn, path.batch_id).await?;
    Ok(([("HX-Trigger", "woChanged, showToast")], Html(body)))
}

/// 批次工序委外：发料（POST，om send 发料到供应商虚拟仓）。
///
/// 取活跃委外单 → send → 刷新 drawer body（动作位 OsaDraft→OsaSent）+ woChanged。
#[require_permission("WORK_ORDER", "update")]
pub async fn osa_send(path: WcBatchOsaSendPath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let batch_svc = state.production_batch_service();
    let om_svc = state.outsourcing_order_service();
    let batch = batch_svc
        .find_by_id(&service_ctx, &mut tx, path.batch_id)
        .await?;
    let order = om_svc
        .find_active_for_routing(
            &service_ctx,
            &mut tx,
            batch.work_order_id,
            path.routing_id,
            Some(batch.id),
        )
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| DomainError::not_found("活跃委外单"))?;
    om_svc
        .send(
            &service_ctx,
            &mut tx,
            SendOutsourcingReq {
                id: order.id,
                expected_version: order.version,
                remark: None,
            },
        )
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    crate::toast::add_toast(
        service_ctx.operator_id,
        "已发料给供应商",
        crate::toast::ToastType::Success,
    );
    let mut conn = state
        .pool
        .acquire()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let body = load_batch_drawer_html(&state, &service_ctx, &mut conn, path.batch_id).await?;
    Ok(([("HX-Trigger", "woChanged, showToast")], Html(body)))
}

/// 批次工序委外：收货（POST，om receive 产出半成品入 WIP-SHOP + 立加工费 AP）。
///
/// 收货入车间在制仓(WIP-SHOP)，衔接下一道倒冲；om receive 发 OutsourcingReceived 事件 →
/// EventHandler 回写工序进度 → 动作位 OsaSent→OsaDone。
#[require_permission("WORK_ORDER", "update")]
pub async fn osa_receive(
    path: WcBatchOsaReceivePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let batch_svc = state.production_batch_service();
    let om_svc = state.outsourcing_order_service();
    let batch = batch_svc
        .find_by_id(&service_ctx, &mut tx, path.batch_id)
        .await?;
    let order = om_svc
        .find_active_for_routing(
            &service_ctx,
            &mut tx,
            batch.work_order_id,
            path.routing_id,
            Some(batch.id),
        )
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| DomainError::not_found("活跃委外单"))?;
    // 委外收货入车间在制仓(WIP-SHOP)，衔接下一道倒冲
    let wip_wh =
        abt_core::mes::production_batch::implt::resolve_wip_warehouse_id(&mut tx).await?;
    let card_sn = batch.card_sn.clone();
    om_svc
        .receive(
            &service_ctx,
            &mut tx,
            ReceiveOutsourcingReq {
                id: order.id,
                expected_version: order.version,
                received_qty: order.planned_qty,
                warehouse_id: Some(wip_wh),
                iqc_passed_qty: Some(order.planned_qty),
                remark: Some(format!("批次{}委外收货", card_sn)),
            },
        )
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    crate::toast::add_toast(
        service_ctx.operator_id,
        "委外收货完成",
        crate::toast::ToastType::Success,
    );
    let mut conn = state
        .pool
        .acquire()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let body = load_batch_drawer_html(&state, &service_ctx, &mut conn, path.batch_id).await?;
    Ok(([("HX-Trigger", "woChanged, showToast")], Html(body)))
}

// =============================================================================
// 渲染辅助
// =============================================================================

/// 锚点条：待办总数 + 待下达 / 生产中 chip（点击锚定对应 card）。
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
