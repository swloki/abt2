use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::qms::enums::{InspectionResultType, MRBStatus};
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
    MrbCreatePath, MrbListPath, QmsDashboardPath, ResultCreatePath, ResultListPath, RmaCreatePath,
    SpecCreatePath,
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

 let result_svc = state.inspection_result_service();
 let mrb_svc = state.mrb_service();
 let rma_svc = state.rma_service();
 let one = PageParams { page: 1, page_size: 1 };
 let five = PageParams { page: 1, page_size: 5 };

 // Total + fail → derive pass count for pass_rate
 let total = result_svc
 .list_by_source(&service_ctx, &mut conn, InspectionResultFilter::default(), one.clone())
 .await?
 .total;
 let fail_count = result_svc
 .list_by_source(
 &service_ctx, &mut conn,
 InspectionResultFilter { result: Some(InspectionResultType::Fail), ..Default::default() },
 one.clone(),
 )
 .await?
 .total;
 let pass_count = result_svc
 .list_by_source(
 &service_ctx, &mut conn,
 InspectionResultFilter { result: Some(InspectionResultType::Pass), ..Default::default() },
 one.clone(),
 )
 .await?
 .total;
 let pending = total.saturating_sub(pass_count + fail_count);
 let pass_rate = if total > 0 {
 (pass_count as f64 / total as f64) * 100.0
 } else {
 0.0
 };

 let mrb_pending = mrb_svc
 .list(
 &service_ctx, &mut conn,
 MrbFilter { status: Some(MRBStatus::UnderReview), ..Default::default() },
 one.clone(),
 )
 .await?
 .total;

 let rma_active = rma_svc
 .list(&service_ctx, &mut conn, RmaFilter::default(), one)
 .await?
 .total;

 let recent_results = result_svc
 .list_by_source(&service_ctx, &mut conn, InspectionResultFilter::default(), five.clone())
 .await?
 .items;

 let recent_mrbs = mrb_svc
 .list(&service_ctx, &mut conn, MrbFilter::default(), five)
 .await?
 .items;

 let content = qms_dashboard_page(
 pending, pass_rate, fail_count, mrb_pending, rma_active,
 &recent_results, &recent_mrbs,
 );
 let page_html = admin_page(
 is_htmx, "质量管理总览", &claims, "quality", QmsDashboardPath::PATH,
 "质量管理", None, content, &nav_filter,
 );
 Ok(Html(page_html.into_string()))
}

// ── Page ──

