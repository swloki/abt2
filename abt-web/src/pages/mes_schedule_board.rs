use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};
use maud::{html, Markup};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::Deserialize;

use abt_core::master_data::work_center::model::work_center_type_label;
use abt_core::mes::dashboard::MesDashboardService;
use abt_core::mes::dashboard::model::{
    GanttBooking, GanttData, ScheduleCard, ScheduleStats, WcDailyLoad,
};
use abt_core::mes::enums::BatchStatus;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_batch::ScheduleBoardPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ============================================================================
// Query Params
// ============================================================================

#[derive(Deserialize, Clone, Default)]
pub struct ScheduleBoardQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub view: Option<String>,
}

// ============================================================================
// Handler
// ============================================================================

#[require_permission("WORK_ORDER", "read")]
pub async fn get_schedule_board(
    _path: ScheduleBoardPath,
    Query(q): Query<ScheduleBoardQuery>,
    ctx: RequestContext,
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

    let today = Local::now().date_naive();
    let from = q
        .from
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or(today);
    let to = q
        .to
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or(from + Duration::days(14));
    let active_view = q.view.as_deref().unwrap_or("gantt");

    let svc = state.mes_dashboard_service();
    let stats = svc.get_schedule_stats(&service_ctx, &mut conn).await?;
    let cards = svc.get_schedule_cards(&service_ctx, &mut conn).await?;
    let gantt = svc
        .get_gantt_data(&service_ctx, &mut conn, from, to, None)
        .await?;
    let loads = svc
        .get_work_center_load(&service_ctx, &mut conn, from, to)
        .await?;

    let content = schedule_board_content(
        &stats,
        &cards,
        &gantt,
        &loads,
        from,
        to,
        today,
        active_view,
    );
    Ok(Html(
        admin_page(
            is_htmx,
            "排程看板",
            &claims,
            "production",
            ScheduleBoardPath::PATH,
            "生产管理",
            None,
            content,
            &nav_filter,
        )
        .into_string(),
    ))
}

// ============================================================================
// Top-level content wrapper
// ============================================================================

fn schedule_board_content(
    stats: &ScheduleStats,
    cards: &[ScheduleCard],
    gantt: &GanttData,
    loads: &[WcDailyLoad],
    from: NaiveDate,
    to: NaiveDate,
    today: NaiveDate,
    active_view: &str,
) -> Markup {
    html! {
        div id="schedule-content" class="min-w-0" {
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "排程看板" }
            }
            // ── Stats Row ──
            (stats_row(stats))
            // ── Toolbar: tabs + date nav ──
            (toolbar(from, to, active_view))
            // ── Gantt View ──
            div id="gantt-view" class=(if active_view == "gantt" { "" } else { "hidden" }) {
                (gantt_view(gantt, today))
            }
            // ── Load View ──
            div id="load-view" class=(if active_view == "load" { "" } else { "hidden" }) {
                (load_view(loads, &gantt.date_range, &gantt.work_centers, today))
            }
            // ── Kanban View ──
            div id="kanban-view" class=(if active_view == "kanban" { "" } else { "hidden" }) {
                (kanban_view(cards))
            }
        }
    }
}

// ============================================================================
// Stats Row
// ============================================================================

fn stats_row(stats: &ScheduleStats) -> Markup {
    html! {
        div class="grid grid-cols-5 gap-3 mb-6" {
            ({
                stat_card(
                    &stats.active_orders.to_string(),
                    "活跃工单",
                    "bg-accent/10 border-l-4 border-l-accent",
                    "text-accent",
                )
            })
            ({
                stat_card(
                    &stats.pending_batches.to_string(),
                    "待排产",
                    "bg-warn/10 border-l-4 border-l-warn",
                    "text-warn",
                )
            })
            ({
                stat_card(
                    &stats.in_progress_batches.to_string(),
                    "进行中",
                    "bg-purple-600/10 border-l-4 border-l-purple-600",
                    "text-purple-600",
                )
            })
            ({
                stat_card(
                    &stats.pending_receipt_batches.to_string(),
                    "待入库",
                    "bg-purple/10 border-l-4 border-l-purple",
                    "text-purple",
                )
            })
            ({
                stat_card(
                    &stats.completed_batches.to_string(),
                    "已完成",
                    "bg-success/10 border-l-4 border-l-success",
                    "text-success",
                )
            })
        }
    }
}

