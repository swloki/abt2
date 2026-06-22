use axum::response::Html;
use maud::{html, Markup};

use crate::errors::Result;
use crate::routes::wms_transfer::TransferDetailPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;
use crate::layout::page::admin_page;

use abt_core::wms::enums::TransferStatus;
use abt_core::wms::transfer::{TransferItem, TransferService};
use abt_core::master_data::product::ProductService;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::shared::identity::UserService;
use crate::components::icon;

// ── Form Data ──

#[derive(Debug, serde::Deserialize)]
pub struct TransferActionForm {
 pub action: String,
}

// ── Handlers ──

#[require_permission("INVENTORY", "read")]
pub async fn get_transfer_detail(
 path: TransferDetailPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.transfer_service();

 let transfer = svc.get(&service_ctx, &mut conn, path.id).await?;
 let items = svc.get_items(&service_ctx, &mut conn, path.id).await?;

 let from_wh_name = state.warehouse_service()
 .get(&service_ctx, &mut conn, transfer.from_warehouse_id)
 .await
 .map(|w| w.name)
 .unwrap_or_else(|_| "—".into());

 let to_wh_name = state.warehouse_service()
 .get(&service_ctx, &mut conn, transfer.to_warehouse_id)
 .await
 .map(|w| w.name)
 .unwrap_or_else(|_| "—".into());

 let operator_name = state.user_service()
 .get_user(&service_ctx, &mut conn, transfer.operator_id)
 .await
 .map(|u| u.display_name.unwrap_or(u.username))
 .unwrap_or_else(|_| "—".into());

 let product_svc = state.product_service();
 let mut product_codes: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
 let mut product_names: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
 let mut product_specs: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
 let mut product_units: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
 for item in &items {
 if !product_names.contains_key(&item.product_id)
 && let Ok(p) = product_svc.get(&service_ctx, &mut conn, item.product_id).await {
 product_codes.insert(item.product_id, p.product_code.clone());
 product_names.insert(item.product_id, p.pdt_name.clone());
 let spec = p.meta.specification.trim().to_string();
 if !spec.is_empty() {
 product_specs.insert(item.product_id, spec);
 }
 product_units.insert(item.product_id, p.unit.clone());
 }
 }

 let detail_path = TransferDetailPath { id: path.id }.to_string();
 let ctx = TransferDetailContext { items: &items, detail_path: &detail_path, from_wh_name: &from_wh_name, to_wh_name: &to_wh_name, operator_name: &operator_name, product_codes: &product_codes, product_names: &product_names, product_specs: &product_specs, product_units: &product_units };
 let content = transfer_detail_page(&transfer, &ctx);
 let page_html = admin_page(
 is_htmx,
 "调拨单详情",
 &claims,
 "inventory",
 "/admin/wms/transfers",
 "库存管理",
 None,
 content, &nav_filter, );
 Ok(Html(page_html.into_string()))
}

#[require_permission("INVENTORY", "update")]
pub async fn post_transfer_action(
 path: TransferDetailPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<TransferActionForm>,
) -> crate::errors::Result<axum::response::Response> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.transfer_service();

 match form.action.as_str() {
 "dispatch" => svc.dispatch(&service_ctx, &mut conn, path.id).await?,
 "complete" => svc.complete(&service_ctx, &mut conn, path.id).await?,
 "cancel" => svc.cancel(&service_ctx, &mut conn, path.id).await?,
 _ => {}
 }

 let redirect_url = TransferDetailPath { id: path.id }.to_string();
 let mut resp = axum::response::Response::default();
 resp.headers_mut().insert(
 axum::http::HeaderName::from_static("hx-redirect"),
 redirect_url.parse().unwrap(),
 );

 Ok(resp)
}


struct TransferDetailContext<'a> {
 items: &'a [TransferItem],
 detail_path: &'a str,
 from_wh_name: &'a str,
 to_wh_name: &'a str,
 operator_name: &'a str,
 product_codes: &'a std::collections::HashMap<i64, String>,
 product_names: &'a std::collections::HashMap<i64, String>,
 product_specs: &'a std::collections::HashMap<i64, String>,
 product_units: &'a std::collections::HashMap<i64, String>,
}

