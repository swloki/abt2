use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum::Form;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::model::*;
use abt_core::master_data::customer::CustomerService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs, TabItem};
use crate::layout::page::admin_page;
use crate::routes::customer::{CreateCustomerPath, CustomerDetailPath, CustomerListPath, CustomerTablePath, EditCustomerFormPath, UpdateCustomerPath, DeleteCustomerPath};
use crate::utils::{empty_as_none, RequestContext};
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
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.customer_service();

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);

    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    let content = customer_list_page(&claims, &result, &params);
    let page_html = admin_page(
        is_htmx, "客户管理", &claims, "sales", CustomerListPath::PATH, "销售管理", Some("客户管理"), content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("CUSTOMER", "read")]
pub async fn get_customer_table(
    ctx: RequestContext,
    Query(params): Query<CustomerQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.customer_service();

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);

    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    Ok(Html(customer_table_fragment(&result, &params).into_string()))
}

#[require_permission("CUSTOMER", "create")]
pub async fn create_customer(
    _path: CreateCustomerPath,
    ctx: RequestContext,
    Form(form): Form<CreateCustomerForm>,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.customer_service();

    let category = form
        .category
        .and_then(CustomerCategory::from_i16)
        .unwrap_or(CustomerCategory::DirectCustomer);

    let credit_limit = form
        .credit_limit
        .and_then(|s| s.parse::<rust_decimal::Decimal>().ok());

    let req = CreateCustomerReq {
        customer_name: form.customer_name,
        short_name: form.short_name,
        category,
        tax_number: form.tax_number,
        invoice_title: form.invoice_title,
        credit_limit,
        payment_terms: form.payment_terms,
        receivable_account: None,
        remark: form.remark,
    };

    let id = svc.create(&service_ctx, &mut conn, req).await?;

    Ok((
        [("HX-Redirect", format!("/admin/customers/{id}"))],
        Html(String::new()),
    ))
}

#[require_permission("CUSTOMER", "read")]
pub async fn get_edit_customer_form(
    path: EditCustomerFormPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.customer_service();

    let customer = svc.get(&service_ctx, &mut conn, path.id).await?;

    let update_path = UpdateCustomerPath { id: path.id };
    let form_html = customer_form(&Some(customer), "edit-customer-form", &update_path.to_string());

    Ok(Html(form_html.into_string()))
}

#[require_permission("CUSTOMER", "update")]
pub async fn update_customer(
    path: UpdateCustomerPath,
    ctx: RequestContext,
    Form(form): Form<CreateCustomerForm>,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.customer_service();

    let req = UpdateCustomerReq {
        customer_name: Some(form.customer_name),
        short_name: form.short_name,
        category: form.category.and_then(CustomerCategory::from_i16),
        status: None,
        tax_number: form.tax_number,
        invoice_title: form.invoice_title,
        credit_limit: form.credit_limit.and_then(|s| s.parse::<rust_decimal::Decimal>().ok()),
        payment_terms: form.payment_terms,
        receivable_account: None,
        remark: form.remark,
    };

    svc.update(&service_ctx, &mut conn, path.id, req).await?;

    let redirect = CustomerDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
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

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub(crate) struct CreateCustomerForm {
    customer_name: String,
    short_name: Option<String>,
    category: Option<i16>,
    credit_limit: Option<String>,
    payment_terms: Option<String>,
    tax_number: Option<String>,
    invoice_title: Option<String>,
    remark: Option<String>,
}

// ── Shared Form Component ──

fn customer_form(customer: &Option<Customer>, form_id: &str, action_url: &str) -> Markup {
    let c = customer.as_ref();
    let title = if c.is_some() { "编辑客户" } else { "新建客户" };
    let submit_label = if c.is_some() { "保存修改" } else { "保存客户" };
    let category_val = c.map_or(String::new(), |c| match c.category {
        CustomerCategory::Distributor => "1".to_string(),
        CustomerCategory::DirectCustomer => "2".to_string(),
        CustomerCategory::OEM => "3".to_string(),
        CustomerCategory::Retailer => "4".to_string(),
    });
    let credit_val = c.and_then(|c| c.credit_limit.map(|l| format!("{:.2}", l))).unwrap_or_default();
    let payment_val = c.and_then(|c| c.payment_terms.clone()).unwrap_or_default();
    let remark_val = c.map_or("", |c| &c.remark);

    html! {
        div id="customer-modal-content" {
            div class="modal-head" {
                h2 { (title) }
                button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                    onclick="hsRemove(null,'#customer-create-modal','is-open')" { "×" }
            }
            form id=(form_id) class="modal-body"
                hx-post=(action_url)
                hx-target="this" {

                // ── Section: 基本信息 ──
                div class="form-section-title" { "基本信息" }
                div class="form-grid" {
                    div class="form-field" {
                        label { "客户名称 " span style="color:var(--danger)" { "*" } }
                        input type="text" name="customer_name" required placeholder="请输入客户全称"
                            value=(c.map_or("", |c| &c.name));
                    }
                    div class="form-field" {
                        label { "客户简称" }
                        input type="text" name="short_name" placeholder="请输入客户简称"
                            value=(c.and_then(|c| c.short_name.clone()).unwrap_or_default());
                    }
                    div class="form-field" {
                        label { "客户分类" }
                        select name="category" {
                            option value="2" selected[category_val == "2"] { "直客" }
                            option value="1" selected[category_val == "1"] { "经销商" }
                            option value="3" selected[category_val == "3"] { "OEM" }
                            option value="4" selected[category_val == "4"] { "零售商" }
                        }
                    }
                }

                // ── Section: 财务信息 ──
                div class="form-section-title" { "财务信息" }
                div class="form-grid" {
                    div class="form-field" {
                        label { "信用额度 (元)" }
                        input type="number" name="credit_limit" placeholder="请输入信用额度" step="0.01"
                            value=(credit_val);
                    }
                    div class="form-field" {
                        label { "付款条款" }
                        select name="payment_terms" {
                            option value="月结30天" selected[payment_val == "月结30天"] { "月结 30 天" }
                            option value="月结60天" selected[payment_val == "月结60天"] { "月结 60 天" }
                            option value="月结90天" selected[payment_val == "月结90天"] { "月结 90 天" }
                            option value="预付款" selected[payment_val == "预付款"] { "预付款" }
                            option value="货到付款" selected[payment_val == "货到付款"] { "货到付款" }
                        }
                    }
                    div class="form-field" {
                        label { "税号" }
                        input type="text" name="tax_number" placeholder="请输入纳税人识别号"
                            value=(c.and_then(|c| c.tax_number.clone()).unwrap_or_default());
                    }
                    div class="form-field" {
                        label { "发票抬头" }
                        input type="text" name="invoice_title" placeholder="请输入发票抬头"
                            value=(c.and_then(|c| c.invoice_title.clone()).unwrap_or_default());
                    }
                }

                // ── Section: 其他信息 ──
                div class="form-section-title" { "其他信息" }
                div class="form-grid" {
                    div class="form-field field-full" {
                        label { "备注" }
                        textarea name="remark" placeholder="请输入备注信息" { (remark_val) }
                    }
                }
            }
            div class="modal-foot" {
                button type="button" class="btn btn-default"
                    onclick="hsRemove(null,'#customer-create-modal','is-open')" { "取消" }
                button type="submit" class="btn btn-primary" form=(form_id) { (submit_label) }
            }
        }
    }
}

// ── Components ──

fn customer_list_page(
    _claims: &abt_core::shared::identity::model::Claims,
    result: &abt_core::shared::types::PaginatedResult<Customer>,
    params: &CustomerQueryParams,
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
                    button class="btn btn-primary"
                        onclick="hsAdd(null,'#customer-create-modal','is-open')" {
                        (icon::plus_icon("w-4 h-4"))
                        "新建客户"
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
            (customer_table_fragment(result, params))

            div class="modal-overlay" id="customer-create-modal"
                onclick="hsRemove(this,null,'is-open')" {
                div class="modal" onclick="event.stopPropagation()" {
                    (customer_form(&None, "create-customer-form", CreateCustomerPath::PATH))
                }
            }
        }
    }
}

