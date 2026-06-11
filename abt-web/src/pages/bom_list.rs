use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use std::collections::HashMap;
use abt_core::master_data::bom::model::*;
use abt_core::shared::identity::UserService;
use abt_core::master_data::bom::{BomCategoryService, BomCommandService, BomQueryService};
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::export_button::{self, ExportItem};
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs, TabItem};
use crate::layout::page::admin_page;
use crate::routes::bom::{
    BomCostDrawerPath, BomCreatePath, BomDeletePath, BomDetailPath, BomListPath, BomLaborCostDrawerPath, BomTablePath,
};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct BomQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub category_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub category_name: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub date_from: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub date_to: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}
// ── Handlers ──
#[require_permission("BOM", "read")]
pub async fn get_bom_list(
    _path: BomListPath,
    ctx: RequestContext,
    Query(mut params): Query<BomQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let can_view_cost = ctx.has_permission("COST", "read").await;
    let can_view_labor_cost = ctx.has_permission("LABOR_COST", "read").await;
    let can_create = ctx.has_permission("BOM", "create").await;
    let can_delete = ctx.has_permission("BOM", "delete").await;
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    resolve_category_name(&state, &service_ctx, &mut conn, &mut params).await;
    let svc = state.bom_query_service();
    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);
    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;
    let (cat_map, cat_list) = load_categories(&state, &service_ctx, &mut conn).await;
    let user_map = resolve_creator_names(&state.user_service(), &service_ctx, &mut conn, &result.items).await;
    let content = bom_list_page(&result, &params, &cat_map, &cat_list, &user_map, can_view_labor_cost, can_view_cost, can_create, can_delete);
    let current_path = match &params.category_name {
        Some(cn) if !cn.is_empty() => format!("{}?category_name={}", BomListPath::PATH, cn),
        _ => BomListPath::PATH.to_string(),
    };
    let page_name = params.category_name.as_deref().unwrap_or("BOM管理");
    let page_html = admin_page(
        is_htmx, "BOM管理", &claims, "md", &current_path,
        "主数据管理", Some(page_name), content, &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}
#[require_permission("BOM", "read")]
pub async fn get_bom_table(
    ctx: RequestContext,
    Query(mut params): Query<BomQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let can_view_cost = ctx.has_permission("COST", "read").await;
    let can_view_labor_cost = ctx.has_permission("LABOR_COST", "read").await;
    let can_delete = ctx.has_permission("BOM", "delete").await;
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    resolve_category_name(&state, &service_ctx, &mut conn, &mut params).await;
    let svc = state.bom_query_service();
    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);
    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;
    let (cat_map, cat_list) = load_categories(&state, &service_ctx, &mut conn).await;
    let user_map = resolve_creator_names(&state.user_service(), &service_ctx, &mut conn, &result.items).await;
    Ok(Html(bom_table_fragment(&result, &params, &cat_map, &cat_list, &user_map, can_view_labor_cost, can_view_cost, can_delete).into_string()))
}
#[require_permission("BOM", "delete")]
pub async fn delete_bom(
    path: BomDeletePath,
    ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.bom_command_service();
    svc.delete(&service_ctx, &mut conn, path.id).await?;
    Ok(([("HX-Redirect", BomListPath::PATH)], Html(String::new())))
}

use crate::state::AppState;
use abt_core::shared::types::{PgExecutor, ServiceContext};

async fn resolve_category_name(state: &AppState, ctx: &ServiceContext, db: PgExecutor<'_>, params: &mut BomQueryParams) {
    if params.category_id.is_none() && params.category_name.is_some() {
        let cats = state.bom_category_service();
        let query = BomCategoryQuery { name: params.category_name.clone() };
        if let Ok(result) = cats.list(ctx, db, query, PageParams::new(1, 1)).await
            && let Some(cat) = result.items.first() {
                params.category_id = Some(cat.bom_category_id);
            }
    }
}
async fn load_categories(state: &AppState, ctx: &ServiceContext, db: PgExecutor<'_>) -> (HashMap<i64, String>, Vec<BomCategory>) {
    let cat_svc = state.bom_category_service();
    let cats = cat_svc.list(ctx, db, BomCategoryQuery::default(), PageParams::new(1, 200)).await
        .map(|r| r.items)
        .unwrap_or_default();
    let map: HashMap<i64, String> = cats.iter().map(|c| (c.bom_category_id, c.bom_category_name.clone())).collect();
    (map, cats)
}

