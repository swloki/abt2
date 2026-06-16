use axum::response::Html;
use axum_extra::routing::TypedPath;
use chrono::Utc;
use maud::{html, Markup};

use abt_core::master_data::work_calendar::{model::*, WorkCalendarService};

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::md_work_calendar::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[require_permission("BOM", "read")]
pub async fn get_work_calendar_detail(
    path: WorkCalendarDetailPath,
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

    let svc = state.work_calendar_service();
    let cal = svc.get_calendar(&service_ctx, &mut conn, path.id).await?;
    let lines = svc.list_lines(&service_ctx, &mut conn, path.id).await?;

    let today = Utc::now().date_naive();
    let from = today - chrono::Duration::days(30);
    let to = today + chrono::Duration::days(365);
    let exceptions = svc
        .list_exceptions(&service_ctx, &mut conn, path.id, from, to)
        .await
        .unwrap_or_default();

    let content = work_calendar_detail_page(&cal, &lines, &exceptions);
    Ok(Html(
        admin_page(
            is_htmx,
            &format!("工作日历 {}", cal.name),
            &claims,
            "md",
            &format!("/admin/md/work-calendars/{}", path.id),
            "工程",
            Some(&cal.name),
            content,
            &nav_filter,
        )
        .into_string(),
    ))
}

fn work_calendar_detail_page(
    cal: &WorkCalendar,
    lines: &[CalendarLine],
    exceptions: &[CalendarException],
) -> Markup {
    html! {
        div class="flex items-center justify-between mb-6" {
            div class="flex items-center justify-between mb-6-left" {
                a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(WorkCalendarListPath::PATH) { "← 返回列表" }
                h1 class="text-xl font-bold text-fg tracking-tight" { "工作日历 " (cal.name) }
            }
        }

        // 基本信息
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
            div class="info-section-title" { "基本信息" }
            div class="grid gap-4" {
                div class="flex flex-col gap-1" { label { "名称" } span { (cal.name) } }
                div class="flex flex-col gap-1" {
                    label { "描述" }
                    span { (cal.description.as_deref().unwrap_or("—")) }
                }
                div class="flex flex-col gap-1" {
                    label { "创建时间" }
                    span class="mono" { (cal.created_at.format("%Y-%m-%d %H:%M")) }
                }
            }
        }

        // 工作时间明细
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
            div class="info-section-title" { "工作时间明细" }
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]-scroll" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "星期" }
                            th { "开始时间" }
                            th { "结束时间" }
                        }
                    }
                    tbody {
                        @for line in lines {
                            tr {
                                td { (weekday_label(line.weekday)) }
                                td class="mono" { (line.from_time.format("%H:%M")) }
                                td class="mono" { (line.to_time.format("%H:%M")) }
                            }
                        }
                        @if lines.is_empty() {
                            tr { td colspan="3" class="empty-row" { "暂无工作时间设置" } }
                        }
                    }
                }
            }
        }

        // 例外日
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
            div class="info-section-title" { "例外日（节假日/特殊工作日）" }
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]-scroll" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "日期" }
                            th { "类型" }
                            th { "工作时间" }
                            th { "备注" }
                        }
                    }
                    tbody {
                        @for ex in exceptions {
                            tr {
                                td class="mono" { (ex.exception_date.format("%Y-%m-%d")) }
                                td {
                                    @if ex.is_workday {
                                        span class="status-pill status-active" { "特殊工作日" }
                                    } @else {
                                        span class="status-pill status-inactive" { "休息日" }
                                    }
                                }
                                td class="mono" {
                                    @if let (Some(f), Some(t)) = (ex.from_time, ex.to_time) {
                                        (f.format("%H:%M")) " - " (t.format("%H:%M"))
                                    } @else {
                                        "—"
                                    }
                                }
                                td { (ex.remark.as_deref().unwrap_or("—")) }
                            }
                        }
                        @if exceptions.is_empty() {
                            tr { td colspan="4" class="empty-row" { "暂无例外日设置" } }
                        }
                    }
                }
            }
        }
    }
}

fn weekday_label(w: i16) -> &'static str {
    match w {
        0 => "周日",
        1 => "周一",
        2 => "周二",
        3 => "周三",
        4 => "周四",
        5 => "周五",
        6 => "周六",
        _ => "—",
    }
}
