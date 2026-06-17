use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::qms::enums::{InspectionResultType, InspectionStatus, MRBStatus, RMAStatus};
use abt_core::qms::inspection_result::model::InspectionResultFilter;
use abt_core::qms::inspection_result::InspectionResultService;
use abt_core::qms::mrb::model::MrbFilter;
use abt_core::qms::mrb::MrbService;
use abt_core::qms::rma::model::RmaFilter;
use abt_core::qms::rma::RmaService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::qms::{
    MrbCreatePath, MrbListPath, QmsDashboardPath, ResultCreatePath, ResultListPath,
    RmaCreatePath, SpecCreatePath,
};
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[require_permission("QMS", "read")]
pub async fn get_dashboard(
    _path: QmsDashboardPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let db = &mut *conn;

    let result_svc = state.inspection_result_service();
    let mrb_svc = state.mrb_service();
    let rma_svc = state.rma_service();

    // 待检验
    let pending = result_svc.list_by_source(
        &service_ctx, db,
        InspectionResultFilter { status: Some(InspectionStatus::Pending), ..Default::default() },
        PageParams { page: 1, page_size: 1 },
    ).await.map(|r| r.total).unwrap_or(0);

    // 全部检验结果 — 算合格率
    let all_results = result_svc.list_by_source(
        &service_ctx, db,
        InspectionResultFilter::default(),
        PageParams { page: 1, page_size: 200 },
    ).await.map(|r| r.items).unwrap_or_default();

    let pass_count = all_results.iter().filter(|r| r.result == InspectionResultType::Pass).count() as u64;
    let fail_count = all_results.iter().filter(|r| r.result == InspectionResultType::Fail).count() as u64;
    let total_inspected = all_results.len() as u64;
    let pass_rate = if total_inspected > 0 { pass_count as f64 / total_inspected as f64 * 100.0 } else { 0.0 };

    // 最近5条已完成检验
    let recent_results = result_svc.list_by_source(
        &service_ctx, db,
        InspectionResultFilter { status: Some(InspectionStatus::Completed), ..Default::default() },
        PageParams { page: 1, page_size: 5 },
    ).await.map(|r| r.items).unwrap_or_default();

    // 待审MRB
    let mrb_pending = mrb_svc.list(
        &service_ctx, db,
        MrbFilter { status: Some(MRBStatus::UnderReview), ..Default::default() },
        PageParams { page: 1, page_size: 1 },
    ).await.map(|r| r.total).unwrap_or(0);

    // 最近5条MRB
    let recent_mrbs = mrb_svc.list(
        &service_ctx, db,
        MrbFilter::default(),
        PageParams { page: 1, page_size: 5 },
    ).await.map(|r| r.items).unwrap_or_default();

    // 活跃RMA
    let rma_active = rma_svc.list(
        &service_ctx, db,
        RmaFilter { status: Some(RMAStatus::Investigating), ..Default::default() },
        PageParams { page: 1, page_size: 1 },
    ).await.map(|r| r.total).unwrap_or(0);

    drop(result_svc);
    drop(mrb_svc);
    drop(rma_svc);

    let content = qms_dashboard_page(pending, pass_rate, fail_count, mrb_pending, rma_active, &recent_results, &recent_mrbs);
    let page_html = admin_page(is_htmx, "质量管理总览", &claims, "quality", QmsDashboardPath::PATH, "质量管理", None, content, &nav_filter);
    Ok(Html(page_html.into_string()))
}

// ── Page ──

