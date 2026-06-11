use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::model::*;
use abt_core::master_data::customer::CustomerService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::layout::page::admin_page;
use crate::routes::customer::{CustomerDetailPath, CustomerListPath, CustomerTablePath, EditCustomerPath, DeleteCustomerPath};
use crate::utils::{empty_as_none, RequestContext};
use crate::utils::fmt_qty;
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct CustomerQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub category: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("CUSTOMER", "read")]
pub async fn get_customer_list(
    _path: CustomerListPath,
    ctx: RequestContext,

    Query(params): Query<CustomerQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let can_create = ctx.has_permission("CUSTOMER", "create").await;
    let can_delete = ctx.has_permission("CUSTOMER", "delete").await;
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.customer_service();

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);

    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    let content = customer_list_page(&claims, &result, &params, can_create, can_delete);
    let page_html = admin_page(
        is_htmx, "客户管理", &claims, "sales", CustomerListPath::PATH, "销售管理", Some("客户管理"), content, &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("CUSTOMER", "read")]
pub async fn get_customer_table(
    ctx: RequestContext,
    Query(params): Query<CustomerQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let can_delete = ctx.has_permission("CUSTOMER", "delete").await;
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.customer_service();

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);

    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    Ok(Html(customer_table_fragment(&result, &params, can_delete).into_string()))
}

#[require_permission("CUSTOMER", "delete")]
pub async fn delete_customer(
    path: DeleteCustomerPath,
    ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.customer_service();

    svc.delete(&service_ctx, &mut conn, path.id).await?;

    Ok(([("HX-Redirect", CustomerListPath::PATH)], Html(String::new())))
}

// ── Helpers ──

fn build_filter(params: &CustomerQueryParams) -> CustomerQuery {
    CustomerQuery {
        name: params.keyword.clone(),
        status: params.status.and_then(CustomerStatus::from_i16),
        category: params.category.and_then(CustomerCategory::from_i16),
        owner_id: None,
    }
}

// ── Components ──

