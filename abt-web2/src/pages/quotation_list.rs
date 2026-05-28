use std::collections::{HashMap, HashSet};

use axum::extract::Query;
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse};
use axum::Form;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::model::CustomerQuery;
use abt_core::master_data::customer::CustomerService;
use abt_core::sales::quotation::model::*;
use abt_core::sales::quotation::QuotationService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::quotation::*;
use crate::utils::{empty_as_none, RequestContext};

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct QuotationQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub customer_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub date_range: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Helpers ──

fn parse_date_range(range: &str) -> (Option<chrono::NaiveDate>, Option<chrono::NaiveDate>) {
    let today = chrono::Local::now().date_naive();
    match range {
        "7d" => (Some(today - chrono::Days::new(7)), None),
        "30d" => (Some(today - chrono::Days::new(30)), None),
        "3m" => (Some(today - chrono::Months::new(3)), None),
        _ => (None, None),
    }
}

fn build_filter(params: &QuotationQueryParams) -> QuotationQuery {
    let (date_from, date_to) = params
        .date_range
        .as_deref()
        .map(parse_date_range)
        .unwrap_or((None, None));
    QuotationQuery {
        keyword: params.keyword.clone(),
        status: params.status.and_then(QuotationStatus::from_i16),
        customer_id: params.customer_id,
        date_from,
        date_to,
    }
}

fn build_query_string(params: &QuotationQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref kw) = params.keyword {
        q.push(format!("keyword={kw}"));
    }
    if let Some(s) = params.status {
        q.push(format!("status={s}"));
    }
    if let Some(c) = params.customer_id {
        q.push(format!("customer_id={c}"));
    }
    if let Some(ref dr) = params.date_range {
        q.push(format!("date_range={dr}"));
    }
    q.join("&")
}

// ── Status Labels ──

fn status_label(s: QuotationStatus) -> (&'static str, &'static str) {
    match s {
        QuotationStatus::Draft => ("草稿", "status-draft"),
        QuotationStatus::Sent => ("已发送", "status-sent"),
        QuotationStatus::Accepted => ("已接受", "status-accepted"),
        QuotationStatus::Rejected => ("已拒绝", "status-rejected"),
        QuotationStatus::Expired => ("已过期", "status-expired"),
    }
}

// ── Handlers ──

pub async fn get_quotation_list(
    _path: QuotationListPath,
    ctx: RequestContext,
    headers: HeaderMap,
    Query(params): Query<QuotationQueryParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.quotation_service();
    let customer_svc = state.customer_service();

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);
    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    let mut names = HashMap::new();
    let mut seen = HashSet::new();
    for q in &result.items {
        if seen.insert(q.customer_id)
            && let Ok(c) = customer_svc.get(&service_ctx, &mut conn, q.customer_id).await {
                names.insert(q.customer_id, c.name);
            }
    }

    let customers = customer_svc
        .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
        .await?;

    let content = quotation_list_page(&claims, &result, &names, &customers.items, &params);
    let page_html = admin_page(
        &headers, "报价单", &claims, "sales", QuotationListPath::PATH, "销售管理", Some("报价单"), content,
    );

    Ok(Html(page_html.into_string()))
}

pub async fn get_quotation_table(
    ctx: RequestContext,
    Query(params): Query<QuotationQueryParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.quotation_service();
    let customer_svc = state.customer_service();

    let filter = build_filter(&params);
    let page = PageParams::new(params.page.unwrap_or(1), 20);
    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

    let mut names = HashMap::new();
    let mut seen = HashSet::new();
    for q in &result.items {
        if seen.insert(q.customer_id)
            && let Ok(c) = customer_svc.get(&service_ctx, &mut conn, q.customer_id).await {
                names.insert(q.customer_id, c.name);
            }
    }

    let customers = customer_svc
        .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
        .await?;

    Ok(Html(quotation_table_fragment(&result, &names, &customers.items, &params).into_string()))
}

pub async fn get_edit_quotation_form(
    path: EditQuotationFormPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.quotation_service();

    let quotation = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;

    let update_path = UpdateQuotationPath { id: path.id };
    let form_html = quotation_edit_form(&quotation, &update_path.to_string());

    Ok(Html(form_html.into_string()))
}

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub struct UpdateQuotationForm {
    payment_terms: Option<String>,
    delivery_terms: Option<String>,
    remark: Option<String>,
}