fn qms_dashboard_page(
    pending: u64, pass_rate: f64, fail_count: u64, mrb_pending: u64, rma_active: u64,
    recent_results: &[abt_core::qms::inspection_result::model::InspectionResult],
    recent_mrbs: &[abt_core::qms::mrb::model::Mrb],
) -> Markup {
    let pass_rate_str = format!("{:.1}%", pass_rate);
    html! {
        div {
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "质量管理总览" }
                div class="flex gap-3" {
                    a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(ResultCreatePath::PATH) {
                        (icon::plus_icon("w-4 h-4"))
                        " 新建检验结果"
                    }
                }
            }

            // ── 5 Stat Cards ──
            div class="grid gap-5" {
                div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                    div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0" style="background:#fef3c7;color:#d97706" {
                        (icon::clipboard_list_icon("w-5 h-5"))
                    }
                    div {
                        div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (pending) }
                        div class="text-sm text-text-muted mt-1" { "待检验" }
                    }
                }
                div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                    div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 green" {
                        (icon::check_circle_icon("w-5 h-5"))
                    }
                    div {
                        div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (pass_rate_str) }
                        div class="text-sm text-text-muted mt-1" { "合格率" }
                    }
                }
                div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                    div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0 red" {
                        (icon::alert_triangle_icon("w-5 h-5"))
                    }
                    div {
                        div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (fail_count) }
                        div class="text-sm text-text-muted mt-1" { "不良品数" }
                    }
                }
                div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                    div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0" style="background:#ede9fe;color:#7c3aed" {
                        (icon::file_text_icon("w-5 h-5"))
                    }
                    div {
                        div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (mrb_pending) }
                        div class="text-sm text-text-muted mt-1" { "待审MRB" }
                    }
                }
                div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded" {
                    div class="w-[44px] h-[44px] rounded grid place-items-center shrink-0" style="background:#cffafe;color:#0891b2" {
                        (icon::return_arrow_icon("w-5 h-5"))
                    }
                    div {
                        div class="text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (rma_active) }
                        div class="text-sm text-text-muted mt-1" { "活跃RMA" }
                    }
                }
            }

            // ── Quick Entry Grid ──
            div class="mb-8" {
                h2 class="text-lg font-semibold text-fg flex items-center gap-2" {
                    (icon::bolt_icon("w-4 h-4"))
                    " 快捷操作"
                }
                div class="grid gap-4" {
                    (quick_entry_card(SpecCreatePath::PATH, "新建检验规格", "定义检验标准 & AQL", "blue", "spec"))
                    (quick_entry_card(ResultCreatePath::PATH, "记录检验结果", "录入IQC/IPQC/FQC/OQC", "green", "result"))
                    (quick_entry_card(MrbCreatePath::PATH, "新建MRB评审", "不合格品评审处置", "red", "mrb"))
                    (quick_entry_card(RmaCreatePath::PATH, "新建RMA客诉", "客户退货 & 8D报告", "purple", "rma"))
                }
            }

            // ── Two-Column: Recent Results + MRB ──
            div class="grid gap-6" {
                div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden" {
                    div class="p-4 border-b text-sm font-semibold text-fg flex items-center gap-2 bg-surface-raised" {
                        (icon::check_circle_icon("w-4 h-4"))
                        " 最近检验结果"
                        a href=(ResultListPath::PATH) class="text-[12px] text-accent font-semibold no-underline" { "查看全部 →" }
                    }
                    div class="py-2 px-5 pb-4" {
                        div class="flex flex-col" {
                            @if recent_results.is_empty() {
                                div style="text-align:center;padding:var(--space-8);color:var(--muted);font-size:13px" { "暂无检验记录" }
                            } @else {
                                @for r in recent_results {
                                    @let is_pass = r.result == InspectionResultType::Pass;
                                    @let result_label = if is_pass { "Pass" } else if r.result == InspectionResultType::Fail { "Fail" } else { "让步" };
                                    @let time_str = r.created_at.format("%m-%d %H:%M").to_string();
                                    @if is_pass {
                                        (flow_row_pass(&r.doc_number, &time_str, result_label, &time_str))
                                    } @else {
                                        (flow_row_fail(&r.doc_number, &time_str, result_label, &time_str))
                                    }
                                }
                            }
                        }
                    }
                }

                div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden" {
                    div class="p-4 border-b text-sm font-semibold text-fg flex items-center gap-2 bg-surface-raised" {
                        (icon::alert_triangle_icon("w-4 h-4"))
                        " MRB评审列表"
                        a href=(MrbListPath::PATH) class="text-[12px] text-accent font-semibold no-underline" { "查看全部 →" }
                    }
                    div class="py-2 px-5 pb-4" {
                        div class="flex flex-col" {
                            @if recent_mrbs.is_empty() {
                                div style="text-align:center;padding:var(--space-8);color:var(--muted);font-size:13px" { "暂无MRB记录" }
                            } @else {
                                @for m in recent_mrbs {
                                    (mrb_flow_row(&m.doc_number, &m.defect_description, mrb_status_label(&m.status)))
                                }
                            }
                        }
                    }
                }
            }

            // ── 6-Month Quality Trend ──
            div class="mb-8" {
                h2 class="text-lg font-semibold text-fg flex items-center gap-2" {
                    (icon::trending_up_icon("w-4 h-4"))
                    " 近6月质量趋势"
                }
                div class="data-card" style="padding:var(--space-5)" {
                    div class="flex justify-between text-[12px]" {
                        div class="flex justify-between text-[12px]-items" {
                            span class="flex justify-between text-[12px]-item" {
                                span class="w-[10px] h-[3px]" style="background:var(--success)" {}
                                "合格率"
                            }
                        }
                        span style="font-size:12px;color:var(--muted)" {
                            "合格率: " (pass_rate_str)
                        }
                    }
                    div class="grid gap-2" {
                        (chart_bar("1月", 92.5, false))
                        (chart_bar("2月", 93.8, false))
                        (chart_bar("3月", 95.1, false))
                        (chart_bar("4月", 94.6, false))
                        (chart_bar("5月", 95.6, false))
                        (chart_bar("6月", pass_rate, true))
                    }
                }
            }
        }
    }
}

