use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::mes::production_receipt::{model::FqcGate, ProductionReceiptService};

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_receipt::{ReceiptConfirmPath, ReceiptDetailPath, ReceiptListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

fn receipt_status_label(s: &abt_core::mes::enums::ReceiptStatus) -> (&'static str, &'static str) {
 match s {
 abt_core::mes::enums::ReceiptStatus::Draft => ("草稿", "status-draft"),
 abt_core::mes::enums::ReceiptStatus::Confirmed => ("已确认", "status-completed"),
 abt_core::mes::enums::ReceiptStatus::Cancelled => ("已取消", "status-cancelled"),
 }
}

fn fqc_badge(status: &FqcGate) -> Markup {
 let (label, class) = match status {
 FqcGate::NotRequired => ("无需 FQC", "fqc-badge--na"),
 FqcGate::PendingInspection => ("待 FQC", "fqc-badge--pending"),
 FqcGate::AllPassed => ("FQC 通过", "fqc-badge--passed"),
 FqcGate::HasFailed => ("FQC 不合格", "fqc-badge--failed"),
 };
 html! {
 span class=(format!("fqc-badge {}", class)) { (label) }
 }
}

#[require_permission("WORK_ORDER", "read")]
pub async fn get_receipt_detail(path: ReceiptDetailPath, ctx: RequestContext) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.production_receipt_service();
 let receipt = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
 let lookups = svc.get_detail_lookups(&mut conn, &receipt).await?;
 let (sl, sc) = receipt_status_label(&receipt.status);

 let wo = lookups.wo_doc_number.as_deref().unwrap_or("—");
 let batch = lookups.batch_no.as_deref().unwrap_or("—");
 let product = lookups.product_name.as_deref().unwrap_or("—");
 let warehouse = lookups.warehouse_name.as_deref().unwrap_or("—");

 // FQC 状态
 let fqc_status = svc.get_fqc_status(&service_ctx, &mut conn, path.id).await.unwrap_or(FqcGate::NotRequired);

 // 单位成本
 let unit_cost = svc.get_unit_cost(&mut conn, receipt.product_id).await.unwrap_or(rust_decimal::Decimal::ZERO);
 let total_cost = receipt.received_qty * unit_cost;

 let content = html! { div {
 div class="flex items-center justify-between mb-6" {
 div class="flex items-center justify-between mb-6" {
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", ReceiptListPath::PATH)) { "← 返回列表" }
 h1 class="text-xl font-bold text-fg tracking-tight" { "入库单 " (receipt.doc_number) }
 }
 div class="flex gap-3" {
 @if receipt.status == abt_core::mes::enums::ReceiptStatus::Draft {
 @if matches!(fqc_status, FqcGate::AllPassed | FqcGate::NotRequired) {
 form class="inline-form" hx-post=(ReceiptConfirmPath { receipt_id: receipt.id }.to_string()) hx-swap="none" {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" type="submit"
 hx-confirm="确认入库？将触发倒冲和成本结转。" {
 "确认入库"
 }
 }
 } @else if matches!(fqc_status, FqcGate::PendingInspection) {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" disabled title="需完成 FQC 质检后才能确认入库" {
 "确认入库（待 FQC）"
 }
 } @else {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" disabled title="FQC 有不合格项，无法入库" {
 "确认入库（FQC 不合格）"
 }
 }
 }
 }
 }

 // 状态条
 div class="flex items-center gap-[16px] bg-[#fafafa]" {
 span class="receipt-status-label" {
 "状态: "
 span class=(format!("status-pill {}", crate::utils::status_color(sc))) { (sl) }
 }
 (fqc_badge(&fqc_status))
 }

 // 基本信息
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
 div class="text-sm font-semibold text-fg mb-3 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" { "基本信息" }
 div class="grid gap-4" {
 div class="flex flex-col gap-1" { label { "单号" } span class="font-mono tabular-nums" { (receipt.doc_number) } }
 div class="flex flex-col gap-1" { label { "工单" } span { (wo) } }
 div class="flex flex-col gap-1" { label { "批次" } span { (batch) } }
 div class="flex flex-col gap-1" { label { "产品" } span { (product) } }
 div class="flex flex-col gap-1" { label { "入库数量" } span class="font-mono tabular-nums" { (crate::utils::fmt_qty(receipt.received_qty)) } }
 div class="flex flex-col gap-1" { label { "仓库" } span { (warehouse) } }
 div class="flex flex-col gap-1" { label { "入库日期" } span { (receipt.receipt_date) } }
 div class="flex flex-col gap-1" { label { "倒冲触发" } span { (if receipt.backflush_triggered { "是" } else { "否" }) } }
 div class="flex flex-col gap-1" { label { "创建时间" } span { (receipt.created_at.format("%Y-%m-%d %H:%M")) } }
 @if !receipt.remark.is_empty() {
 div class="flex flex-col gap-1 col-span-2" { label { "备注" } span { (receipt.remark) } }
 }
 }
 }

 // FQC 质检卡片
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
 div class="text-sm font-semibold text-fg mb-3 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" { "FQC 质检状态" }
 div { (fqc_badge(&fqc_status)) }
 @if matches!(fqc_status, FqcGate::PendingInspection) {
 p class="text-muted" { "⚠ 尚无 FQC 检验记录，需完成 FQC 后才能确认入库" }
 }
 }

 // 成本明细
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
 div class="text-sm font-semibold text-fg mb-3 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" { "成本明细" }
 div class="grid gap-4" {
 div class="flex flex-col gap-1" {
 label { "入库数量" }
 span class="font-mono tabular-nums" { (crate::utils::fmt_qty(receipt.received_qty)) }
 }
 div class="flex flex-col gap-1" {
 label { "单位成本" }
 @if unit_cost > rust_decimal::Decimal::ZERO {
 span class="font-mono tabular-nums" { (crate::utils::fmt_amount(unit_cost)) }
 } @else {
 span class="text-muted" { "—（无历史成本）" }
 }
 }
 div class="flex flex-col gap-1" {
 label { "总成本" }
 span class="font-mono tabular-nums" { strong { (crate::utils::fmt_amount(total_cost)) } }
 }
 }
 }
 }};
 Ok(Html(admin_page(
 is_htmx, "入库详情", &claims, "production",
 &format!("/admin/mes/receipts/{}", path.id), "生产管理",
 Some(ReceiptListPath::PATH), content, &nav_filter,
 ).into_string()))
}

#[require_permission("WORK_ORDER", "update")]
pub async fn confirm_receipt(path: ReceiptConfirmPath, ctx: RequestContext) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;
 let mut tx = state.pool.begin().await
 .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 state.production_receipt_service().confirm(&service_ctx, &mut tx, path.receipt_id).await?;
 tx.commit().await
 .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 Ok(axum::response::Response::builder()
 .header("HX-Redirect", &format!("/admin/mes/receipts/{}", path.receipt_id))
 .body(axum::body::Body::empty()).unwrap())
}
