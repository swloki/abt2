use std::collections::HashMap;

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::model::CustomerQuery;
use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::sales::reconciliation::model::*;
use abt_core::sales::reconciliation::ReconciliationService;
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::sales::shipping_request::ShippingRequestService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::reconciliation::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Query & Form Structs ──

#[derive(Debug, Deserialize)]
pub struct PreviewQuery {
 pub customer_id: Option<i64>,
 pub period: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ReconciliationCreateForm {
 pub customer_id: i64,
 pub period: String,
 pub remark: Option<String>,
}

// ── Helpers ──

struct ProductInfo {
 code: String,
 name: String,
 _unit: String,
}

fn _generate_periods() -> Vec<(String, String)> {
 let now = chrono::Local::now();
 let mut periods = vec![];
 for i in 0..12 {
 let d = now - chrono::Months::new(i);
 let value = d.format("%Y-%m").to_string();
 let label = d.format("%Y年%m月").to_string();
 periods.push((value, label));
 }
 periods
}

// ── Handlers ──

#[require_permission("SALES_ORDER", "create")]
pub async fn get_reconciliation_create(
 _path: ReconciliationCreatePath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
 let username = claims.display_name.as_str();

 let customer_svc = state.customer_service();
 let customers = customer_svc
 .list(&service_ctx, &mut conn, CustomerQuery { name: None, status: None, category: None, owner_id: None }, PageParams::new(1, 200))
 .await?;

 let content = reconciliation_create_page(&customers.items, username);
 let page_html = admin_page(
 is_htmx, "新建对账单", &claims, "sales",
 ReconciliationCreatePath::PATH, "销售管理", Some("新建对账单"), content, &nav_filter,
 );

 Ok(Html(page_html.into_string()))
}

#[require_permission("SALES_ORDER", "create")]
pub async fn post_reconciliation_create(
 _path: ReconciliationCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<ReconciliationCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;

 let reconciliation_svc = state.reconciliation_service();
 let id = reconciliation_svc
 .create(&service_ctx, &mut conn, form.customer_id, form.period)
 .await?;

 let detail_path = ReconciliationDetailPath { id };
 Ok((
 axum::http::StatusCode::OK,
 [("HX-Redirect", detail_path.to_string())],
 "",
 ))
}

#[require_permission("SALES_ORDER", "read")]
pub async fn get_reconciliation_preview(
 ctx: RequestContext,
 Query(params): Query<PreviewQuery>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;

 let customer_id = match params.customer_id {
 Some(id) if id > 0 => id,
 _ => return Ok(Html(preview_empty("请选择客户").into_string())),
 };
 let period = match &params.period {
 Some(p) if !p.is_empty() => p.clone(),
 _ => return Ok(Html(preview_empty("请选择对账期间").into_string())),
 };

 let reconciliation_svc = state.reconciliation_service();
 let items = reconciliation_svc
 .preview(&service_ctx, &mut conn, customer_id, period)
 .await?;

 if items.is_empty() {
 return Ok(Html(preview_empty("该客户在所选期间内没有已发货数据").into_string()));
 }

 // Resolve product details
 let product_svc = state.product_service();
 let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
 let product_map: HashMap<i64, ProductInfo> = product_svc
 .get_by_ids(&service_ctx, &mut conn, product_ids)
 .await
 .map(|products| products.into_iter().map(|p| {
 (p.product_id, ProductInfo { code: p.product_code, name: p.pdt_name, _unit: p.unit })
 }).collect())
 .unwrap_or_default();

 // Resolve order numbers
 let order_svc = state.sales_order_service();
 let order_ids: Vec<i64> = items.iter().map(|i| i.sales_order_id).collect::<std::collections::HashSet<_>>().into_iter().collect();
 let order_numbers: HashMap<i64, String> = {
 let mut map = HashMap::new();
 for &oid in &order_ids {
 if let Ok(order) = order_svc.find_by_id(&service_ctx, &mut conn, oid).await {
 map.insert(oid, order.doc_number);
 }
 }
 map
 };

 // Resolve shipping numbers
 let shipping_svc = state.shipping_service();
 let shipping_ids: Vec<i64> = items.iter().map(|i| i.shipping_request_id).collect::<std::collections::HashSet<_>>().into_iter().collect();
 let shipping_numbers: HashMap<i64, String> = {
 let mut map = HashMap::new();
 for &sid in &shipping_ids {
 if let Ok(shipping) = shipping_svc.find_by_id(&service_ctx, &mut conn, sid).await {
 map.insert(sid, shipping.doc_number);
 }
 }
 map
 };

 let content = preview_table(&items, &product_map, &order_numbers, &shipping_numbers);
 Ok(Html(content.into_string()))
}

// ── Components ──

fn reconciliation_create_page(
 customers: &[abt_core::master_data::customer::model::Customer],
 username: &str,
) -> Markup {
 html! {
 div id="rec-app" class="p-6" {
 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", ReconciliationListPath::PATH)) {
 (icon::arrow_left_icon("w-4 h-4"))
 "返回对账单列表"
 }
 h1 class="text-xl font-bold text-fg tracking-tight" { "新建对账单" }
 }

 form id="rec-create-form"
 hx-post=(ReconciliationCreatePath::PATH)
 hx-swap="none" {

 // ── 对账基本信息 ──
 div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
 (icon::clipboard_document_icon("w-[18px] h-[18px]"))
 "对账基本信息"
 }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "客户 " span class="required" { "*" } }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="customer_id" id="rec-customer-select"
 onchange="triggerPreview()" {
 option value="" { "请选择客户" }
 @for c in customers {
 option value=(c.id) { (c.name) }
 }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "对账期间 " span class="required" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="month" name="period" id="rec-period-select"
 onchange="triggerPreview()" placeholder="选择月份";
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "对账日期" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="date" id="rec-date";
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "销售员" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" readonly value=(username);
 }
 div class="form-field field-full" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "联系人 / 电话" }
 div class="grid grid-cols-2 gap-4" {
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" id="rec-contact-name" readonly placeholder="选择客户后自动填充";
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" id="rec-contact-phone" readonly placeholder="—";
 }
 }
 div class="form-field col-span-2" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "备注" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" placeholder="对账备注信息…";
 }
 }
 }

 // ── 对账明细 ──
 div class="bg-bg border border-border rounded overflow-hidden" id="rec-preview-area"
 hx-get=(ReconciliationPreviewPath::PATH)
 hx-trigger="previewChanged from:#rec-app"
 hx-include="#rec-customer-select,#rec-period-select"
 hx-target="this"
 hx-swap="outerHTML" {
 div class="flex items-center justify-between p-5 border-b border-border-soft" {
 h3 {
 (icon::package_icon("w-[18px] h-[18px]"))
 "对账明细"
 }
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" id="pickOrderBtn" disabled {
 (icon::plus_icon("w-3.5 h-3.5"))
 "从发货单添加"
 }
 }

 // Empty state
 div class="text-center p-6 text-muted text-sm" id="emptyState" {
 (icon::clipboard_list_icon("w-12 h-12"))
 p class="text-center text-muted text-sm font-medium" { "暂无对账明细" }
 p class="text-center text-muted text-sm mt-1" { "请先选择客户，然后从发货单中添加对账明细" }
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)] mt-5" _="on click trigger click on #pickOrderBtn" { "选择发货单" }
 }
 }

 // ── Remark ──
 div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
 (icon::file_text_icon("w-[18px] h-[18px]"))
 "备注"
 }
 textarea class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] min-h-[72px] resize-y leading-1.5" name="remark" placeholder="输入对账相关备注信息…" {}
 }

 // ── Attachment ──
 div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
 (icon::upload_icon("w-[18px] h-[18px]"))
 "附件"
 }
 div class="rounded p-8 text-center cursor-pointer" {
 (icon::upload_icon("w-8 h-8"))
 p class="text-sm font-medium text-fg mt-2" { "点击或拖拽文件到此处上传" }
 p class="text-xs text-muted mt-1" { "支持 PDF、Word、Excel、图片，单个文件不超过 10MB" }
 }
 }

 // ── Action Bar ──
 div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft" {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{}?restore=true", ReconciliationListPath::PATH)) { "取消" }
 div class="action-bar-right" {
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" onclick="show_info_toast('草稿功能开发中')" {
 (icon::save_icon("w-4 h-4"))
 "保存草稿"
 }
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" _="on click trigger submit on #rec-create-form" {
 (icon::send_icon("w-4 h-4"))
 "提交确认"
 }
 }
 }
 }

 // ── Preview trigger helper ──
 (maud::PreEscaped(r#"<script>
function triggerPreview() {
 htmx.trigger(document.getElementById('rec-app'), 'previewChanged');
}
</script>"#))
 }
 }
}
fn preview_empty(message: &str) -> Markup {
 html! {
 div class="bg-bg border border-border rounded overflow-hidden" id="rec-preview-area"
 hx-get=(ReconciliationPreviewPath::PATH)
 hx-trigger="previewChanged from:#rec-app"
 hx-include="#rec-customer-select,#rec-period-select"
 hx-target="this"
 hx-swap="outerHTML" {
 div class="flex items-center justify-between p-5 border-b border-border-soft" {
 h3 {
 (icon::package_icon("w-[18px] h-[18px]"))
 "对账明细"
 }
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" id="pickOrderBtn" disabled {
 (icon::plus_icon("w-3.5 h-3.5"))
 "从发货单添加"
 }
 }
 div class="text-center p-6 text-muted text-sm" id="emptyState" {
 (icon::clipboard_list_icon("w-12 h-12"))
 p class="text-center p-6 text-muted text-sm-title" { (message) }
 }
 }
 }
}