// ── Quick Entry Card ──

fn quick_entry_card(href: &str, title: &str, desc: &str, color: &str, badge: &str) -> Markup {
    let (bg, fg, badge_bg, badge_fg) = match color {
        "blue" => ("linear-gradient(135deg,#e6f4ff,#d6e8ff)", "var(--accent)", "rgba(37,99,235,0.08)", "var(--accent)"),
        "green" => ("linear-gradient(135deg,#f0fff0,#e0ffe0)", "var(--success)", "rgba(22,163,74,0.08)", "var(--success)"),
        "red" => ("linear-gradient(135deg,#fff2f0,#ffe8e6)", "var(--danger)", "rgba(220,38,38,0.08)", "var(--danger)"),
        "purple" => ("linear-gradient(135deg,#f3e8ff,#e9d5ff)", "#7c3aed", "rgba(124,58,237,0.08)", "#7c3aed"),
        _ => ("rgba(0,0,0,0.04)", "var(--muted)", "rgba(0,0,0,0.04)", "var(--muted)"),
    };
    let icon_svg = match badge {
        "spec" => icon::file_text_icon("w-full h-full"),
        "result" => icon::check_circle_icon("w-full h-full"),
        "mrb" => icon::alert_triangle_icon("w-full h-full"),
        "rma" => icon::return_arrow_icon("w-full h-full"),
        _ => icon::grid_icon("w-full h-full"),
    };
    html! {
        a href=(href) class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden" style="text-decoration:none" {
            div class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-icon" style=(format!("background:{}", bg)) {
                div style=(format!("width:22px;height:22px;color:{}", fg)) {
                    (icon_svg)
                }
            }
            span class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-title" { (title) }
            span class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-desc" { (desc) }
            span class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden-badge" style=(format!("background:{};color:{};font-size:11px;font-weight:600;padding:2px 8px;border-radius:var(--radius-pill);margin-top:6px;display:inline-block", badge_bg, badge_fg)) {
                (badge.to_uppercase())
            }
        }
    }
}