pub async fn update_quotation(
    path: UpdateQuotationPath,
    ctx: RequestContext,
    Form(form): Form<UpdateQuotationForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.quotation_service();

    let req = UpdateQuotationReq {
        payment_terms: form.payment_terms,
        delivery_terms: form.delivery_terms,
        remark: form.remark,
        ..Default::default()
    };

    svc.update(&service_ctx, &mut conn, path.id, req).await?;

    let redirect = QuotationDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

pub async fn delete_quotation(
    path: DeleteQuotationPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.quotation_service();

    svc.delete(&service_ctx, &mut conn, path.id).await?;

    Ok(([("HX-Redirect", QuotationListPath::PATH)], Html(String::new())))
}

// ── Components ──

fn quotation_list_page(
    _claims: &abt_core::shared::identity::model::Claims,
    result: &abt_core::shared::types::PaginatedResult<Quotation>,
    names: &HashMap<i64, String>,
    customers: &[abt_core::master_data::customer::model::Customer],
    params: &QuotationQueryParams,
) -> Markup {
    html! {
        div x-data="{ editModalOpen: false }" {
            // ── Page Header ──
            div class="page-header" {
                h1 class="page-title" { "报价单" }
                div class="page-actions" {
                    a class="btn btn-primary" href=(QuotationCreatePath::PATH) {
                        (icon::plus_icon("w-4 h-4"))
                        "新建报价单"
                    }
                }
            }

            // ── Tabs + Filter + Data Table (HTMX panel) ──
            (quotation_table_fragment(result, names, customers, params))

            // ── Edit Modal ──
            div class="modal-overlay"
                x-bind:class="{ 'is-open': editModalOpen }"
                x-on:click="editModalOpen = false" {
                div class="modal" x-on:click="event.stopPropagation()" {
                    div id="quotation-edit-modal-content" {
                        "加载中..."
                    }
                }
            }
        }
    }
}

fn quotation_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<Quotation>,
    names: &HashMap<i64, String>,
    customers: &[abt_core::master_data::customer::model::Customer],
    params: &QuotationQueryParams,
) -> Markup {
    let query = build_query_string(params);
    let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "草稿", count: None },
        TabItem { value: "2".into(), label: "已发送", count: None },
        TabItem { value: "3".into(), label: "已接受", count: None },
        TabItem { value: "4".into(), label: "已拒绝", count: None },
        TabItem { value: "5".into(), label: "已过期", count: None },
    ];

    let selected_customer = params.customer_id.map(|id| id.to_string()).unwrap_or_default();
    let selected_range = params.date_range.as_deref().unwrap_or("");

    html! {
        div class="quotation-list-panel" {
            (status_tabs(QuotationTablePath::PATH, "closest .quotation-list-panel", ".filter-bar input, .filter-bar select", tabs, &active_value))

            // ── Filter Bar ──
            div class="filter-bar" {
                div class="search-wrap" {
                    (icon::search_icon("w-4 h-4"))
                    input class="search-input" type="text" name="keyword"
                        placeholder="搜索报价单号、客户名称…"
                        value=(params.keyword.as_deref().unwrap_or(""))
                        hx-get=(QuotationTablePath::PATH)
                        hx-trigger="keyup changed delay:300ms"
                        hx-target="closest .quotation-list-panel"
                        hx-swap="outerHTML";
                }
                select class="filter-select" name="customer_id"
                    hx-get=(QuotationTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .quotation-list-panel"
                    hx-swap="outerHTML"
                    hx-include=".filter-bar input, .filter-bar select" {
                    option value="" { "全部客户" }
                    @for c in customers {
                        option value=(c.id) selected[selected_customer == c.id.to_string()] { (c.name) }
                    }
                }
                select class="filter-select" name="date_range"
                    hx-get=(QuotationTablePath::PATH)
                    hx-trigger="change"
                    hx-target="closest .quotation-list-panel"
                    hx-swap="outerHTML"
                    hx-include=".filter-bar input, .filter-bar select" {
                    option value="" selected[selected_range.is_empty()] { "报价日期" }
                    option value="7d" selected[selected_range == "7d"] { "最近7天" }
                    option value="30d" selected[selected_range == "30d"] { "最近30天" }
                    option value="3m" selected[selected_range == "3m"] { "最近3个月" }
                }
            }

            // ── Data Table ──
            div class="data-card" {
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "报价单号" }
                                th { "客户名称" }
                                th { "状态" }
                                th class="num-right" { "总金额" }
                                th { "报价日期" }
                                th { "有效期至" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            @for q in &result.items {
                                (quotation_row(q, names))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="7" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无报价单数据"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(QuotationListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

fn quotation_row(q: &Quotation, names: &HashMap<i64, String>) -> Markup {
    let detail_path = QuotationDetailPath { id: q.id };
    let (status_text, status_class) = status_label(q.status);
    let edit_form_path = EditQuotationFormPath { id: q.id };
    let delete_path = DeleteQuotationPath { id: q.id };
    let form_id = format!("delete-quotation-form-{}", q.id);
    let is_draft = q.status == QuotationStatus::Draft;
    let customer_name = names.get(&q.customer_id).map(|s| s.as_str()).unwrap_or("—");

    html! {
        tr style="cursor:pointer" {
            td class="link-cell mono" onclick=(format!("location.href='{}'", detail_path)) { (q.doc_number) }
            td onclick=(format!("location.href='{}'", detail_path)) { (customer_name) }
            td onclick=(format!("location.href='{}'", detail_path)) {
                span class=(format!("status-pill {status_class}")) { (status_text) }
            }
            td class="num-right" onclick=(format!("location.href='{}'", detail_path)) {
                span class="mono" { "¥ " (format!("{:.2}", q.total_amount)) }
            }
            td onclick=(format!("location.href='{}'", detail_path)) { (q.quotation_date.format("%Y-%m-%d")) }
            td onclick=(format!("location.href='{}'", detail_path)) { (q.valid_until.format("%Y-%m-%d")) }
            td onclick="event.stopPropagation()" {
                @if is_draft {
                    div class="row-actions" x-data="{ deleteOpen: false }" {
                        button class="row-action-btn" title="编辑"
                            hx-get=(edit_form_path)
                            hx-target="#quotation-edit-modal-content"
                            hx-swap="innerHTML"
                            x-on:click="editModalOpen = true" {
                            (icon::edit_icon("w-4 h-4"))
                        }
                        button type="button" class="row-action-btn text-danger" title="删除"
                            x-on:click="deleteOpen = true" {
                            (icon::trash_icon("w-4 h-4"))
                        }
                        (crate::components::confirm_dialog::confirm_dialog(
                            "deleteOpen",
                            "确认删除",
                            "删除后无法恢复，确定要删除该报价单吗？",
                            "确认删除",
                            &form_id,
                            html! {
                                form id=(form_id) style="display:none"
                                    hx-post=(delete_path)
                                    hx-target="closest tr"
                                    hx-swap="outerHTML swap:0.5s" {}
                            },
                        ))
                    }
                }
            }
        }
    }
}

// ── Edit Form ──

fn quotation_edit_form(quotation: &Quotation, action_url: &str) -> Markup {
    let payment_val = &quotation.payment_terms;
    let delivery_val = &quotation.delivery_terms;
    let remark_val = &quotation.remark;

    html! {
        div id="quotation-edit-modal-content" {
            div class="modal-head" {
                h2 { "编辑报价单" }
                button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                    x-on:click="editModalOpen = false" { "×" }
            }
            form class="modal-body" hx-post=(action_url) hx-target="this" {
                div class="form-section-title" { "报价信息" }
                div class="form-grid" {
                    div class="form-field" {
                        label { "付款条款" }
                        select name="payment_terms" {
                            option value="30天净额" selected[payment_val == "30天净额"] { "30天净额" }
                            option value="60天净额" selected[payment_val == "60天净额"] { "60天净额" }
                            option value="预付30%" selected[payment_val == "预付30%"] { "预付30%" }
                            option value="货到付款" selected[payment_val == "货到付款"] { "货到付款" }
                            option value="月结30天" selected[payment_val == "月结30天"] { "月结30天" }
                        }
                    }
                    div class="form-field" {
                        label { "交货条款" }
                        select name="delivery_terms" {
                            option value="FOB 深圳" selected[delivery_val == "FOB 深圳"] { "FOB 深圳" }
                            option value="FOB 广州" selected[delivery_val == "FOB 广州"] { "FOB 广州" }
                            option value="CIF 目的港" selected[delivery_val == "CIF 目的港"] { "CIF 目的港" }
                            option value="EXW 工厂交货" selected[delivery_val == "EXW 工厂交货"] { "EXW 工厂交货" }
                        }
                    }
                    div class="form-field field-full" {
                        label { "备注" }
                        textarea name="remark" placeholder="输入备注信息" { (remark_val) }
                    }
                }
            }
            div class="modal-foot" {
                button type="button" class="btn btn-default"
                    x-on:click="editModalOpen = false" { "取消" }
                button type="submit" class="btn btn-primary" { "保存修改" }
            }
        }
    }
}
