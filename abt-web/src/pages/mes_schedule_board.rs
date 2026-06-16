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
        div id="schedule-content" {
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
        div class="flex gap-[12px]" {
            (stat_card(&stats.active_orders.to_string(), "活跃工单", "bs-primary"))
            (stat_card(&stats.pending_batches.to_string(), "待排产", "bs-pending"))
            (stat_card(&stats.in_progress_batches.to_string(), "进行中", "bs-progress"))
            (stat_card(&stats.pending_receipt_batches.to_string(), "待入库", "bs-receipt"))
            (stat_card(&stats.completed_batches.to_string(), "已完成", "bs-done"))
        }
    }
}

fn stat_card(value: &str, label: &str, cls: &str) -> Markup {
    html! {
        div class=(format!("board-stat-card {cls}")) {
            span class="board-text-2xl font-bold font-font-mono tabular-nums tabular-nums text-fg" { (value) }
            span class="board-text-sm text-text-muted mt-1" { (label) }
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
        html! {
            button
                class=(format!("tab-btn{}", if is_active { " active" } else { "" }))
                hx-get=(url_for(view))
                hx-target="#schedule-content"
                hx-swap="outerHTML"
                { (label) }
        }
    };

    html! {
        div class="flex justify-between items-center gap-[12px] flex-wrap" {
            div class="inline-flex bg-[#f5f5f5] gap-[2px]" {
                (tab_btn("甘特图", "gantt"))
                (tab_btn("负荷分析", "load"))
                (tab_btn("状态看板", "kanban"))
            }
            div class="flex items-center gap-[8px]" {
                button
                    class="date-nav-inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative"
                    hx-get=(prev_url)
                    hx-target="#schedule-content"
                    hx-swap="outerHTML"
                    { "‹" }
                span class="text-[14px] font-medium text-[#595959] text-center" {
                    (from.format("%m/%d").to_string()) " - " (to.format("%m/%d").to_string())
                }
                button
                    class="date-nav-inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative"
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
        div class="overflow-x-auto bg-white" {
            table class="gantt-table" {
                // ── Header row: dates ──
                thead {
                    tr {
                        th class="gantt-corner" {}
                        @for date in &data.date_range {
                            (gantt_date_header(*date, today))
                        }
                    }
                }
                // ── Body rows: work centers ──
                tbody {
                    @for (wi, wc) in data.work_centers.iter().enumerate() {
                        tr {
                            td class="border-b bg-[#fafafa] text-center sticky z-[1]" {
                                span class=(format!("gantt-wc-dot gantt-color-{}", wi % 8)) {}
                                span class="text-[13px] font-medium text-[#262626]" { (wc.name) }
                                span class="block text-[11px] text-[#bfbfbf]" { (work_center_type_label(wc.work_center_type)) }
                            }
                            @for date in &data.date_range {
                                (gantt_cell(data, wc.id, *date, today))
                            }
                        }
                    }
                }
            }
        }
    }
}

fn gantt_date_header(date: NaiveDate, today: NaiveDate) -> Markup {
    let is_today = date == today;
    let is_weekend = matches!(date.weekday(), Weekday::Sat | Weekday::Sun);
    let cls = format!(
        "gantt-date-header{}{}",
        if is_today { " gantt-date-today" } else { "" },
        if is_weekend { " gantt-date-weekend" } else { "" }
    );
    html! {
        th class=(cls) {
            span class="block text-[13px] font-semibold text-[#262626]" { (date.format("%m/%d").to_string()) }
            span class="block text-[11px] text-[#8c8c8c]" { (weekday_cn(date.weekday())) }
        }
    }
}

fn gantt_cell(data: &GanttData, wc_id: i64, date: NaiveDate, today: NaiveDate) -> Markup {
    let bookings: Vec<&GanttBooking> = data
        .bookings
        .iter()
        .filter(|b| {
            b.work_center_id == wc_id
                && date_in_range(date, b.date_from.date_naive(), b.date_to.date_naive())
        })
        .collect();

    let is_today = date == today;
    let is_weekend = matches!(date.weekday(), Weekday::Sat | Weekday::Sun);
    let cls = format!(
        "gantt-cell{}{}",
        if is_today { " gantt-cell-today" } else { "" },
        if is_weekend { " gantt-cell-weekend" } else { "" }
    );

    html! {
        td class=(cls) {
            @for b in &bookings {
                (gantt_block(b))
            }
            @if bookings.is_empty() {
                span class="border-b border-l h-[48px]-empty" {}
            }
        }
    }
}

fn gantt_block(b: &GanttBooking) -> Markup {
    let color_cls = format!("gantt-color-{}", (b.work_order_id as usize) % 8);
    let border_color = batch_status_border_color(b.batch_status);
    let spans_days = b.date_to.date_naive() > b.date_from.date_naive();


    // Determine if this is a "continuation" (booking started before this cell's date)
    let title = b.wo_doc_number.as_deref().unwrap_or("—");
    let process = b.process_name.as_deref().unwrap_or("");
    let product = b.product_name.as_deref().unwrap_or("");
    let hours = {
        let mins = b.duration_minutes;
        let h = mins / Decimal::from(60);
        h.round_dp(1).to_string()
    };

    html! {
        div
            class=(format!("gantt-block {color_cls}"))
            style=(format!("border-left-color: {border_color}"))
            title=(format!("{title} · {process} · {product} · {hours}h"))
        {
            div class="block no-underline text-[11px] cursor-pointer text-[#fff]-title" { (title) }
            div class="block no-underline text-[11px] cursor-pointer text-[#fff]-process" { (process) }
            @if spans_days {
                span class="block no-underline text-[11px] cursor-pointer text-[#fff]-arrow" { "→" }
            }
        }
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
        div class="overflow-x-auto bg-white" {
            table class="load-table" {
                thead {
                    tr {
                        th class="load-corner" {}
                        @for date in date_range {
                            (load_date_header(*date, today))
                        }
                    }
                }
                tbody {
                    @for wc in work_centers {
                        tr {
                            td class="border-b bg-[#fafafa] sticky z-[1]" {
                                span class="text-[13px] font-medium text-[#262626]" { (wc.name) }
                                span class="block text-[11px] text-[#bfbfbf]" { (work_center_type_label(wc.work_center_type)) }
                            }
                            @for date in date_range {
                                (load_cell(loads, wc.id, *date))
                            }
                        }
                    }
                }
            }
            div class="flex gap-[16px] justify-end" {
                span class="flex gap-[16px] justify-end-item" {
                    span class="flex gap-[16px] justify-end-swatch bg-[#f5f5f5] text-[#bfbfbf]" {}
                    "无排程"
                }
                span class="flex gap-[16px] justify-end-item" {
                    span class="flex gap-[16px] justify-end-swatch bg-[rgba(82,196,26,.12)] text-[#389e0d]" {}
                    "<70%"
                }
                span class="flex gap-[16px] justify-end-item" {
                    span class="flex gap-[16px] justify-end-swatch bg-[rgba(250,140,22,.15)] text-[#d46b08]" {}
                    "70-90%"
                }
                span class="flex gap-[16px] justify-end-item" {
                    span class="flex gap-[16px] justify-end-swatch bg-[rgba(245,34,45,.12)] text-[#cf1322]" {}
                    ">90%"
                }
            }
        }
    }
}

fn load_date_header(date: NaiveDate, today: NaiveDate) -> Markup {
    let is_today = date == today;
    let is_weekend = matches!(date.weekday(), Weekday::Sat | Weekday::Sun);
    let cls = format!(
        "load-date-header{}{}",
        if is_today { " gantt-date-today" } else { "" },
        if is_weekend { " gantt-date-weekend" } else { "" }
    );
    html! {
        th class=(cls) {
            span class="block text-[13px] font-semibold text-[#262626]" { (date.format("%m/%d").to_string()) }
            span class="block text-[11px] text-[#8c8c8c]" { (weekday_cn(date.weekday())) }
        }
    }
}

fn load_cell(loads: &[WcDailyLoad], wc_id: i64, date: NaiveDate) -> Markup {
    let load = loads.iter().find(|l| l.work_center_id == wc_id && l.date == date);
    let (pct, booked, avail, level_cls) = match load {
        Some(l) => {
            let pct_val = l.load_pct.to_f64().unwrap_or(0.0);
            let level = if l.available_minutes.is_zero() {
                "load-level-empty"
            } else if pct_val > 90.0 {
                "load-level-high"
            } else if pct_val > 70.0 {
                "load-level-mid"
            } else if pct_val > 0.0 {
                "load-level-low"
            } else {
                "load-level-empty"
            };
            (
                l.load_pct.to_string(),
                format_hours(l.booked_minutes),
                format_hours(l.available_minutes),
                level,
            )
        }
        None => ("0".to_string(), "0h".to_string(), "0h".to_string(), "load-level-empty"),
    };

    html! {
        td class="border-b border-l text-center h-[48px]" {
            div
                class=(format!("load-cell-block {level_cls}"))
                title=(format!("已排 {booked} / 可用 {avail}"))
            {
                span class="border-b border-l text-center h-[48px]-pct" { (pct) "%" }
            }
        }
    }
}

// ============================================================================
// Kanban View (existing, preserved)
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
            (kanban_column("待排产", &pending, "kanban-col-pending"))
            (kanban_column("进行中", &in_progress, "kanban-col-progress"))
            (kanban_column("待入库", &pending_receipt, "kanban-col-receipt"))
            (kanban_column("已完成", &completed, "kanban-col-done"))
        }
    }
}

