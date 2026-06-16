use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::html;

use abt_core::fms::cash_journal::CashJournalService;
use abt_core::fms::enums::{CashDirection, JournalStatus, JournalType};

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms::{JournalDetailPath, JournalListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn journal_type_label(t: &JournalType) -> &'static str {
    match t {
        JournalType::SalesReceipt => "销售回款",
        JournalType::PurchasePayment => "采购付款",
        JournalType::Expense => "费用报销",
        JournalType::Payroll => "工资支付",
        JournalType::Other => "其他",
    }
}

fn direction_text(d: &CashDirection) -> &'static str {
    match d {
        CashDirection::Inflow => "流入",
        CashDirection::Outflow => "流出",
    }
}

fn status_text(s: &JournalStatus) -> (&'static str, &'static str) {
    match s {
        JournalStatus::Draft => ("草稿", "status-draft"),
        JournalStatus::Confirmed => ("已确认", "status-active"),
        JournalStatus::Cancelled => ("已取消", "status-inactive"),
    }
}

fn fmt_direction_amount(amount: rust_decimal::Decimal, d: &CashDirection) -> String {
    match d {
        CashDirection::Inflow => format!("+¥{amount:.2}"),
        CashDirection::Outflow => format!("-¥{amount:.2}"),
    }
}

fn amount_color(d: &CashDirection) -> &'static str {
    match d {
        CashDirection::Inflow => "var(--success)",
        CashDirection::Outflow => "var(--danger)",
    }
}

// ── Handler ──

#[require_permission("FMS", "read")]
pub async fn get_detail(path: JournalDetailPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    let svc = state.cash_journal_service();
    let journal = svc.get(&service_ctx, &mut conn, path.id).await?;

    let (s_text, s_class) = status_text(&journal.status);

    let content = html! { div {
        div class="flex items-center justify-between mb-6" {
            div class="flex items-center justify-between mb-6-left" {
                a class="inline-flex items-center gap-2 text-sm text-text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", JournalListPath::PATH)) { "\u{2190} 返回列表" }
                h1 class="text-xl font-bold text-fg tracking-tight" {
                    "单号 " (journal.doc_number)
                    " "
                    span class=(format!("status-pill {s_class}")) { (s_text) }
                }
            }
        }

        // ── 基本信息 ──
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
            h3 { "基本信息" }
            div class="grid gap-4" {
                div class="flex flex-col gap-1" { label { "单号" } span class="font-mono tabular-nums" { (journal.doc_number) } }
                div class="flex flex-col gap-1" { label { "日记账类型" } span { (journal_type_label(&journal.journal_type)) } }
                div class="flex flex-col gap-1" { label { "方向" } span { (direction_text(&journal.direction)) } }
                div class="flex flex-col gap-1" {
                    label { "金额" }
                    span class="font-mono tabular-nums" style=(format!("font-weight:700;color:{}", amount_color(&journal.direction))) {
                        (fmt_direction_amount(journal.amount, &journal.direction))
                    }
                }
                div class="flex flex-col gap-1" { label { "银行账户" } span class="font-mono tabular-nums" { (journal.bank_account) } }
                div class="flex flex-col gap-1" { label { "交易日期" } span { (journal.transaction_date.format("%Y-%m-%d")) } }
                div class="flex flex-col gap-1" { label { "期间" } span class="font-mono tabular-nums" { (journal.period) } }
                div class="flex flex-col gap-1" { label { "状态" } span { (s_text) } }
                div class="flex flex-col gap-1 span-2" { label { "备注" } span { (if journal.remark.is_empty() { "—".into() } else { journal.remark.clone() }) } }
            }
        }
    }};

    let current_path = JournalDetailPath { id: path.id }.to_string();
    let html = admin_page(
        is_htmx,
        "日记账详情",
        &claims,
        "finance",
        &current_path,
        "财务管理",
        Some(JournalListPath::PATH),
        content, &nav_filter,    );
    Ok(Html(html.into_string()))
}
