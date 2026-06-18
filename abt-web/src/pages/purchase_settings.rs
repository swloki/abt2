use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup, PreEscaped};
use serde::Deserialize;

use abt_core::purchase::settings::{PurchaseSettingsService, model::UpdatePurchaseSettingsRequest};
use abt_core::purchase::TaxRateService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_order::POListPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/settings")]
pub struct PurchaseSettingsPath;

#[derive(Debug, Deserialize)]
pub struct SettingsForm {
 pub over_delivery_allowance_pct: Option<String>,
 pub over_shortage_allowance_pct: Option<String>,
 pub maintain_same_rate: Option<String>,
 pub po_required_for_receipt: Option<String>,
 pub receipt_required_for_invoice: Option<String>,
 pub default_currency_code: Option<String>,
 pub default_tax_rate_id: Option<String>,
}

// ══════════════════════════════════════════════════════════════════
// Handlers
// ══════════════════════════════════════════════════════════════════

#[require_permission("SUPPLIER", "read")]
pub async fn get_purchase_settings(
 _path: PurchaseSettingsPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;

 let svc = state.purchase_settings_service();
 let settings = svc.get(&service_ctx, &mut conn).await?;
 let tax_rates = state
 .tax_rate_service()
 .list_active(&service_ctx, &mut conn)
 .await
 .unwrap_or_default();

 let content = settings_page(&settings, &tax_rates);
 let page_html = admin_page(
 is_htmx,
 "采购参数配置",
 &claims,
 "purchase",
 PurchaseSettingsPath::PATH,
 "采购管理",
 Some("参数配置"),
 content,
 &nav_filter,
 );
 Ok(Html(page_html.into_string()))
}

#[require_permission("SUPPLIER", "update")]
pub async fn update_purchase_settings(
 _path: PurchaseSettingsPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<SettingsForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.purchase_settings_service();

 let req = UpdatePurchaseSettingsRequest {
 over_delivery_allowance_pct: form.over_delivery_allowance_pct
 .and_then(|s| s.parse().ok()),
 over_shortage_allowance_pct: form.over_shortage_allowance_pct
 .and_then(|s| s.parse().ok()),
 maintain_same_rate: form.maintain_same_rate.map(|_| true),
 po_required_for_receipt: form.po_required_for_receipt.map(|_| true),
 receipt_required_for_invoice: form.receipt_required_for_invoice.map(|_| true),
 default_currency_code: form.default_currency_code,
 default_tax_rate_id: Some(
 form.default_tax_rate_id
 .and_then(|s| if s.is_empty() { None } else { s.parse().ok() })
 ),
 };

 svc.update(&service_ctx, &mut conn, req).await?;

 let redirect = PurchaseSettingsPath.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ══════════════════════════════════════════════════════════════════
// Rendering
// ══════════════════════════════════════════════════════════════════

fn settings_page(
 s: &abt_core::purchase::settings::model::PurchaseSettings,
 tax_rates: &[abt_core::purchase::tax::model::TaxRate],
) -> Markup {
 html! {
 div {
 div class="flex items-center justify-between mb-6" {
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "采购参数配置" }
 }
 }
 form hx-post=(PurchaseSettingsPath::PATH) hx-swap="none" {
 // ── Tolerance ──
 div class="data-card" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" { "收货容差" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "超收容差百分比 (%)" }
 input type="number" step="0.01" min="0" max="100"
 name="over_delivery_allowance_pct"
 value=(s.over_delivery_allowance_pct)
 class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]";
 span class="text-muted" {
 "收货数量超过订单数量的最大允许百分比，0 表示不允许超收"
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "超欠容差百分比 (%)" }
 input type="number" step="0.01" min="0" max="100"
 name="over_shortage_allowance_pct"
 value=(s.over_shortage_allowance_pct)
 class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]";
 span class="text-muted" {
 "收货数量少于订单数量的最大允许百分比"
 }
 }
 }
 }

 // ── Business Rules ──
 div class="data-card" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" { "业务规则" }
 div class="form-field" {
 label class="flex items-center gap-2 text-[13px] text-fg cursor-pointer mt-1.5" {
 input type="checkbox" name="maintain_same_rate" value="true"
 checked[s.maintain_same_rate] {};
 span { "启用价格一致性校验" }
 }
 span class="text-muted" {
 "确认订单时校验单价是否与关联报价单一致"
 }
 }
 div class="form-field" {
 label class="flex items-center gap-2 text-[13px] text-fg cursor-pointer mt-1.5" {
 input type="checkbox" name="po_required_for_receipt" value="true"
 checked[s.po_required_for_receipt] {};
 span { "收货必须关联采购订单" }
 }
 }
 div class="form-field" {
 label class="flex items-center gap-2 text-[13px] text-fg cursor-pointer mt-1.5" {
 input type="checkbox" name="receipt_required_for_invoice" value="true"
 checked[s.receipt_required_for_invoice] {};
 span { "开票前必须完成收货" }
 }
 }
 }

 // ── Defaults ──
 div class="data-card" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" { "默认值" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "默认币种" }
 select name="default_currency_code" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" {
 option value="CNY" selected[s.default_currency_code == "CNY"] { "CNY 人民币" }
 option value="USD" selected[s.default_currency_code == "USD"] { "USD 美元" }
 option value="EUR" selected[s.default_currency_code == "EUR"] { "EUR 欧元" }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "默认税率" }
 select name="default_tax_rate_id" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" {
 option value="" selected[s.default_tax_rate_id.is_none()] { "— 不设置 —" }
 @for tr in tax_rates {
 option value=(tr.id)
 selected[s.default_tax_rate_id == Some(tr.id)] {
 (tr.name) " (" (tr.rate) "%)"
 }
 }
 }
 }
 }
 }

 // ── Actions ──
 div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg [border-top:1px_solid_var(--border-soft)]" {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(POListPath::PATH) { "返回采购订单" }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" { "保存配置" }
 }
 }
 // 成功保存后 toast（HX-Redirect 触发）
 (PreEscaped(r#"<script>
 document.body.addEventListener('htmx:afterRequest', function(evt) {
 if (evt.detail.xhr && evt.detail.xhr.status === 200 && evt.detail.xhr.getResponseHeader('HX-Redirect')) {
 if (typeof Notyf !== 'undefined') {
 var n = new Notyf();
 n.success('配置已保存');
 }
 }
 });
 </script>"#))
 }
 }
}
