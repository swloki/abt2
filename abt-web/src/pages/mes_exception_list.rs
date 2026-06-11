use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::enums::{ExceptionStatus, ExceptionType, ReasonCategory};
use abt_core::mes::production_exception::ProductionExceptionService;
use abt_core::shared::types::PaginatedResult;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_exception::{ExceptionListPath, ExceptionTablePath};
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

#[derive(Debug, Deserialize, Default)]
pub struct ExceptionQueryParams {
    pub page: Option<u32>,
    pub exception_type: Option<i16>,
    pub status: Option<i16>,
    pub reason_category: Option<i16>,
    pub keyword: Option<String>,
}

#[require_permission("WORK_ORDER", "read")]
pub async fn get_exception_table(
    _path: ExceptionTablePath,
    ctx: RequestContext,
    Query(params): Query<ExceptionQueryParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_exception_service();

    let page = params.page.unwrap_or(1);
    let filter = abt_core::mes::production_exception::model::ExceptionListFilter {
        exception_type: params.exception_type.and_then(ExceptionType::from_i16),
        status: params.status.and_then(ExceptionStatus::from_i16),
        reason_category: params.reason_category.and_then(ReasonCategory::from_i16),
        keyword: params.keyword,
        date_from: None,
        date_to: None,
    };

    let result = svc.list(&service_ctx, &mut conn, filter, page, 20).await?;
    Ok(Html(exception_table_fragment(&result).into_string()))
}

fn exception_list_page(
    stats: &abt_core::mes::production_exception::model::ExceptionStats,
    result: &PaginatedResult<abt_core::mes::production_exception::model::ExceptionListItem>,
) -> Markup {
    html! { div {
        div class="page-header" {
            h1 class="page-title" { "生产异常" }
        }

        // Stats row
        div class="board-stats" {
            div class="stat-card" {
                div class="stat-card-value" { (stats.total_month) }
                div class="stat-card-label" { "本月异常" }
            }
            div class="stat-card" {
                div class="stat-card-value stat-progress" { (stats.batch_suspended) }
                div class="stat-card-label" { "批次暂停" }
            }
            div class="stat-card" {
                div class="stat-card-value" style="color:var(--danger)" { (stats.batch_scrapped) }
                div class="stat-card-label" { "报废批次" }
            }
            div class="stat-card" {
                div class="stat-card-value stat-pending" { (stats.inspection_failed) }
                div class="stat-card-label" { "报检不合格" }
            }
        }

        // Filter bar
        div class="filter-bar" {
            input class="form-input" type="text" name="keyword" placeholder="搜索编号或描述..."
                hx-get=(ExceptionTablePath::PATH)
                hx-target="#exception-table"
                hx-trigger="keyup changed delay:300ms"
                hx-swap="innerHTML" {}
            select class="form-select" name="exception_type"
                hx-get=(ExceptionTablePath::PATH)
                hx-target="#exception-table"
                hx-trigger="change"
                hx-swap="innerHTML" {
                option value="" { "全部类型" }
                option value="1" { "批次暂停" }
                option value="2" { "批次报废" }
                option value="3" { "不良异常" }
                option value="4" { "报检不合格" }
                option value="5" { "设备故障" }
            }
        }

        // Table
        div id="exception-table" {
            (exception_table_fragment(result))
        }
    }}
}

fn exception_table_fragment(
    result: &PaginatedResult<abt_core::mes::production_exception::model::ExceptionListItem>,
) -> Markup {
    html! {
        div class="data-card" {
            div class="data-card-scroll" {
                table class="data-table" {
                    thead { tr {
                        th { "异常编号" }
                        th { "类型" }
                        th { "关联" }
                        th { "描述" }
                        th class="num-right" { "影响数量" }
                        th { "发现时间" }
                        th { "状态" }
                    }}
                    tbody {
                        @if result.items.is_empty() {
                            tr { td colspan="7" class="text-center-empty" { "暂无异常记录" } }
                        }
                        @for item in &result.items {
                            tr {
                                td class="mono" {
                                    a href=(format!("/admin/mes/exceptions/{}", item.id)) class="link-cell" { (item.doc_number) }
                                }
                                td { (exception_type_label(&item.exception_type)) }
                                td {
                                    div class="cell-stack" {
                                        @if let Some(ref wo) = item.wo_doc_number {
                                            span class="sub" {
                                                a href=(format!("/admin/mes/orders/{}", item.work_order_id.unwrap_or(0))) class="link-cell" { (wo) }
                                            }
                                        }
                                        @if let Some(ref bn) = item.batch_no {
                                            a href=(format!("/admin/mes/batches/{}", item.batch_id.unwrap_or(0))) class="link-cell" { (bn) }
                                        }
                                    }
                                }
                                td style="max-width:200px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap" {
                                    (item.description.as_deref().unwrap_or("—"))
                                }
                                td class="num-right mono" {
                                    (item.impact_qty.map(crate::utils::fmt_qty).unwrap_or_else(|| "—".to_string()))
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
            div class="pagination" {
                span class="pagination-info" { "共 " (result.total) " 条" }
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
    html! { span class=(format!("status-pill {cls}")) { (label) } }
}

fn exception_status_label(s: &ExceptionStatus) -> Markup {
    let (label, cls) = match s {
        ExceptionStatus::Pending => ("待处理", "status-draft"),
        ExceptionStatus::Processing => ("处理中", "status-progress"),
        ExceptionStatus::Closed => ("已关闭", "status-completed"),
        ExceptionStatus::ConditionalRelease => ("条件放行", "status-inspecting"),
        ExceptionStatus::Resolved => ("已恢复", "status-completed"),
    };
    html! { span class=(format!("status-pill {cls}")) { (label) } }
}
