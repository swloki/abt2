use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::supplier::model::*;
use abt_core::master_data::supplier::SupplierService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::layout::page::admin_page;
use crate::routes::supplier::{
    SupplierCreatePath, SupplierDeletePath, SupplierDetailPath, SupplierListPath, SupplierTablePath,
};
use crate::utils::{empty_as_none, RequestContext};

use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct SupplierQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub category: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("SUPPLIER", "read")]
pub async fn get_supplier_list(
    _path: SupplierListPath,
    ctx: RequestContext,
    Query(params): Query<SupplierQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_create = ctx.has_permission("SUPPLIER", "create").await;
    let can_delete = ctx.has_permission("SUPPLIER", "delete").await;
    let can_edit = ctx.has_permission("SUPPLIER", "update").await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.supplier_service();

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);

    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;


    let content = supplier_list_page(&result, &params, can_create, can_delete, can_edit);
    let page_html = admin_page(
        is_htmx,
        "供应商管理",
        &claims,
        "purchase",
        SupplierListPath::PATH,
        "主数据管理",
        Some("供应商管理"),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("SUPPLIER", "read")]
pub async fn get_supplier_table(
    ctx: RequestContext,
    Query(params): Query<SupplierQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let can_delete = ctx.has_permission("SUPPLIER", "delete").await;
    let can_edit = ctx.has_permission("SUPPLIER", "update").await;
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.supplier_service();

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);

    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    Ok(Html(supplier_table_fragment(&result, &params, can_delete, can_edit).into_string()))
}

#[require_permission("SUPPLIER", "delete")]
pub async fn delete_supplier(
    path: SupplierDeletePath,
    ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.supplier_service();

    svc.delete(&service_ctx, &mut conn, path.id).await?;

    Ok(([("HX-Redirect", SupplierListPath::PATH)], Html(String::new())))
}

// ── Helpers ──

fn build_filter(params: &SupplierQueryParams) -> SupplierQuery {
    SupplierQuery {
        name: params.keyword.clone(),
        status: params.status.and_then(SupplierStatus::from_i16),
        category: params.category.and_then(SupplierCategory::from_i16),
    }
}

// ── Components ──

fn supplier_list_page(
    result: &abt_core::shared::types::PaginatedResult<Supplier>,
    params: &SupplierQueryParams,
    can_create: bool,
    can_delete: bool,
    can_edit: bool,
) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="page-header" {
                h1 class="page-title" { "供应商管理" }
                div class="page-actions" {
                    @if can_create {
                        a class="btn btn-primary" href=(SupplierCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建供应商"
                        }
                    }
                }
            }


            // ── Tabs + Filter + Data Table (HTMX panel) ──
            (supplier_table_fragment(result, params, can_delete, can_edit))
        }
    }
}

fn supplier_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<Supplier>,
    params: &SupplierQueryParams,
    can_delete: bool,
    can_edit: bool,
) -> Markup {
    let query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "2".into(), label: "合格", count: None },
        TabItem { value: "3".into(), label: "试用期", count: None },
        TabItem { value: "1".into(), label: "潜在", count: None },
        TabItem { value: "4".into(), label: "不合格", count: None },
        TabItem { value: "5".into(), label: "黑名单", count: None },
    ];

    html! {
        div class="supplier-list-panel" {
            (status_tabs_with_param(SupplierTablePath::PATH, "#supplier-data-card", "#supplier-filter-form", tabs, &active_value, "status"))

            // ── Filter Bar ──
            form class="filter-bar filter-form" id="supplier-filter-form"
                hx-get=(SupplierTablePath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#supplier-data-card"
                hx-select="#supplier-data-card"
                hx-swap="outerHTML"
                hx-include="#supplier-filter-form" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        placeholder="搜索供应商名称、编码…"
                        value=(params.keyword.as_deref().unwrap_or(""));
                }
                select class="filter-select" name="category" {
                    option value="" { "全部类别" }
                    option value="1" selected[params.category == Some(1)] { "原材料" }
                    option value="2" selected[params.category == Some(2)] { "包装材料" }
                    option value="3" selected[params.category == Some(3)] { "外协加工" }
                    option value="4" selected[params.category == Some(4)] { "辅料" }
                    option value="5" selected[params.category == Some(5)] { "服务" }
                }
            }

            // ── Data Table ──
            div class="data-card" id="supplier-data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "供应商编码" }
                                th { "供应商名称" }
                                th { "供应类别" }
                                th { "联系人" }
                                th { "电话" }
                                th { "交货天数" }
                                th { "状态" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            @for s in &result.items {
                                (supplier_row(s, can_delete, can_edit))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="8" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无供应商数据"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(SupplierListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

fn supplier_row(s: &Supplier, can_delete: bool, can_edit: bool) -> Markup {
    let detail_path = SupplierDetailPath { id: s.id };
    let delete_path = SupplierDeletePath { id: s.id };

    let category_label = match s.category {
        SupplierCategory::RawMaterial => "原材料",
        SupplierCategory::Packaging => "包装材料",
        SupplierCategory::Outsourcing => "外协加工",
        SupplierCategory::Consumable => "辅料",
        SupplierCategory::Service => "服务",
    };

    let (status_label, status_class) = match s.status {
        SupplierStatus::Prospective => ("潜在", "status-draft"),
        SupplierStatus::Qualified => ("合格", "status-accepted"),
        SupplierStatus::Probation => ("试用期", "status-progress"),
        SupplierStatus::Disqualified => ("不合格", "status-rejected"),
        SupplierStatus::Blacklisted => ("黑名单", "status-rejected"),
    };

    html! {
        tr style="cursor:pointer" {
            td class="link-cell mono" onclick=(format!("location.href='{}'", detail_path)) { (s.code) }
            td onclick=(format!("location.href='{}'", detail_path)) { strong { (s.name) } }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span class="tag-chip tag-normal" { (category_label) }
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span style="color:var(--muted)" { "—" }
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span style="color:var(--muted)" { "—" }
            }
            td class="mono" onclick=(format!("location.href='{}'", detail_path)) {
                @if s.lead_time_days > 0 {
                    (s.lead_time_days) " 天"
                } @else {
                    span style="color:var(--muted)" { "—" }
                }
            }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span class=(format!("status-pill {status_class}")) { (status_label) }
            }
            td onclick="event.stopPropagation()" {
                div class="row-actions" {
                    @if can_edit {
                        a class="row-action-btn" title="编辑"
                            href=(SupplierDetailPath { id: s.id }.to_string()) {
                            (icon::edit_icon("w-4 h-4"))
                        }
                    }
                    @if can_delete {
                        button type="button" class="row-action-btn text-danger" title="删除"
                            hx-post=(delete_path)
                            hx-confirm=(format!("删除后无法恢复，确定要删除供应商 <strong>{}</strong> 吗？", s.name))
                            hx-target="closest tr"
                            hx-swap="outerHTML swap:0.5s" {
                            (icon::trash_icon("w-4 h-4"))
                        }
                    }
                }
            }
        }
    }
}

fn build_query_string(params: &SupplierQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref kw) = params.keyword {
        q.push(format!("keyword={kw}"));
    }
    if let Some(s) = params.status {
        q.push(format!("status={s}"));
    }
    if let Some(c) = params.category {
        q.push(format!("category={c}"));
    }
    q.join("&")
}
