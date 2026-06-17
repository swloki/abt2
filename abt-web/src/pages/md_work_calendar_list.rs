use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::work_calendar::{model::*, WorkCalendarService};

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::md_work_calendar::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[require_permission("BOM", "read")]
pub async fn get_work_calendar_list(
    _path: WorkCalendarListPath,
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

    let calendars = state
        .work_calendar_service()
        .list_calendars(&mut conn)
        .await?;

    let content = work_calendar_list_page(&calendars);
    Ok(Html(
        admin_page(
            is_htmx,
            "工作日历管理",
            &claims,
            "md",
            WorkCalendarListPath::PATH,
            "工程",
            Some(WorkCalendarListPath::PATH),
            content,
            &nav_filter,
        )
        .into_string(),
    ))
}

fn work_calendar_list_page(calendars: &[WorkCalendar]) -> Markup {
    html! {
        div class="flex items-center justify-between mb-6" {
            div class="flex items-center justify-between mb-6-left" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "工作日历管理" }
            }
            div class="flex gap-3" {
                a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(WorkCalendarCreatePath::PATH) {
                    (icon::plus_icon("w-4 h-4"))
                    "新建日历"
                }
            }
        }

        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] overflow-x-auto" {
                table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer group/tr [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" {
                    thead {
                        tr {
                            th { "名称" }
                            th { "描述" }
                            th { "创建时间" }
                            th class="text-right" { "操作" }
                        }
                    }
                    tbody {
                        @for cal in calendars {
                            tr {
                                td { strong { (cal.name) } }
                                td { (cal.description.as_deref().unwrap_or("—")) }
                                td class="font-mono tabular-nums" { (cal.created_at.format("%Y-%m-%d %H:%M")) }
                                td {
                                    a href=(WorkCalendarDetailPath { id: cal.id }.to_string()) {
                                        (icon::eye_icon("w-4 h-4"))
                                    }
                                }
                            }
                        }
                        @if calendars.is_empty() {
                            tr { td colspan="4" class="text-center text-text-muted text-sm" { "暂无工作日历数据" } }
                        }
                    }
                }
            }
        }
    }
}
