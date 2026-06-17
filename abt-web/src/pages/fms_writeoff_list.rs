use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::fms::enums::WriteOffType;
use abt_core::fms::write_off::model::{WriteOff, WriteOffListFilter};
use abt_core::fms::write_off::WriteOffService;
use abt_core::shared::enums::document_type::DocumentType;
use abt_core::shared::identity::UserService;
use abt_core::shared::types::PaginatedResult;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms::WriteoffListPath;
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Helpers ──

/// 根据核销类型返回 (CSS class, 标签文字)
fn writeoff_type_class(t: &WriteOffType) -> (&'static str, &'static str) {
    match t {
        WriteOffType::SalesReceipt => ("sales", "销售回款核销"),
        WriteOffType::PurchasePayment => ("purchase", "采购付款核销"),
    }
}

fn source_type_label(dt: &DocumentType) -> &'static str {
    match dt {
        DocumentType::SalesOrder => "SO",
        DocumentType::ShippingRequest => "SHIP",
        DocumentType::PurchaseOrder => "PO",
        DocumentType::ArrivalNotice => "ARR",
        _ => "OTHER",
    }
}

fn build_query_string(params: &WriteoffQueryParams) -> String {
    let mut parts = Vec::new();
    if let Some(v) = params.writeoff_type {
        parts.push(format!("writeoff_type={v}"));
    }
    if let Some(ref v) = params.keyword
        && !v.is_empty() {
            parts.push(format!("keyword={}", v.replace(' ', "%20")));
        }
    if let Some(v) = params.start_date {
        parts.push(format!("start_date={v}"));
    }
    if let Some(v) = params.end_date {
        parts.push(format!("end_date={v}"));
    }
    if let Some(v) = params.page
        && v > 1 {
            parts.push(format!("page={v}"));
        }
    if parts.is_empty() { String::new() } else { format!("?{}", parts.join("&")) }
}

/// 批量解析操作人姓名
async fn resolve_operator_names(
    state: &crate::state::AppState,
    ctx: &abt_core::shared::types::ServiceContext,
    db: &mut abt_core::shared::types::PgPoolConn,
    items: &[WriteOff],
) -> HashMap<i64, String> {
    let ids: Vec<i64> = items.iter().map(|i| i.operator_id).collect();
    if ids.is_empty() {
        return HashMap::new();
    }
    let svc = state.user_service();
    let mut names = HashMap::new();
    if let Ok(users) = UserService::get_users_by_ids(&svc, ctx, &mut *db, ids).await {
        for u in users {
            names.insert(u.user.user_id, u.user.display_name.unwrap_or(u.user.username));
        }
    }
    names
}

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct WriteoffQueryParams {
    #[serde(default, deserialize_with = "empty_as_none")]
    pub writeoff_type: Option<i16>,
    #[serde(default)]
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub start_date: Option<chrono::NaiveDate>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub end_date: Option<chrono::NaiveDate>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("FMS", "read")]
pub async fn get_list(
    _path: WriteoffListPath,
    ctx: RequestContext,
    Query(params): Query<WriteoffQueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.write_off_service();

    let filter = WriteOffListFilter {
        write_off_type: params.writeoff_type.and_then(WriteOffType::from_i16),
        keyword: params.keyword.clone(),
        start_date: params.start_date,
        end_date: params.end_date,
    };
    let page_num = params.page.unwrap_or(1);
    let result = svc
        .list(&service_ctx, &mut conn, filter, abt_core::shared::types::PageParams::new(page_num, 20))
        .await?;

    let operator_names = resolve_operator_names(&state, &service_ctx, &mut conn, &result.items).await;
    let content = writeoff_list_page(&result, &params, &operator_names);
    let page_html = admin_page(
        is_htmx,
        "核销管理",
        &claims,
        "finance",
        WriteoffListPath::PATH,
        "财务管理",
        None,
        content, &nav_filter,    );
    Ok(Html(page_html.into_string()))
}

// ── Components ──

