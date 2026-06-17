use std::collections::HashMap;

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::product::ProductService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::enums::PurchaseReturnStatus;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::purchase::return_order::model::*;
use abt_core::purchase::return_order::PurchaseReturnService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_return::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn status_label(s: PurchaseReturnStatus) -> (&'static str, &'static str) {
    match s {
        PurchaseReturnStatus::Draft => ("草稿", "status-draft"),
        PurchaseReturnStatus::Confirmed => ("已确认", "status-confirmed"),
        PurchaseReturnStatus::Shipped => ("已发货", "status-shipped"),
        PurchaseReturnStatus::Settled => ("已结算", "status-completed"),
        PurchaseReturnStatus::Cancelled => ("已取消", "status-cancelled"),
    }
}

// ── Handlers ──

#[require_permission("PURCHASE_RETURN", "read")]
pub async fn get_pr_detail(
    path: PRDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_return_service();
    let supplier_svc = state.supplier_service();
    let order_svc = state.purchase_order_service();
    let product_svc = state.product_service();
    let user_svc = state.user_service();

    let pr = svc.get(&service_ctx, &mut conn, path.id).await?;
    let items = svc.list_items(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

    let supplier_name = supplier_svc
        .get(&service_ctx, &mut conn, pr.supplier_id)
        .await
        .map(|s| s.name)
        .unwrap_or_else(|_| "未知供应商".into());

    let order_doc_number = order_svc
        .get(&service_ctx, &mut conn, pr.order_id)
        .await
        .map(|o| o.doc_number)
        .ok();

    let operator_name = user_svc
        .get_user(&service_ctx, &mut conn, pr.operator_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());

    let (product_names, product_codes, product_specs, product_units) = {
        let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
        if product_ids.is_empty() {
            (HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new())
        } else {
            let products = product_svc.get_by_ids(&service_ctx, &mut conn, product_ids).await.unwrap_or_default();
            let names: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.pdt_name.clone())).collect();
            let codes: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.product_code.clone())).collect();
            let specs: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.meta.specification.clone())).collect();
            let units: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.unit.clone())).collect();
            (names, codes, specs, units)
        }
    };

    let content = pr_detail_page(&pr, &items, &supplier_name, &order_doc_number, &operator_name, &product_names, &product_codes, &product_specs, &product_units);
    let page_html = admin_page(
        is_htmx, "退货详情", &claims, "purchase",
        &format!("{}/{}", PRListPath::PATH, path.id),
        "采购管理", Some("退货详情"), content, &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("PURCHASE_RETURN", "update")]
pub async fn confirm_pr(
    path: PRConfirmPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_return_service();

    svc.confirm(&service_ctx, &mut conn, path.id, None).await?;

    let redirect = PRDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("PURCHASE_RETURN", "update")]