fn stat_card(value: &str, label: &str, card_cls: &str, num_cls: &str) -> Markup {
    html! {
        div class=({
                format!(
                    "rounded-md p-4 text-center shadow-xs transition-all duration-150 hover:shadow-sm hover:-translate-y-px {}",
                    card_cls,
                )
            })
        {
            div class=({
                    format!(
                        "text-xl font-bold font-mono tabular-nums tracking-tight {}",
                        num_cls,
                    )
                })
            { (value) }
            div class="text-[11px] text-fg-2 mt-[3px] font-medium" { (label) }
        }
    }
}

// ============================================================================
// Toolbar (tabs + date navigation)
// ============================================================================

fn toolbar(from: NaiveDate, to: NaiveDate, active_view: &str) -> Markup {
    let prev_from = from - Duration::days(7);
    let prev_to = to - Duration::days(7);
    let next_from = from + Duration::days(7);
    let next_to = to + Duration::days(7);

    let base = "/admin/mes/schedule";
    let url_for = |v: &str| -> String {
        format!(
            "{base}?from={}&to={}&view={v}",
            from.format("%Y-%m-%d"),
            to.format("%Y-%m-%d")
        )
    };
    let prev_url = format!(
        "{base}?from={}&to={}&view={active_view}",
        prev_from.format("%Y-%m-%d"),
        prev_to.format("%Y-%m-%d")
    );
    let next_url = format!(
        "{base}?from={}&to={}&view={active_view}",
        next_from.format("%Y-%m-%d"),
        next_to.format("%Y-%m-%d")
    );

    let tab_btn = |label: &str, view: &str| -> Markup {
        let is_active = active_view == view;
        let class = if is_active {
            "px-4 py-[7px] text-sm bg-accent text-white font-semibold cursor-pointer whitespace-nowrap transition-colors"
        } else {
            "px-4 py-[7px] text-sm bg-bg text-muted font-medium cursor-pointer whitespace-nowrap transition-colors hover:bg-accent-bg hover:text-accent"
        };
        let sep = if view != "kanban" {
            " [border-right:1px_solid_var(--border)]"
        } else {
            ""
        };
        html! {
            a   class=(format!("{class}{sep}"))
                hx-get=(url_for(view))
                hx-target="#schedule-content"
                hx-swap="outerHTML"
               
            { (label) }
        }
    };

    html! {
        div class="flex items-center gap-3 flex-wrap mb-6" {
            div class="flex border border-border rounded-md overflow-hidden shadow-xs" {
                (tab_btn("甘特图", "gantt"))
                (tab_btn("负荷分析", "load"))
                (tab_btn("状态看板", "kanban"))
            }
            div class="flex items-center gap-2 ml-auto" {
                button
                    class="w-[30px] h-[30px] inline-grid place-items-center border border-border rounded-sm bg-bg text-fg cursor-pointer transition-colors hover:border-accent hover:text-accent hover:bg-accent-bg"
                    hx-get=(prev_url)
                    hx-target="#schedule-content"
                    hx-swap="outerHTML"
                { "‹" }
                span class="font-mono text-[13px] font-semibold text-fg text-center min-w-[110px]" {
                    (from.format("%m/%d").to_string())
                    " - "
                    (to.format("%m/%d").to_string())
                }
                button
                    class="w-[30px] h-[30px] inline-grid place-items-center border border-border rounded-sm bg-bg text-fg cursor-pointer transition-colors hover:border-accent hover:text-accent hover:bg-accent-bg"
                    hx-get=(next_url)
                    hx-target="#schedule-content"
                    hx-swap="outerHTML"
                { "›" }
            }
        }
    }
}

