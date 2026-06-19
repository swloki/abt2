use std::collections::HashMap;

use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;

use abt_core::gl::account::GlAccountService;
use abt_core::gl::entry::{GlEntry, GlEntryLine, GlEntryService};
use abt_core::gl::enums::EntryStatus;
use abt_core::shared::enums::document_type::DocumentType;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::gl::{GlEntryDetailPath, GlEntryListPath};
use crate::utils::{fmt_amount, RequestContext};
use abt_macros::require_permission;

// ── Helpers ──

fn source_type_label(t: &DocumentType) -> &'static str {
    use DocumentType::*;
    match t {
        CashJournal => "出纳日记账",
        ExpenseReimbursement => "费用报销",
        PaymentRequest => "付款申请",
        GlEntry => "手工凭证",
        SalesInvoice => "销售发票",
        PurchaseInvoice => "采购发票",
        WriteOff => "核销",
        _ => "其他单据",
    }
}

fn status_label(s: &EntryStatus) -> (&'static str, &'static str, &'static str) {
    // (label, bg, color)
    match s {
        EntryStatus::Draft => ("Draft", "rgba(0,0,0,0.04)", "var(--muted)"),
        EntryStatus::Posted => ("Posted", "rgba(22,163,74,0.08)", "#16a34a"),
        EntryStatus::Cancelled => ("Cancelled", "rgba(220,38,38,0.08)", "#dc2626"),
    }
}

/// 详情页需要按 line.account_id 解析科目编码/名称（GlEntryLine 不含此信息）
async fn resolve_account_info(
    state: &crate::state::AppState,
    ctx: &abt_core::shared::types::ServiceContext,
    db: &mut abt_core::shared::types::PgPoolConn,
    lines: &[GlEntryLine],
) -> HashMap<i64, (String, String)> {
    let svc = state.gl_account_service();
    let mut map = HashMap::new();
    for line in lines {
        if map.contains_key(&line.account_id) {
            continue;
        }
        if let Ok(acc) = GlAccountService::get(&svc, ctx, &mut *db, line.account_id).await {
            map.insert(line.account_id, (acc.code, acc.name));
        }
    }
    map
}

// ── Handler ──