fn writeoff_list_page(
    result: &PaginatedResult<WriteOff>,
    params: &WriteoffQueryParams,
    operator_names: &HashMap<i64, String>,
) -> Markup {
    html! {
        div class="p-6 relative" {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "核销管理" }
                div class="flex gap-3" {
                    button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" type="button" {
                        (icon::download_icon("w-4 h-4"))
                        "导出"
                    }
                }
            }
            (writeoff_table_fragment(result, params, operator_names))
        }
    }
}

fn writeoff_table_fragment(
    result: &PaginatedResult<WriteOff>,
    params: &WriteoffQueryParams,
    operator_names: &HashMap<i64, String>,
) -> Markup {
    let selected_type = params.writeoff_type.map(|v| v.to_string()).unwrap_or_default();
    let total_count = result.total;

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "销售回款核销", count: None },
        TabItem { value: "2".into(), label: "采购付款核销", count: None },
    ];

    html! {
        div {
            // 类型 Tab
            (status_tabs_with_param(
                WriteoffListPath::PATH,
                "#writeoff-data-card",
                "#writeoff-filter-form",
                tabs,
                &selected_type,
                "writeoff_type",
            ))

            // 筛选栏 — 暂时只放占位搜索和日期筛选
            form id="writeoff-filter-form" class="flex items-center gap-3 mb-5 flex-wrap filter-form"
                hx-get=(WriteoffListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#writeoff-data-card"
                hx-select="#writeoff-data-card"
                hx-swap="outerHTML"
                hx-include="#writeoff-filter-form"
                hx-push-url="true" {
                div class="relative flex-1 max-w-xs" {
                    (icon::search_icon("w-4 h-4"))
                    input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword"
                        style="width:200px"
                        placeholder="搜索日记账号…";
                }
                input type="date" name="start_date" class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" style="width:150px" title="起始日期";
                input type="date" name="end_date" class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" style="width:150px" title="截止日期";
            }

            (writeoff_data_card(result, params, operator_names))
        }
    }
}

fn writeoff_data_card(
    result: &PaginatedResult<WriteOff>,
    params: &WriteoffQueryParams,
    operator_names: &HashMap<i64, String>,
) -> Markup {
    let query = build_query_string(params);
    html! {
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" id="writeoff-data-card" {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] overflow-x-auto" {
                table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:text-left [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" style="min-width:1000px" {
                    thead {
                        tr {
                            th { "核销类型" }
                            th { "日记账号" }
                            th { "来源单号" }
                            th { "来源总额" }
                            th { "本次核销金额" }
                            th { "未核销余额" }
                            th { "核销状态" }
                            th { "核销日期" }
                            th { "操作人" }
                        }
                    }
                    tbody {
                        @for item in &result.items {
                            @let (css_class, type_label) = writeoff_type_class(&item.write_off_type);
                            @let source_prefix = source_type_label(&item.source_type);
                            @let operator_name = operator_names.get(&item.operator_id)
                                .cloned()
                                .unwrap_or_else(|| item.operator_id.to_string());
                            tr {
                                td {
                                    span class=(format!("wo-type {css_class}")) { (type_label) }
                                }
                                td class="text-accent font-medium cursor-pointer" { "CJ-" (item.cash_journal_id) }
                                td class="text-accent font-medium cursor-pointer" { (source_prefix) "-" (item.source_id) }
                                td class="text-right text-[13px]" { "—" }
                                td class="text-right text-[13px] font-bold text-accent" { "¥" (format!("{:.2}", item.amount)) }
                                td class="text-right text-[13px]" style="color:var(--warn)" { "—" }
                                td {
                                    span class="inline-flex items-center gap-[5px] rounded-full text-[12px] font-medium whitespace-nowrap full" { "已核销完毕" }
                                }
                                td class="text-text-muted text-[13px]" { (item.write_off_date.format("%Y-%m-%d")) }
                                td { (operator_name) }
                            }
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="9" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无核销记录"
                                }
                            }
                        }
                    }
                }
            }
            (pagination(WriteoffListPath::PATH, &query, result.total, result.page, result.total_pages))
        }
    }
}
