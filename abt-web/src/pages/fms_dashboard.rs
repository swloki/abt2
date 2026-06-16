use axum::response::Html;
use axum_extra::routing::TypedPath;
use chrono::{Datelike, Utc};
use maud::{html, Markup, PreEscaped};
use rust_decimal::Decimal;

use abt_core::fms::cash_journal::CashJournalService;
use abt_core::fms::cash_journal::model::CashJournal;
use abt_core::fms::enums::{CashDirection, JournalType};
use abt_core::fms::expense::ExpenseReimbursementService;
use abt_core::fms::expense::model::ExpenseReimbursement;

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

    // 当月期间
    let now = Utc::now();
    let current_period = now.format("%Y-%m").to_string();

    // 1. 当月收支汇总
    let balance = journal_svc
        .get_balance(&service_ctx, &mut conn, current_period.clone())
        .await
        .unwrap_or_else(|_| abt_core::fms::cash_journal::model::BalanceSummary {
            total_inflow: Decimal::ZERO,
            total_outflow: Decimal::ZERO,
            net_balance: Decimal::ZERO,
            currency: "CNY".to_string(),
        });

    // 2. 待审报销统计
    let (pending_count, pending_amount) = expense_svc
        .pending_summary(&service_ctx, &mut conn)
        .await
        .unwrap_or((0i64, Decimal::ZERO));

    // 3. 按类型分布
    let distribution = journal_svc
        .distribution_by_type(&service_ctx, &mut conn, current_period.clone())
        .await
        .unwrap_or_default();

    // 4. 近6月趋势
    let trend = journal_svc
        .monthly_trend(&service_ctx, &mut conn, 5)
        .await
        .unwrap_or_default();

    // 5. 最近5条日记账
    let recent_journals = journal_svc
        .list_recent(&service_ctx, &mut conn, 5)
        .await
        .unwrap_or_default();

    // 6. 待审报销列表
    let pending_expenses = expense_svc
        .list_pending(&service_ctx, &mut conn, 5)
        .await
        .unwrap_or_default();

    let content = fms_dashboard_page(
        &current_period,
        now,
        &balance,
        pending_count,
        pending_amount,
        &distribution,
        &trend,
        &recent_journals,
        &pending_expenses,
    );
    let page_html = admin_page(
        is_htmx,
        "财务管理",
        &claims,
        "finance",
        FmsDashboardPath::PATH,
        "财务管理",
        None,
        content, &nav_filter,    );
    Ok(Html(page_html.into_string()))
}

// ── Helpers ──

fn svg_icon(path_d: &str, size: &str) -> Markup {
    PreEscaped(format!(
        r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:{size};height:{size}"><path d="{path_d}"/></svg>"#
    ))
}

fn stat_card(title: &str, value: &str, accent: &str, sub: Markup, icon_bg: &str, icon_color: &str, icon_path: &str) -> Markup {
    html! {
        div class=(format!("mes-stat-card {accent}")) {
            div class="mes-w-[44px] h-[44px] rounded grid place-items-center shrink-0" style=(format!("background:{icon_bg};color:{icon_color}")) {
                (svg_icon(icon_path, "24px"))
            }
            div class="mes-stat-body" {
                div class="mes-text-sm text-muted mt-1" { (title) }
                div class="mes-text-2xl font-bold font-mono tabular-nums text-fg" { (PreEscaped(value)) }
                div class="mes-stat-sub" { (sub) }
            }
        }
    }
}

/// 格式化金额为 "万" 单位，保留1位小数
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