fn customer_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<Customer>,
    params: &CustomerQueryParams,
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
            (status_tabs(CustomerTablePath::PATH, "closest .customer-list-panel", ".filter-bar input, .filter-bar select", tabs, &active_value))

            // ── Filter Bar ──
            div class="filter-bar" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        placeholder="搜索客户名称、联系人、电话…"
                        value=(params.keyword.as_deref().unwrap_or(""))
                        hx-get=(CustomerTablePath::PATH)
                        hx-trigger="keyup changed delay:300ms"
                        hx-target="closest .customer-list-panel"
                        hx-swap="outerHTML";
                }
                select class="filter-select" name="category"
                    hx-get=(CustomerTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .customer-list-panel"
                    hx-swap="outerHTML" {
                    option value="" { "全部分类" }
                    option value="1" selected[params.category == Some(1)] { "经销商" }
                    option value="2" selected[params.category == Some(2)] { "直客" }
                    option value="3" selected[params.category == Some(3)] { "OEM" }
                    option value="4" selected[params.category == Some(4)] { "零售商" }
                }
            }

            // ── Data Table ──
            div class="data-card" {
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
                                (customer_row(c))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="7" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
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

fn customer_row(c: &Customer) -> Markup {
    let detail_path = CustomerDetailPath { id: c.id };
    let edit_form_path = EditCustomerFormPath { id: c.id };
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
        tr style="cursor:pointer" {
            td class="link-cell mono" onclick=(format!("location.href='{}'", detail_path)) { (c.code) }
            td onclick=(format!("location.href='{}'", detail_path)) { strong { (c.name) } }
            td onclick=(format!("location.href='{}'", detail_path)) { span class=(format!("tag-chip {}", category_label.1)) { (category_label.0) } }
            td onclick=(format!("location.href='{}'", detail_path)) {
                @if let Some(limit) = c.credit_limit {
                    div style="display:flex;align-items:center;gap:6px" {
                        span class="mono" style="font-size:12px" { "¥ " (format_amount(limit)) }
                        div class="credit-bar" {
                            div class="credit-bar-fill" style="width:0%;background:var(--accent)" {}
                        }
                    }
                } @else {
                    span style="color:var(--muted)" { "—" }
                }
            }
            td onclick=(format!("location.href='{}'", detail_path)) { span class=(format!("status-pill {status_class}")) { (status_label) } }
            td onclick=(format!("location.href='{}'", detail_path)) { (c.created_at.format("%Y-%m-%d")) }
            td onclick="event.stopPropagation()" {
                div class="row-actions" {
                    button class="row-action-btn" title="编辑"
                        hx-get=(edit_form_path)
                        hx-target="#customer-create-modal .modal-body"
                        hx-swap="innerHTML"
                        hx-on::after-request="hsAdd(null,'#customer-create-modal','is-open')" {
                        (icon::edit_icon("w-4 h-4"))
                    }
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

fn format_amount(d: rust_decimal::Decimal) -> String {
    let abs = d.abs();
    let threshold = rust_decimal::Decimal::ONE_HUNDRED * rust_decimal::Decimal::ONE_THOUSAND;
    if abs >= threshold {
        format!("{:.0}", d)
    } else {
        format!("{}", d)
    }
}

