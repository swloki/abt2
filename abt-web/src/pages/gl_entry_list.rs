use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::gl::entry::model::{GlEntry, GlEntryFilter};
use abt_core::gl::entry::GlEntryService;
use abt_core::gl::enums::EntryStatus;
use abt_core::shared::enums::document_type::DocumentType;
use abt_core::shared::types::PaginatedResult;

use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::gl::{GlEntryDetailPath, GlEntryListPath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct EntryQueryParams {
    /// 模糊匹配 doc_number（服务端 filter 未直接支持，列表保留 query 透传给详情返回）
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub period: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub source_type: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub voucher_type: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("GL", "read")]
pub async fn get_list(
    _path: GlEntryListPath,
    ctx: RequestContext,
    Query(params): Query<EntryQueryParams>,
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

    let svc = state.gl_entry_service();
    let filter = build_filter(&params);
    let page_num = params.page.unwrap_or(1);
    let result = svc
        .list(
            &service_ctx,
            &mut conn,
            filter,
            abt_core::shared::types::PageParams::new(page_num, 20),
        )
        .await?;

    let content = entry_list_page(&result, &params);
    let page_html = admin_page(
        is_htmx,
        "会计凭证",
        &claims,
        "gl",
        GlEntryListPath::PATH,
        "总账管理",
        None,
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

fn build_filter(params: &EntryQueryParams) -> GlEntryFilter {
    GlEntryFilter {
        period: params.period.clone(),
        source_type: params.source_type.and_then(DocumentType::from_i16),
        status: params.status.and_then(EntryStatus::from_i16),
        voucher_type: params.voucher_type.clone(),
    }
}

fn build_query_string(params: &EntryQueryParams) -> String {
    let mut parts = Vec::new();
    if let Some(ref v) = params.period {
        parts.push(format!("period={v}"));
    }
    if let Some(v) = params.source_type {
        parts.push(format!("source_type={v}"));
    }
    if let Some(v) = params.status {
        parts.push(format!("status={v}"));
    }
    if let Some(ref v) = params.voucher_type {
        parts.push(format!("voucher_type={v}"));
    }
    if let Some(v) = params.page
        && v > 1
    {
        parts.push(format!("page={v}"));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    }
}

// ── Components ──

/// 凭证来源单据类型的可读标签 + 颜色（status badge 风格）
fn source_type_label(t: &DocumentType) -> (&'static str, &'static str, &'static str) {
    // (label, bg, color)
    use DocumentType::*;
    match t {
        CashJournal => ("出纳日记账", "rgba(37,99,235,0.08)", "#2563eb"),
        ExpenseReimbursement => ("费用报销", "rgba(217,119,6,0.08)", "#b45309"),
        PaymentRequest => ("付款申请", "rgba(124,58,237,0.08)", "#7c3aed"),
        GlEntry => ("手工凭证", "rgba(148,163,184,0.12)", "#475569"),
        SalesInvoice => ("销售发票", "rgba(22,163,74,0.08)", "#16a34a"),
        PurchaseInvoice => ("采购发票", "rgba(220,38,38,0.08)", "#dc2626"),
        WriteOff => ("核销", "rgba(0,0,0,0.04)", "var(--muted)"),
        _ => ("其他单据", "rgba(0,0,0,0.04)", "var(--muted)"),
    }
}

/// 凭证状态标签：Draft/Posted/Cancelled
fn status_label(s: &EntryStatus) -> (&'static str, &'static str, &'static str) {
    // (label, bg, color)
    match s {
        EntryStatus::Draft => ("Draft", "rgba(0,0,0,0.04)", "var(--muted)"),
        EntryStatus::Posted => ("Posted", "rgba(22,163,74,0.08)", "#16a34a"),
        EntryStatus::Cancelled => ("Cancelled", "rgba(220,38,38,0.08)", "#dc2626"),
    }
}

fn entry_list_page(result: &PaginatedResult<GlEntry>, params: &EntryQueryParams) -> Markup {
    html! {
        div {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "会计凭证" }
            }
            (entry_table_fragment(result, params))
        }
    }
}

fn entry_table_fragment(result: &PaginatedResult<GlEntry>, params: &EntryQueryParams) -> Markup {
    let total_count = result.total;
    let selected_status = params.status.map(|v| v.to_string()).unwrap_or_default();

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "Draft", count: None },
        TabItem { value: "2".into(), label: "Posted", count: None },
        TabItem { value: "3".into(), label: "Cancelled", count: None },
    ];

    html! {
        div {
            ({
                status_tabs_with_param(
                    GlEntryListPath::PATH,
                    "#gl-entry-data-card",
                    "#gl-entry-filter-form",
                    tabs,
                    &selected_status,
                    "status",
                )
            })

            form
                class="flex items-center gap-3 mb-5 flex-wrap filter-form"
                id="gl-entry-filter-form"
                hx-get=(GlEntryListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#gl-entry-data-card"
                hx-select="#gl-entry-data-card"
                hx-swap="outerHTML"
                hx-include="#gl-entry-filter-form"
                hx-push-url="true"
            {
                input
                    class="w-[120px] px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent font-mono tabular-nums"
                    type="text"
                    name="period"
                    placeholder="期间 YYYY-MM"
                    value=(params.period.as_deref().unwrap_or(""));
                select
                    class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer"
                    name="source_type"
                {
                    option value="" selected[params.source_type.is_none()] { "全部来源" }
                    option value="30" selected[params.source_type == Some(30)] { "出纳日记账" }
                    option value="32" selected[params.source_type == Some(32)] { "费用报销" }
                    option value="23" selected[params.source_type == Some(23)] { "付款申请" }
                    option value="45" selected[params.source_type == Some(45)] { "手工凭证" }
                    option value="46" selected[params.source_type == Some(46)] { "销售发票" }
                    option value="47" selected[params.source_type == Some(47)] { "采购发票" }
                }
                input
                    class="w-[120px] px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                    type="text"
                    name="voucher_type"
                    placeholder="凭证字"
                    value=(params.voucher_type.as_deref().unwrap_or(""));
            }

            (entry_data_card(result, params))
        }
    }
}