async fn resolve_creator_names<S: UserService>(
    svc: &S,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    boms: &[Bom],
) -> HashMap<i64, String> {
    let ids: Vec<i64> = boms.iter().filter_map(|b| b.created_by).collect();
    if ids.is_empty() {
        return HashMap::new();
    }
    svc.get_users_by_ids(ctx, db, ids)
        .await
        .map(|users| {
            users.into_iter()
                .map(|u| (u.user.user_id, u.user.display_name.unwrap_or(u.user.username)))
                .collect()
        })
        .unwrap_or_default()
}
// ── Helpers ──
fn build_filter(params: &BomQueryParams) -> BomQuery {
    BomQuery {
        name: params.keyword.clone(),
        status: params.status.and_then(BomStatus::from_i16),
        bom_category_id: params.category_id,
        date_from: params.date_from.clone(),
        date_to: params.date_to.clone(),
    }
}
fn build_query_string(params: &BomQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref kw) = params.keyword {
        q.push(format!("keyword={kw}"));
    }
    if let Some(s) = params.status {
        q.push(format!("status={s}"));
    }
    if let Some(c) = params.category_id {
        q.push(format!("category_id={c}"));
    }
    if let Some(ref df) = params.date_from {
        q.push(format!("date_from={df}"));
    }
    if let Some(ref dt) = params.date_to {
        q.push(format!("date_to={dt}"));
    }
    q.join("&")
}

// ── Components ──