fn transfer_detail_page(
 transfer: &abt_core::wms::transfer::InventoryTransfer,
 ctx: &TransferDetailContext,
) -> Markup {
 let (status_label, status_class) = match transfer.status {
 TransferStatus::Draft => ("草稿", "status-draft"),
 TransferStatus::InTransit => ("在途", "status-progress"),
 TransferStatus::Completed => ("已完成", "status-completed"),
 TransferStatus::Cancelled => ("已取消", "status-cancelled"),
 };

 html! {
    div {
        a   href="/admin/wms/transfers"
            class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
        { (icon::chevron_left_icon("w-4 h-4")) "返回库存调拨列表" }

        div class="block bg-bg border border-border-soft rounded-lg p-6" {
            div {
                div class="flex items-center justify-between" {
                    h1 class="text-2xl font-extrabold font-mono tabular-nums" {
                        (transfer.doc_number)
                    }
                    span class=({
                        format!(
                            "status-pill {}",
                            crate::utils::status_color(status_class),
                        )
                    }) { (status_label) }
                }
            }
            div class="flex gap-3" { (transfer_action_buttons(transfer.status, ctx.detail_path)) }
        }
        // ── Workflow Steps ──
        (transfer_workflow_steps(transfer.status))
        // ── Info Card ──
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
            div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
                "调拨信息"
            }
            div class="grid gap-4" {
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "调拨单号" }
                    span class="text-sm text-fg font-medium font-mono tabular-nums" {
                        (transfer.doc_number)
                    }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "调出仓库" }
                    span class="text-sm text-fg font-medium" { (ctx.from_wh_name) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "调入仓库" }
                    span class="text-sm text-fg font-medium" { (ctx.to_wh_name) }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "调拨日期" }
                    span class="text-sm text-fg font-medium font-mono tabular-nums" {
                        (transfer.transfer_date.to_string())
                    }
                }
                div class="flex flex-col gap-1" {
                    span class="text-xs text-muted font-medium" { "操作员" }
                    span class="text-sm text-fg font-medium" { (ctx.operator_name) }
                }
            }
        }
        // ── Items Table ──
        div class="data-card" {
            div class="px-6 pt-5 pb-3" {
                div class="text-base font-semibold text-fg mb-4 pb-3 border-b border-border-soft mb-0 [border-bottom:none] pb-0"
                { "调拨明细" }
            }
            table class="data-table" {
                thead {
                    tr {
                        th { "行号" }
                        th { "产品编码" }
                        th { "产品名称" }
                        th { "规格" }
                        th { "单位" }
                        th class="text-right text-[13px]" { "调拨数量" }
                        th { "批次号" }
                    }
                }
                tbody {
                    @for (i, item) in ctx.items.iter().enumerate() {
                        tr {
                            td class="font-mono tabular-nums" { (i + 1) }
                            td class="font-mono tabular-nums" {
                                ({
                                    ctx.product_codes
                                        .get(&item.product_id)
                                        .map(|c| c.as_str())
                                        .unwrap_or("—")
                                })
                            }
                            td {
                                ({
                                    ctx.product_names
                                        .get(&item.product_id)
                                        .map(|n| n.as_str())
                                        .unwrap_or("—")
                                })
                            }
                            td {
                                ({
                                    ctx.product_specs
                                        .get(&item.product_id)
                                        .map(|s| s.as_str())
                                        .unwrap_or("—")
                                })
                            }
                            td {
                                ({
                                    ctx.product_units
                                        .get(&item.product_id)
                                        .map(|u| u.as_str())
                                        .unwrap_or("—")
                                })
                            }
                            td class="text-right text-[13px]" { (format!("{:.2}", item.quantity)) }
                            td class="font-mono tabular-nums" {
                                @if let Some(ref batch) = item.batch_no { (batch) } @else { "—" }
                            }
                        }
                    }
                    @if ctx.items.is_empty() {
                        tr {
                            td colspan="7" class="text-center text-muted py-8" { "暂无明细数据" }
                        }
                    }
                }
            }
        }
    }
}
}

fn transfer_action_buttons(status: TransferStatus, detail_path: &str) -> Markup {
 match status {
 TransferStatus::Draft => {
 html! {
    button
        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
        hx-post=(detail_path)
        hx-vals=r#"{"action":"cancel"}"#
        hx-confirm="确定要取消此调拨单吗？"
        hx-redirect=(detail_path)
    { (icon::x_icon("w-4 h-4")) "取消" }
    button
        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
        hx-post=(detail_path)
        hx-vals=r#"{"action":"dispatch"}"#
        hx-confirm="确定要发货吗？"
        hx-redirect=(detail_path)
    { (icon::arrow_right_icon("w-4 h-4")) "发货" }
}
 }
 TransferStatus::InTransit => {
 html! {
    button
        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
        hx-post=(detail_path)
        hx-vals=r#"{"action":"complete"}"#
        hx-confirm="确定要完成调拨吗？"
        hx-redirect=(detail_path)
    { (icon::check_circle_icon("w-4 h-4")) "确认完成" }
}
 }
 _ => html! {},
 }
}

fn transfer_workflow_steps(status: TransferStatus) -> Markup {
 let steps = [
 ("草稿", TransferStatus::Draft),
 ("在途", TransferStatus::InTransit),
 ("已完成", TransferStatus::Completed),
 ];

 let current_idx = match status {
 TransferStatus::Draft => 0,
 TransferStatus::InTransit => 1,
 TransferStatus::Completed => 2,
 TransferStatus::Cancelled => 0,
 };
 let is_cancelled = matches!(status, TransferStatus::Cancelled);

 html! {
    div class="flex items-center" {
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