// ============================================================================
// Gantt View
// ============================================================================

fn gantt_view(data: &GanttData, today: NaiveDate) -> Markup {
    if data.work_centers.is_empty() {
        return empty_state("暂无活跃工作中心，请先在主数据中创建工作中心");
    }

    html! {
        div class="bg-bg border border-border-soft rounded-md p-5 shadow-[var(--shadow-card)] min-w-0"
        {
            // ── Legend ──
            div class="flex gap-4 mb-4 text-xs text-muted" {
                (legend_dot("bg-accent/25 border border-accent", "计划"))
                (legend_dot("bg-accent", "进行中"))
                (legend_dot("bg-warn", "待入库"))
                (legend_dot("bg-success", "已完成"))
            }
            // ── Header: label + date ticks ──
            (gantt_header(&data.date_range, today))
            // ── Rows: one track per work center ──
            div class="flex flex-col" {
                @for wc in &data.work_centers { (gantt_row(data, wc, today)) }
            }
        }
    }
}

fn legend_dot(dot_cls: &str, label: &str) -> Markup {
    html! {
        span class="flex items-center gap-1" {
            span class=(format!("inline-block w-3 h-3 {}", dot_cls)) {}
            (label)
        }
    }
}

fn gantt_header(date_range: &[NaiveDate], today: NaiveDate) -> Markup {
    html! {
        div class="flex items-stretch pb-2 mb-2 border-b border-border-soft" {
            div class="w-[160px] shrink-0 pr-3 flex items-center text-[11px] text-muted font-semibold"
            { "工作中心" }
            div class="flex-1 flex" {
                @for date in date_range { (gantt_header_tick(*date, today)) }
            }
        }
    }
}

fn gantt_header_tick(date: NaiveDate, today: NaiveDate) -> Markup {
    let is_today = date == today;
    let is_weekend = matches!(date.weekday(), Weekday::Sat | Weekday::Sun);
    let date_cls = if is_today {
        "block text-[11px] font-semibold text-accent"
    } else if is_weekend {
        "block text-[11px] font-medium text-muted"
    } else {
        "block text-[11px] font-semibold text-fg-2"
    };
    html! {
        div class="flex-1 text-center" {
            span class=(date_cls) { (date.format("%m/%d").to_string()) }
            span class="block text-[10px] text-muted" { (weekday_cn(date.weekday())) }
        }
    }
}

fn gantt_row(
    data: &GanttData,
    wc: &abt_core::mes::dashboard::model::WorkCenterInfo,
    today: NaiveDate,
) -> Markup {
    let range_start = *data.date_range.first().unwrap_or(&today);
    let range_end = *data.date_range.last().unwrap_or(&today);
    let total = data.date_range.len().max(1) as f64;

    // Today vertical marker position (only when today falls inside the range)
    let today_marker = if date_in_range(today, range_start, range_end) {
        let idx = (today - range_start).num_days() as f64;
        let left_pct = (idx / total) * 100.0;
        Some(format!(
            "<div class=\"absolute top-0 bottom-0 w-px bg-accent/30\" style=\"left:{}%\"></div>",
            left_pct
        ))
    } else {
        None
    };

    html! {
        div class="flex items-stretch py-1.5 [&:not(:last-child)]:border-b [&:not(:last-child)]:border-border-soft"
        {
            // ── Work center label ──
            div class="w-[160px] shrink-0 pr-3" {
                div class="text-sm font-medium text-fg truncate" { (wc.name) }
                div class="text-[11px] text-muted mt-0.5" {
                    (work_center_type_label(wc.work_center_type))
                }
            }
            // ── Track ──
            div class="flex-1 relative h-7 bg-surface rounded-sm" {
                @if let Some(html) = today_marker { (maud::PreEscaped(html)) }
                @for b in data.bookings.iter().filter(|b| b.work_center_id == wc.id) {
                    (gantt_bar(b, range_start, range_end, total))
                }
            }
        }
    }
}

