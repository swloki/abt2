use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;

use abt_core::purchase::enums::MiscRequestStatus;
use abt_core::purchase::misc_request::model::*;
use abt_core::purchase::misc_request::MiscellaneousRequestService;
use abt_core::shared::identity::{DepartmentService, UserService};

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::misc_request::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: MiscRequestStatus) -> (&'static str, &'static str) {
 match s {
 MiscRequestStatus::Draft => ("草稿", "status-draft"),
 MiscRequestStatus::Approved => ("已审批", "status-confirmed"),
 MiscRequestStatus::Purchasing => ("采购中", "status-progress"),
 MiscRequestStatus::Received => ("已收货", "status-shipped"),
 MiscRequestStatus::Closed => ("已关闭", "status-completed"),
 MiscRequestStatus::Cancelled => ("已取消", "status-cancelled"),
 }
}

// ── Handlers ──

#[require_permission("MISC_REQUEST", "read")]
pub async fn get_misc_detail(
 path: MiscDetailPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
 let svc = state.misc_request_service();
 let user_svc = state.user_service();

 let req = svc.get(&service_ctx, &mut conn, path.id).await?;
 let items = svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

 let operator_name = user_svc
 .get_user(&service_ctx, &mut conn, req.operator_id)
 .await
 .map(|u| u.display_name.unwrap_or(u.username))
 .unwrap_or_else(|_| "—".into());

 let dept_svc = state.department_service();
 let department_name = dept_svc
 .get_department(&service_ctx, &mut conn, req.department_id)
 .await
 .map(|d| d.department_name)
 .unwrap_or_else(|_| "—".into());

 let total_amount: Decimal = items.iter().map(|i| {
 i.estimated_price.unwrap_or(Decimal::ZERO) * i.quantity
 }).sum();

 let content = misc_detail_page(&req, &items, &department_name, &operator_name, total_amount);
 let page_html = admin_page(
 is_htmx, "零星请购详情", &claims, "purchase",
 &format!("{}/{}", MiscListPath::PATH, path.id),
 "采购管理", Some("零星请购详情"), content, &nav_filter,
 );

 Ok(Html(page_html.into_string()))
}

#[require_permission("MISC_REQUEST", "update")]
pub async fn approve_misc(
 path: MiscApprovePath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.misc_request_service();

 svc.approve(&service_ctx, &mut conn, path.id, None).await?;

 let redirect = MiscDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("MISC_REQUEST", "update")]
pub async fn cancel_misc(
 path: MiscCancelPath,
 ctx: RequestContext,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.misc_request_service();

 svc.cancel(&service_ctx, &mut conn, path.id, None).await?;

 let redirect = MiscDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Workflow Steps ──

fn workflow_steps(current: MiscRequestStatus) -> Markup {
 let steps: &[(&str, MiscRequestStatus)] = &[
 ("草稿", MiscRequestStatus::Draft),
 ("已审批", MiscRequestStatus::Approved),
 ("采购中", MiscRequestStatus::Purchasing),
 ("已收货", MiscRequestStatus::Received),
 ("已关闭", MiscRequestStatus::Closed),
 ];
 let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);
 let is_cancelled = current == MiscRequestStatus::Cancelled;

 html! {
    div class="flex items-center mt-6 mb-6" {
        @for (i, (label, _)) in steps.iter().enumerate() {
            @if i > 0 {
                div class=({
                        format!(
                            "w-[48px] h-[2px] {}",
                            if i <= current_idx && !is_cancelled {
                                "bg-success"
                            } else {
                                "bg-border"
                            },
                        )
                    }) {}
            }
            @let (dot_cls, text_cls, ring_cls) = if is_cancelled {
                ("bg-border-soft", "text-muted", "")
            } else if i < current_idx {
                ("bg-success", "text-success", "")
            } else if i == current_idx {
                (
                    "bg-accent",
                    "text-accent font-semibold",
                    "shadow-[0_0_0_3px_rgba(37,99,235,0.1)]",
                )
            } else {
                ("bg-slate-300", "text-slate-400", "")
            };
            div class="flex items-center gap-2 shrink-0" {
                span class=(format!("w-2.5 h-2.5 rounded-full shrink-0 {} {}", dot_cls, ring_cls)) {}
                span class=(format!("text-xs whitespace-nowrap font-medium {}", text_cls)) { (label) }
            }
        }
        @if is_cancelled {
            div class="w-[48px] h-[2px] bg-border" {}
            div class="flex items-center gap-2 shrink-0" {
                span class="w-2.5 h-2.5 rounded-full shrink-0 bg-danger-500" {}
                span class="text-xs text-danger-500 font-semibold whitespace-nowrap" { "已取消" }
            }
        }
    }
}
}