pub async fn cancel_pr(
    path: PRCancelPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_return_service();

    svc.cancel(&service_ctx, &mut conn, path.id, None).await?;

    let redirect = PRDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Workflow Steps ──

fn workflow_steps(current: PurchaseReturnStatus) -> Markup {
    let steps: &[(&str, PurchaseReturnStatus)] = &[
        ("草稿", PurchaseReturnStatus::Draft),
        ("已确认", PurchaseReturnStatus::Confirmed),
        ("已发货", PurchaseReturnStatus::Shipped),
        ("已结算", PurchaseReturnStatus::Settled),
    ];
    let current_idx = steps.iter().position(|(_, s)| *s == current).unwrap_or(0);
    let is_cancelled = current == PurchaseReturnStatus::Cancelled;

    html! {
        div class="flex items-center" {
            @for (i, (label, _)) in steps.iter().enumerate() {
                @if i > 0 {
                    @let line_class = if i <= current_idx && !is_cancelled { "wf-line completed" } else { "wf-line" };
                    div class=(line_class) {}
                }
                @let step_class = if is_cancelled {
                    "wf-step"
                } else if i < current_idx {
                    "wf-step completed"
                } else if i == current_idx {
                    "wf-step current"
                } else {
                    "wf-step"
                };
                div class=(step_class) {
                    span class="w-[10px] h-[10px] rounded-full bg-border" {}
                    (label)
                }
            }
            @if is_cancelled {
                div class="w-[48px] h-[2px] bg-border" {}
                div class="flex items-center gap-2 text-xs text-text-muted" style="color:var(--danger)" {
                    span class="w-[10px] h-[10px] rounded-full bg-border" {}
                    "已取消"
                }
            }
        }
    }
}

// ── Components ──

fn pr_detail_page(
    pr: &PurchaseReturn,
    items: &[PurchaseReturnItem],
    supplier_name: &str,
    order_doc_number: &Option<String>,
    operator_name: &str,
    product_names: &HashMap<i64, String>,
    product_codes: &HashMap<i64, String>,
    product_specs: &HashMap<i64, String>,
    product_units: &HashMap<i64, String>,
) -> Markup {
    let (status_text, status_class) = status_label(pr.status);

    html! {
        div {
            // ── Back Link ──
            a class="inline-flex items-center gap-2 text-sm text-text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", PRListPath::PATH)) {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回采购退货列表"
            }

            // ── Detail Header ──
            div class="block bg-bg border border-border-soft rounded-lg p-6" {
                div {
                    div class="flex items-center justify-between" {
                        h1 class="text-2xl font-extrabold font-font-mono tabular-nums" { (pr.doc_number) }
                        span class=(format!("status-pill {status_class}")) { (status_text) }
                    }
                }
                div class="flex gap-3" {
                    @if pr.status == PurchaseReturnStatus::Draft {
                        button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                            hx-post=(PRConfirmPath { id: pr.id }.to_string())
                            hx-confirm="确认此退货单？确认后将执行退货。" {
                            (icon::check_circle_icon("w-4 h-4"))
                            "确认退货"
                        }
                        button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90"
                            hx-post=(PRCancelPath { id: pr.id }.to_string())
                            hx-confirm="确认取消此退货单？取消后不可恢复。" {
                            "取消"
                        }
                    }
                }
            }

            // ── Workflow Steps ──
            (workflow_steps(pr.status))

            // ── Return Info ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]-title" { "退货信息" }
                div class="grid gap-4" {
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "供应商名称" }
                        span class="text-sm text-fg font-medium" { (supplier_name) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "关联订单" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" {
                            @if let Some(doc) = order_doc_number {
                                (doc)
                            } @else {
                                "—"
                            }
                        }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "退货日期" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" { (pr.return_date.format("%Y-%m-%d")) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "退货原因" }
                        span class="text-sm text-fg font-medium" { (pr.return_reason.as_str()) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "操作人" }
                        span class="text-sm text-fg font-medium" { (operator_name) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-text-muted font-medium" { "创建时间" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" { (pr.created_at.format("%Y-%m-%d %H:%M")) }
                    }
                }
            }

            // ── Items Table ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" {
                div class="overflow-x-auto" {
                    table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:text-left [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" {
                        thead {
                            tr {
                                th { "行号" }
                                th { "物料编码" }
                                th { "物料名称" }
                                th { "规格" }
                                th { "单位" }
                                th class="text-right text-[13px]" { "退货数量" }
                                th class="text-right text-[13px]" { "单价" }
                                th class="text-right text-[13px]" { "退货金额" }
                            }
                        }
                        tbody {
                            @for (idx, item) in items.iter().enumerate() {
                                (item_row(idx, item, product_names, product_codes, product_specs, product_units))
                            }
                            @if items.is_empty() {
                                tr {
                                    td colspan="8" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无明细"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Amount Summary ──
            div class="flex justify-end gap-8 p-5 border-t bg-surface-raised" {
                div class="flex gap-3" {
                    span class="text-[11px] text-text-muted font-medium uppercase" { "退货总额" }
                    span class="text-[20px] font-bold text-fg accent" { (format!("¥ {:.2}", pr.total_amount)) }
                }
            }
        }
    }
}

fn item_row(
    idx: usize,
    item: &PurchaseReturnItem,
    names: &HashMap<i64, String>,
    codes: &HashMap<i64, String>,
    specs: &HashMap<i64, String>,
    units: &HashMap<i64, String>,
) -> Markup {
    let product_name = names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let product_code = codes.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let product_spec = specs.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    let product_unit = units.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
    html! {
        tr {
            td class="font-mono tabular-nums" { (idx + 1) }
            td class="font-mono tabular-nums" { (product_code) }
            td { (product_name) }
            td { (product_spec) }
            td { (product_unit) }
            td class="text-right text-[13px]" { (item.returned_qty) }
            td class="text-right text-[13px]" { (format!("{:.2}", item.unit_price)) }
            td class="text-right text-[13px]" { (format!("{:.2}", item.amount)) }
        }
    }
}
