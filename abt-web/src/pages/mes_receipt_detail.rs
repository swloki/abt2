use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::mes::production_receipt::ProductionReceiptService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_receipt::{ReceiptDetailPath, ReceiptListPath, ReceiptConfirmPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

fn receipt_status_label(s: &abt_core::mes::enums::ReceiptStatus) -> (&'static str, &'static str, &'static str) {
    match s {
        abt_core::mes::enums::ReceiptStatus::Draft => ("草稿", "rgba(0,0,0,0.04)", "var(--muted)"),
        abt_core::mes::enums::ReceiptStatus::Confirmed => ("已确认", "rgba(82,196,26,0.08)", "var(--success)"),
        abt_core::mes::enums::ReceiptStatus::Cancelled => ("已取消", "rgba(245,63,63,0.06)", "#f53f3f"),
    }
}

#[require_permission("MES", "read")]
pub async fn get_receipt_detail(path: ReceiptDetailPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.production_receipt_service();
    let receipt = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
    let lookups = svc.get_detail_lookups(&mut conn, &receipt).await?;
    let (sl, sb, sc) = receipt_status_label(&receipt.status);

    let wo = lookups.wo_doc_number.as_deref().unwrap_or("—");
    let batch = lookups.batch_no.as_deref().unwrap_or("—");
    let product = lookups.product_name.as_deref().unwrap_or("—");
    let warehouse = lookups.warehouse_name.as_deref().unwrap_or("—");

    let content = html! { div {
        div class="page-header" {
            div class="page-header-left" { a class="back-link" href=(ReceiptListPath::PATH) { "\u{2190} 返回列表" } h1 class="page-title" { "入库单 " (receipt.doc_number) } }
            div class="page-actions" {
                @if receipt.status == abt_core::mes::enums::ReceiptStatus::Draft {
                    form hx-post=(format!("/admin/mes/receipts/{}/confirm", receipt.id)) hx-swap="none" style="display:inline" {
                        button class="btn btn-primary" type="submit" { "确认入库" }
                    }
                }
            }
        }
        div class="info-card" {
            div class="info-grid" {
                div class="info-item" { label { "单号" } span class="mono" { (receipt.doc_number) } }
                div class="info-item" { label { "工单" } span { (wo) } }
                div class="info-item" { label { "批次" } span { (batch) } }
                div class="info-item" { label { "产品" } span { (product) } }
                div class="info-item" { label { "入库数量" } span class="mono" { (crate::utils::fmt_qty(receipt.received_qty)) } }
                div class="info-item" { label { "仓库" } span { (warehouse) } }
                div class="info-item" { label { "入库日期" } span { (receipt.receipt_date) } }
                div class="info-item" { label { "状态" } span style=(format!("display:inline-flex;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", sb, sc)) { (sl) } }
                div class="info-item" { label { "倒冲触发" } span { (if receipt.backflush_triggered { "是" } else { "否" }) } }
                div class="info-item" { label { "创建时间" } span { (receipt.created_at.format("%Y-%m-%d %H:%M")) } }
                @if !receipt.remark.is_empty() {
                    div class="info-item span-2" { label { "备注" } span { (receipt.remark) } }
                }
            }
        }
    }};
    Ok(Html(admin_page(is_htmx, "入库详情", &claims, "production", &format!("/admin/mes/receipts/{}", path.id), "生产管理", Some(ReceiptListPath::PATH), content).into_string()))
}

#[require_permission("MES", "write")]
pub async fn confirm_receipt(path: ReceiptConfirmPath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    state.production_receipt_service().confirm(&service_ctx, &mut conn, path.receipt_id).await?;
    Ok(axum::response::Response::builder().header("HX-Redirect", &format!("/admin/mes/receipts/{}", path.receipt_id)).body(axum::body::Body::empty()).unwrap())
}
