use axum::response::Html;
use axum_extra::routing::TypedPath;
use chrono::{Datelike, Utc};
use maud::{html, Markup};
use rust_decimal::Decimal;

use abt_core::fms::cash_journal::CashJournalService;
use abt_core::fms::cash_journal::model::CashJournal;
use abt_core::fms::enums::{CashDirection, JournalType};
use abt_core::fms::expense::ExpenseReimbursementService;
use abt_core::fms::expense::model::ExpenseReimbursement;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms::{
    CostAnalysisPath, ExpenseCreatePath, ExpenseListPath, FmsDashboardPath, JournalCreatePath,
    JournalListPath, WriteoffListPath,
};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Handler ──

#[require_permission("FMS", "read")]
pub async fn get_dashboard(
    _path: FmsDashboardPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    let journal_svc = state.cash_journal_service();
    let expense_svc = state.expense_service();

    let now = Utc::now();
    let current_period = now.format("%Y-%m").to_string();

    let balance = journal_svc
        .get_balance(&service_ctx, &mut conn, current_period.clone())
        .await
        .unwrap_or_else(|_| abt_core::fms::cash_journal::model::BalanceSummary {
            total_inflow: Decimal::ZERO,
            total_outflow: Decimal::ZERO,
            net_balance: Decimal::ZERO,
            currency: "CNY".to_string(),
        });

    let (pending_count, pending_amount) = expense_svc
        .pending_summary(&service_ctx, &mut conn)
        .await
        .unwrap_or((0i64, Decimal::ZERO));

    let distribution = journal_svc
        .distribution_by_type(&service_ctx, &mut conn, current_period.clone())
        .await
        .unwrap_or_default();

    let trend = journal_svc
        .monthly_trend(&service_ctx, &mut conn, 5)
        .await
        .unwrap_or_default();

    let recent_journals = journal_svc
        .list_recent(&service_ctx, &mut conn, 5)
        .await
        .unwrap_or_default();

    let pending_expenses = expense_svc
        .list_pending(&service_ctx, &mut conn, 5)
        .await
        .unwrap_or_default();

    let content = fms_dashboard_page(
        &current_period, now, &balance, pending_count, pending_amount,
        &distribution, &trend, &recent_journals, &pending_expenses,
    );
    let page_html = admin_page(
        is_htmx, "财务管理", &claims, "finance", FmsDashboardPath::PATH,
        "财务管理", None, content, &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

// ── Helpers ──

fn fmt_wan(d: Decimal) -> String {
    let wan = d / Decimal::from(10000);
    let rounded = wan.round_dp(1);
    format!("{rounded}")
}

fn journal_type_label(t: &JournalType) -> &'static str {
    match t {
        JournalType::SalesReceipt => "销售回款",
        JournalType::PurchasePayment => "采购付款",
        JournalType::Expense => "费用报销",
        JournalType::Payroll => "工资支付",
        JournalType::Other => "其他",
    }
}

fn fmt_amount(amount: Decimal, direction: &CashDirection) -> String {
    match direction {
        CashDirection::Inflow => format!("+¥{amount:.2}"),
        CashDirection::Outflow => format!("-¥{amount:.2}"),
    }
}

fn amount_color_cls(direction: &CashDirection) -> &'static str {
    match direction {
        CashDirection::Inflow => "text-success",
        CashDirection::Outflow => "text-danger",
    }
}

// ── Stat Card ──

fn stat_card(title: &str, value: &str, sub: Markup, icon_svg: Markup, icon_bg: &str, icon_fg: &str) -> Markup {
    html! {
        div class="data-card flex items-center gap-4 p-5" {
            div class="w-11 h-11 rounded-md grid place-items-center shrink-0"
                style=(format!("background:{};color:{}", icon_bg, icon_fg)) {
                (icon_svg)
            }
            div class="flex-1 min-w-0" {
                div class="text-sm text-muted" { (title) }
                div class="text-2xl font-bold font-mono tabular-nums text-fg mt-1" {
                    (maud::PreEscaped(value))
                }
                div class="text-xs text-muted mt-1" { (sub) }
            }
        }
    }
}

// ── Page ──

