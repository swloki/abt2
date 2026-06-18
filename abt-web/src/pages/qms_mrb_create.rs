use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::product::model::{Product, ProductQuery};
use abt_core::master_data::product::ProductService;
use abt_core::qms::enums::{InspectionResultType, InspectionStatus};
use abt_core::qms::enums::{MRBDisposition, ResponsibleParty};
use abt_core::qms::inspection_result::model::InspectionResultFilter;
use abt_core::qms::inspection_result::InspectionResultService;
use abt_core::qms::inspection_result::model::InspectionResult;
use abt_core::qms::mrb::model::CreateMrbReq;
use abt_core::qms::mrb::MrbService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::qms::{MrbCreatePath, MrbListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form request ──

#[derive(Debug, Deserialize)]
pub struct MrbCreateForm {
 pub inspection_result_id: i64,
 pub product_id: i64,
 pub defect_description: String,
 pub disposition: i16,
 pub responsible_party: i16,
 pub cost_impact: String,
 pub remark: String,
}

// ── Handlers ──

#[require_permission("QMS", "create")]
pub async fn get_create(
 _path: MrbCreatePath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 claims,
 ..
 } = ctx;

 // Load products
 let product_svc = state.product_service();
 let products = product_svc
 .list(&service_ctx, &mut conn, ProductQuery::default(), PageParams::new(1, 500))
 .await?;

 // Load failed inspection results (Completed + Fail)
 let insp_svc = state.inspection_result_service();
 let filter = InspectionResultFilter {
 result: Some(InspectionResultType::Fail),
 status: Some(InspectionStatus::Completed),
 ..Default::default()
 };
 let failed_results = insp_svc
 .list_by_source(&service_ctx, &mut conn, filter, PageParams::new(1, 200))
 .await?;

 let content = mrb_create_page(&products.items, &failed_results.items);
 let page_html = admin_page(
 is_htmx,
 "新建MRB评审",
 &claims,
 "quality",
 MrbCreatePath::PATH,
 "质量管理",
 Some(MrbListPath::PATH),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

#[require_permission("QMS", "create")]
pub async fn create(
 _path: MrbCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<MrbCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;

 let disposition = MRBDisposition::from_i16(form.disposition).ok_or_else(|| {
 abt_core::shared::types::DomainError::Validation("无效处置方式".into())
 })?;

 let responsible_party = ResponsibleParty::from_i16(form.responsible_party).ok_or_else(|| {
 abt_core::shared::types::DomainError::Validation("无效责任方".into())
 })?;

 let cost_impact: rust_decimal::Decimal = form.cost_impact.parse().unwrap_or_default();

 let req = CreateMrbReq {
 inspection_result_id: form.inspection_result_id,
 product_id: form.product_id,
 defect_description: form.defect_description,
 disposition,
 responsible_party,
 cost_impact,
 remark: form.remark,
 };

 let svc = state.mrb_service();
 let _id = svc.create(&service_ctx, &mut conn, req).await?;

 Ok(
 axum::response::Response::builder()
 .header("HX-Redirect", MrbListPath::PATH)
 .body(axum::body::Body::empty())
 .unwrap(),
 )
}

// ── Page rendering ──

fn mrb_create_page(products: &[Product], failed_results: &[InspectionResult]) -> Markup {
 html! {
 div {
 // ── Page header ──
 div class="flex items-center justify-between mb-6" {
 div class="flex items-center justify-between mb-6-left" {
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", MrbListPath::PATH)) {
 (icon::arrow_left_icon("w-4 h-4"))
 "返回列表"
 }
 h1 class="text-xl font-bold text-fg tracking-tight" { "新建MRB评审" }
 }
 }

 form id="mrb-form" hx-post=(MrbCreatePath::PATH) hx-swap="none" {

 // ── Section 1: 关联信息 ──
 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
 (icon::link_icon("w-4 h-4"))
 "关联信息"
 }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" class="col-span-2" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap required" { "关联检验结果" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" name="inspection_result_id" required {
 option value="" disabled selected { "请选择检验结果" }
 @for r in failed_results {
 option value=(r.id) { (r.doc_number) " — " (r.batch_no) }
 }
 }
 }
 div class="form-field" class="col-span-2" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap required" { "产品" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" name="product_id" required {
 option value="" disabled selected { "请选择产品" }
 @for p in products {
 option value=(p.product_id) { (p.product_code) " — " (p.pdt_name) }
 }
 }
 }
 }
 }

 // ── Section 2: 缺陷信息 ──
 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
 (icon::alert_triangle_icon("w-4 h-4"))
 "缺陷信息"
 }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field col-span-full" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap required" { "缺陷描述" }
 textarea class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent min-h-[72px] resize-y leading-1.5" name="defect_description" rows="3" required placeholder="请描述缺陷详情…" {}
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap required" { "处置方式" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" name="disposition" required {
 option value="" disabled selected { "请选择处置方式" }
 option value="1" { "报废" }
 option value="2" { "退货" }
 option value="3" { "降级" }
 option value="4" { "返工" }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap required" { "责任方" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" name="responsible_party" required {
 option value="" disabled selected { "请选择责任方" }
 option value="1" { "内部" }
 option value="2" { "供应商" }
 option value="3" { "客户" }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "成本影响" }
 div class="relative" {
 span class="absolute left-3 top-1/2 -translate-y-1/2 text-sm text-muted pointer-events-none" { "¥" }
 input class="w-full pl-7 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" type="number" name="cost_impact" step="any"
 placeholder="0.00";
 }
 }
 }
 }

 // ── Section 3: 备注 ──
 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
 (icon::file_text_icon("w-4 h-4"))
 "备注"
 }
 div class="form-field" {
 textarea class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent min-h-[72px] resize-y leading-1.5" name="remark" rows="3" placeholder="填写备注信息…" {}
 }
 }

 // ── Action bar ──
 div class="flex items-center justify-end gap-3 pt-4 border-t border-border-soft" {
 a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{}?restore=true", MrbListPath::PATH)) { "取消" }
 button type="button" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
 onclick="document.getElementById('mrb-form').querySelector('[name=remark]').value+='[草稿]';htmx.trigger('#mrb-form','submit')" {
 "保存草稿"
 }
 button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" { "提交审批" }
 }
 }
 }
 }
}