fn amount_color(direction: &CashDirection) -> &'static str {
    match direction {
        CashDirection::Inflow => "var(--success)",
        CashDirection::Outflow => "var(--danger)",
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
    // 计算分布最大值用于百分比
    let dist_max = distribution.iter().map(|(_, v)| *v).max().unwrap_or(Decimal::ONE);
    let dist_max = if dist_max == Decimal::ZERO { Decimal::ONE } else { dist_max };

    // 趋势最大值用于柱高
    let trend_max = trend.iter()
        .flat_map(|(_, inf, out)| [*inf, *out])
        .max()
        .unwrap_or(Decimal::ONE);
    let trend_max = if trend_max == Decimal::ZERO { Decimal::ONE } else { trend_max };

    // 4个分布类型固定显示
    let dist_types = [
        (JournalType::SalesReceipt as i16, "销售回款", "var(--success)", "green"),
        (JournalType::PurchasePayment as i16, "采购付款", "var(--danger)", "red"),
        (JournalType::Expense as i16, "费用报销", "var(--warn)", "orange"),
        (JournalType::Payroll as i16, "工资支付", "#7c3aed", "blue"),
    ];

    html! {
        div class="relative overflow-hidden" {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "财务管理总览" }
                div class="flex gap-3" {
                    a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-accent text-accent-on border-none hover:bg-accent-hover" href=(JournalCreatePath::PATH) {
                        (svg_icon("M12 4v16m8-8H4", "16px"))
                        "新建日记账"
                    }
                }
            }

            // ── 核心统计卡片 ──
            div class="stat-grid grid gap-5" style="margin-bottom:var(--space-8)" {
                (stat_card("本月流入",
                    &format!("¥{}<span style=\"font-size:14px;-webkit-text-fill-color:var(--muted)\">万</span>", fmt_wan(balance.total_inflow)),
                    "accent-green",
                    html! { "已确认流入" },
                    "linear-gradient(135deg,#dcfce7,#bbf7d0)", "#16a34a",
                    "M12 2v20M17 5H9.5a3.5 3.5 0 000 7h5a3.5 3.5 0 010 7H6"))
                (stat_card("本月流出",
                    &format!("¥{}<span style=\"font-size:14px;-webkit-text-fill-color:var(--muted)\">万</span>", fmt_wan(balance.total_outflow)),
                    "accent-red",
                    html! { "已确认流出" },
                    "linear-gradient(135deg,#fee2e2,#fecaca)", "#dc2626",
                    "M17 9V7a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2m2 4h10a2 2 0 002-2v-6a2 2 0 00-2-2H9a2 2 0 00-2 2v6a2 2 0 002 2zm7-5a2 2 0 11-4 0 2 2 0 014 0z"))
                @let net = balance.net_balance;
                @let net_sign = if net >= Decimal::ZERO { "+" } else { "" };
                (stat_card("净现金流",
                    &format!("{net_sign}¥{}<span style=\"font-size:14px;-webkit-text-fill-color:var(--muted)\">万</span>", fmt_wan(net.abs())),
                    "accent-blue",
                    html! { "期间 " span style="font-weight:600" { (current_period) } },
                    "linear-gradient(135deg,#dbeafe,#bfdbfe)", "#2563eb",
                    "M20 12V8H6a2 2 0 01-2-2c0-1.1.9-2 2-2h12v4M4 6v12c0 1.1.9 2 2 2h14v-4M18 12a2 2 0 00-2 2c0 1.1.9 2 2 2h4v-4h-4z"))
                (stat_card("待核销金额",
                    &format!("¥{}<span style=\"font-size:14px;-webkit-text-fill-color:var(--muted)\">万</span>", fmt_wan(Decimal::ZERO)),
                    "accent-orange",
                    html! { "暂无核销数据" },
                    "linear-gradient(135deg,#fef3c7,#fde68a)", "#d97706",
                    "M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-6 9l2 2 4-4"))
                (stat_card("待审报销",
                    &pending_count.to_string(),
                    "accent-purple",
                    html! { "金额合计 " span style="color:#7c3aed;font-weight:600" { (format!("¥{}", fmt_wan(pending_amount))) "万" } },
                    "linear-gradient(135deg,#ede9fe,#ddd6fe)", "#7c3aed",
                    "M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8zM14 2v6h6M16 13H8M16 17H8M10 9H8"))
            }

            // ── 快捷入口 ──
            div class="section-block" {
                div class="text-lg font-semibold text-fg flex items-center gap-2" {
                    (svg_icon("M13 10V3L4 14h7v7l9-11h-7z", "18px"))
                    "快捷操作"
                }
                div class="grid gap-4" {
                    a href=(JournalCreatePath::PATH) class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden" {
                        div class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-icon" style="background:linear-gradient(135deg,#dbeafe,#bfdbfe);color:#2563eb" {
                            (svg_icon("M12 4v16m8-8H4", "20px"))
                        }
                        div class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-title" { "新建日记账" }
                        div class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-desc" { "录入现金收支" }
                        div class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-badge blue" { "CashJournal" }
                    }
                    a href=(ExpenseCreatePath::PATH) class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden" {
                        div class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-icon" style="background:linear-gradient(135deg,#ede9fe,#ddd6fe);color:#7c3aed" {
                            (svg_icon("M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8zM14 2v6h6", "20px"))
                        }
                        div class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-title" { "费用报销" }
                        div class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-desc" { "提交报销申请" }
                        div class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-badge purple" { "Expense" }
                    }
                    a href=(WriteoffListPath::PATH) class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden" {
                        div class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-icon" style="background:linear-gradient(135deg,#dcfce7,#bbf7d0);color:#16a34a" {
                            (svg_icon("M9 11l3 3L22 4M21 12v7a2 2 0 01-2 2H5a2 2 0 01-2-2V5a2 2 0 012-2h11", "20px"))
                        }
                        div class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-title" { "核销管理" }
                        div class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-desc" { "按单核销收款/付款" }
                        div class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-badge green" { "WriteOff" }
                    }
                    a href=(CostAnalysisPath::PATH) class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden" {
                        div class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-icon" style="background:linear-gradient(135deg,#fef3c7,#fde68a);color:#d97706" {
                            (svg_icon("M18 20V10M12 20V4M6 20v-6", "20px"))
                        }
                        div class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-title" { "成本核算" }
                        div class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-desc" { (PreEscaped("利润分析 &amp; P&amp;L")) }
                        div class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-badge orange" { "CostEntry" }
                    }
                }
            }

            // ── 两列：最近流水 + 费用报销 ──
            div style="display:grid;grid-template-columns:1fr 1fr;gap:var(--space-6);margin-bottom:var(--space-8)" {
                // 最近日记账
                div class="bg-bg border border-border-soft rounded-lg overflow-hidden" {
                    div class="p-4 border-b text-sm font-semibold text-fg flex items-center gap-2 bg-surface-raised" {
                        (PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="width:18px;height:18px;color:var(--accent)"><path d="M4 19.5A2.5 2.5 0 016.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 014 19.5v-15A2.5 2.5 0 016.5 2z"/></svg>"#))
                        "最近日记账"
                        a href=(JournalListPath::PATH) style="margin-left:auto;font-size:12px;color:var(--accent);font-weight:600;letter-spacing:0" { "查看全部 →" }
                    }
                    div style="padding:var(--space-2) var(--space-5) var(--space-4)" {
                        @if journals.is_empty() {
                            div style="text-align:center;padding:var(--space-6);color:var(--muted)" { "暂无日记账记录" }
                        } @else {
                            @for j in journals {
                                @let dot_class = match &j.direction {
                                    CashDirection::Inflow => "inflow",
                                    CashDirection::Outflow => "outflow",
                                };
                                div class="flex items-center gap-3 p-3 rounded-sm" {
                                    div class=(format!("flow-dot {dot_class}")) style="flex-shrink:0" {}
                                    div style="flex:1;min-width:0;margin-left:var(--space-3)" {
                                        div style="font-size:var(--text-sm);font-weight:600;color:var(--fg)" class="truncate" { (j.doc_number) }
                                        div style="font-size:12px;color:var(--muted)" { (journal_type_label(&j.journal_type)) " · " (j.remark.as_str()) }
                                    }
                                    div style="text-align:right" {
                                        div style=(format!("font-size:var(--text-sm);font-weight:700;color:{};font-family:var(--font-mono)", amount_color(&j.direction))) {
                                            (fmt_amount(j.amount, &j.direction))
                                        }
                                        div style="font-size:11px;color:var(--muted)" { (j.transaction_date.format("%m-%d")) }
                                    }
                                }
                            }
                        }
                    }
                }

                // 待审批报销
                div class="bg-bg border border-border-soft rounded-lg overflow-hidden" {
                    div class="p-4 border-b text-sm font-semibold text-fg flex items-center gap-2 bg-surface-raised" {
                        (PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="width:18px;height:18px;color:#7c3aed"><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><path d="M14 2v6h6"/></svg>"#))
                        "待审批报销"
                        a href=(ExpenseListPath::PATH) style="margin-left:auto;font-size:12px;color:var(--accent);font-weight:600;letter-spacing:0" { "查看全部 →" }
                    }
                    div style="padding:var(--space-2) var(--space-5) var(--space-4)" {
                        @if expenses.is_empty() {
                            div style="text-align:center;padding:var(--space-6);color:var(--muted)" { "暂无待审批报销" }
                        } @else {
                            @for e in expenses {
                                div class="flex items-center gap-3 p-3 rounded-sm" {
                                    div class="mini-inline-grid place-items-center rounded-full text-white font-semibold shrink-0 select-none" style="background:linear-gradient(135deg,#dbeafe,#bfdbfe);color:var(--accent)" {
                                        (e.doc_number.chars().next().unwrap_or('—'))
                                    }
                                    div style="flex:1;min-width:0;margin-left:var(--space-3)" {
                                        div style="font-size:var(--text-sm);font-weight:600;color:var(--fg)" { (e.remark.as_str()) }
                                        div style="font-size:12px;color:var(--muted)" { (e.doc_number) }
                                    }
                                    div style="text-align:right" {
                                        div style="font-size:var(--text-sm);font-weight:700;color:var(--fg);font-family:var(--font-mono)" {
                                            (format!("¥{:.2}", e.total_amount))
                                        }
                                        span class="inline-flex items-center gap-[5px] rounded-full text-[12px] font-medium whitespace-nowrap bg-[#fff8eb] text-[#d46b08]" style="font-size:11px" { "待审批" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── 日记账类型分布 + 月度趋势 ──
            div style="display:grid;grid-template-columns:1fr 1.5fr;gap:var(--space-6);margin-bottom:var(--space-8)" {
                // 类型分布（真实数据）
                div class="bg-bg border border-border-soft rounded-lg overflow-hidden" {
                    div class="p-4 border-b text-sm font-semibold text-fg flex items-center gap-2 bg-surface-raised" {
                        (svg_icon("M21 21H3V3M18 9l-5 5-2-2-4 4", "18px"))
                        "本月日记账分布"
                    }
                    div style="padding:var(--space-5)" {
                        div style="display:flex;flex-direction:column;gap:var(--space-5)" {
                            @for (type_id, label, color, _fill_class) in dist_types {
                                @let amount = distribution.iter().find(|(t, _)| *t == type_id).map(|(_, v)| *v).unwrap_or(Decimal::ZERO);
                                @let pct = if dist_max > Decimal::ZERO { (amount / dist_max * Decimal::from(100)).round_dp(0) } else { Decimal::ZERO };
                                (distribution_bar(label, &format!("¥{}万", fmt_wan(amount)), color, &format!("{}%", pct)))
                            }
                        }
                    }
                }

                // 月度趋势（真实数据）
                div class="bg-bg border border-border-soft rounded-lg overflow-hidden" {
                    div class="p-4 border-b text-sm font-semibold text-fg flex items-center gap-2 bg-surface-raised" {
                        (svg_icon("M18 20V10M12 20V4M6 20v-6", "18px"))
                        "近6月现金流趋势（万元）"
                    }
                    div style="padding:var(--space-5)" {
                        div style="display:flex;justify-content:space-between;margin-bottom:var(--space-5);font-size:12px" {
                            div style="display:flex;gap:var(--space-4)" {
                                span style="display:flex;align-items:center;gap:4px" {
                                    span style="width:10px;height:3px;border-radius:2px;background:var(--success)" {}
                                    "流入"
                                }
                                span style="display:flex;align-items:center;gap:4px" {
                                    span style="width:10px;height:3px;border-radius:2px;background:var(--danger)" {}
                                    "流出"
                                }
                                span style="display:flex;align-items:center;gap:4px" {
                                    span style="width:10px;height:3px;border-radius:2px;background:var(--accent)" {}
                                    "净额"
                                }
                            }
                            span style="color:var(--muted)" { "单位：万元" }
                        }
                        div style="display:grid;grid-template-columns:repeat(6,1fr);gap:var(--space-2)" {
                            // 补齐6个月（有的月份可能没有数据）
                            @for i in 0..6 {
                                @let month_offset = 5 - i; // 5=最早月, 0=当月
                                @let target_date = (chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), 1).unwrap())
                                    .checked_sub_months(chrono::Months::new(month_offset as u32))
                                    .unwrap();
                                @let target_period = target_date.format("%Y-%m").to_string();
                                @let month_label = format!("{}月", target_date.month());
                                @let is_current = month_offset == 0;

                                // 查找该月数据
                                @let (inflow, outflow) = trend.iter()
                                    .find(|(p, _, _)| p == &target_period)
                                    .map(|(_, i, o)| (*i, *o))
                                    .unwrap_or((Decimal::ZERO, Decimal::ZERO));
                                @let net = inflow - outflow;

                                // 计算柱高（最大140px）
                                @let inflow_h = (inflow / trend_max * Decimal::from(130) + Decimal::from(10)).round_dp(0).to_string();
                                @let outflow_h = (outflow / trend_max * Decimal::from(130) + Decimal::from(10)).round_dp(0).to_string();
                                @let net_display = if net >= Decimal::ZERO { format!("+{}", fmt_wan(net)) } else { fmt_wan(net) };
                                @let net_color = if is_current { "var(--accent)" } else if net >= Decimal::ZERO { "var(--success)" } else { "var(--danger)" };

                                (trend_bar(&month_label, &format!("{}px", inflow_h), &format!("{}px", outflow_h), &net_display, net_color, is_current))
                            }
                        }
                    }
                }
            }
        }
    }
}

fn distribution_bar(label: &str, value: &str, color: &str, width: &str) -> Markup {
    html! {
        div {
            div style="display:flex;justify-content:space-between;margin-bottom:8px" {
                span style="font-size:var(--text-sm);font-weight:600;color:var(--fg)" { (label) }
                span style=(format!("font-size:var(--text-sm);font-weight:700;color:{color};font-family:var(--font-mono)")) { (value) }
            }
            div class="progress-bar" {
                div class="progress-bar-fill" style=(format!("width:{width};background:{color}")) {}
            }
        }
    }
}

fn trend_bar(month: &str, inflow_h: &str, outflow_h: &str, net: &str, color: &str, is_current: bool) -> Markup {
    let month_style = if is_current {
        format!("font-size:12px;color:{color};margin-top:8px;font-weight:700")
    } else {
        "font-size:12px;color:var(--muted);margin-top:8px;font-weight:500".to_string()
    };
    let net_style = if is_current {
        format!("font-size:12px;font-weight:800;color:{color};font-family:var(--font-mono);margin-top:2px")
    } else {
        format!("font-size:11px;font-weight:700;color:{color};font-family:var(--font-mono);margin-top:2px")
    };
    let bar_style = if is_current {
        format!("width:100%;max-width:48px;height:{inflow_h};background:linear-gradient(180deg,rgba(37,99,235,0.2),rgba(37,99,235,0.04));border-top:2.5px solid var(--accent);box-shadow:0 -4px 12px rgba(37,99,235,0.1)")
    } else {
        format!("width:100%;max-width:48px;height:{inflow_h};background:linear-gradient(180deg,rgba(22,163,74,0.2),rgba(22,163,74,0.04));border-top:2.5px solid var(--success)")
    };
    html! {
        div style="text-align:center" {
            div style="display:flex;flex-direction:column;align-items:center;gap:4px;height:140px;justify-content:flex-end" {
                div class="relative overflow-hidden" style=(bar_style) {}
                div class="relative overflow-hidden" style=(format!("width:100%;max-width:48px;height:{outflow_h};background:linear-gradient(180deg,rgba(220,38,38,0.15),rgba(220,38,38,0.03));border-top:2.5px solid var(--danger)")) {}
            }
            div style=(month_style) { (month) }
            div style=(net_style) { (net) }
        }
    }
}