fn entry_data_card(result: &PaginatedResult<GlEntry>, params: &EntryQueryParams) -> Markup {
    let query = build_query_string(params);
    html! {
        div class="data-card" id="gl-entry-data-card" {
            div class="overflow-x-auto" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "凭证号" }
                            th { "期间" }
                            th { "凭证日期" }
                            th { "凭证字" }
                            th { "来源" }
                            th { "状态" }
                            th class="text-right" { "借方合计" }
                            th class="text-right" { "贷方合计" }
                            th class="w-[80px]" { "操作" }
                        }
                    }
                    tbody {
                        @for item in &result.items {
                            @let (src_label, src_bg, src_color) = source_type_label(
                                &item.source_type,
                            );
                            @let (status_text, status_bg, status_color) = status_label(
                                &item.status,
                            );
                            @let detail_path = GlEntryDetailPath { id: item.id };
                            tr  class="cursor-pointer"
                                onclick=(format!("location.href='{}'", detail_path.to_string()))
                            {
                                td class="font-mono tabular-nums text-accent" { (&item.doc_number) }
                                td class="font-mono tabular-nums" { (&item.period) }
                                td class="text-xs text-muted" { (item.entry_date.format("%Y-%m-%d")) }
                                td class="text-fg-2" { (&item.voucher_type) }
                                td {
                                    span
                                        style=({
                                            format!(
                                                "display:inline-flex;align-items:center;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}",
                                                src_bg,
                                                src_color,
                                            )
                                        })
                                    { (src_label) }
                                }
                                td {
                                    span
                                        style=({
                                            format!(
                                                "display:inline-flex;align-items:center;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}",
                                                status_bg,
                                                status_color,
                                            )
                                        })
                                    { (status_text) }
                                }
                                td class="font-mono tabular-nums text-right text-fg" {
                                    (crate::utils::fmt_amount(item.total_debit))
                                }
                                td class="font-mono tabular-nums text-right text-fg" {
                                    (crate::utils::fmt_amount(item.total_credit))
                                }
                                td {
                                    a href=(detail_path.to_string()) class="text-accent text-xs" {
                                        "查看"
                                    }
                                }
                            }
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="9" class="text-center text-muted py-8" { "暂无凭证记录" }
                            }
                        }
                    }
                }
            }
            ({
                pagination(
                    GlEntryListPath::PATH,
                    &query,
                    result.total,
                    result.page,
                    result.total_pages,
                )
            })
        }
    }
}
