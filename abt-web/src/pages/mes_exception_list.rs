use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::mes::enums::{ExceptionStatus, ExceptionType};
use abt_core::mes::production_exception::ProductionExceptionService;
use abt_core::shared::types::PaginatedResult;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_exception::ExceptionListPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[require_permission("WORK_ORDER", "read")]
pub async fn get_exception_list(_path: ExceptionListPath, ctx: RequestContext) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

 let svc = state.production_exception_service();
 let stats = svc.get_stats(&service_ctx, &mut conn).await?;
 let filter = abt_core::mes::production_exception::model::ExceptionListFilter::default();
 let result = svc.list(&service_ctx, &mut conn, filter, 1, 20).await?;

 let content = exception_list_page(&stats, &result);
 Ok(Html(admin_page(is_htmx, "生产异常", &claims, "production", ExceptionListPath::PATH, "生产管理", None, content, &nav_filter).into_string()))
}

fn exception_list_page(
 stats: &abt_core::mes::production_exception::model::ExceptionStats,
 result: &PaginatedResult<abt_core::mes::production_exception::model::ExceptionListItem>,
) -> Markup {
 html! {
    div {
        div class="flex items-center justify-between mb-6" {
            h1 class="text-xl font-bold text-fg tracking-tight" { "生产异常" }
        }
        // ── Stats row ──
        div class="grid grid-cols-2 lg:grid-cols-4 gap-4 mb-5" {
            div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md" {
                div class="w-11 h-11 rounded-md grid place-items-center shrink-0 bg-accent-bg text-accent"
                { (icon::circle_alert_icon("w-5 h-5")) }
                div {
                    div class="text-2xl font-bold font-mono tabular-nums text-fg" {
                        (stats.total_month)
                    }
                    div class="text-sm text-muted mt-1" { "本月异常" }
                }
            }
            div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md" {
                div class="w-11 h-11 rounded-md grid place-items-center shrink-0 bg-warn-bg text-warn"
                { (icon::clock_icon("w-5 h-5")) }
                div {
                    div class="text-2xl font-bold font-mono tabular-nums text-fg" {
                        (stats.batch_suspended)
                    }
                    div class="text-sm text-muted mt-1" { "批次暂停" }
                }
            }
            div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md" {
                div class="w-11 h-11 rounded-md grid place-items-center shrink-0 bg-danger-bg text-danger"
                { (icon::trash_icon("w-5 h-5")) }
                div {
                    div class="text-2xl font-bold font-mono tabular-nums text-danger" {
                        (stats.batch_scrapped)
                    }
                    div class="text-sm text-muted mt-1" { "报废批次" }
                }
            }
            div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md" {
                div class="w-11 h-11 rounded-md grid place-items-center shrink-0 bg-danger-bg text-danger"
                { (icon::alert_triangle_icon("w-5 h-5")) }
                div {
                    div class="text-2xl font-bold font-mono tabular-nums text-muted" {
                        (stats.inspection_failed)
                    }
                    div class="text-sm text-muted mt-1" { "报检不合格" }
                }
            }
        }
        // ── Filter bar ──
        div class="flex items-center gap-3 mb-5 flex-wrap" {
            div class="relative w-60" {
                ({
                    icon::search_icon(
                        "absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted",
                    )
                })
                input
                    class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input"
                    type="text"
                    name="keyword"
                    placeholder="搜索编号或描述…"
                    hx-get=(ExceptionListPath::PATH)
                    hx-target="#exception-table"
                    hx-trigger="keyup changed delay:300ms"
                    hx-sync="this:replace"
                    hx-swap="innerHTML" {}
            }
            select
                class="w-40 px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent cursor-pointer"
                name="exception_type"
                hx-get=(ExceptionListPath::PATH)
                hx-target="#exception-table"
                hx-trigger="change"
                hx-swap="innerHTML"
            {
                option value="" { "全部类型" }
                option value="1" { "批次暂停" }
                option value="2" { "批次报废" }
                option value="3" { "不良异常" }
                option value="4" { "报检不合格" }
                option value="5" { "设备故障" }
            }
        }
        // Table
        div id="exception-table" { (exception_table_fragment(result)) }
    }
}
}