fn customer_list_page(
    _claims: &abt_core::shared::identity::model::Claims,
    result: &abt_core::shared::types::PaginatedResult<Customer>,
    params: &CustomerQueryParams,
    can_create: bool,
    can_delete: bool,
) -> Markup {
    let total_count = result.total;

    html! {
        div {
            // ── Page Header ──
            div class="page-header" {
                h1 class="page-title" { "客户管理" }
                div class="page-actions" {
                    button class="btn btn-default" {
                        (icon::download_icon("w-4 h-4"))
                        "导出"
                    }
                    @if can_create {
                        a class="btn btn-primary" href="/admin/customers/new" {
                            (icon::plus_icon("w-4 h-4"))
                            "新建客户"
                        }
                    }
                }
            }

            // ── Stat Cards ──
            div class="customer-stats" {
                div class="stat-card" {
                    div class="stat-icon blue" {
                        (icon::users_icon("w-6 h-6"))
                    }
                    div {
                        div class="stat-value" { (total_count) }
                        div class="stat-label" { "客户总数" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon green" {
                        (icon::check_circle_icon("w-6 h-6"))
                    }
                    div {
                        div class="stat-value" { "—" }
                        div class="stat-label" { "活跃客户" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon orange" {
                        (icon::trending_up_icon("w-6 h-6"))
                    }
                    div {
                        div class="stat-value" { "—" }
                        div class="stat-label" { "本月交易额" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon red" {
                        (icon::circle_alert_icon("w-6 h-6"))
                    }
                    div {
                        div class="stat-value" { "—" }
                        div class="stat-label" { "信用预警" }
                    }
                }
            }

            // ── Tabs + Filter + Data Table (HTMX panel) ──
            (customer_table_fragment(result, params, can_delete))
        }
    }
}

fn customer_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<Customer>,
    params: &CustomerQueryParams,
    can_delete: bool,
) -> Markup {
    let query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "2".into(), label: "活跃", count: None },
        TabItem { value: "1".into(), label: "潜在客户", count: None },
        TabItem { value: "3".into(), label: "已停用", count: None },
        TabItem { value: "4".into(), label: "黑名单", count: None },
    ];

    html! {
        div class="customer-list-panel" {
            (status_tabs_with_param(CustomerTablePath::PATH, "#customer-data-card", "#customer-filter-form", tabs, &active_value, "status"))

            // ── Filter Bar ──
            form class="filter-bar filter-form" id="customer-filter-form"
                hx-get=(CustomerTablePath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#customer-data-card"
                hx-select="#customer-data-card"
                hx-swap="outerHTML"
                hx-include="#customer-filter-form" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        placeholder="搜索客户名称、联系人、电话…"
                        value=(params.keyword.as_deref().unwrap_or(""));
                }
                select class="filter-select" name="category" {
                    option value="" { "全部分类" }
                    option value="1" selected[params.category == Some(1)] { "经销商" }
                    option value="2" selected[params.category == Some(2)] { "直客" }
                    option value="3" selected[params.category == Some(3)] { "OEM" }
                    option value="4" selected[params.category == Some(4)] { "零售商" }
                }
            }

            // ── Data Table ──
            div class="data-card" id="customer-data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "客户编码" }
                                th { "客户名称" }
                                th { "分类" }
                                th { "信用额度" }
                                th { "状态" }
                                th { "创建时间" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            @for c in &result.items {
                                (customer_row(c, can_delete))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="7" class="td-empty" {
                                        "暂无客户数据"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(CustomerListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

fn build_query_string(params: &CustomerQueryParams) -> String {
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

fn customer_row(c: &Customer, can_delete: bool) -> Markup {
    let detail_path = CustomerDetailPath { id: c.id };
    let edit_path = EditCustomerPath { id: c.id };
    let delete_path = DeleteCustomerPath { id: c.id };
    let category_label = match c.category {
        CustomerCategory::Distributor => ("经销商", "tag-normal"),
        CustomerCategory::DirectCustomer => ("直客", "tag-normal"),
        CustomerCategory::OEM => ("OEM", "tag-normal"),
        CustomerCategory::Retailer => ("零售商", "tag-normal"),
    };
    let (status_label, status_class) = match c.status {
        CustomerStatus::Prospective => ("潜在客户", "status-draft"),
        CustomerStatus::Active => ("活跃", "status-accepted"),
        CustomerStatus::Inactive => ("已停用", "status-rejected"),
        CustomerStatus::Blacklisted => ("黑名单", "status-rejected"),
    };

    html! {
        tr {
            td class="link-cell mono" onclick=(format!("location.href='{}'", detail_path)) { (c.code) }
            td onclick=(format!("location.href='{}'", detail_path)) { strong { (c.name) } }
            td onclick=(format!("location.href='{}'", detail_path)) { span class=(format!("tag-chip {}", category_label.1)) { (category_label.0) } }
            td onclick=(format!("location.href='{}'", detail_path)) {
                @if let Some(limit) = c.credit_limit {
                    div class="credit-cell" {
                        span class="mono text-xs" { "¥ " (format_amount(limit)) }
                        div class="credit-bar" {
                            div class="credit-bar-fill" style="width:0%;background:var(--accent)" {}
                        }
                    }
                } @else {
                    span class="text-muted" { "—" }
                }
            }
            td onclick=(format!("location.href='{}'", detail_path)) { span class=(format!("status-pill {status_class}")) { (status_label) } }
            td onclick=(format!("location.href='{}'", detail_path)) { (c.created_at.format("%Y-%m-%d")) }
            td onclick="event.stopPropagation()" {
                div class="row-actions" {
                    a class="row-action-btn" title="编辑" href=(edit_path) {
                        (icon::edit_icon("w-4 h-4"))
                    }
                    @if can_delete {
                        button type="button" class="row-action-btn text-danger" title="删除"
                            hx-post=(delete_path)
                            hx-confirm=(format!("删除后无法恢复，确定要删除客户 <strong>{}</strong> 吗？", c.name))
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

fn format_amount(d: rust_decimal::Decimal) -> String {
    if d == rust_decimal::Decimal::ZERO {
        "—".into()
    } else {
        fmt_qty(d)
    }
}