// ── Components ──

fn misc_detail_page(
 req: &MiscellaneousRequest,
 items: &[MiscRequestItem],
 department_name: &str,
 operator_name: &str,
 total_amount: Decimal,
) -> Markup {
 let (status_text, status_class) = status_label(req.status);

 html! {
    div {
        // ── Back Link ──
        a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
            href=(format!("{}?restore=true", MiscListPath::PATH))
        { (icon::chevron_left_icon("w-4 h-4")) "返回零星请购列表" }
        // ── Detail Header ──
        div class="block bg-bg border border-border-soft rounded-lg p-6" {
            div {
                div class="flex items-center justify-between" {
                    h1 class="text-2xl font-extrabold font-mono tabular-nums" { (req.doc_number) }
                    span class=({
                        format!(
                            "status-pill {}",
                            crate::utils::status_color(status_class),
                        )
                    }) { (status_text) }
                }
            }
            div class="flex gap-3" {
                @if req.status == MiscRequestStatus::Draft {
                    button
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                        hx-post=(MiscApprovePath { id: req.id }.to_string())
                        hx-confirm="确认审批此零星请购？"
                    { (icon::check_circle_icon("w-4 h-4")) "审批" }
                    button
                        class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90"
                        hx-post=(MiscCancelPath { id: req.id }.to_string())
                        hx-confirm="确认取消此零星请购？取消后不可恢复。"
                    { "取消" }
                }
            }
        }
        // ── Workflow Steps ──
        (workflow_steps(req.status))
        // ── Request Info ──
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
            div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
                "请购信息"
            }
            div class="grid gap-4" {
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "用途说明" }
                    span class="text-sm text-fg font-medium" { (req.purpose) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "申请部门" }
                    span class="text-sm text-fg font-medium" { (department_name) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "申请日期" }
                    span class="text-sm text-fg font-medium font-mono tabular-nums" {
                        (req.request_date.format("%Y-%m-%d"))
                    }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "申请人" }
                    span class="text-sm text-fg font-medium" { (operator_name) }
                }
            }
        }
        // ── Items Table ──
        div class="data-card" {
            div class="overflow-x-auto" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "行号" }
                            th { "物品名称" }
                            th { "规格" }
                            th class="text-right text-[13px]" { "数量" }
                            th { "单位" }
                            th class="text-right text-[13px]" { "预估单价" }
                            th class="text-right text-[13px]" { "预估小计" }
                            th { "备注" }
                        }
                    }
                    tbody {
                        @for item in items { (item_row(item)) }
                        @if items.is_empty() {
                            tr {
                                td colspan="8" class="text-center text-muted py-8" { "暂无明细" }
                            }
                        }
                    }
                }
            }
        }
        // ── Amount Summary ──
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]"
            class="mt-6"
        {
            div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
                "金额汇总"
            }
            div class="grid gap-4" {
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "总金额" }
                    span
                        class="text-sm text-fg font-medium font-mono tabular-nums"
                        class="font-semibold"
                        style="font-size:1.125rem"
                    { (format!("{:.2}", total_amount)) }
                }
            }
        }
        // ── Remarks ──
        @if !req.remark.is_empty() {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]"
                class="mt-6"
            {
                div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
                    "备注"
                }
                p class="text-muted" { (req.remark.as_str()) }
            }
        }
    }
}
}

fn item_row(item: &MiscRequestItem) -> Markup {
 let spec = item.specification.as_deref().unwrap_or("—");
 let price = item.estimated_price
 .map(|p| format!("{:.2}", p))
 .unwrap_or_else(|| "—".into());
 let subtotal = item.estimated_price
 .map(|p| format!("{:.2}", p * item.quantity))
 .unwrap_or_else(|| "—".into());
 let remark = item.remark.as_deref().unwrap_or("—");

 html! {
    tr {
        td class="font-mono tabular-nums" { (item.line_no) }
        td { (item.item_name) }
        td { (spec) }
        td class="text-right text-[13px] font-mono tabular-nums" { (format!("{:.2}", item.quantity)) }
        td { (item.unit) }
        td class="text-right text-[13px]" { (price) }
        td class="text-right text-[13px]" { (subtotal) }
        td { (remark) }
    }
}
}