fn gantt_bar(
    b: &GanttBooking,
    range_start: NaiveDate,
    range_end: NaiveDate,
    total: f64,
) -> Markup {
    // Clip the booking to the visible range and compute left%/width%
    let raw_start = b.date_from.date_naive();
    let raw_end = b.date_to.date_naive();
    let vis_start = raw_start.max(range_start);
    let vis_end = raw_end.min(range_end);
    if vis_end < range_start || vis_start > range_end || vis_end < vis_start {
        return html! {};
    }
    let start_idx = (vis_start - range_start).num_days() as f64;
    let span = (vis_end - vis_start).num_days() as f64 + 1.0;
    let left_pct = (start_idx / total) * 100.0;
    let width_pct = (span / total) * 100.0;

    let color_cls = booking_color(b.batch_status);
    let title = b.wo_doc_number.as_deref().unwrap_or("—");
    let process = b.process_name.as_deref().unwrap_or("");
    let product = b.product_name.as_deref().unwrap_or("");
    let hours = format_hours(b.duration_minutes);

    // Pure colored block (no label, no rounded corners) — reads like a
    // timeline/progress-bar segment. Full info stays available on hover.
    html! {
        div class=(format!("absolute top-0 bottom-0 cursor-pointer {}", color_cls))
            style=(format!("left:{}%;width:{}%", left_pct, width_pct))
            title=(format!("{title} · {process} · {product} · {hours}")) {}
    }
}

/// Semantic bar background (+ optional border) by batch status.
/// Pending=1, InProgress=2, Suspended=3, PendingReceipt=4, Completed=5, Cancelled=6
fn booking_color(status: Option<i16>) -> &'static str {
    match status {
        Some(1) => "bg-accent/25 border border-accent",
        Some(2) => "bg-accent",
        Some(3) => "bg-surface-raised border border-border",
        Some(4) => "bg-warn",
        Some(5) => "bg-success",
        Some(6) => "bg-danger",
        _ => "bg-accent",
    }
}

// ============================================================================
// Load Analysis View
// ============================================================================