fn kanban_column(
    title: &str,
    cards: &[&ScheduleCard],
    col_class: &str,
) -> Markup {
    html! {
        div class=(format!("kanban-column {col_class}")) {
            div class="flex items-center justify-between px-4 py-3 border-b border-border-soft" {
                span class="text-sm font-semibold text-fg" { (title) }
                span class="text-xs text-muted bg-[rgba(0,0,0,0.04)] px-2 py-0.5 rounded-full" { (cards.len()) }
            }
            div class="flex-1 p-3 flex flex-col gap-3 overflow-y-auto" {
                @for card in cards {
                    (kanban_card(card))
                }
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
        BatchStatus::Pending => ("待排产", "pill-pending"),
        BatchStatus::InProgress => ("进行中", "pill-progress"),
        BatchStatus::Suspended => ("已暂停", "pill-suspended"),
        BatchStatus::PendingReceipt => ("待入库", "pill-receipt"),
        BatchStatus::Completed => ("已完成", "pill-done"),
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
        a class="block bg-white rounded-md border border-border-soft p-4 shadow-xs hover:shadow-md hover:-translate-y-px transition-all duration-200 cursor-pointer" href=(format!("/admin/mes/batches/{}", card.id)) {
            div class="block bg-white rounded-md border border-border-soft p-4 shadow-xs hover:shadow-md hover:-translate-y-px transition-all duration-200 cursor-pointer-top" {
                span class="block bg-white rounded-md border border-border-soft p-4 shadow-xs hover:shadow-md hover:-translate-y-px transition-all duration-200 cursor-pointer-no font-mono tabular-nums" { (card.batch_no) }
                span class=(format!("kanban-card-pill {status_cls}")) { (status_label) }
            }
            div class="block bg-white rounded-md border border-border-soft p-4 shadow-xs hover:shadow-md hover:-translate-y-px transition-all duration-200 cursor-pointer-product" {
                (card.product_name.as_deref().unwrap_or("—"))
            }
            div class="block bg-white rounded-md border border-border-soft p-4 shadow-xs hover:shadow-md hover:-translate-y-px transition-all duration-200 cursor-pointer-meta" {
                span { (crate::utils::fmt_qty(card.completed_qty)) " / " (crate::utils::fmt_qty(card.batch_qty)) }
            }
            @if card.current_step > 0 {
                div class="block bg-white rounded-md border border-border-soft p-4 shadow-xs hover:shadow-md hover:-translate-y-px transition-all duration-200 cursor-pointer-progress" {
                    div class="h-1.5 bg-[rgba(0,0,0,0.06)] rounded-full overflow-hidden" {
                        div class="h-full bg-accent rounded-full transition-all duration-300" style=(format!("width:{}%", progress_pct)) {}
                    }
                    span class="text-[10px] text-muted mt-1" { (step_display) }
                }
            }
            @if !card.wo_doc_number.as_ref().is_none_or(|s| s.is_empty()) {
                div class="block bg-white rounded-md border border-border-soft p-4 shadow-xs hover:shadow-md hover:-translate-y-px transition-all duration-200 cursor-pointer-tag" {
                    "工单 " (card.wo_doc_number.as_deref().unwrap_or(""))
                }
            }
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn empty_state(msg: &str) -> Markup {
    html! {
        div class="text-center text-[#8c8c8c]" {
            div class="text-center text-[#8c8c8c]-icon" { "📭" }
            p { (msg) }
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

fn batch_status_border_color(status: Option<i16>) -> &'static str {
    match status {
        Some(1) => "#3b82f6",  // Pending - blue
        Some(2) => "#22c55e",  // InProgress - green
        Some(3) => "#f59e0b",  // Suspended - amber
        Some(4) => "#8b5cf6",  // PendingReceipt - purple
        Some(5) => "#9ca3af",  // Completed - gray
        _ => "#6b7280",         // Unknown - gray
    }
}

fn format_hours(minutes: Decimal) -> String {
    let h = minutes / Decimal::from(60);
    format!("{}h", h.round_dp(1))
}