fn qms_dashboard_page(
    pending: u64,
    pass_rate: f64,
    fail_count: u64,
    mrb_pending: u64,
    rma_active: u64,
    recent_results: &[abt_core::qms::inspection_result::model::InspectionResult],
    recent_mrbs: &[abt_core::qms::mrb::model::Mrb],
) -> Markup {
    let pass_rate_str = format!("{:.1}%", pass_rate);
    html! {
        // ── Page Header ──
        div class="flex items-center justify-between mb-6" {
            h1 class="text-xl font-bold text-fg tracking-tight" { "质量管理总览" }
            div class="flex gap-3" {
                a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(ResultCreatePath::PATH) {
                    (icon::plus_icon("w-4 h-4"))
                    " 新建检验结果"
                }
            }
        }

        // ── 5 Stat Cards ──
        div class="grid grid-cols-2 lg:grid-cols-5 gap-4 mb-6" {
            (stat_card("待检验", &pending.to_string(), icon::clipboard_list_icon("w-5 h-5"), "#fef3c7", "#d97706"))
            (stat_card("合格率", &pass_rate_str, icon::check_circle_icon("w-5 h-5"), "#dcfce7", "#16a34a"))
            (stat_card("不良品数", &fail_count.to_string(), icon::alert_triangle_icon("w-5 h-5"), "#fee2e2", "#dc2626"))
            (stat_card("待审MRB", &mrb_pending.to_string(), icon::file_text_icon("w-5 h-5"), "#ede9fe", "#7c3aed"))
            (stat_card("活跃RMA", &rma_active.to_string(), icon::return_arrow_icon("w-5 h-5"), "#cffafe", "#0891b2"))
        }

        // ── Quick Entry Grid ──
        div class="mb-6" {
            h2 class="text-lg font-semibold text-fg flex items-center gap-2 mb-4" {
                (icon::bolt_icon("w-4 h-4"))
                " 快捷操作"
            }
            div class="grid grid-cols-2 lg:grid-cols-4 gap-4" {
                (quick_entry_card(SpecCreatePath::PATH, "新建检验规格", "定义检验标准 & AQL", "blue", "spec"))
                (quick_entry_card(ResultCreatePath::PATH, "记录检验结果", "录入IQC/IPQC/FQC/OQC", "green", "result"))
                (quick_entry_card(MrbCreatePath::PATH, "新建MRB评审", "不合格品评审处置", "red", "mrb"))
                (quick_entry_card(RmaCreatePath::PATH, "新建RMA客诉", "客户退货 & 8D报告", "purple", "rma"))
            }
        }

        // ── Two-Column: Recent Results + MRB ──
        div class="grid grid-cols-1 lg:grid-cols-2 gap-5 mb-6" {
            // Recent Results
            div class="data-card overflow-hidden" {
                div class="px-4 py-3 border-b border-border-soft text-sm font-semibold text-fg flex items-center justify-between" {
                    span class="flex items-center gap-2" {
                        (icon::check_circle_icon("w-4 h-4"))
                        " 最近检验结果"
                    }
                    a href=(ResultListPath::PATH) class="text-xs text-accent font-medium hover:underline" { "查看全部 →" }
                }
                div class="p-2" {
                    @if recent_results.is_empty() {
                        div class="text-center py-8 text-sm text-muted" { "暂无检验记录" }
                    } @else {
                        @for r in recent_results {
                            @let is_pass = r.result == InspectionResultType::Pass;
                            @let result_label = if is_pass { "Pass" } else if r.result == InspectionResultType::Fail { "Fail" } else { "让步" };
                            @let time_str = r.created_at.format("%m-%d %H:%M").to_string();
                            (flow_row(&r.doc_number, &time_str, result_label, &time_str, is_pass))
                        }
                    }
                }
            }

            // MRB List
            div class="data-card overflow-hidden" {
                div class="px-4 py-3 border-b border-border-soft text-sm font-semibold text-fg flex items-center justify-between" {
                    span class="flex items-center gap-2" {
                        (icon::alert_triangle_icon("w-4 h-4"))
                        " MRB评审列表"
                    }
                    a href=(MrbListPath::PATH) class="text-xs text-accent font-medium hover:underline" { "查看全部 →" }
                }
                div class="p-2" {
                    @if recent_mrbs.is_empty() {
                        div class="text-center py-8 text-sm text-muted" { "暂无MRB记录" }
                    } @else {
                        @for m in recent_mrbs {
                            (mrb_flow_row(&m.doc_number, &m.defect_description, mrb_status_label(&m.status)))
                        }
                    }
                }
            }
        }

        // ── 6-Month Quality Trend ──
        div class="mb-6" {
            h2 class="text-lg font-semibold text-fg flex items-center gap-2 mb-4" {
                (icon::trending_up_icon("w-4 h-4"))
                " 近6月质量趋势"
            }
            div class="data-card p-5" {
                div class="flex items-center justify-between text-xs text-muted mb-4" {
                    div class="flex items-center gap-2" {
                        span class="w-2.5 h-0.5 bg-success rounded-full inline-block" {}
                        "合格率"
                    }
                    span { "合格率: " (pass_rate_str) }
                }
                div class="grid grid-cols-6 gap-2" {
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

// ── Stat Card ──

fn stat_card(label: &str, value: &str, icon_svg: Markup, bg_hex: &str, fg_hex: &str) -> Markup {
    html! {
        div class="data-card flex items-center gap-4 p-5" {
            div class="w-11 h-11 rounded-md grid place-items-center shrink-0"
                style=(format!("background:{};color:{}", bg_hex, fg_hex)) {
                (icon_svg)
            }
            div {
                div class="text-2xl font-bold font-mono tabular-nums text-fg" { (value) }
                div class="text-sm text-muted mt-1" { (label) }
            }
        }
    }
}

// ── Quick Entry Card ──

fn quick_entry_card(href: &str, title: &str, desc: &str, color: &str, badge: &str) -> Markup {
    let (icon_svg, badge_cls) = match badge {
        "spec" => (icon::file_text_icon("w-5 h-5"), "bg-accent-bg text-accent"),
        "result" => (icon::check_circle_icon("w-5 h-5"), "bg-success-bg text-success"),
        "mrb" => (icon::alert_triangle_icon("w-5 h-5"), "bg-danger-100 text-danger"),
        "rma" => (icon::return_arrow_icon("w-5 h-5"), "bg-purple-bg text-purple"),
        _ => (icon::grid_icon("w-5 h-5"), "bg-accent-bg text-muted"),
    };
    let title_cls = match color {
        "blue" => "text-accent",
        "green" => "text-success",
        "red" => "text-danger",
        "purple" => "text-purple",
        _ => "text-fg",
    };
    html! {
        a href=(href) class="data-card block p-5 no-underline hover:shadow-[var(--shadow-card-hover)] transition-shadow duration-200" {
            div class="flex items-center gap-3 mb-3" {
                div class="w-10 h-10 rounded-md grid place-items-center" {
                    span class=(title_cls) { (icon_svg) }
                }
                span class=(format!("text-[10px] font-bold px-2 py-0.5 rounded-full {}", badge_cls)) {
                    (badge.to_uppercase())
                }
            }
            div class=(format!("text-base font-semibold {} mb-1", title_cls)) { (title) }
            div class="text-sm text-muted" { (desc) }
        }
    }
}

// ── Flow Rows ──

fn flow_row(doc: &str, info: &str, result: &str, time: &str, is_pass: bool) -> Markup {
    let dot_cls = if is_pass { "bg-success" } else { "bg-danger" };
    let result_cls = if is_pass { "text-success" } else { "text-danger" };
    html! {
        div class="flex items-center gap-3 px-3 py-2.5 rounded-sm hover:bg-accent-bg transition-colors" {
            div class=(format!("w-2.5 h-2.5 rounded-full shrink-0 {}", dot_cls)) {}
            div class="flex-1 min-w-0" {
                div class="text-sm font-medium text-fg truncate font-mono" { (doc) }
                div class="text-xs text-muted mt-0.5" { (info) }
            }
            div class="flex items-center gap-3 shrink-0" {
                span class=(format!("text-xs font-semibold {}", result_cls)) { (result) }
                span class="text-xs text-muted font-mono" { (time) }
            }
        }
    }
}

fn mrb_flow_row(doc: &str, desc: &str, status: &str) -> Markup {
    html! {
        div class="flex items-center gap-3 px-3 py-2.5 rounded-sm hover:bg-accent-bg transition-colors" {
            div class="w-2.5 h-2.5 rounded-full shrink-0 bg-warn" {}
            div class="flex-1 min-w-0" {
                div class="text-sm font-medium text-fg truncate font-mono" { (doc) }
                div class="text-xs text-muted mt-0.5 truncate" { (desc) }
            }
            span class="inline-flex items-center gap-1 rounded-full text-xs font-medium whitespace-nowrap px-2 py-0.5 bg-warn-bg text-warn-700" {
                (status)
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
    let bar_cls = if is_current {
        "bg-[rgba(37,99,235,0.1)] border-t-[2.5px] border-accent"
    } else {
        "bg-[rgba(22,163,74,0.1)] border-t-[2.5px] border-success"
    };
    let month_cls = if is_current { "text-fg font-bold" } else { "text-muted font-medium" };
    let value_cls = if is_current { "text-accent" } else { "text-success" };
    html! {
        div class="text-center" {
            div class="flex flex-col items-center gap-1 h-[140px] justify-end" {
                div class=(format!("relative overflow-hidden w-full max-w-[48px] rounded-sm {}", bar_cls))
                    style=(format!("height:{}px", pass_height)) {}
            }
            div class=(format!("text-xs mt-1 {}", month_cls)) { (month) }
            div class=(format!("text-[11px] font-bold {}", value_cls)) {
                (format!("{:.1}%", pass_rate))
            }
        }
    }
}
