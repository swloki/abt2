use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::html;
use rust_decimal::Decimal;

use abt_core::qms::enums::{MRBDisposition, MRBStatus, ResponsibleParty};
use abt_core::qms::inspection_result::InspectionResultService;
use abt_core::qms::mrb::MrbService;
use abt_core::master_data::product::ProductService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::qms::{MrbDetailPath, MrbListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn disposition_label(d: &MRBDisposition) -> (&'static str, &'static str) {
 match d {
 MRBDisposition::Scrap => ("报废", "status-danger"),
 MRBDisposition::Return => ("退货", "status-warning"),
 MRBDisposition::Degrade => ("降级", "status-purple"),
 MRBDisposition::Rework => ("返工", "status-info"),
 }
}

fn responsible_party_label(r: &ResponsibleParty) -> (&'static str, &'static str) {
 match r {
 ResponsibleParty::Internal => ("内部", "status-active"),
 ResponsibleParty::Supplier => ("供应商", "status-info"),
 ResponsibleParty::Customer => ("客户", "status-purple"),
 }
}

fn status_label(s: &MRBStatus) -> (&'static str, &'static str) {
 match s {
 MRBStatus::Draft => ("草稿", "status-draft"),
 MRBStatus::UnderReview => ("审批中", "status-warning"),
 MRBStatus::Approved => ("已批准", "status-active"),
 MRBStatus::Completed => ("已完成", "status-info"),
 }
}

fn fmt_cost(v: Decimal) -> String {
 if v.is_zero() {
 "—".into()
 } else {
 format!("¥{}", v)
 }
}

// ── Handler ──

#[require_permission("QMS", "read")]
pub async fn get_detail(path: MrbDetailPath, ctx: RequestContext) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

 let svc = state.mrb_service();
 let mrb = svc.get(&service_ctx, &mut conn, path.id).await?;

 // Resolve product name
 let product_name = state
 .product_service()
 .get(&service_ctx, &mut conn, mrb.product_id)
 .await
 .map(|p| p.pdt_name)
 .unwrap_or_else(|_| "—".into());

 // Resolve linked inspection result doc number
 let inspection_doc = state
 .inspection_result_service()
 .get(&service_ctx, &mut conn, mrb.inspection_result_id)
 .await
 .map(|r| r.doc_number)
 .unwrap_or_else(|_| "—".into());

 let (status_text, status_class) = status_label(&mrb.status);
 let (disp_text, disp_class) = disposition_label(&mrb.disposition);
 let (party_text, party_class) = responsible_party_label(&mrb.responsible_party);

 let content = html! { div {
 div class="flex items-center justify-between mb-6" {
 div class="flex items-center justify-between mb-6-left" {
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", MrbListPath::PATH)) { "\u{2190} 返回列表" }
 h1 class="text-xl font-bold text-fg tracking-tight" {
 "MRB单号 " (&mrb.doc_number)
 " "
 span class=(format!("status-pill {}", crate::utils::status_color(status_class))) { (status_text) }
 }
 }
 }

 // ── 基本信息 ──
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
 h3 { "基本信息" }
 div class="grid gap-4" {
 div class="flex flex-col gap-1" {
 label { "关联检验结果单号" }
 span class="font-mono tabular-nums" { (inspection_doc) }
 }
 div class="flex flex-col gap-1" { label { "产品" } span { (product_name) } }
 div class="flex flex-col gap-1" {
 label { "处置方式" }
 span class=(format!("status-pill {}", crate::utils::status_color(disp_class))) { (disp_text) }
 }
 div class="flex flex-col gap-1" {
 label { "责任方" }
 span class=(format!("status-pill {}", crate::utils::status_color(party_class))) { (party_text) }
 }
 div class="flex flex-col gap-1" { label { "成本影响" } span class="font-mono tabular-nums text-right text-[13px]" { (fmt_cost(mrb.cost_impact)) } }
 }
 }

 // ── 缺陷描述 ──
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
 h3 { "缺陷描述" }
 p style="white-space: pre-wrap;" { (&mrb.defect_description) }
 }

 // ── 备注 ──
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
 h3 { "备注" }
 p style="white-space: pre-wrap;" { (if mrb.remark.is_empty() { "—" } else { &mrb.remark }) }
 }

 // ── 其他信息 ──
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
 h3 { "其他信息" }
 div class="grid gap-4" {
 div class="flex flex-col gap-1" { label { "创建时间" } span { (mrb.created_at.format("%Y-%m-%d %H:%M")) } }
 }
 }
 }};

 let current_path = MrbDetailPath { id: path.id }.to_string();
 let html = admin_page(
 is_htmx,
 "MRB评审详情",
 &claims,
 "quality",
 &current_path,
 "质量管理",
 Some(MrbListPath::PATH),
 content, &nav_filter, );
 Ok(Html(html.into_string()))
}
