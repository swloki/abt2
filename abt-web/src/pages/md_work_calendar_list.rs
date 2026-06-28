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
 "production",
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
        div class="flex items-center justify-between mb-6" {
            h1 class="text-xl font-bold text-fg tracking-tight" { "工作日历管理" }
        }
        div class="flex gap-3" {
            a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                href=(WorkCalendarCreatePath::PATH)
            { (icon::plus_icon("w-4 h-4")) "新建日历" }
        }
    }

    div class="data-card" {
        div class="overflow-x-auto" {
            table class="data-table" {
                thead {
                    tr {
                        th { "名称" }
                        th { "描述" }
                        th { "创建时间" }
                        th class="!text-right" { "操作" }
                    }
                }
                tbody {
                    @for cal in calendars {
                        tr {
                            td {
                                strong { (cal.name) }
                            }
                            td { (cal.description.as_deref().unwrap_or("—")) }
                            td class="font-mono tabular-nums" {
                                (cal.created_at.format("%Y-%m-%d %H:%M"))
                            }
                            td {
                                a href=({
                                    WorkCalendarDetailPath {
                                        id: cal.id,
                                    }
                                        .to_string()
                                }) { (icon::eye_icon("w-4 h-4")) }
                            }
                        }
                    }
                    @if calendars.is_empty() {
                        tr {
                            td colspan="4" class="text-center text-muted text-sm" { "暂无工作日历数据" }
                        }
                    }
                }
            }
        }
    }
}
}