#[allow(clippy::too_many_arguments)]
fn bom_list_page(
    result: &abt_core::shared::types::PaginatedResult<Bom>,
    params: &BomQueryParams,
    cat_map: &HashMap<i64, String>,
    cat_list: &[BomCategory],
    user_map: &HashMap<i64, String>,
    can_view_labor_cost: bool,
    can_view_cost: bool,
    can_create: bool,
    can_delete: bool,
) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="page-header" {
                h1 class="page-title" { "BOM管理" }
                div class="page-actions" {
                    (export_button::export_dropdown(&[
                        ExportItem { label: "缺少人工成本BOM", export_type: "boms-no-labor-cost" },
                    ]))
                    @if can_create {
                        a href=(BomCreatePath::PATH) class="btn btn-primary" {
                            (icon::plus_icon("w-4 h-4"))
                            "新建BOM"
                        }
                    }
                }
            }
            // ── Tabs + Filter + Data Table (HTMX panel) ──
            (bom_table_fragment(result, params, cat_map, cat_list, user_map, can_view_labor_cost, can_view_cost, can_delete))

            @if can_view_cost {
                // ── Cost Drawer ──
                div id="cost-drawer" class="drawer-overlay"
                    onclick="hsRemove(null,'#cost-drawer','open')" {
                    div class="drawer-panel" style="max-width:1000px;width:100%" onclick="event.stopPropagation()" {
                        div class="drawer-head" {
                            h2 { (icon::currency_icon("w-5 h-5")) " BOM成本报告" }
                            button style="background:none;border:none;cursor:pointer;font-size:22px;color:var(--muted);padding:4px;line-height:1"
                                onclick="hsRemove(null,'#cost-drawer','open')" { "×" }
                        }
                        div class="drawer-body" {
                            div id="cost-drawer-body" {
                                div style="text-align:center;padding:40px;color:var(--muted)" { "加载中..." }
                            }
                        }
                        div class="drawer-foot" {
                            button type="button" class="btn btn-default"
                                onclick="hsRemove(null,'#cost-drawer','open')" { "关闭" }
                        }
                    }
                }
            } @else if can_view_labor_cost {
                // ── Labor Cost Drawer ──
                div id="labor-drawer" class="drawer-overlay"
                    onclick="hsRemove(null,'#labor-drawer','open')" {
                    div class="drawer-panel" style="max-width:800px;width:100%" onclick="event.stopPropagation()" {
                        div class="drawer-head" {
                            h2 { (icon::bolt_icon("w-5 h-5")) " BOM 人工成本" }
                            button style="background:none;border:none;cursor:pointer;font-size:22px;color:var(--muted);padding:4px;line-height:1"
                                onclick="hsRemove(null,'#labor-drawer','open')" { "×" }
                        }
                        div class="drawer-body" {
                            div id="labor-drawer-body" {
                                div style="text-align:center;padding:40px;color:var(--muted)" { "加载中..." }
                            }
                        }
                        div class="drawer-foot" {
                            button type="button" class="btn btn-default"
                                onclick="hsRemove(null,'#labor-drawer','open')" { "关闭" }
                        }
                    }
                }
            }
        }
        script src="/cost-drawer.js?v=20260602" {}
    }
}
#[allow(clippy::too_many_arguments)]
fn bom_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<Bom>,
    params: &BomQueryParams,
    cat_map: &HashMap<i64, String>,
    cat_list: &[BomCategory],
    user_map: &HashMap<i64, String>,
    can_view_labor_cost: bool,
    can_view_cost: bool,
    can_delete: bool,
) -> Markup {
    let query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;
    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: if active_value.is_empty() { Some(total_count) } else { None } },
        TabItem { value: "1".into(), label: "草稿", count: if active_value == "1" { Some(total_count) } else { None } },
        TabItem { value: "2".into(), label: "已发布", count: if active_value == "2" { Some(total_count) } else { None } },
    ];
    let hx_attrs = ".filter-bar input, .filter-bar select".to_string();
    html! {
        div class="bom-list-panel" {
            (status_tabs(BomTablePath::PATH, "closest .bom-list-panel", &hx_attrs, tabs, &active_value))
            // ── Filter Bar ──
            div class="filter-bar" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        placeholder="搜索BOM名称…"
                        value=(params.keyword.as_deref().unwrap_or(""))
                        hx-get=(BomTablePath::PATH)
                        hx-trigger="keyup changed delay:300ms"
                        hx-target="closest .bom-list-panel"
                        hx-swap="outerHTML";
                }
                select class="filter-select" name="status"
                    hx-get=(BomTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .bom-list-panel"
                    hx-swap="outerHTML" {
                    option value="" { "全部状态" }
                    option value="1" selected[params.status == Some(1)] { "草稿" }
                    option value="2" selected[params.status == Some(2)] { "已发布" }
                }
                select class="filter-select" name="category_id"
                    hx-get=(BomTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .bom-list-panel"
                    hx-swap="outerHTML" {
                    option value="" { "全部分类" }
                    @for cat in cat_list {
                        option value=(cat.bom_category_id) selected[params.category_id == Some(cat.bom_category_id)] { (cat.bom_category_name) }
                    }
                }
                input class="filter-date" type="date" name="date_from"
                    value=(params.date_from.as_deref().unwrap_or(""))
                    hx-get=(BomTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .bom-list-panel"
                    hx-swap="outerHTML"
                    title="开始日期" {}
                span style="color:var(--muted);font-size:var(--text-sm)" { "—" }
                input class="filter-date" type="date" name="date_to"
                    value=(params.date_to.as_deref().unwrap_or(""))
                    hx-get=(BomTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .bom-list-panel"
                    hx-swap="outerHTML"
                    title="结束日期" {}
            }
            // ── Data Table ──
            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th style="width:40%" { "BOM名称" }
                                th style="width:100px" { "BOM分类" }
                                th style="width:60px" { "版本" }
                                th style="width:80px" { "状态" }
                                th style="width:80px" { "创建者" }
                                th style="width:100px" { "更新时间" }
                                th style="width:100px" { "操作" }
                            }
                        }
                        tbody {
                            @for bom in &result.items {
                                (bom_row(bom, cat_map, user_map, can_view_labor_cost, can_view_cost, can_delete))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="7" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无BOM数据"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(BomListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

fn bom_row(bom: &Bom, cat_map: &HashMap<i64, String>, user_map: &HashMap<i64, String>, can_view_labor_cost: bool, can_view_cost: bool, can_delete: bool) -> Markup {
    let detail_path = BomDetailPath { id: bom.bom_id };
    let delete_path = BomDeletePath { id: bom.bom_id };

    let (status_label, status_class) = match bom.status {
        BomStatus::Draft => ("草稿", "status-bom-draft"),
        BomStatus::Published => ("已发布", "status-bom-published"),
    };

    html! {
        tr id=(format!("bom-row-{}", bom.bom_id)) style="cursor:pointer" {
            td onclick=(format!("location.href='{}'", detail_path)) {
                strong { (bom.bom_name) }
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                @if let Some(ref cat_id) = bom.bom_category_id {
                    @if let Some(name) = cat_map.get(cat_id) {
                        (name)
                    } @else {
                        span style="color:var(--muted)" { "—" }
                    }
                } @else {
                    span style="color:var(--muted)" { "—" }
                }
            }
            td class="mono" style="width:60px" onclick=(format!("location.href='{}'", detail_path)) {
                "v"(bom.version)
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span class=(format!("status-pill {status_class}")) { (status_label) }
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                @if let Some(creator_id) = bom.created_by {
                    @if let Some(name) = user_map.get(&creator_id) {
                        (name)
                    } @else {
                        span style="color:var(--muted)" { "—" }
                    }
                } @else {
                    span style="color:var(--muted)" { "—" }
                }
            }
            td class="mono" onclick=(format!("location.href='{}'", detail_path)) {
                @if let Some(ua) = bom.update_at {
                    (ua.format("%Y-%m-%d").to_string())
                } @else {
                    span style="color:var(--muted)" { "—" }
                }
            }
            td onclick="event.stopPropagation()" {
                div class="row-actions" {
                    a class="row-action-btn" title="查看"
                        href=(detail_path) {
                        (icon::eye_icon("w-4 h-4"))
                    }
                    button type="button" class="row-action-btn" title="导出BOM"
                        hx-post=(format!("{}/bom?bom_id={}", crate::routes::excel::EXPORT_START_PATH, bom.bom_id))
                        hx-confirm=(format!("确定要导出「{}」吗？", bom.bom_name))
                        hx-swap="none" {
                        (icon::download_icon("w-4 h-4"))
                    }
                    @if can_view_cost {
                        button type="button" class="row-action-btn" title="查看成本"
                            hx-get=(BomCostDrawerPath { id: bom.bom_id }.to_string())
                            hx-target="#cost-drawer-body"
                            hx-swap="innerHTML"
                            hx-on::after-request="hsAdd(null,'#cost-drawer','open')" {
                            (icon::currency_icon("w-4 h-4"))
                        }
                    } @else if can_view_labor_cost {
                        button type="button" class="row-action-btn" title="查看人工成本"
                            hx-get=(BomLaborCostDrawerPath { id: bom.bom_id }.to_string())
                            hx-target="#labor-drawer-body"
                            hx-swap="innerHTML"
                            hx-on::after-request="hsAdd(null,'#labor-drawer','open')" {
                            (icon::bolt_icon("w-4 h-4"))
                        }
                    }
                    @if can_delete {
                        button type="button" class="row-action-btn text-danger" title="删除"
                            hx-confirm=(format!("确认删除BOM {}？", bom.bom_name))
                            hx-post=(delete_path)
                            hx-target=(format!("#bom-row-{}", bom.bom_id))
                            hx-swap="outerHTML swap:0.5s" {
                            (icon::trash_icon("w-4 h-4"))
                        }
                    }
                }
            }
        }
    }
}
