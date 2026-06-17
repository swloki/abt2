use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::fms::cash_journal::model::{CashJournal, CashJournalFilter};
use abt_core::fms::cash_journal::CashJournalService;
use abt_core::fms::enums::{CashDirection, CounterpartyType, JournalStatus, JournalType};
use abt_core::shared::types::PaginatedResult;
use std::collections::HashMap;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms::{JournalCreatePath, JournalDetailPath, JournalListPath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Helpers ──

fn journal_type_label(t: &JournalType) -> (&'static str, &'static str, &'static str) {
    match t {
        JournalType::SalesReceipt => ("销售回款", "rgba(22,163,74,0.08)", "#16a34a"),
        JournalType::PurchasePayment => ("采购付款", "rgba(37,99,235,0.08)", "#2563eb"),
        JournalType::Expense => ("费用报销", "rgba(217,119,6,0.08)", "#b45309"),
        JournalType::Payroll => ("工资支付", "rgba(124,58,237,0.08)", "#7c3aed"),
        JournalType::Other => ("其他", "rgba(0,0,0,0.04)", "var(--muted)"),
    }
}

fn direction_label(d: &CashDirection) -> (&'static str, &'static str, &'static str) {
    match d {
        CashDirection::Inflow => ("↑ 流入", "rgba(22,163,74,0.08)", "#16a34a"),
        CashDirection::Outflow => ("↓ 流出", "rgba(220,38,38,0.08)", "#dc2626"),
    }
}

fn status_label(s: &JournalStatus) -> (&'static str, &'static str, &'static str) {
    match s {
        JournalStatus::Draft => ("草稿", "rgba(0,0,0,0.04)", "var(--muted)"),
        JournalStatus::Confirmed => ("已确认", "rgba(22,163,74,0.08)", "#16a34a"),
        JournalStatus::Cancelled => ("已取消", "rgba(220,38,38,0.08)", "#dc2626"),
    }
}

fn fmt_amount(amount: rust_decimal::Decimal, direction: &CashDirection) -> String {
    let abs = format!("{amount:.2}");
    match direction {
        CashDirection::Inflow => format!("+¥{abs}"),
        CashDirection::Outflow => format!("-¥{abs}"),
    }
}

fn amount_color(d: &CashDirection) -> &'static str {
    match d {
        CashDirection::Inflow => "var(--success)",
        CashDirection::Outflow => "var(--danger)",
    }
}

fn counterparty_name(item: &CashJournal, names: &HashMap<(CounterpartyType, i64), String>) -> String {
    match item.counterparty_type {
        CounterpartyType::Customer | CounterpartyType::Supplier | CounterpartyType::Employee => {
            names.get(&(item.counterparty_type, item.counterparty_id))
                .cloned()
                .unwrap_or_else(|| item.counterparty_id.to_string())
        }
        CounterpartyType::Other => "—".to_string(),
    }
}

async fn resolve_counterparty_names(
    state: &crate::state::AppState,
    ctx: &abt_core::shared::types::ServiceContext,
    db: &mut abt_core::shared::types::PgPoolConn,
    items: &[CashJournal],
) -> HashMap<(CounterpartyType, i64), String> {
    use abt_core::master_data::customer::CustomerService;
    use abt_core::master_data::supplier::SupplierService;
    use abt_core::shared::identity::UserService;
    let mut names = HashMap::new();
    let customer_ids: Vec<i64> = items.iter()
        .filter(|j| j.counterparty_type == CounterpartyType::Customer)
        .map(|j| j.counterparty_id)
        .collect();
    let supplier_ids: Vec<i64> = items.iter()
        .filter(|j| j.counterparty_type == CounterpartyType::Supplier)
        .map(|j| j.counterparty_id)
        .collect();
    let employee_ids: Vec<i64> = items.iter()
        .filter(|j| j.counterparty_type == CounterpartyType::Employee)
        .map(|j| j.counterparty_id)
        .collect();

    if !customer_ids.is_empty() {
        let svc = state.customer_service();
        if let Ok(customers) = CustomerService::get_by_ids(&svc, ctx, &mut *db, &customer_ids).await {
            for c in customers {
                names.insert((CounterpartyType::Customer, c.id), c.name);
            }
        }
    }
    if !supplier_ids.is_empty() {
        let svc = state.supplier_service();
        for sid in &supplier_ids {
            if let Ok(s) = SupplierService::get(&svc, ctx, &mut *db, *sid).await {
                names.insert((CounterpartyType::Supplier, s.id), s.name);
            }
        }
    }
    if !employee_ids.is_empty() {
        let svc = state.user_service();
        if let Ok(users) = UserService::get_users_by_ids(&svc, ctx, &mut *db, employee_ids.clone()).await {
            for u in users {
                names.insert((CounterpartyType::Employee, u.user.user_id), u.user.display_name.unwrap_or(u.user.username));
            }
        }
    }
    names
}