fn preview_table(
 items: &[ReconciliationPreviewItem],
 product_map: &HashMap<i64, ProductInfo>,
 _order_numbers: &HashMap<i64, String>,
 shipping_numbers: &HashMap<i64, String>,
) -> Markup {
 let total_amount: rust_decimal::Decimal = items.iter().map(|i| i.amount).sum();
 let _total_qty: rust_decimal::Decimal = items.iter().map(|i| i.quantity).sum();
 let item_count = items.len();

 html! {
 div class="bg-bg border border-border rounded overflow-hidden" id="rec-preview-area"
 hx-get=(ReconciliationPreviewPath::PATH)
 hx-trigger="previewChanged from:#rec-app"
 hx-include="#rec-customer-select,#rec-period-select"
 hx-target="this"
 hx-swap="outerHTML" {
 div class="flex items-center justify-between p-5 border-b border-border-soft" {
 h3 {
 (icon::package_icon("w-[18px] h-[18px]"))
 "对账明细"
 }
 div class="flex items-center gap-2" {
 span class="text-xs text-muted" {
 (item_count) " 行"
 }
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" id="pickOrderBtn" {
 (icon::plus_icon("w-3.5 h-3.5"))
 "从发货单添加"
 }
 }
 }
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th class="w-12" { "行号" }
 th { "关联发货单" }
 th { "产品编码" }
 th { "产品名称" }
 th class="w-24" { "发货数量" }
 th class="w-24" { "退货数量" }
 th class="w-28" { "退货金额" }
 th class="w-28" { "单价" }
 th class="w-32" { "应收金额" }
 th class="w-16" { }
 }
 }
 tbody {
 @for (i, item) in items.iter().enumerate() {
 @let product = product_map.get(&item.product_id);
 @let product_code = product.map(|p| p.code.as_str()).unwrap_or("—");
 @let product_name = product.map(|p| p.name.as_str()).unwrap_or("—");
 @let shipping_num = shipping_numbers.get(&item.shipping_request_id).map(|s| s.as_str()).unwrap_or("—");
 @let shipping_detail = ShippingDetailPath { id: item.shipping_request_id };

 tr {
 td class="text-muted text-xs text-center" { (i + 1) }
 td {
 a href=(shipping_detail.to_string()) class="link-accent" { (shipping_num) }
 }
 td class="font-mono tabular-nums" { (product_code) }
 td { (product_name) }
 td class="text-right text-[13px]" { (item.quantity) }
 td class="text-right text-[13px]" { "—" }
 td class="text-right text-[13px]" { "—" }
 td class="text-right text-[13px] font-mono tabular-nums" { (format!("{:.2}", item.unit_price)) }
 td class="text-right text-[13px] font-mono tabular-nums" { (format!("{:.2}", item.amount)) }
 td {
 button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center" title="删除" {
 (icon::x_icon("w-3.5 h-3.5"))
 }
 }
 }
 }
 }
 }
 }
 }

 // ── 金额汇总 ──
 div class="flex justify-end gap-8 p-5 border-t border-border-soft bg-surface-raised" {
 div {
 span class="text-xs text-muted" { "发货总额" }
 span class="text-lg font-bold font-mono tabular-nums text-fg" { (crate::utils::fmt_amount(total_amount)) }
 }
 div {
 span class="text-xs text-muted" { "退货总额" }
 span class="text-lg font-bold font-mono tabular-nums text-danger" { "— ¥ 0.00" }
 }
 div {
 span class="text-xs text-muted" { "调整金额" }
 span class="text-lg font-bold font-mono tabular-nums text-muted" { "¥ 0.00" }
 }
 div {
 span class="text-xs text-muted" { "净额（应收）" }
 span class="text-lg font-bold font-mono tabular-nums text-fg" { (crate::utils::fmt_amount(total_amount)) }
 }
 }
 }
}

// ── Referenced paths from other route modules ──

use crate::routes::shipping::ShippingDetailPath;
