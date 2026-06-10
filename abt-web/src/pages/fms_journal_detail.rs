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
        div class="page-header" {
            div class="page-header-left" {
                a class="back-link" href=(JournalListPath::PATH) { "\u{2190} 返回列表" }
                h1 class="page-title" {
                    "单号 " (journal.doc_number)
                    " "
                    span class=(format!("status-pill {s_class}")) { (s_text) }
                }
            }
        }

        // ── 基本信息 ──
        div class="info-card" {
            h3 { "基本信息" }
            div class="info-grid" {
                div class="info-item" { label { "单号" } span class="mono" { (journal.doc_number) } }
                div class="info-item" { label { "日记账类型" } span { (journal_type_label(&journal.journal_type)) } }
                div class="info-item" { label { "方向" } span { (direction_text(&journal.direction)) } }
                div class="info-item" {
                    label { "金额" }
                    span class="mono" style=(format!("font-weight:700;color:{}", amount_color(&journal.direction))) {
                        (fmt_direction_amount(journal.amount, &journal.direction))
                    }
                }
                div class="info-item" { label { "银行账户" } span class="mono" { (journal.bank_account) } }
                div class="info-item" { label { "交易日期" } span { (journal.transaction_date.format("%Y-%m-%d")) } }
                div class="info-item" { label { "期间" } span class="mono" { (journal.period) } }
                div class="info-item" { label { "状态" } span { (s_text) } }
                div class="info-item span-2" { label { "备注" } span { (if journal.remark.is_empty() { "—".into() } else { journal.remark.clone() }) } }
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