fn load_view(
    loads: &[WcDailyLoad],
    date_range: &[NaiveDate],
    work_centers: &[abt_core::mes::dashboard::model::WorkCenterInfo],
    today: NaiveDate,
) -> Markup {
    if work_centers.is_empty() {
        return empty_state("暂无工作中心数据");
    }

    html! {
        div class="bg-bg border border-border-soft rounded-md p-5 shadow-[var(--shadow-card)] min-w-0"
        {
            // ── Legend ──
            div class="flex gap-4 mb-4 text-xs text-muted" {
                (legend_dot("bg-surface border border-border", "无排程"))
                (legend_dot("bg-success-bg [border:1px_solid_var(--success)]", "<70%"))
                (legend_dot("bg-warn-bg [border:1px_solid_var(--warn)]", "70-90%"))
                (legend_dot("bg-danger-bg [border:1px_solid_var(--danger)]", ">90%"))
            }
            // ── Heatmap table ──
            div class="overflow-x-auto" {
                table class="border-collapse w-full" {
                    thead {
                        tr {
                            th  class="sticky left-0 z-10 bg-surface-raised w-[140px] min-w-[140px] px-3 py-2 text-left [border-right:1px_solid_var(--border-soft)] border-b border-border-soft"
                            {
                                span class="text-[11px] text-muted font-semibold" { "工作中心" }
                            }
                            @for date in date_range { (load_date_header(*date, today)) }
                        }
                    }
                    tbody {
                        @for wc in work_centers {
                            tr {
                                td  class="sticky left-0 z-[1] px-3 py-2 bg-surface-raised [border-right:1px_solid_var(--border-soft)] border-b border-border-soft"
                                {
                                    div class="text-sm font-medium text-fg truncate" { (wc.name) }
                                    div class="text-[11px] text-muted mt-0.5" {
                                        (work_center_type_label(wc.work_center_type))
                                    }
                                }
                                @for date in date_range { (load_cell(loads, wc.id, *date)) }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn load_date_header(date: NaiveDate, today: NaiveDate) -> Markup {
    let is_today = date == today;
    let is_weekend = matches!(date.weekday(), Weekday::Sat | Weekday::Sun);
    let date_cls = if is_today {
        "block text-[11px] font-semibold text-accent"
    } else if is_weekend {
        "block text-[11px] font-medium text-muted"
    } else {
        "block text-[11px] font-semibold text-fg-2"
    };
    html! {
        th  class="text-center px-1 py-2 min-w-[64px] w-[64px] border-b border-border-soft [border-left:1px_solid_var(--border-soft)]"
        {
            span class=(date_cls) { (date.format("%m/%d").to_string()) }
            span class="block text-[10px] text-muted" { (weekday_cn(date.weekday())) }
        }
    }
}

fn load_cell(loads: &[WcDailyLoad], wc_id: i64, date: NaiveDate) -> Markup {
    let load = loads.iter().find(|l| l.work_center_id == wc_id && l.date == date);
    let (display, level_cls, booked, avail) = match load {
        Some(l) => {
            let pct_val = l.load_pct.to_f64().unwrap_or(0.0);
            let level_cls = if l.available_minutes.is_zero() {
                "bg-surface text-muted"
            } else if pct_val > 90.0 {
                "bg-danger-bg text-danger"
            } else if pct_val > 70.0 {
                "bg-warn-bg text-warn"
            } else if pct_val > 0.0 {
                "bg-success-bg text-success"
            } else {
                "bg-surface text-muted"
            };
            let display = if l.available_minutes.is_zero() || pct_val == 0.0 {
                "—".to_string()
            } else {
                format!("{}%", l.load_pct)
            };
            (display, level_cls, format_hours(l.booked_minutes), format_hours(l.available_minutes))
        }
        None => ("—".to_string(), "bg-surface text-muted", "0h".to_string(), "0h".to_string()),
    };

    html! {
        td  class="text-center h-[44px] border-b border-border-soft [border-left:1px_solid_var(--border-soft)]"
        {
            div class=({
                    format!(
                        "h-full flex items-center justify-center text-xs font-semibold font-mono {}",
                        level_cls,
                    )
                })
                title=(format!("已排 {booked} / 可用 {avail}"))
            { (display) }
        }
    }
}

// ============================================================================
// Kanban View
// ============================================================================

fn kanban_view(cards: &[ScheduleCard]) -> Markup {
    let pending: Vec<_> = cards.iter().filter(|c| c.status == BatchStatus::Pending).collect();
    let in_progress: Vec<_> = cards
        .iter()
        .filter(|c| matches!(c.status, BatchStatus::InProgress | BatchStatus::Suspended))
        .collect();
    let pending_receipt: Vec<_> = cards
        .iter()
        .filter(|c| c.status == BatchStatus::PendingReceipt)
        .collect();
    let completed: Vec<_> = cards.iter().filter(|c| c.status == BatchStatus::Completed).collect();

    html! {
        div class="grid grid-cols-4 gap-4" {
            (kanban_column("待排产", &pending, "border-t-warn"))
            (kanban_column("进行中", &in_progress, "border-t-info"))
            (kanban_column("待入库", &pending_receipt, "border-t-purple"))
            (kanban_column("已完成", &completed, "border-t-success"))
        }
    }
}

fn kanban_column(
    title: &str,
    cards: &[&ScheduleCard],
    col_class: &str,
) -> Markup {
    html! {
        div class=({
                format!(
                    "bg-white rounded-md border border-border-soft border-t-[3px] flex flex-col {}",
                    col_class,
                )
            })
        {
            div class="flex items-center justify-between px-4 py-3 border-b border-border-soft" {
                span class="text-sm font-semibold text-fg" { (title) }
                span class="text-xs text-muted bg-[rgba(0,0,0,0.04)] px-2 py-0.5 rounded-full" {
                    (cards.len())
                }
            }
            div class="flex-1 p-3 flex flex-col gap-3 overflow-y-auto min-h-[200px]" {
                @for card in cards { (kanban_card(card)) }
                @if cards.is_empty() {
                    div class="text-sm text-muted text-center py-8" { "暂无数据" }
                }
            }
        }
    }
}

fn kanban_card(card: &ScheduleCard) -> Markup {
    let progress_pct = if card.batch_qty > Decimal::ZERO {
        let pct = (card.completed_qty / card.batch_qty * rust_decimal::Decimal::ONE_HUNDRED)
            .min(rust_decimal::Decimal::ONE_HUNDRED);
        pct.to_string()
    } else {
        "0".to_string()
    };

    let (status_label, status_cls) = match card.status {
        BatchStatus::Pending => ("待排产", "bg-warn-bg text-warn"),
        BatchStatus::InProgress => ("进行中", "bg-accent-bg text-accent"),
        BatchStatus::Suspended => ("已暂停", "bg-slate-50 text-muted"),
        BatchStatus::PendingReceipt => ("待入库", "bg-[rgba(124,58,237,0.1)] text-purple"),
        BatchStatus::Completed => ("已完成", "bg-success-bg text-success"),
        _ => ("", ""),
    };

    let step_display = if card.current_step == 0 {
        "未开始".to_string()
    } else {
        let total = card.total_steps.unwrap_or(0);
        let name = card.current_step_name.as_deref().unwrap_or("—");
        format!("{}/{} {}", card.current_step, total, name)
    };

    html! {
        a   class="block bg-white rounded-md border border-border-soft p-4 shadow-xs hover:shadow-md hover:-translate-y-px transition-all duration-200 cursor-pointer"
            href=(format!("/admin/mes/batches/{}", card.id))
        {
            div class="flex items-center justify-between mb-2" {
                span class="text-xs font-mono tabular-nums text-fg font-semibold" { (card.batch_no) }
                span
                    class=({
                        format!(
                            "text-[11px] px-2 py-0.5 rounded-full font-medium {}",
                            status_cls,
                        )
                    })
                { (status_label) }
            }
            div class="text-sm text-fg mb-2" { (card.product_name.as_deref().unwrap_or("—")) }
            div class="text-xs text-muted mb-2" {
                span {
                    (crate::utils::fmt_qty(card.completed_qty))
                    " / "
                    (crate::utils::fmt_qty(card.batch_qty))
                }
            }
            @if card.current_step > 0 {
                div class="mb-2" {
                    div class="h-1.5 bg-[rgba(0,0,0,0.06)] rounded-full overflow-hidden" {
                        div class="h-full bg-accent rounded-full transition-all duration-300"
                            style=(format!("width:{}%", progress_pct)) {}
                    }
                    span class="text-[10px] text-muted mt-1 block" { (step_display) }
                }
            }
            @if !card.wo_doc_number.as_ref().is_none_or(|s| s.is_empty()) {
                div class="text-[10px] text-muted bg-[rgba(0,0,0,0.04)] px-2 py-0.5 rounded inline-block"
                { "工单 " (card.wo_doc_number.as_deref().unwrap_or("")) }
            }
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn empty_state(msg: &str) -> Markup {
    html! {
        div class="text-center text-muted py-12" {
            div class="text-3xl mb-2" { "📭" }
            p class="text-sm" { (msg) }
        }
    }
}

fn weekday_cn(d: Weekday) -> &'static str {
    match d {
        Weekday::Mon => "周一",
        Weekday::Tue => "周二",
        Weekday::Wed => "周三",
        Weekday::Thu => "周四",
        Weekday::Fri => "周五",
        Weekday::Sat => "周六",
        Weekday::Sun => "周日",
    }
}

fn date_in_range(date: NaiveDate, start: NaiveDate, end: NaiveDate) -> bool {
    date >= start && date <= end
}


fn format_hours(minutes: Decimal) -> String {
    let hours = minutes / Decimal::from(60);
    format!("{:.1}h", hours.round_dp(1))
}