#[allow(clippy::too_many_arguments)]
fn fms_dashboard_page(
    current_period: &str,
    now: chrono::DateTime<Utc>,
    balance: &abt_core::fms::cash_journal::model::BalanceSummary,
    pending_count: i64,
    pending_amount: Decimal,
    distribution: &[(i16, Decimal)],
    trend: &[(String, Decimal, Decimal)],
    journals: &[CashJournal],
    expenses: &[ExpenseReimbursement],
) -> Markup {
    let dist_max = distribution.iter().map(|(_, v)| *v).max().unwrap_or(Decimal::ONE);
    let dist_max = if dist_max == Decimal::ZERO { Decimal::ONE } else { dist_max };

    let trend_max = trend.iter()
        .flat_map(|(_, inf, out)| [*inf, *out])
        .max()
        .unwrap_or(Decimal::ONE);
    let trend_max = if trend_max == Decimal::ZERO { Decimal::ONE } else { trend_max };

    let dist_types = [
        (JournalType::SalesReceipt as i16, "销售回款", "#16a34a"),
        (JournalType::PurchasePayment as i16, "采购付款", "#dc2626"),
        (JournalType::Expense as i16, "费用报销", "#d97706"),
        (JournalType::Payroll as i16, "工资支付", "#7c3aed"),
    ];

    html! {
        // ── Page Header ──
        div class="flex items-center justify-between mb-6" {
            h1 class="text-xl font-bold text-fg tracking-tight" { "财务管理总览" }
            div class="flex gap-3" {
                a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(JournalCreatePath::PATH) {
                    (icon::plus_icon("w-4 h-4"))
                    " 新建日记账"
                }
            }
        }

        // ── Stat Cards ──
        div class="grid grid-cols-2 lg:grid-cols-5 gap-4 mb-6" {
            (stat_card(
                "本月流入",
                &format!("¥{} <span class=\"text-sm text-muted\">万</span>", fmt_wan(balance.total_inflow)),
                html! { "已确认流入" },
                icon::dollar_icon("w-5 h-5"),
                "#dcfce7", "#16a34a",
            ))
            (stat_card(
                "本月流出",
                &format!("¥{} <span class=\"text-sm text-muted\">万</span>", fmt_wan(balance.total_outflow)),
                html! { "已确认流出" },
                icon::dollar_icon("w-5 h-5"),
                "#fee2e2", "#dc2626",
            ))
            @let net = balance.net_balance;
            @let net_sign = if net >= Decimal::ZERO { "+" } else { "" };
            (stat_card(
                "净现金流",
                &format!("{}¥{} <span class=\"text-sm text-muted\">万</span>", net_sign, fmt_wan(net.abs())),
                html! { "期间 " span class="font-semibold" { (current_period) } },
                icon::dollar_icon("w-5 h-5"),
                "#dbeafe", "#2563eb",
            ))
            (stat_card(
                "待核销金额",
                &format!("¥{} <span class=\"text-sm text-muted\">万</span>", fmt_wan(Decimal::ZERO)),
                html! { "暂无核销数据" },
                icon::dollar_icon("w-5 h-5"),
                "#fef3c7", "#d97706",
            ))
            (stat_card(
                "待审报销",
                &pending_count.to_string(),
                html! { "金额合计 " span class="font-semibold text-[#7c3aed]" { (format!("¥{}万", fmt_wan(pending_amount))) } },
                icon::dollar_icon("w-5 h-5"),
                "#ede9fe", "#7c3aed",
            ))
        }

        // ── Quick Entry ──
        div class="mb-6" {
            h2 class="text-lg font-semibold text-fg flex items-center gap-2 mb-4" {
                (icon::bolt_icon("w-4 h-4"))
                " 快捷操作"
            }
            div class="grid grid-cols-2 lg:grid-cols-4 gap-4" {
                (quick_entry_card(JournalCreatePath::PATH, "新建日记账", "录入现金收支", "CashJournal", "blue"))
                (quick_entry_card(ExpenseCreatePath::PATH, "费用报销", "提交报销申请", "Expense", "purple"))
                (quick_entry_card(WriteoffListPath::PATH, "核销管理", "按单核销收款/付款", "WriteOff", "green"))
                (quick_entry_card(CostAnalysisPath::PATH, "成本核算", "利润分析 & P&L", "CostEntry", "orange"))
            }
        }

        // ── Two-Column: Recent Journals + Pending Expenses ──
        div class="grid grid-cols-1 lg:grid-cols-2 gap-5 mb-6" {
            // Recent Journals
            div class="data-card overflow-hidden" {
                div class="px-4 py-3 border-b border-border-soft text-sm font-semibold text-fg flex items-center justify-between" {
                    span class="flex items-center gap-2" {
                        (icon::dollar_icon("w-4 h-4"))
                        " 最近日记账"
                    }
                    a href=(JournalListPath::PATH) class="text-xs text-accent font-medium hover:underline" { "查看全部 →" }
                }
                div class="p-2" {
                    @if journals.is_empty() {
                        div class="text-center py-8 text-sm text-muted" { "暂无日记账记录" }
                    } @else {
                        @for j in journals {
                            @let dot_cls = if j.direction == CashDirection::Inflow { "bg-success" } else { "bg-danger" };
                            @let amt_cls = amount_color_cls(&j.direction);
                            div class="flex items-center gap-3 px-3 py-2.5 rounded-sm hover:bg-accent-bg transition-colors" {
 div class=(format!("w-2.5 h-2.5 rounded-full shrink-0 {}", dot_cls)) {}
                                div class="flex-1 min-w-0" {
                                    div class="text-sm font-medium text-fg truncate font-mono" { (j.doc_number) }
                                    div class="text-xs text-muted mt-0.5 truncate" { (journal_type_label(&j.journal_type)) " · " (j.remark.as_str()) }
                                }
                                div class="text-right shrink-0" {
                                    div class=(format!("text-sm font-bold font-mono {}", amt_cls)) {
                                        (fmt_amount(j.amount, &j.direction))
                                    }
                                    div class="text-xs text-muted font-mono" { (j.transaction_date.format("%m-%d")) }
                                }
                            }
                        }
                    }
                }

            }
            // Pending Expenses
            div class="data-card overflow-hidden" {
                div class="px-4 py-3 border-b border-border-soft text-sm font-semibold text-fg flex items-center justify-between" {
                    span class="flex items-center gap-2" {
                        (icon::alert_triangle_icon("w-4 h-4"))
                        " 待审批报销"
                    }
                    a href=(ExpenseListPath::PATH) class="text-xs text-accent font-medium hover:underline" { "查看全部 →" }
                }
                div class="p-2" {
                    @if expenses.is_empty() {
                        div class="text-center py-8 text-sm text-muted" { "暂无待审批报销" }
                    } @else {
                        @for e in expenses {
                            div class="flex items-center gap-3 px-3 py-2.5 rounded-sm hover:bg-accent-bg transition-colors" {
                                div class="w-8 h-8 rounded-full grid place-items-center shrink-0 text-xs font-bold text-white bg-accent" {
                                    (e.doc_number.chars().next().unwrap_or('—'))
                                }
                                div class="flex-1 min-w-0" {
                                    div class="text-sm font-medium text-fg truncate" { (e.remark.as_str()) }
                                    div class="text-xs text-muted font-mono" { (e.doc_number) }
                                }
                                div class="text-right shrink-0" {
                                    div class="text-sm font-bold font-mono text-fg" {
                                        (format!("¥{:.2}", e.total_amount))
                                    }
                                    span class="inline-flex items-center rounded-full text-xs font-medium px-2 py-0.5 bg-[#fff8eb] text-[#d46b08]" { "待审批" }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Distribution + Trend ──
        div class="grid grid-cols-1 lg:grid-cols-[1fr_1.5fr] gap-5 mb-6" {
            // Type Distribution
            div class="data-card overflow-hidden" {
                div class="px-4 py-3 border-b border-border-soft text-sm font-semibold text-fg flex items-center gap-2" {
                    (icon::dollar_icon("w-4 h-4"))
                    " 本月日记账分布"
                }
                div class="p-5 flex flex-col gap-5" {
                    @for (type_id, label, color) in dist_types {
                        @let amount = distribution.iter().find(|(t, _)| *t == type_id).map(|(_, v)| *v).unwrap_or(Decimal::ZERO);
                        @let pct = if dist_max > Decimal::ZERO {
                            (amount / dist_max * Decimal::from(100)).round_dp(0)
                        } else {
                            Decimal::ZERO
                        };
                        (distribution_bar(label, &format!("¥{}万", fmt_wan(amount)), color, &format!("{}%", pct)))
                    }
                }
            }

            // Monthly Trend
            div class="data-card overflow-hidden" {
                div class="px-4 py-3 border-b border-border-soft text-sm font-semibold text-fg flex items-center gap-2" {
                    (icon::trending_up_icon("w-4 h-4"))
                    " 近6月现金流趋势（万元）"
                }
                div class="p-5" {
                    div class="flex items-center justify-between text-xs text-muted mb-5" {
                        div class="flex items-center gap-4" {
                            span class="flex items-center gap-1" {
                                span class="w-2.5 h-0.5 rounded bg-success inline-block" {}
                                "流入"
                            }
                            span class="flex items-center gap-1" {
                                span class="w-2.5 h-0.5 rounded bg-danger inline-block" {}
                                "流出"
                            }
                            span class="flex items-center gap-1" {
                                span class="w-2.5 h-0.5 rounded bg-accent inline-block" {}
                                "净额"
                            }
                        }
                        span { "单位：万元" }
                    }
                    div class="grid grid-cols-6 gap-2" {
                        @for i in 0..6 {
                            @let month_offset = 5 - i;
                            @let target_date = (chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), 1).unwrap())
                                .checked_sub_months(chrono::Months::new(month_offset as u32))
                                .unwrap();
                            @let target_period = target_date.format("%Y-%m").to_string();
                            @let month_label = format!("{}月", target_date.month());
                            @let is_current = month_offset == 0;
                            @let (inflow, outflow) = trend.iter()
                                .find(|(p, _, _)| p == &target_period)
                                .map(|(_, i, o)| (*i, *o))
                                .unwrap_or((Decimal::ZERO, Decimal::ZERO));
                            @let net = inflow - outflow;
                            @let inflow_h = (inflow / trend_max * Decimal::from(130) + Decimal::from(10)).round_dp(0);
                            @let outflow_h = (outflow / trend_max * Decimal::from(130) + Decimal::from(10)).round_dp(0);
                            @let net_display = if net >= Decimal::ZERO { format!("+{}", fmt_wan(net)) } else { fmt_wan(net) };
                            @let net_cls = if is_current { "text-accent" }
                                else if net >= Decimal::ZERO { "text-success" }
                                else { "text-danger" };
                            (trend_bar(&month_label, inflow_h, outflow_h, &net_display, net_cls, is_current))
                        }
                    }
                }
            }
        }
    }
}

// ── Quick Entry Card ──

fn quick_entry_card(href: &str, title: &str, desc: &str, badge: &str, color: &str) -> Markup {
    let (icon_svg, title_cls, badge_cls) = match color {
        "blue" => (icon::dollar_icon("w-5 h-5"), "text-accent", "bg-[#e6f4ff] text-accent"),
        "purple" => (icon::alert_triangle_icon("w-5 h-5"), "text-[#7c3aed]", "bg-[#f3e8ff] text-[#7c3aed]"),
        "green" => (icon::check_circle_icon("w-5 h-5"), "text-success", "bg-[#dcfce7] text-success"),
        "orange" => (icon::trending_up_icon("w-5 h-5"), "text-[#d97706]", "bg-[#fef3c7] text-[#d97706]"),
        _ => (icon::grid_icon("w-5 h-5"), "text-fg", "bg-accent-bg text-muted"),
    };
    html! {
        a href=(href) class="data-card block p-5 no-underline hover:shadow-[var(--shadow-card-hover)] transition-shadow duration-200" {
            div class="flex items-center gap-3 mb-3" {
                span class=(title_cls) { (icon_svg) }
 span class=(format!("text-[10px] font-bold px-2 py-0.5 rounded-full {}", badge_cls)) {
 (badge.to_uppercase())
 }
            }
 div class=(format!("text-base font-semibold {} mb-1", title_cls)) { (title) }
            div class="text-sm text-muted" { (desc) }
        }
    }
}

// ── Distribution Bar ──

fn distribution_bar(label: &str, value: &str, color: &str, width: &str) -> Markup {
    html! {
        div {
            div class="flex items-center justify-between mb-2" {
                span class="text-sm font-medium text-fg" { (label) }
                span class="text-sm font-bold font-mono" style=(format!("color:{}", color)) { (value) }
            }
            div class="h-1.5 bg-[rgba(0,0,0,0.06)] rounded-full overflow-hidden" {
                div class="h-full rounded-full" style=(format!("width:{};background:{}", width, color)) {}
            }
        }
    }
}

// ── Trend Bar ──

fn trend_bar(month: &str, inflow_h: Decimal, outflow_h: Decimal, net: &str, net_cls: &str, is_current: bool) -> Markup {
    let month_cls = if is_current { "text-xs font-bold mt-2" } else { "text-xs text-muted mt-2 font-medium" };
    let net_weight = if is_current { "font-extrabold" } else { "font-bold" };
    html! {
        div class="text-center" {
            div class="flex flex-col items-center gap-1 h-[140px] justify-end" {
                div class="relative overflow-hidden w-full max-w-[48px] rounded-sm"
                    style=(format!("height:{}px;background:linear-gradient(180deg,rgba(37,99,235,0.2),rgba(37,99,235,0.04));border-top:2.5px solid var(--accent)", inflow_h)) {}
                div class="relative overflow-hidden w-full max-w-[48px] rounded-sm"
                    style=(format!("height:{}px;background:linear-gradient(180deg,rgba(220,38,38,0.15),rgba(220,38,38,0.03));border-top:2.5px solid var(--danger)", outflow_h)) {}
            }
            div class=(month_cls) { (month) }
 div class=(format!("text-xs font-mono {} {}", net_cls, net_weight)) { (net) }
        }
    }
}
