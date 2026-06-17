use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::html;

use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::qms::enums::{RMAStatus, Severity};
use abt_core::qms::rma::RmaService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::qms::{RmaDetailPath, RmaListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn severity_label(s: &Severity) -> (&'static str, &'static str) {
 match s {
 Severity::Minor => ("Minor", "status-active"),
 Severity::Major => ("Major", "status-warning"),
 Severity::Critical => ("Critical", "status-danger"),
 }
}

fn status_label(s: &RMAStatus) -> (&'static str, &'static str) {
 match s {
 RMAStatus::Reported => ("已报告", "status-warning"),
 RMAStatus::Investigating => ("调查中", "status-info"),
 RMAStatus::ActionTaken => ("已采取措施", "status-active"),
 RMAStatus::Closed => ("已关闭", "status-default"),
 }
}

// ── Handler ──

#[require_permission("QMS", "read")]
pub async fn get_detail(path: RmaDetailPath, ctx: RequestContext) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

 let svc = state.rma_service();
 let rma = svc.get(&service_ctx, &mut conn, path.id).await?;

 let customer_name = state
 .customer_service()
 .get(&service_ctx, &mut conn, rma.customer_id)
 .await
 .map(|c| c.name)
 .unwrap_or_else(|_| "—".into());

 let product_name = state
 .product_service()
 .get(&service_ctx, &mut conn, rma.product_id)
 .await
 .map(|p| p.pdt_name)
 .unwrap_or_else(|_| "—".into());

 let (severity_text, severity_class) = severity_label(&rma.severity);
 let (status_text, status_class) = status_label(&rma.status);

 let content = html! { div {
 div class="flex items-center justify-between mb-6" {
 div class="flex items-center justify-between mb-6-left" {
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", RmaListPath::PATH)) { "\u{2190} 返回列表" }
 h1 class="text-xl font-bold text-fg tracking-tight" {
 "RMA单号 " (&rma.doc_number)
 " "
 span class=(format!("status-pill {}", crate::utils::status_color(status_class))) { (status_text) }
 }
 }
 }

 // ── 基本信息 ──
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
 h3 { "基本信息" }
 div class="grid gap-4" {
 div class="flex flex-col gap-1" { label { "客户" } span { (customer_name) } }
 div class="flex flex-col gap-1" { label { "产品" } span { (product_name) } }
 div class="flex flex-col gap-1" {
 label { "严重程度" }
 span class=(format!("status-pill {}", crate::utils::status_color(severity_class))) { (severity_text) }
 }
 div class="flex flex-col gap-1" {
 label { "关联销售单" }
 span {
 (rma.sales_order_id.map(|id| id.to_string()).unwrap_or_else(|| "—".into()))
 }
 }
 div class="flex flex-col gap-1" {
 label { "关联发货单" }
 span {
 (rma.shipping_request_id.map(|id| id.to_string()).unwrap_or_else(|| "—".into()))
 }
 }
 div class="flex flex-col gap-1" {
 label { "关联检验结果" }
 span {
 (rma.linked_inspection_result_id.map(|id| id.to_string()).unwrap_or_else(|| "—".into()))
 }
 }
 }
 }

 // ── 缺陷描述 ──
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
 h3 { "缺陷描述" }
 p style="white-space: pre-wrap;" { (&rma.defect_description) }
 }

 // ── 根因分析 ──
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
 h3 { "根因分析" }
 div class="grid gap-4" {
 div class="flex flex-col gap-1" {
 label { "根本原因" }
 span {
 (rma.root_cause.as_deref().unwrap_or("待填写"))
 }
 }
 div class="flex flex-col gap-1" {
 label { "纠正措施" }
 span {
 (rma.corrective_action.as_deref().unwrap_or("待填写"))
 }
 }
 }
 }

 // ── 其他信息 ──
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
 h3 { "其他信息" }
 div class="grid gap-4" {
 div class="flex flex-col gap-1" { label { "备注" } span { (or_dash(&rma.remark)) } }
 div class="flex flex-col gap-1" { label { "创建时间" } span { (rma.created_at.format("%Y-%m-%d %H:%M")) } }
 }
 }
 }};

 let current_path = RmaDetailPath { id: path.id }.to_string();
 let html = admin_page(
 is_htmx,
 "RMA详情",
 &claims,
 "quality",
 &current_path,
 "质量管理",
 Some(RmaListPath::PATH),
 content, &nav_filter, );
 Ok(Html(html.into_string()))
}

fn or_dash(s: &str) -> &str {
 if s.is_empty() { "—" } else { s }
}