fn build_query_string(params: &JournalQueryParams) -> String {
    let mut parts = Vec::new();
    if let Some(ref v) = params.keyword {
        parts.push(format!("keyword={v}"));
    }
    if let Some(v) = params.journal_type {
        parts.push(format!("journal_type={v}"));
    }
    if let Some(v) = params.direction {
        parts.push(format!("direction={v}"));
    }
    if let Some(v) = params.status {
        parts.push(format!("status={v}"));
    }
    if let Some(v) = params.page
        && v > 1 {
            parts.push(format!("page={v}"));
        }
    if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    }
}

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct JournalQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub journal_type: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub direction: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("FMS", "read")]
pub async fn get_list(
    _path: JournalListPath,
    ctx: RequestContext,
    Query(params): Query<JournalQueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_create = ctx.has_permission("FMS", "create").await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.cash_journal_service();

    let filter = build_filter(&params);
    let page_num = params.page.unwrap_or(1);
    let result = svc
        .list(&service_ctx, &mut conn, filter, abt_core::shared::types::PageParams::new(page_num, 20))
        .await?;

    let counterparty_names = resolve_counterparty_names(&state, &service_ctx, &mut conn, &result.items).await;
    let content = journal_list_page(&result, &params, &counterparty_names, can_create);
    let page_html = admin_page(
        is_htmx,
        "出纳日记账",
        &claims,
        "finance",
        JournalListPath::PATH,
        "财务管理",
        None,
        content, &nav_filter,    );
    Ok(Html(page_html.into_string()))
}

fn build_filter(params: &JournalQueryParams) -> CashJournalFilter {
    let status_vec = match &params.status {
        Some(1) => vec![JournalStatus::Draft],
        Some(2) => vec![JournalStatus::Confirmed],
        Some(3) => vec![JournalStatus::Cancelled],
        _ => vec![],
    };
    CashJournalFilter {
        period: None,
        journal_type: params.journal_type.and_then(JournalType::from_i16),
        status: status_vec,
        counterparty_id: None,
        transaction_date_from: None,
        transaction_date_to: None,
    }
}

// ── Components ──

fn journal_list_page(result: &PaginatedResult<CashJournal>, params: &JournalQueryParams, counterparty_names: &HashMap<(CounterpartyType, i64), String>, can_create: bool) -> Markup {
    html! {
        div {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "出纳日记账" }
                div class="flex gap-3" {
                    button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" type="button" { (icon::download_icon("w-4 h-4")) "导出" }
                    @if can_create {
                        a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(JournalCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建日记账"
                        }
                    }
                }
            }
            (journal_table_fragment(result, params, counterparty_names))
        }
    }
}