#[require_permission("GL", "read")]
pub async fn get_detail(path: GlEntryDetailPath, ctx: RequestContext) -> Result<Html<String>> {
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
    let (entry, lines) = svc.get(&service_ctx, &mut conn, path.id).await?;

    let accounts = resolve_account_info(&state, &service_ctx, &mut conn, &lines).await;

    let content = detail_page(&entry, &lines, &accounts);
    let current_path = GlEntryDetailPath { id: path.id }.to_string();
    let page_html = admin_page(
        is_htmx,
        "凭证详情",
        &claims,
        "gl",
        &current_path,
        "总账管理",
        Some(GlEntryListPath::PATH),
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

// ── Components ──

fn detail_page(
    entry: &GlEntry,
    lines: &[GlEntryLine],
    accounts: &HashMap<i64, (String, String)>,
) -> Markup {
    let (status_text, status_bg, status_color) = status_label(&entry.status);
    let src_label = source_type_label(&entry.source_type);

    // 借贷平衡自检（基于实际分录行累计，而非头表 total，避免数据漂移）
    let mut sum_debit = Decimal::ZERO;
    let mut sum_credit = Decimal::ZERO;
    for l in lines {
        sum_debit += l.debit;
        sum_credit += l.credit;
    }
    let balanced = sum_debit == sum_credit;

    html! {
        div {
            div class="flex items-center justify-between mb-6" {
                a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", GlEntryListPath::PATH)) {
                    (crate::components::icon::arrow_left_icon("w-4 h-4"))
                    "返回列表"
                }
                h1 class="text-xl font-bold text-fg tracking-tight" {
                    "凭证 " (entry.doc_number) " "
                    span style=(format!("display:inline-flex;align-items:center;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", status_bg, status_color)) {
                        (status_text)
                    }
                }
            }

            // ── 基本信息 ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
                h3 class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "基本信息" }
                div class="grid gap-4 grid-cols-2 md:grid-cols-3" {
                    div class="flex flex-col gap-1" {
                        label class="text-xs text-fg-2" { "凭证号" }
                        span class="font-mono tabular-nums" { (&entry.doc_number) }
                    }
                    div class="flex flex-col gap-1" {
                        label class="text-xs text-fg-2" { "期间" }
                        span class="font-mono tabular-nums" { (&entry.period) }
                    }
                    div class="flex flex-col gap-1" {
                        label class="text-xs text-fg-2" { "凭证日期" }
                        span { (entry.entry_date.format("%Y-%m-%d")) }
                    }
                    div class="flex flex-col gap-1" {
                        label class="text-xs text-fg-2" { "凭证字" }
                        span { (&entry.voucher_type) }
                    }
                    div class="flex flex-col gap-1" {
                        label class="text-xs text-fg-2" { "来源" }
                        span { (src_label) }
                    }
                    div class="flex flex-col gap-1" {
                        label class="text-xs text-fg-2" { "状态" }
                        span { (status_text) }
                    }
                    div class="flex flex-col gap-1" {
                        label class="text-xs text-fg-2" { "期初凭证" }
                        span { @if entry.is_opening { "是" } @else { "否" } }
                    }
                    div class="flex flex-col gap-1" {
                        label class="text-xs text-fg-2" { "借方合计" }
                        span class="font-mono tabular-nums font-semibold" { (fmt_amount(entry.total_debit)) }
                    }
                    div class="flex flex-col gap-1" {
                        label class="text-xs text-fg-2" { "贷方合计" }
                        span class="font-mono tabular-nums font-semibold" { (fmt_amount(entry.total_credit)) }
                    }
                    @if !entry.description.is_empty() {
                        div class="flex flex-col gap-1 col-span-full" {
                            label class="text-xs text-fg-2" { "摘要" }
                            span { (&entry.description) }
                        }
                    }
                }
            }

            // ── 分录行 ──
            div class="data-card" {
                div class="flex items-center justify-between px-1 mb-3" {
                    h3 class="text-base font-semibold text-fg" { "分录行" }
                    // 借贷平衡自检标识
                    span class=(format!("inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium {}", if balanced { "bg-success-bg text-success" } else { "bg-danger-bg text-danger" })) {
                        @if balanced { "借贷平衡" } @else { "借贷不平衡" }
                    }
                }
                div class="overflow-x-auto" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "科目编码" }
                                th { "科目名称" }
                                th class="text-right" { "借方" }
                                th class="text-right" { "贷方" }
                                th { "币种" }
                                th { "备注" }
                            }
                        }
                        tbody {
                            @for line in lines {
                                @let (code, name) = accounts.get(&line.account_id).cloned().unwrap_or_else(|| ("—".to_string(), format!("#{}", line.account_id)));
                                tr {
                                    td class="font-mono tabular-nums text-accent" { (code) }
                                    td { (name) }
                                    td class="font-mono tabular-nums text-right text-fg" { (fmt_amount(line.debit)) }
                                    td class="font-mono tabular-nums text-right text-fg" { (fmt_amount(line.credit)) }
                                    td class="font-mono tabular-nums text-muted text-xs" { (&line.currency) }
                                    td class="text-fg-2 text-xs" { @if line.memo.is_empty() { "—" } @else { (&line.memo) } }
                                }
                            }
                            @if lines.is_empty() {
                                tr {
                                    td colspan="6" class="text-center text-muted py-8" { "暂无分录行" }
                                }
                            }
                        }
                        // 合计行（自检）
                        tfoot {
                            tr class="font-semibold" {
                                td colspan="2" class="text-right text-fg-2" { "合计" }
                                td class=(format!("font-mono tabular-nums text-right {}", if balanced { "text-fg" } else { "text-danger" })) { (fmt_amount(sum_debit)) }
                                td class=(format!("font-mono tabular-nums text-right {}", if balanced { "text-fg" } else { "text-danger" })) { (fmt_amount(sum_credit)) }
                                td colspan="2" {}
                                td class="text-muted text-xs" {
                                    @if balanced { "✓ 借贷相等" } @else { "✗ 借贷不等" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