fn exception_table_fragment(
 result: &PaginatedResult<abt_core::mes::production_exception::model::ExceptionListItem>,
) -> Markup {
 html! {
    div class="data-card" {
        div class="overflow-x-auto" {
            table class="data-table" {
                thead {
                    tr {
                        th { "异常编号" }
                        th { "类型" }
                        th { "关联" }
                        th { "描述" }
                        th class="text-right text-[13px]" { "影响数量" }
                        th { "发现时间" }
                        th { "状态" }
                    }
                }
                tbody {
                    @if result.items.is_empty() {
                        tr {
                            td colspan="7" class="text-center py-8 text-sm text-muted" { "暂无异常记录" }
                        }
                    }
                    @for item in &result.items {
                        tr {
                            td class="font-mono tabular-nums" {
                                a   href=(format!("/admin/mes/exceptions/{}", item.id))
                                    class="text-accent font-medium cursor-pointer"
                                { (item.doc_number) }
                            }
                            td { (exception_type_label(&item.exception_type)) }
                            td {
                                div class="flex flex-col gap-1" {
                                    @if let Some(ref wo) = item.wo_doc_number {
                                        span class="text-sm" {
                                            a   href=({
                                                    format!(
                                                        "/admin/mes/orders/{}",
                                                        item.work_order_id.unwrap_or(0),
                                                    )
                                                })
                                                class="text-accent font-medium hover:underline"
                                            { (wo) }
                                        }
                                    }
                                    @if let Some(ref bn) = item.batch_no {
                                        a   href=(format!("/admin/mes/batches/{}", item.batch_id.unwrap_or(0)))
                                            class="text-accent font-medium text-sm hover:underline"
                                        { (bn) }
                                    }
                                }
                            }
                            td class="max-w-[200px] truncate text-sm text-fg-2" {
                                (item.description.as_deref().unwrap_or("—"))
                            }
                            td class="text-right text-[13px] font-mono tabular-nums" {
                                ({
                                    item.impact_qty
                                        .map(crate::utils::fmt_qty)
                                        .unwrap_or_else(|| "—".to_string())
                                })
                            }
                            td { (item.found_at.format("%Y-%m-%d %H:%M")) }
                            td { (exception_status_label(&item.status)) }
                        }
                    }
                }
            }
        }
    }
    @if result.total_pages > 1 {
        div class="flex items-center justify-between py-4 px-5" {
            span class="text-sm text-muted" { "共 " (result.total) " 条" }
        }
    }
}
}

fn exception_type_label(t: &ExceptionType) -> Markup {
 let (label, cls) = match t {
 ExceptionType::BatchSuspended => ("批次暂停", "status-suspended"),
 ExceptionType::BatchScrapped => ("批次报废", "status-defect"),
 ExceptionType::DefectAnomaly => ("不良异常", "status-inspecting"),
 ExceptionType::InspectionFailed => ("报检不合格", "status-confirmed"),
 ExceptionType::EquipmentFault => ("设备故障", "status-progress"),
 };
 html! {
    span class=(format!("status-pill {}", crate::utils::status_color(cls))) { (label) }
}
}

fn exception_status_label(s: &ExceptionStatus) -> Markup {
 let (label, cls) = match s {
 ExceptionStatus::Pending => ("待处理", "status-draft"),
 ExceptionStatus::Processing => ("处理中", "status-progress"),
 ExceptionStatus::Closed => ("已关闭", "status-completed"),
 ExceptionStatus::ConditionalRelease => ("条件放行", "status-inspecting"),
 ExceptionStatus::Resolved => ("已恢复", "status-completed"),
 };
 html! {
    span class=(format!("status-pill {}", crate::utils::status_color(cls))) { (label) }
}
}