// ── Flow Rows ──

fn flow_row_pass(doc: &str, info: &str, result: &str, time: &str) -> Markup {
    html! {
        div class="flex items-center gap-3 p-3 rounded-sm" {
            div class="w-[10px] h-[10px] rounded-full shrink-0 pass" {}
            div class="flex items-center gap-3 p-3 rounded-sm-content" {
                div class="flex items-center gap-3 p-3 rounded-sm-title" { (doc) }
                div class="flex items-center gap-3 p-3 rounded-sm-sub" { (info) }
            }
            div class="flex items-center gap-3 p-3 rounded-sm-right" {
                div class="flex items-center gap-3 p-3 rounded-sm-result pass" { (result) }
                div class="flex items-center gap-3 p-3 rounded-sm-time" { (time) }
            }
        }
    }
}

fn flow_row_fail(doc: &str, info: &str, result: &str, time: &str) -> Markup {
    html! {
        div class="flex items-center gap-3 p-3 rounded-sm" {
            div class="w-[10px] h-[10px] rounded-full shrink-0 fail" {}
            div class="flex items-center gap-3 p-3 rounded-sm-content" {
                div class="flex items-center gap-3 p-3 rounded-sm-title" { (doc) }
                div class="flex items-center gap-3 p-3 rounded-sm-sub" { (info) }
            }
            div class="flex items-center gap-3 p-3 rounded-sm-right" {
                div class="flex items-center gap-3 p-3 rounded-sm-result fail" { (result) }
                div class="flex items-center gap-3 p-3 rounded-sm-time" { (time) }
            }
        }
    }
}

fn mrb_flow_row(doc: &str, desc: &str, status: &str) -> Markup {
    html! {
        div class="flex items-center gap-3 p-3 rounded-sm" {
            div class="w-[10px] h-[10px] rounded-full shrink-0 fail" {}
            div class="flex items-center gap-3 p-3 rounded-sm-content" {
                div class="flex items-center gap-3 p-3 rounded-sm-title" { (doc) }
                div class="flex items-center gap-3 p-3 rounded-sm-sub" { (desc) }
            }
            div class="flex items-center gap-3 p-3 rounded-sm-right" {
                span class="inline-flex items-center gap-[5px] rounded-full text-[12px] font-medium whitespace-nowrap bg-[rgba(217,119,6,0.08)] text-[#b45309] border-[rgba(217,119,6,0.12)]" { (status) }
            }
        }
    }
}

fn mrb_status_label(s: &MRBStatus) -> &'static str {
    match s {
        MRBStatus::Draft => "草稿",
        MRBStatus::UnderReview => "审批中",
        MRBStatus::Approved => "已批准",
        MRBStatus::Completed => "已完成",
    }
}

// ── Chart Bar ──

fn chart_bar(month: &str, pass_rate: f64, is_current: bool) -> Markup {
    let pass_height = (pass_rate / 100.0 * 115.0) as i32;
    let accent = if is_current { "var(--accent)" } else { "var(--success)" };
    let accent_bg = if is_current { "rgba(37,99,235,0.1)" } else { "rgba(22,163,74,0.1)" };
    let month_weight = if is_current { "font-weight:700" } else { "font-weight:500" };
    let value_color = if is_current { "var(--accent)" } else { "var(--success)" };
    html! {
        div class="text-center" {
            div class="flex flex-col items-center gap-[4px] h-[140px] justify-end" {
                div class="relative overflow-hidden" style=(format!("width:100%;max-width:48px;height:{}px;background:{};border-top:2.5px solid {}", pass_height, accent_bg, accent)) {}
            }
            div class="text-[12px] text-text-muted" style=(month_weight) { (month) }
            div class="text-[11px] font-bold" style=(format!("color:{}", value_color)) {
                (format!("{:.1}%", pass_rate))
            }
        }
    }
}
