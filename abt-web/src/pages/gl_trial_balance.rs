use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;
use serde::Deserialize;

use abt_core::gl::entry::model::TrialBalance;
use abt_core::gl::entry::GlEntryService;
use abt_core::gl::period::{AccountingPeriod, GlPeriodService};

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::gl::{GlPeriodListPath, GlTrialBalancePath};
use crate::utils::{empty_as_none, fmt_amount, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct TrialBalanceQueryParams {
    /// 期间（YYYY-MM）；未指定时默认取第一个 open 期间
    #[serde(default, deserialize_with = "empty_as_none")]
    pub period: Option<String>,
}

// ── Handlers ──

#[require_permission("GL", "read")]
pub async fn get_trial_balance(
    _path: GlTrialBalancePath,
    ctx: RequestContext,
    Query(params): Query<TrialBalanceQueryParams>,
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

    let period_svc = state.gl_period_service();
    let all_periods = period_svc
        .list(&service_ctx, &mut conn, Default::default())
        .await?;

    // 解析当前选中期间：优先 query；否则取第一个 open 期间；否则取最后一个期间
    let selected_period = params
        .period
        .clone()
        .or_else(|| {
            all_periods
                .iter()
                .find(|p| p.status == abt_core::gl::enums::PeriodStatus::Open)
                .map(|p| p.name.clone())
        })
        .or_else(|| all_periods.last().map(|p| p.name.clone()))
        .unwrap_or_default();

    let trial = if selected_period.is_empty() {
        TrialBalance {
            rows: Vec::new(),
            total_debit: Decimal::ZERO,
            total_credit: Decimal::ZERO,
        }
    } else {
        state
            .gl_entry_service()
            .trial_balance(&service_ctx, &mut conn, selected_period.clone())
            .await?
    };

    let content = trial_balance_page(&trial, &all_periods, &selected_period);
    let page_html = admin_page(
        is_htmx,
        "试算平衡表",
        &claims,
        "gl",
        GlTrialBalancePath::PATH,
        "总账管理",
        None,
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

// ── Helpers ──

// 期初余额由后端 TrialBalanceRow.opening_balance 直接提供
// （= gl_accounts.opening_balance 静态期初 + 本期之前 posted 累计），
// 前端无需按 balance_direction 倒推。

// ── Components ──

fn trial_balance_page(
    trial: &TrialBalance,
    all_periods: &[AccountingPeriod],
    selected_period: &str,
) -> Markup {
    html! {
        div {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "试算平衡表" }
            }
            (period_selector(all_periods, selected_period))
            (trial_card(trial))
        }
    }
}

/// 期间选择：select change 触发 hx-get 重新查询
fn period_selector(all_periods: &[AccountingPeriod], selected: &str) -> Markup {
    html! {
        form
            class="flex items-center gap-3 mb-5 flex-wrap"
            id="gl-trial-filter-form"
            hx-get=(GlTrialBalancePath::PATH)
            hx-trigger="change"
            hx-target="#gl-trial-data-card"
            hx-select="#gl-trial-data-card"
            hx-swap="outerHTML"
            hx-push-url="true"
            hx-include="#gl-trial-filter-form"
        {
            label class="text-sm text-fg-2" for="period" { "期间" }
            select
                class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer min-w-[160px]"
                id="period"
                name="period"
            {
                @if all_periods.is_empty() {
                    option value="" { "无可用期间" }
                }
                @for p in all_periods {
                    option value=(p.name) selected[p.name == selected] {
                        (p.name)
                        " ("
                        (p.status.as_str())
                        ")"
                    }
                }
            }
            a href=(GlPeriodListPath::PATH) class="text-xs text-accent hover:underline ml-2" {
                "管理期间 →"
            }
        }
    }
}

fn trial_card(trial: &TrialBalance) -> Markup {
    let balanced = trial.total_debit == trial.total_credit;
    // 期初合计：所有行的 opening_balance 之和
    let total_opening: Decimal = trial.rows.iter().map(|r| r.opening_balance).sum();
    let total_end: Decimal = trial.rows.iter().map(|r| r.end_balance).sum();

    html! {
        div class="data-card" id="gl-trial-data-card" {
            div class="overflow-x-auto" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "科目编码" }
                            th { "科目名称" }
                            th class="text-right" { "期初余额" }
                            th class="text-right" { "本期借方" }
                            th class="text-right" { "本期贷方" }
                            th class="text-right" { "期末余额" }
                        }
                    }
                    tbody {
                        @for row in &trial.rows {
                            tr {
                                td class="font-mono tabular-nums text-accent" { (&row.code) }
                                td { (&row.name) }
                                td class="font-mono tabular-nums text-right text-fg-2" {
                                    (fmt_amount(row.opening_balance))
                                }
                                td class="font-mono tabular-nums text-right text-fg" {
                                    (fmt_amount(row.period_debit))
                                }
                                td class="font-mono tabular-nums text-right text-fg" {
                                    (fmt_amount(row.period_credit))
                                }
                                td class="font-mono tabular-nums text-right text-fg font-medium" {
                                    (fmt_amount(row.end_balance))
                                }
                            }
                        }
                        @if trial.rows.is_empty() {
                            tr {
                                td colspan="6" class="text-center text-muted py-8" { "该期间暂无凭证数据" }
                            }
                        }
                    }
                    tfoot {
                        tr class="border-t-2 border-border font-medium bg-surface" {
                            td colspan="2" class="text-fg" { "合计" }
                            td class="font-mono tabular-nums text-right text-fg-2" {
                                (fmt_amount(total_opening))
                            }
                            td class="font-mono tabular-nums text-right text-fg" {
                                (fmt_amount(trial.total_debit))
                            }
                            td class="font-mono tabular-nums text-right text-fg" {
                                (fmt_amount(trial.total_credit))
                            }
                            td class="font-mono tabular-nums text-right text-fg-2" {
                                (fmt_amount(total_end))
                            }
                        }
                        tr {
                            td  colspan="6"
                                class={
                                    "text-xs py-2 "
                                    @if balanced { "text-success" } @else {
                                        "text-danger font-medium"
                                    }
                                }
                            {
                                @if balanced { "✓ 借贷平衡（借方合计 = 贷方合计）" } @else {
                                    "✗ 借贷不平：借方合计 ≠ 贷方合计，请检查凭证数据"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
