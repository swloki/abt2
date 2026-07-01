use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::mes::production_exception::ProductionExceptionService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_exception::{ExceptionDetailPath, ExceptionListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[require_permission("WORK_ORDER", "read")]
pub async fn get_exception_detail(path: ExceptionDetailPath, ctx: RequestContext) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
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
 content, &nav_filter, ).into_string()))
}

fn exception_detail_page(
 exc: &abt_core::mes::production_exception::model::ProductionException,
 lookups: &abt_core::mes::production_exception::model::ExceptionDetailLookups,
 events: &[abt_core::mes::production_exception::model::ExceptionEvent],
) -> Markup {
 use abt_core::mes::enums::{ExceptionSeverity, ExceptionStatus, ExceptionType, ReasonCategory};

 let (type_label, type_cls) = match exc.exception_type {
 ExceptionType::BatchSuspended => ("批次暂停", "status-suspended"),
 ExceptionType::BatchScrapped => ("批次报废", "status-defect"),
 ExceptionType::DefectAnomaly => ("不良异常", "status-inspecting"),
 ExceptionType::InspectionFailed => ("报检不合格", "status-confirmed"),
 ExceptionType::EquipmentFault => ("设备故障", "status-progress"),
 };

 let (status_label, status_cls) = match exc.status {
 ExceptionStatus::Pending => ("待处理", "status-draft"),
 ExceptionStatus::Processing => ("处理中", "status-progress"),
 ExceptionStatus::Closed => ("已关闭", "status-completed"),
 ExceptionStatus::ConditionalRelease => ("条件放行", "status-inspecting"),
 ExceptionStatus::Resolved => ("已恢复", "status-completed"),
 };

 let (severity_label, severity_cls) = match exc.severity {
 ExceptionSeverity::Urgent => ("紧急", "status-suspended"),
 ExceptionSeverity::Normal => ("一般", "status-progress"),
 ExceptionSeverity::Low => ("低", "status-neutral"),
 };

 let reason_label = exc.reason_category.map(|r| match r {
 ReasonCategory::MaterialDefect => "物料不良",
 ReasonCategory::EquipmentFault => "设备故障",
 ReasonCategory::OperatorError => "操作失误",
 ReasonCategory::ProcessIssue => "工艺问题",
 }).unwrap_or("—");

 let impact_display = exc.impact_qty
 .map(crate::utils::fmt_qty)
 .unwrap_or_else(|| "—".to_string());

 html! {
    div {
        div class="flex items-center justify-between mb-6" {
            div class="flex items-center justify-between mb-6" {
                a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
                    href=(format!("{}?restore=true", ExceptionListPath::PATH))
                { "\u{2190} 返回列表" }
                h1 class="text-xl font-bold text-fg tracking-tight" { "异常 " (exc.doc_number) }
            }
        }
        // Status + severity
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
            div class="flex items-center mb-4 gap-3" {
                span class=(format!("status-pill {}", crate::utils::status_color(type_cls))) {
                    (type_label)
                }
                span class=(format!("status-pill {}", crate::utils::status_color(status_cls))) {
                    (status_label)
                }
                span class=(format!("status-pill {}", crate::utils::status_color(severity_cls))) {
                    (severity_label)
                }
            }
            div class="grid gap-4" {
                div class="flex flex-col gap-1" {
                    label { "异常类型" }
                    span { (type_label) }
                }
                div class="flex flex-col gap-1" {
                    label { "原因分类" }
                    span { (reason_label) }
                }
                div class="flex flex-col gap-1" {
                    label { "关联工单" }
                    span class="font-mono tabular-nums text-accent font-medium" {
                        @if let Some(ref wo) = lookups.wo_doc_number {
                            (wo)
                        } @else { "—" }
                    }
                }
                div class="flex flex-col gap-1" {
                    label { "关联批次" }
                    span class="font-mono tabular-nums" {
                        @if let Some(ref bn) = lookups.batch_no {
                            a   href=(format!("/admin/mes/batches/{}", exc.batch_id.unwrap_or(0)))
                                class="text-accent font-medium cursor-pointer"
                            { (bn) }
                        } @else { "—" }
                    }
                }
                div class="flex flex-col gap-1" {
                    label { "产品" }
                    span { (lookups.product_name.as_deref().unwrap_or("—")) }
                }
                div class="flex flex-col gap-1" {
                    label { "影响数量" }
                    span class="font-mono tabular-nums" { (impact_display) }
                }
                div class="flex flex-col gap-1" {
                    label { "发现时间" }
                    span { (exc.found_at.format("%Y-%m-%d %H:%M")) }
                }
                div class="flex flex-col gap-1" {
                    label { "发现人" }
                    span { (lookups.finder_name.as_deref().unwrap_or("—")) }
                }
                div class="flex flex-col gap-1" {
                    label { "负责人" }
                    span { (lookups.owner_name.as_deref().unwrap_or("—")) }
                }
                div class="flex flex-col gap-1" {
                    label { "处置方式" }
                    span { (exc.disposition.as_deref().unwrap_or("—")) }
                }
                div class="flex flex-col gap-1" {
                    label { "优先级" }
                    span { (severity_label) }
                }
                div class="flex flex-col gap-1" {
                    label { "状态" }
                    span { (status_label) }
                }
            }
        }
        // Description
        @if let Some(ref desc) = exc.description {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]"
            {
                div class="font-semibold mb-2" { "异常描述" }
                div class="whitespace-pre-wrap leading-relaxed" { (desc) }
            }
        }
        // Timeline
        @if !events.is_empty() {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]"
            {
                div class="font-semibold mb-4" { "处理时间线" }
                div class="flex flex-col gap-0" {
                    @for event in events {
                        div class="flex flex-col gap-0-item" {
                            div class="flex flex-col gap-0-dot" {}
                            div class="flex flex-col gap-0-content" {
                                div class="flex flex-col gap-0-time" {
                                    (event.created_at.format("%Y-%m-%d %H:%M"))
                                }
                                div class="flex flex-col gap-0-action" {
                                    (event_type_label(&event.event_type))
                                }
                                @if let Some(ref desc) = event.description {
                                    div class="flex flex-col gap-0-desc" { (desc) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
}


fn event_type_label(t: &str) -> String {
 match t {
 "reported" => "异常上报".to_string(),
 "suspended" => "批次暂停".to_string(),
 "repair_submitted" => "提交维修".to_string(),
 "repair_in_progress" => "维修进行中".to_string(),
 "resolved" => "已恢复".to_string(),
 "closed" => "已关闭".to_string(),
 "processing" => "处理中".to_string(),
 _ => t.to_string(),
 }
}