fn journal_table_fragment(result: &PaginatedResult<CashJournal>, params: &JournalQueryParams, counterparty_names: &HashMap<(CounterpartyType, i64), String>) -> Markup {
    let total_count = result.total;
    let selected_status = params.status.map(|v| v.to_string()).unwrap_or_default();

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "1".into(), label: "草稿", count: None },
        TabItem { value: "2".into(), label: "已确认", count: None },
        TabItem { value: "3".into(), label: "已取消", count: None },
    ];

    html! {
        div {
            (status_tabs_with_param(JournalListPath::PATH, "#journal-data-card", "#journal-filter-form", tabs, &selected_status, "status"))

            form class="flex items-center gap-3 mb-5 flex-wrap filter-form" id="journal-filter-form"
                hx-get=(JournalListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#journal-data-card"
                hx-select="#journal-data-card"
                hx-swap="outerHTML"
                hx-include="#journal-filter-form"
                hx-push-url="true" {
                div class="relative flex-1 max-w-xs" {
                    (icon::search_icon("w-4 h-4"))
                    input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword"
                        style="width:200px"
                        placeholder="搜索单号、往来方名称…"
                        value=(params.keyword.as_deref().unwrap_or(""));
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="journal_type" {
                    option value="" selected[params.journal_type.is_none()] { "全部类型" }
                    option value="1" selected[params.journal_type == Some(1)] { "销售回款" }
                    option value="2" selected[params.journal_type == Some(2)] { "采购付款" }
                    option value="3" selected[params.journal_type == Some(3)] { "费用报销" }
                    option value="4" selected[params.journal_type == Some(4)] { "工资支付" }
                    option value="5" selected[params.journal_type == Some(5)] { "其他" }
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="direction" {
                    option value="" selected[params.direction.is_none()] { "全部方向" }
                    option value="1" selected[params.direction == Some(1)] { "流入" }
                    option value="2" selected[params.direction == Some(2)] { "流出" }
                }
            }

            (journal_data_card(result, params, counterparty_names))
        }
    }
}

fn journal_data_card(result: &PaginatedResult<CashJournal>, params: &JournalQueryParams, counterparty_names: &HashMap<(CounterpartyType, i64), String>) -> Markup {
    let query = build_query_string(params);
    html! {
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" id="journal-data-card" {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] overflow-x-auto" {
                table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:text-left [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" {
                    thead {
                        tr {
                            th { "单号" }
                            th { "日记账类型" }
                            th { "方向" }
                            th { "金额" }
                            th { "往来方" }
                            th { "银行账户" }
                            th { "交易日期" }
                            th { "期间" }
                            th { "状态" }
                            th style="width:80px" { "操作" }
                        }
                    }
                    tbody {
                        @for item in &result.items {
                            @let (type_label, type_bg, type_color) = journal_type_label(&item.journal_type);
                            @let (dir_label, dir_bg, dir_color) = direction_label(&item.direction);
                            @let (status_text, status_bg, status_color) = status_label(&item.status);
                            @let detail_path = JournalDetailPath { id: item.id };
                            tr style="cursor:pointer" onclick=(format!("location.href='{}'", detail_path.to_string())) {
                                td class="font-mono tabular-nums" style="color:var(--accent)" { (item.doc_number) }
                                td {
                                    span style=(format!("display:inline-flex;align-items:center;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", type_bg, type_color)) {
                                        (type_label)
                                    }
                                }
                                td {
                                    span style=(format!("display:inline-flex;align-items:center;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", dir_bg, dir_color)) {
                                        (dir_label)
                                    }
                                }
                                td class="font-mono tabular-nums text-right text-[13px]" style=(format!("font-weight:600;color:{}", amount_color(&item.direction))) {
                                    (fmt_amount(item.amount, &item.direction))
                                }
                                td { (counterparty_name(item, counterparty_names)) }
                                td class="font-mono tabular-nums" style="color:var(--muted)" { (&item.bank_account) }
                                td style="font-size:12px;color:var(--muted)" { (item.transaction_date.format("%Y-%m-%d")) }
                                td class="font-mono tabular-nums" { (&item.period) }
                                td {
                                    span style=(format!("display:inline-flex;align-items:center;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", status_bg, status_color)) {
                                        (status_text)
                                    }
                                }
                                td {
                                    a href=(detail_path.to_string()) style="color:var(--accent);font-size:12px" onclick="event.stopPropagation()" { "查看" }
                                }
                            }
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="10" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无日记账记录"
                                }
                            }
                        }
                    }
                }
            }
            (pagination(JournalListPath::PATH, &query, result.total, result.page, result.total_pages))
        }
    }
}
