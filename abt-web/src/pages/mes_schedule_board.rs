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
        div class="flex gap-3 mb-6" {
            (stat_card(&stats.active_orders.to_string(), "活跃工单", "bg-[#e6f4ff] text-[#1677ff]"))
            (stat_card(&stats.pending_batches.to_string(), "待排产", "bg-[#fff7e6] text-[#fa8c16]"))
            (stat_card(&stats.in_progress_batches.to_string(), "进行中", "bg-[#e6fffb] text-[#13c2c2]"))
            (stat_card(&stats.pending_receipt_batches.to_string(), "待入库", "bg-[#f9f0ff] text-[#722ed1]"))
            (stat_card(&stats.completed_batches.to_string(), "已完成", "bg-[#f6ffed] text-[#52c41a]"))
        }
    }
}

fn stat_card(value: &str, label: &str, cls: &str) -> Markup {
    html! {
        div class=(format!("flex-1 rounded-md p-4 min-w-[120px] {}", cls)) {
            span class="block text-2xl font-bold font-mono tabular-nums" { (value) }
            span class="block text-[13px] mt-1 opacity-80" { (label) }
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
            "px-4 py-3 text-sm text-accent font-semibold cursor-pointer whitespace-nowrap relative [border-bottom:2px_solid_var(--accent)] -mb-px"
        } else {
            "px-4 py-3 text-sm text-muted cursor-pointer whitespace-nowrap relative [border-bottom:2px_solid_transparent] -mb-px hover:text-fg transition-colors"
        };
        html! {
            a class=(class)
                hx-get=(url_for(view))
                hx-target="#schedule-content"
                hx-swap="outerHTML"
                hx-push-url="true"
                { (label) }
        }
    };

    html! {
        div class="flex justify-between items-end gap-3 flex-wrap mb-6" {
            div class="flex gap-1 [border-bottom:1px_solid_var(--border-soft)]" {
                (tab_btn("甘特图", "gantt"))
                (tab_btn("负荷分析", "load"))
                (tab_btn("状态看板", "kanban"))
            }
            div class="flex items-center gap-2 pb-2" {
                button
                    class="inline-flex items-center justify-center w-7 h-7 rounded-sm text-sm font-medium cursor-pointer border border-border-soft bg-white text-fg-2 hover:bg-surface hover:border-accent transition-colors"
                    hx-get=(prev_url)
                    hx-target="#schedule-content"
                    hx-swap="outerHTML"
                    { "‹" }
                span class="text-[14px] font-medium text-[#595959] text-center min-w-[120px]" {
                    (from.format("%m/%d").to_string()) " - " (to.format("%m/%d").to_string())
                }
                button
                    class="inline-flex items-center justify-center w-7 h-7 rounded-sm text-sm font-medium cursor-pointer border border-border-soft bg-white text-fg-2 hover:bg-surface hover:border-accent transition-colors"
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
        div class="overflow-x-auto bg-white rounded-md border border-border-soft min-w-0" {
            table class="border-collapse w-full" {
                // ── Header row: dates ──
                thead {
                    tr {
                        th class="sticky left-0 z-10 bg-[#fafafa] w-[140px] min-w-[140px] border-r border-border-soft border-b border-border-soft" {}
                        @for date in &data.date_range {
                            (gantt_date_header(*date, today))
                        }
                    }
                }
                // ── Body rows: work centers ──
                tbody {
                    @for (wi, wc) in data.work_centers.iter().enumerate() {
                        tr {
                            td class="border-b border-border-soft bg-[#fafafa] text-center sticky left-0 z-[1] p-2 border-r border-border-soft" {
                                span class=(format!("inline-block w-2 h-2 rounded-full mr-1.5 align-middle {}", gantt_block_bg(wi))) {}
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
    let bg = if is_today {
        " bg-[#e6f4ff]"
    } else if is_weekend {
        " bg-[#fafafa]"
    } else {
        ""
    };
    html! {
        th class=(format!("text-center px-1 py-2 min-w-[72px] w-[72px] border-b border-border-soft border-l border-border-soft{}", bg)) {
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
    let bg = if is_today {
        " bg-[#e6f4ff]/30"
    } else if is_weekend {
        " bg-[#fafafa]/50"
    } else {
        ""
    };

    html! {
        td class=(format!("relative border-b border-border-soft border-l h-[48px] min-w-[72px] w-[72px] align-top p-0.5{}", bg)) {
            @for b in &bookings {
                (gantt_block(b))
            }
            @if bookings.is_empty() {
                span class="block h-full" {}
            }
        }
    }
}

fn gantt_block(b: &GanttBooking) -> Markup {
    let bg = gantt_block_bg((b.work_order_id as usize) % 8);
    let spans_days = b.date_to.date_naive() > b.date_from.date_naive();

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
            class=(format!("rounded-sm px-1 py-0.5 mb-0.5 text-[11px] leading-tight truncate text-white cursor-pointer {}", bg))
            title=(format!("{title} · {process} · {product} · {hours}h"))
        {
            span class="block" { (title) }
            span class="block opacity-80" { (process) }
            @if spans_days {
                span class="block opacity-60" { "→" }
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
        div class="overflow-x-auto bg-white rounded-md border border-border-soft min-w-0" {
            table class="border-collapse w-full" {
                thead {
                    tr {
                        th class="sticky left-0 z-10 bg-[#fafafa] w-[140px] min-w-[140px] border-r border-border-soft border-b border-border-soft" {}
                        @for date in date_range {
                            (load_date_header(*date, today))
                        }
                    }
                }
                tbody {
                    @for wc in work_centers {
                        tr {
                            td class="border-b border-border-soft bg-[#fafafa] sticky left-0 z-[1] p-2 border-r border-border-soft" {
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
            div class="flex gap-4 justify-end mt-4 text-[12px]" {
                span class="flex items-center gap-1.5" {
                    span class="w-3 h-3 rounded-sm bg-[#f5f5f5]" {}
                    "无排程"
                }
                span class="flex items-center gap-1.5" {
                    span class="w-3 h-3 rounded-sm bg-[rgba(82,196,26,0.12)]" {}
                    "<70%"
                }
                span class="flex items-center gap-1.5" {
                    span class="w-3 h-3 rounded-sm bg-[rgba(250,140,22,0.15)]" {}
                    "70-90%"
                }
                span class="flex items-center gap-1.5" {
                    span class="w-3 h-3 rounded-sm bg-[rgba(245,34,45,0.12)]" {}
                    ">90%"
                }
            }
        }
    }
}

fn load_date_header(date: NaiveDate, today: NaiveDate) -> Markup {
    let is_today = date == today;
    let is_weekend = matches!(date.weekday(), Weekday::Sat | Weekday::Sun);
    let bg = if is_today {
        " bg-[#e6f4ff]"
    } else if is_weekend {
        " bg-[#fafafa]"
    } else {
        ""
    };
    html! {
        th class=(format!("text-center px-1 py-2 min-w-[72px] w-[72px] border-b border-border-soft border-l border-border-soft{}", bg)) {
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
                "bg-[#f5f5f5] text-[#bfbfbf]"
            } else if pct_val > 90.0 {
                "bg-[rgba(245,34,45,0.12)] text-[#cf1322]"
            } else if pct_val > 70.0 {
                "bg-[rgba(250,140,22,0.15)] text-[#d46b08]"
            } else if pct_val > 0.0 {
                "bg-[rgba(82,196,26,0.12)] text-[#389e0d]"
            } else {
                "bg-[#f5f5f5] text-[#bfbfbf]"
            };
            (
                l.load_pct.to_string(),
                format_hours(l.booked_minutes),
                format_hours(l.available_minutes),
                level,
            )
        }
        None => (
            "0".to_string(),
            "0h".to_string(),
            "0h".to_string(),
            "bg-[#f5f5f5] text-[#bfbfbf]",
        ),
    };

    html! {
        td class="border-b border-border-soft border-l text-center h-[48px]" {
            div
                class=(format!("h-full flex items-center justify-center text-[13px] font-semibold font-mono {}", level_cls))
                title=(format!("已排 {booked} / 可用 {avail}"))
            {
                (pct) "%"
            }
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
            (kanban_column("待入库", &pending_receipt, "border-t-[#7c3aed]"))
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
        div class=(format!("bg-white rounded-md border border-border-soft border-t-[3px] flex flex-col {}", col_class)) {
            div class="flex items-center justify-between px-4 py-3 border-b border-border-soft" {
                span class="text-sm font-semibold text-fg" { (title) }
                span class="text-xs text-muted bg-[rgba(0,0,0,0.04)] px-2 py-0.5 rounded-full" { (cards.len()) }
            }
            div class="flex-1 p-3 flex flex-col gap-3 overflow-y-auto min-h-[200px]" {
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
        BatchStatus::Pending => ("待排产", "bg-warn-bg text-warn"),
        BatchStatus::InProgress => ("进行中", "bg-accent-bg text-accent"),
        BatchStatus::Suspended => ("已暂停", "bg-[#f5f5f5] text-muted"),
        BatchStatus::PendingReceipt => ("待入库", "bg-[rgba(124,58,237,0.1)] text-[#7c3aed]"),
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
        a class="block bg-white rounded-md border border-border-soft p-4 shadow-xs hover:shadow-md hover:-translate-y-px transition-all duration-200 cursor-pointer" href=(format!("/admin/mes/batches/{}", card.id)) {
            div class="flex items-center justify-between mb-2" {
                span class="text-xs font-mono tabular-nums text-fg font-semibold" { (card.batch_no) }
                span class=(format!("text-[11px] px-2 py-0.5 rounded-full font-medium {}", status_cls)) { (status_label) }
            }
            div class="text-sm text-fg mb-2" {
                (card.product_name.as_deref().unwrap_or("—"))
            }
            div class="text-xs text-muted mb-2" {
                span { (crate::utils::fmt_qty(card.completed_qty)) " / " (crate::utils::fmt_qty(card.batch_qty)) }
            }
            @if card.current_step > 0 {
                div class="mb-2" {
                    div class="h-1.5 bg-[rgba(0,0,0,0.06)] rounded-full overflow-hidden" {
                        div class="h-full bg-accent rounded-full transition-all duration-300" style=(format!("width:{}%", progress_pct)) {}
                    }
                    span class="text-[10px] text-muted mt-1 block" { (step_display) }
                }
            }
            @if !card.wo_doc_number.as_ref().is_none_or(|s| s.is_empty()) {
                div class="text-[10px] text-muted bg-[rgba(0,0,0,0.04)] px-2 py-0.5 rounded inline-block" {
                    "工单 " (card.wo_doc_number.as_deref().unwrap_or(""))
                }
            }
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Returns a Tailwind bg utility for a gantt block color (0-7 rotation).
fn gantt_block_bg(idx: usize) -> &'static str {
    const COLORS: [&str; 8] = [
        "bg-[#1677ff]",
        "bg-[#52c41a]",
        "bg-[#fa8c16]",
        "bg-[#722ed1]",
        "bg-[#eb2f96]",
        "bg-[#13c2c2]",
        "bg-[#f59e0b]",
        "bg-[#ef4444]",
    ];
    COLORS[idx % 8]
}

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
