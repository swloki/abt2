use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::mes::production_exception::ProductionExceptionService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_exception::{ExceptionDetailPath, ExceptionListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[require_permission("MES", "read")]
pub async fn get_exception_detail(path: ExceptionDetailPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    let svc = state.production_exception_service();
    let exc = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
    let lookups = svc.get_detail_lookups(&mut conn, &exc).await?;
    let events = svc.list_events(&service_ctx, &mut conn, path.id).await?;

    let content = exception_detail_page(&exc, &lookups, &events);
    Ok(Html(admin_page(
        is_htmx, "异常详情", &claims, "production",
        &format!("/admin/mes/exceptions/{}", path.id),
        "生产管理", Some(ExceptionListPath::PATH),
        content,
    ).into_string()))
}

fn exception_detail_page(
    exc: &abt_core::mes::production_exception::model::ProductionException,
    lookups: &abt_core::mes::production_exception::model::ExceptionDetailLookups,
    events: &[abt_core::mes::production_exception::model::ExceptionEvent],
) -> Markup {
    use abt_core::mes::enums::{ExceptionSeverity, ExceptionStatus, ExceptionType, ReasonCategory};

    let (type_label, type_cls) = match exc.exception_type {
        ExceptionType::BatchSuspended => ("批次暂停", "pill-suspended"),
        ExceptionType::BatchScrapped => ("批次报废", "pill-pending"),
        ExceptionType::DefectAnomaly => ("不良异常", "pill-progress"),
        ExceptionType::InspectionFailed => ("报检不合格", "pill-receipt"),
        ExceptionType::EquipmentFault => ("设备故障", "pill-suspended"),
    };

    let (status_label, status_cls) = match exc.status {
        ExceptionStatus::Pending => ("待处理", "pill-pending"),
        ExceptionStatus::Processing => ("处理中", "pill-progress"),
        ExceptionStatus::Closed => ("已关闭", "pill-done"),
        ExceptionStatus::ConditionalRelease => ("条件放行", "pill-receipt"),
        ExceptionStatus::Resolved => ("已恢复", "pill-done"),
    };

    let (severity_label, severity_cls) = match exc.severity {
        ExceptionSeverity::Urgent => ("紧急", "pill-suspended"),
        ExceptionSeverity::Normal => ("一般", "pill-progress"),
        ExceptionSeverity::Low => ("低", "pill-pending"),
    };

    let reason_label = exc.reason_category.map(|r| match r {
        ReasonCategory::MaterialDefect => "物料不良",
        ReasonCategory::EquipmentFault => "设备故障",
        ReasonCategory::OperatorError => "操作失误",
        ReasonCategory::ProcessIssue => "工艺问题",
    }).unwrap_or("—");

    let impact_display = exc.impact_qty
        .map(|q| crate::utils::fmt_qty(q))
        .unwrap_or_else(|| "—".to_string());

    html! { div {
        div class="page-header" {
            div class="page-header-left" {
                a class="back-link" href=(ExceptionListPath::PATH) { "\u{2190} 返回列表" }
                h1 class="page-title" { "异常 " (exc.doc_number) }
            }
        }

        // Status + severity
        div class="info-card" {
            div style="display:flex;align-items:center;gap:var(--space-3);margin-bottom:var(--space-4)" {
                span class=(format!("kanban-card-pill {type_cls}")) { (type_label) }
                span class=(format!("kanban-card-pill {status_cls}")) { (status_label) }
                span class=(format!("kanban-card-pill {severity_cls}")) { (severity_label) }
            }
            div class="info-grid" {
                div class="info-item" { label { "异常类型" } span { (type_label) } }
                div class="info-item" { label { "原因分类" } span { (reason_label) } }
                div class="info-item" { label { "关联工单" } span class="mono" {
                    @if let Some(ref wo) = lookups.wo_doc_number {
                        a href=(format!("/admin/mes/orders/{}", exc.work_order_id.unwrap_or(0))) class="link-cell" { (wo) }
                    } @else { "—" }
                }}
                div class="info-item" { label { "关联批次" } span class="mono" {
                    @if let Some(ref bn) = lookups.batch_no {
                        a href=(format!("/admin/mes/batches/{}", exc.batch_id.unwrap_or(0))) class="link-cell" { (bn) }
                    } @else { "—" }
                }}
                div class="info-item" { label { "产品" } span { (lookups.product_name.as_deref().unwrap_or("—")) } }
                div class="info-item" { label { "影响数量" } span class="mono" { (impact_display) } }
                div class="info-item" { label { "发现时间" } span { (exc.found_at.format("%Y-%m-%d %H:%M")) } }
                div class="info-item" { label { "发现人" } span { (lookups.finder_name.as_deref().unwrap_or("—")) } }
                div class="info-item" { label { "负责人" } span { (lookups.owner_name.as_deref().unwrap_or("—")) } }
                div class="info-item" { label { "处置方式" } span { (exc.disposition.as_deref().unwrap_or("—")) } }
                div class="info-item" { label { "优先级" } span { (severity_label) } }
                div class="info-item" { label { "状态" } span { (status_label) } }
            }
        }

        // Description
        @if let Some(ref desc) = exc.description {
            div class="info-card" {
                div style="font-weight:600;margin-bottom:var(--space-2)" { "异常描述" }
                div style="white-space:pre-wrap;line-height:1.6" { (desc) }
            }
        }

        // Timeline
        @if !events.is_empty() {
            div class="info-card" {
                div style="font-weight:600;margin-bottom:var(--space-4)" { "处理时间线" }
                div class="timeline" {
                    @for event in events {
                        div class="timeline-item" {
                            div class="timeline-dot" {}
                            div class="timeline-content" {
                                div class="timeline-time" { (event.created_at.format("%Y-%m-%d %H:%M")) }
                                div class="timeline-action" { (event.event_type) }
                                @if let Some(ref desc) = event.description {
                                    div class="timeline-desc" { (desc) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }}
}
