use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;
use serde::Deserialize;

use abt_core::fms::adjustment::model::{AdjustmentFilter, AdjustmentRow};
use abt_core::fms::adjustment::{AdjustmentDirection, AdjustmentService};
use abt_core::fms::enums::CounterpartyType;
use abt_core::shared::types::PaginatedResult;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms::{
    ApAdjustmentCreatePath, ApAdjustmentListPath, ArAdjustmentCreatePath, ArAdjustmentListPath,
};
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[derive(Deserialize, Debug, Default)]
pub struct ListQuery {
    pub page: Option<u32>,
    pub keyword: Option<String>,
}

fn fmt_amount(amount: Decimal) -> String {
    format!("{amount:.2}")
}

fn adjustment_row(item: &AdjustmentRow) -> Markup {
    let (dir_label, dir_cls) = match item.direction {
        AdjustmentDirection::Increase => ("增加", "text-success bg-success/10"),
        AdjustmentDirection::Decrease => ("减少", "text-danger bg-danger/10"),
    };
    html! {
        tr {
            td class="px-4 py-3 text-sm whitespace-nowrap" { (item.adjustment_date.format("%Y-%m-%d")) }
            td class="px-4 py-3 text-sm font-mono text-accent" { (item.doc_number.as_str()) }
            td class="px-4 py-3 text-sm font-medium" { (item.party_name.as_str()) }
            td class="px-4 py-3 text-sm" {
                span class=(format!("inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium {dir_cls}"))
                { (dir_label) }
            }
            td class="px-4 py-3 text-sm font-mono tabular-nums text-right" { "¥" (fmt_amount(item.amount)) }
            td class="px-4 py-3 text-sm text-fg-2" { (item.int_order_no.as_deref().unwrap_or("—")) }
            td class="px-4 py-3 text-sm text-fg-2" { (item.ext_order_no.as_deref().unwrap_or("—")) }
            td class="px-4 py-3 text-sm text-fg-2 max-w-[200px] truncate" { (item.description.as_str()) }
        }
    }
}

fn adjustment_table(
    items: &[AdjustmentRow],
    total: u64,
    page: u32,
    page_size: u32,
    query_string: &str,
    list_path: &str,
) -> Markup {
    let total_pages = ((total as f64) / (page_size as f64)).ceil() as u32;
    html! {
        div id="data-card" class="data-card" {
            div class="overflow-x-auto" {
                table class="data-table w-full" {
                    thead {
                        tr class="border-b border-border-soft" {
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider" { "调整日期" }
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider" { "调整单号" }
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider" { "往来方" }
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider" { "方向" }
                            th class="px-4 py-3 text-right text-xs font-medium text-fg-2 uppercase tracking-wider" { "金额" }
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider" { "内部订单号" }
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider" { "外部订单号" }
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase tracking-wider" { "说明" }
                        }
                    }
                    tbody class="divide-y divide-border-soft" {
                        @for item in items { (adjustment_row(item)) }
                        @if items.is_empty() {
                            tr { td colspan="8" class="px-4 py-12 text-center text-muted text-sm" { "暂无调整记录" } }
                        }
                    }
                }
            }
            @if total > page_size as u64 {
                (pagination(list_path, query_string, total, page, total_pages))
            }
        }
    }
}

#[require_permission("FMS", "read")]
pub async fn get_ar_list(
    _path: ArAdjustmentListPath,
    ctx: RequestContext,
    Query(q): Query<ListQuery>,
) -> Result<Html<String>> {
    render_list(
        ctx,
        CounterpartyType::Customer,
        q,
        ArAdjustmentListPath::PATH,
        "应收调整",
        ArAdjustmentCreatePath::PATH,
    )
    .await
}

#[require_permission("FMS", "read")]
pub async fn get_ap_list(
    _path: ApAdjustmentListPath,
    ctx: RequestContext,
    Query(q): Query<ListQuery>,
) -> Result<Html<String>> {
    render_list(
        ctx,
        CounterpartyType::Supplier,
        q,
        ApAdjustmentListPath::PATH,
        "应付调整",
        ApAdjustmentCreatePath::PATH,
    )
    .await
}

async fn render_list(
    ctx: RequestContext,
    party_type: CounterpartyType,
    q: ListQuery,
    list_path: &'static str,
    title: &str,
    create_path: &'static str,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    let page = q.page.unwrap_or(1).max(1);
    let page_size = 20u32;
    let keyword_val = q.keyword.as_deref().unwrap_or("").to_string();
    let _ = party_type; // party_type 已通过 filter 传入，此处保留供未来按往来方类型分 tab

    let filter = AdjustmentFilter {
        party_type: Some(party_type),
        keyword: if keyword_val.is_empty() { None } else { Some(keyword_val.clone()) },
        ..Default::default()
    };

    let result = state
        .adjustment_service()
        .list_adjustments(
            &service_ctx,
            &mut conn,
            filter,
            abt_core::shared::types::PageParams::new(page, page_size),
        )
        .await
        .unwrap_or_else(|_| PaginatedResult::new(vec![], 0, page, page_size));

    let mut parts: Vec<String> = Vec::new();
    if !keyword_val.is_empty() {
        parts.push(format!("keyword={}", url_encode(&keyword_val)));
    }
    let query_string = if parts.is_empty() { String::new() } else { format!("?{}", parts.join("&")) };

    let content = html! {
        div {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { (title) }
                a href=(create_path)
                    class="inline-flex items-center gap-2 px-4 py-2 rounded-sm bg-accent text-accent-on text-sm font-medium cursor-pointer hover:bg-accent-hover"
                { (icon::plus_icon("w-4 h-4")) "新建调整" }
            }
            form class="flex items-center gap-3 mb-5 flex-wrap"
                hx-get=(list_path)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#data-card"
                hx-select="#data-card"
                hx-swap="outerHTML"
                hx-push-url="true"
            {
                div class="relative flex-1 max-w-xs icon:absolute icon:left-3 icon:top-1/2 icon:-translate-y-1/2 icon:w-4 icon:h-4 icon:text-muted"
                {
                    (icon::search_icon(""))
                    input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input"
                        type="text" name="keyword" placeholder="搜索单号或往来方…"
                        value=(keyword_val);
                }
            }
            (adjustment_table(&result.items, result.total, result.page, result.page_size, &query_string, list_path))
        }
    };

    let page_html = admin_page(is_htmx, title, &claims, "finance", list_path, "财务管理", None, content, &nav_filter);
    Ok(Html(page_html.into_string()))
}

fn url_encode(s: &str) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            write!(out, "{}", b as char).unwrap();
        } else {
            write!(out, "%{:02X}", b).unwrap();
        }
    }
    out
}
