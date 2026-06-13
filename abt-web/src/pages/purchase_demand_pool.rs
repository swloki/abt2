use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup, PreEscaped};
use serde::Deserialize;

use abt_core::purchase::demand_handler::{
    DemandPoolQuery, DemandSummary, MaterialAggQuery, MaterialAggSummary,
    PurchaseDemandService,
};
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_plan::PlanDetailPath;
use crate::routes::order::OrderDetailPath;
use crate::routes::purchase_demand_pool::*;
use crate::routes::purchase_order::PODetailPath;
use crate::utils::{fmt_qty, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct DemandPoolQueryParams {
    pub view: Option<String>,
    pub keyword: Option<String>,
    pub date_filter: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DemandRowQueryParams {
    pub product_id: i64,
}

// ── Helpers ──

fn demand_status_label(s: i16) -> (&'static str, &'static str) {
    match s {
        1 => ("待处理", "status-draft"),
        2 => ("处理中", "status-confirmed"),
        3 => ("已完成", "status-success"),
        4 => ("已取消", "status-cancelled"),
        _ => ("未知", "status-draft"),
    }
}

fn priority_chip(p: i32) -> (&'static str, &'static str) {
    match p {
        1 => ("紧急", "background:#fee2e2;color:#dc2626"),
        2 => ("高", "background:#fef3c7;color:#d97706"),
        3 => ("中", "background:#f1f5f9;color:#475569"),
        4 => ("低", "background:#f1f5f9;color:#94a3b8"),
        _ => ("—", "background:#f1f5f9;color:#94a3b8"),
    }
}

fn urgency_hint(earliest: Option<chrono::NaiveDate>) -> Option<(String, &'static str)> {
    earliest.and_then(|d| {
        let today = chrono::Local::now().date_naive();
        let diff = (d - today).num_days();
        if diff < 0 {
            Some((format!("已逾期{}天", diff.abs()), "text-danger"))
        } else if diff == 0 {
            Some(("今天到期".to_string(), "text-danger"))
        } else if diff <= 3 {
            Some((format!("{}天后到期", diff), "text-danger"))
        } else if diff <= 7 {
            Some((format!("{}天后到期", diff), "text-warn"))
        } else {
            None
        }
    })
}

fn material_icon(product_id: i64) -> (String, String, Markup) {
    let variant = (product_id % 4) as u8;
    match variant {
        0 => (
            "#ede9fe".into(),
            "#7c3aed".into(),
            icon::tool_icon(""),
        ),
        1 => (
            "#dbeafe".into(),
            "var(--accent)".into(),
            icon::clipboard_document_icon(""),
        ),
        2 => (
            "#fef3c7".into(),
            "var(--warn)".into(),
            icon::cube_icon(""),
        ),
        _ => (
            "#dcfce7".into(),
            "var(--success)".into(),
            icon::activity_icon(""),
        ),
    }
}

fn material_query_string(keyword: Option<&str>, date_filter: Option<&str>) -> String {
    let mut q = vec![];
    if let Some(kw) = keyword
        && !kw.is_empty()
    {
        q.push(format!("keyword={kw}"));
    }
    if let Some(df) = date_filter
        && !df.is_empty()
    {
        q.push(format!("date_filter={df}"));
    }
    q.join("&")
}

fn detail_query_string(keyword: Option<&str>, date_filter: Option<&str>) -> String {
    let mut q = vec!["view=detail".to_string()];
    if let Some(kw) = keyword
        && !kw.is_empty()
    {
        q.push(format!("keyword={kw}"));
    }
    if let Some(df) = date_filter
        && !df.is_empty()
    {
        q.push(format!("date_filter={df}"));
    }
    q.join("&")
}

// ── Handlers ──

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_demand_pool_list(
    _path: PurchaseDemandPoolListPath,
    ctx: RequestContext,
    Query(params): Query<DemandPoolQueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext {
        claims,
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.purchase_demand_service();

    let page_num = params.page.unwrap_or(1);
    let page_size = 20;
    let view_mode = params.view.as_deref().unwrap_or("material");

    // Parse date_filter into date range
    let (date_start, date_end) = match params.date_filter.as_deref() {
        Some("3days") => {
            let today = chrono::Local::now().date_naive();
            (None, Some(today + chrono::TimeDelta::days(3)))
        }
        Some("7days") => {
            let today = chrono::Local::now().date_naive();
            (None, Some(today + chrono::TimeDelta::days(7)))
        }
        Some("30days") => {
            let today = chrono::Local::now().date_naive();
            (None, Some(today + chrono::TimeDelta::days(30)))
        }
        Some("overdue") => {
            let today = chrono::Local::now().date_naive();
            (None, Some(today))
        }
        _ => (None, None),
    };

    // Fetch stats for mini cards (lightweight queries)
    let pending_count = svc
        .list_pending_demands(
            &service_ctx,
            &mut conn,
            DemandPoolQuery {
                status: Some(1),
                product_id: None,
                order_id: None,
                keyword: params.keyword.clone(),
                required_date_start: date_start,
                required_date_end: date_end,
            },
            PageParams::new(1, 1),
        )
        .await
        .map(|r| r.total)
        .unwrap_or(0);

    let material_count = svc
        .list_material_aggregated(
            &service_ctx,
            &mut conn,
            MaterialAggQuery {
                product_id: None,
                keyword: params.keyword.clone(),
                required_date_start: date_start,
                required_date_end: date_end,
            },
            PageParams::new(1, 1),
        )
        .await
        .map(|r| r.total)
        .unwrap_or(0);

    let stats = Stats {
        pending_count,
        material_count,
    };

    let content = if view_mode == "detail" {
        let result = svc
            .list_pending_demands(
                &service_ctx,
                &mut conn,
                DemandPoolQuery {
                    status: None,
                    product_id: None,
                    order_id: None,
                    keyword: params.keyword.clone(),
                    required_date_start: date_start,
                    required_date_end: date_end,
                },
                PageParams::new(page_num, page_size),
            )
            .await?;
        demand_pool_detail_page(&stats, &result, &params)
    } else {
        let result = svc
            .list_material_aggregated(
                &service_ctx,
                &mut conn,
                MaterialAggQuery {
                    product_id: None,
                    keyword: params.keyword.clone(),
                    required_date_start: date_start,
                    required_date_end: date_end,
                },
                PageParams::new(page_num, page_size),
            )
            .await?;
        demand_pool_material_page(&stats, &result, &params)
    };

    let page_html = admin_page(
        is_htmx,
        "采购需求池",
        &claims,
        "purchase",
        PurchaseDemandPoolListPath::PATH,
        "采购管理",
        Some("采购需求池"),
        content,
        &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_demand_rows(
    _path: PurchaseDemandRowsPath,
    ctx: RequestContext,
    Query(params): Query<DemandRowQueryParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.purchase_demand_service();

    let result = svc
        .list_pending_demands(
            &service_ctx,
            &mut conn,
            DemandPoolQuery {
                status: None,
                product_id: Some(params.product_id),
                order_id: None,
                ..Default::default()
            },
            PageParams::new(1, 100),
        )
        .await?;

    Ok(Html(demand_expand_rows(&result.items).into_string()))
}

// ── Data holders ──

struct Stats {
    pending_count: u64,
    material_count: u64,
}

// ── Page rendering ──

fn demand_pool_material_page(
    stats: &Stats,
    result: &abt_core::shared::types::PaginatedResult<MaterialAggSummary>,
    params: &DemandPoolQueryParams,
) -> Markup {
    html! {
        div {
            (page_header())
            (stat_mini_cards(stats))
            div id="demand-pool-data-card" {
                (view_toggle_and_filter("material", params))
                (material_table_fragment(result, params))
            }
            (batch_action_bar())
        }
    }
}

fn demand_pool_detail_page(
    stats: &Stats,
    result: &abt_core::shared::types::PaginatedResult<DemandSummary>,
    params: &DemandPoolQueryParams,
) -> Markup {
    html! {
        div {
            (page_header())
            (stat_mini_cards(stats))
            div id="demand-pool-data-card" {
                (view_toggle_and_filter("detail", params))
                (detail_table_fragment(result, params))
            }
            (batch_action_bar())
        }
    }
}

fn page_header() -> Markup {
    html! {
        div class="page-header" {
            div {
                h1 class="page-title" { "采购需求池" }
                p style="font-size:var(--text-sm);color:var(--muted);margin-top:var(--space-1)" {
                    "销售订单确认后产生的外购需求，按物料聚合展示。可选择需求创建采购订单草稿。"
                }
            }
            div class="page-actions" {
                button class="btn btn-default"
                    hx-get=(PurchaseDemandPoolListPath::PATH)
                    hx-target="#demand-pool-data-card"
                    hx-select="#demand-pool-data-card"
                    hx-swap="outerHTML" {
                    (icon::refresh_icon("w-4 h-4"))
                    "刷新"
                }
            }
        }
    }
}

fn view_toggle_and_filter(active: &str, params: &DemandPoolQueryParams) -> Markup {
    let is_material = active == "material";
    let material_cls = if is_material { "view-toggle-btn active" } else { "view-toggle-btn" };
    let detail_cls = if is_material { "view-toggle-btn" } else { "view-toggle-btn active" };
    let keyword = params.keyword.as_deref().unwrap_or("");
    let date_filter_val = params.date_filter.as_deref().unwrap_or("");

    html! {
        div class="view-toggle-bar" {
            div class="view-toggle" {
                a class=(material_cls)
                    hx-get=(PurchaseDemandPoolListPath::PATH)
                    hx-vals="{\"view\":\"material\"}"
                    hx-target="#demand-pool-data-card"
                    hx-select="#demand-pool-data-card"
                    hx-swap="outerHTML"
                    hx-push-url="true"
                    hx-include="#demand-pool-filter-form" {
                    (icon::grid_4_icon("w-4 h-4"))
                    "物料汇总"
                }
                a class=(detail_cls)
                    hx-get=(PurchaseDemandPoolListPath::PATH)
                    hx-vals="{\"view\":\"detail\"}"
                    hx-target="#demand-pool-data-card"
                    hx-select="#demand-pool-data-card"
                    hx-swap="outerHTML"
                    hx-push-url="true"
                    hx-include="#demand-pool-filter-form" {
                    (icon::rows_icon("w-4 h-4"))
                    "订单行明细"
                }
            }

            form class="filter-bar"
                hx-get=(PurchaseDemandPoolListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#demand-pool-data-card"
                hx-select="#demand-pool-data-card"
                hx-swap="outerHTML"
                hx-push-url="true" {
                input type="hidden" name="view" value=(active);
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        placeholder="搜索物料名称、编码…"
                        value=(keyword);
                }
                select class="filter-select" name="date_filter" {
                    option value="" selected[date_filter_val.is_empty()] { "全部需求日期" }
                    option value="3days" selected[date_filter_val == "3days"] { "近3天到期" }
                    option value="7days" selected[date_filter_val == "7days"] { "近7天到期" }
                    option value="30days" selected[date_filter_val == "30days"] { "近30天到期" }
                    option value="overdue" selected[date_filter_val == "overdue"] { "已逾期" }
                }
            }

            form id="demand-pool-filter-form" style="display:none;" {
                input type="hidden" name="keyword" value=(keyword);
                input type="hidden" name="date_filter" value=(date_filter_val);
            }
        }
    }
}

fn stat_mini_cards(stats: &Stats) -> Markup {
    html! {
        div class="stat-mini-grid" {
            div class="stat-mini" {
                div class="stat-mini-icon" style="background:#ede9fe;color:#7c3aed;" {
                    (icon::clipboard_list_icon(""))
                }
                div {
                    div class="stat-mini-value" { (stats.pending_count) }
                    div class="stat-mini-label" { "待处理需求" }
                }
            }
            div class="stat-mini" {
                div class="stat-mini-icon" style="background:#dbeafe;color:var(--accent);" {
                    (icon::cube_icon(""))
                }
                div {
                    div class="stat-mini-value" { (stats.material_count) }
                    div class="stat-mini-label" { "涉及物料" }
                }
            }
            div class="stat-mini" {
                div class="stat-mini-icon" style="background:#dcfce7;color:var(--success);" {
                    (icon::check_circle_icon(""))
                }
                div {
                    div class="stat-mini-value" { "—" }
                    div class="stat-mini-label" { "处理中" }
                }
            }
            div class="stat-mini" {
                div class="stat-mini-icon" style="background:#fef3c7;color:var(--warn);" {
                    (icon::clock_icon(""))
                }
                div {
                    div class="stat-mini-value" { "—" }
                    div class="stat-mini-label" { "近3日到期" }
                }
            }
        }
    }
}

// ── View Toggle ──

fn batch_action_bar() -> Markup {
    html! {
        // ── Batch Action Bar ──
        div class="batch-bar" id="batchBar" {
            span { "已选择 " span class="batch-count" id="batchCount" { "0" } " 条需求" }
            button class="btn btn-sm" type="button" id="batchCreateBtn"
                onclick=(format!("window.location.href='{}'", PurchaseDemandPoolCreatePath::PATH)) {
                "创建采购单"
            }
            button class="btn btn-sm btn-ghost" type="button" {
                "清除选择"
                (PreEscaped(r#"<script>me().on('click',function(){
                    any('input[type=checkbox].demand-cb').forEach(function(c){
                        if(!c.disabled){c.checked=false;}
                    });
                    me('#batchBar').classRemove('show');
                })</script>"#))
            }
        }

        // ── Global checkbox + batch bar logic ──
        (PreEscaped(r#"<script>
            document.addEventListener('change',function(e){
                if(e.target.type==='checkbox'&&e.target.classList.contains('demand-cb')){
                    var tr=e.target.closest('tr');
                    if(tr){
                        if(e.target.checked){tr.classList.add('demand-row-selected');}
                        else{tr.classList.remove('demand-row-selected');}
                    }
                    updateBatchBar();
                }
            });
            function updateBatchBar(){
                var checked=document.querySelectorAll('input[type=checkbox].demand-cb:checked:not([disabled])');
                var count=checked.length;
                var bar=document.getElementById('batchBar');
                if(count>0){
                    var ids=[];
                    checked.forEach(function(c){ids.push(c.value);});
                    bar.classList.add('show');
                    document.getElementById('batchCount').textContent=count;
                    document.getElementById('batchCreateBtn').href='/admin/purchase/demand-pool/create?demand_ids='+ids.join(',');
                }else{
                    bar.classList.remove('show');
                }
            }
        </script>"#))
    }
}

// ── Material Aggregated View ──

fn material_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<MaterialAggSummary>,
    params: &DemandPoolQueryParams,
) -> Markup {
    let qs = material_query_string(params.keyword.as_deref(), params.date_filter.as_deref());

    html! {
        div class="data-card" id="materialView" {
            (material_table_header())
            @for m in &result.items {
                (material_row(m))
            }
            @if result.items.is_empty() {
                div style="text-align:center;padding:var(--space-8);color:var(--muted);" {
                    "暂无待处理需求"
                }
            }
            (pagination(
                PurchaseDemandPoolListPath::PATH,
                &qs,
                result.total,
                result.page,
                result.total_pages,
            ))
        }
    }
}

fn material_table_header() -> Markup {
    html! {
        div style="padding:var(--space-3) var(--space-6);background:var(--surface-raised);border-bottom:1px solid var(--border-soft);display:flex;align-items:center;gap:var(--space-8);font-size:12px;color:var(--muted);font-weight:600;text-transform:uppercase;letter-spacing:0.04em;" {
            div style="flex:1;" { "物料信息" }
            div style="width:100px;text-align:center;" { "总需求量" }
            div style="width:80px;text-align:center;" { "涉及订单" }
            div style="width:160px;text-align:center;" { "需求日期范围" }
            div style="width:120px;text-align:center;" { "操作" }
        }
    }
}

fn material_row(m: &MaterialAggSummary) -> Markup {
    let earliest = m
        .earliest_required_date
        .map(|d| d.format("%m/%d").to_string())
        .unwrap_or_else(|| "—".into());
    let latest = m
        .latest_required_date
        .map(|d| d.format("%m/%d").to_string())
        .unwrap_or_else(|| "—".into());
    let date_range = format!("{earliest} → {latest}");
    let hint = urgency_hint(m.earliest_required_date);
    let pid = m.product_id;
    let (icon_bg, icon_color, icon_svg) = material_icon(pid);

    html! {
        div class="material-row" {
            // Click row to expand/collapse detail
            (PreEscaped(format!(r#"<script>me().on('click',function(e){{
                if(e.target.closest('button')||e.target.closest('form'))return;
                var el=document.getElementById('expand-mat-{pid}');
                el.classList.toggle('open');
            }})</script>"#)))

            div class="material-info" {
                div class="material-icon" style=(format!("background:{icon_bg};color:{icon_color}")) {
                    (icon_svg)
                    (icon::box_icon("w-5 h-5"))
                }
                div {
                    div class="material-name" { (m.product_name) }
                    div class="material-code" { (m.product_code) }
                }
            }

            div class="material-stat" {
                div class="material-stat-value" { (fmt_qty(m.total_demand_qty)) }
                div class="material-stat-label" { "总需求量" }
            }

            div class="material-stat" {
                div class="material-stat-value" { (m.demand_count) }
                div class="material-stat-label" { "涉及订单" }
            }

            div class="material-stat material-stat-date" {
                div class="date-range-text" { (date_range) }
                @if let Some((hint_text, cls)) = &hint {
                    div class=(format!("urgency-hint {cls}")) { (hint_text) }
                }
            }

            div class="material-actions" {
                form method="get" action=(PurchaseDemandPoolCreatePath::PATH)
                    style="display:inline"
                    onclick="event.stopPropagation()" {
                    input type="hidden" name="product_id" value=(pid) {}
                    button type="submit" class="btn btn-primary btn-sm" { "创建采购单" }
                }
                button class="btn btn-default btn-sm"
                    onclick="event.stopPropagation()"
                    hx-get=(format!("/admin/purchase/demand-pool/demand-rows?product_id={pid}"))
                    hx-target=(format!("#expand-tbody-{pid}"))
                    hx-swap="innerHTML"
                {
                    "展开明细"
                }
            }
        }

        // ── Expandable demand detail ──
        div class="demand-expand" id=(format!("expand-mat-{pid}")) {
            div class="demand-expand-inner" {
                table class="data-table" style="font-size:13px;" {
                    thead {
                        tr {
                            th style="width:40px;" { input type="checkbox" title="全选"; }
                            th { "需求ID" }
                            th { "来源订单" }
                            th class="num-right" { "需求数量" }
                            th { "需求日期" }
                            th { "优先级" }
                            th { "状态" }
                        }
                    }
                    tbody id=(format!("expand-tbody-{pid}")) {
                        tr {
                            td colspan="7" style="text-align:center;padding:var(--space-3);color:var(--muted);" {
                                "点击「展开明细」加载..."
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Demand Expand Rows (HTMX fragment) ──

fn demand_expand_rows(demands: &[DemandSummary]) -> Markup {
    html! {
        @if demands.is_empty() {
            tr {
                td colspan="7" style="text-align:center;padding:var(--space-3);color:var(--muted);" {
                    "暂无需求明细"
                }
            }
        }
        @for d in demands {
            (demand_expand_row(d))
        }
    }
}

fn demand_expand_row(d: &DemandSummary) -> Markup {
    let (status_text, status_class) = demand_status_label(d.demand_status);
    let (pri_text, pri_style) = priority_chip(d.priority);
    let req_date = d
        .required_date
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "—".into());

    html! {
        tr {
            td {
                input type="checkbox" class="demand-cb" value=(d.id);
            }
            td class="mono" style="font-size:12px;" { (d.id) }
            td {
                a class="link-cell" href=(OrderDetailPath { id: d.order_id }.to_string()) { (d.order_no) }
            }
            td class="num-right mono" { (fmt_qty(d.quantity)) }
            td class="mono" { (req_date) }
            td {
                span class="tag-chip" style=(pri_style) { (pri_text) }
            }
            td {
                span class=(format!("status-pill {status_class}")) style="font-size:11px;padding:2px 8px;" { (status_text) }
            }
        }
    }
}

// ── Detail View ──

fn detail_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<DemandSummary>,
    params: &DemandPoolQueryParams,
) -> Markup {
    let qs = detail_query_string(params.keyword.as_deref(), params.date_filter.as_deref());

    html! {
        div class="data-card" id="detailView" {
            div class="data-card-scroll" {
                table class="data-table" {
                    thead {
                        tr {
                            th style="width:40px;" { input type="checkbox" title="全选"; }
                            th { "需求ID" }
                            th { "产品编码" }
                            th { "产品名称" }
                            th { "来源订单" }
                            th class="num-right" { "需求数量" }
                            th { "需求日期" }
                            th { "优先级" }
                            th { "状态" }
                            th { "关联单据" }
                            th { "操作" }
                        }
                    }
                    tbody {
                        @for d in &result.items {
                            (detail_row(d))
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="11" style="text-align:center;padding:var(--space-8);color:var(--muted);" {
                                    "暂无需求数据"
                                }
                            }
                        }
                    }
                }
            }
            (pagination(
                PurchaseDemandPoolListPath::PATH,
                &qs,
                result.total,
                result.page,
                result.total_pages,
            ))
        }
    }
}

fn detail_row(d: &DemandSummary) -> Markup {
    let (status_text, status_class) = demand_status_label(d.demand_status);
    let (pri_text, pri_style) = priority_chip(d.priority);
    let req_date = d
        .required_date
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "—".into());
    let is_pending = d.demand_status == 1;

    html! {
        tr {
            td {
                @if is_pending {
                    input type="checkbox" class="demand-cb" value=(d.id);
                } @else {
                    input type="checkbox" class="demand-cb" disabled;
                }
            }
            td class="mono" style="font-size:12px;" { (d.id) }
            td class="mono" { (d.product_code) }
            td { (d.product_name) }
            td {
                a class="link-cell" href=(OrderDetailPath { id: d.order_id }.to_string()) { (d.order_no) }
            }
            td class="num-right mono" { (fmt_qty(d.quantity)) }
            td class="mono" { (req_date) }
            td {
                span class="tag-chip" style=(pri_style) { (pri_text) }
            }
            td {
                span class=(format!("status-pill {status_class}")) style="font-size:11px;padding:2px 8px;" { (status_text) }
            }
            td class="mono" {
                @if let (Some(doc_type), Some(doc_id)) = (d.target_doc_type, d.target_doc_id) {
                    @if doc_type == 7 {
                        a class="link-cell" href=(PODetailPath { id: doc_id }.to_string()) {
                            "PO-" (doc_id)
                        }
                    } @else if doc_type == 12 {
                        a class="link-cell" href=(PlanDetailPath { id: doc_id }.to_string()) {
                            "PP-" (doc_id)
                        }
                    } @else {
                        "—"
                    }
                } @else {
                    span class="text-muted" { "—" }
                }
            }
            td {
                @if is_pending {
                    form method="get" action=(PurchaseDemandPoolCreatePath::PATH) style="display:inline" {
                        input type="hidden" name="product_id" value=(d.product_id) {}
                        button type="submit" class="btn btn-primary btn-sm" { "创建" }
                    }
                } @else {
                    span class="text-muted text-sm" { "—" }
                }
            }
        }
    }
}
