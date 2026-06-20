use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use rust_decimal::Decimal;

use abt_core::master_data::product::ProductService;
use abt_core::shared::types::DomainError;
use abt_core::shared::enums::DocumentType;
use abt_core::shared::document_sequence::DocumentSequenceService;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::wms::inventory_transaction::InventoryTransactionService;
use abt_core::wms::inventory_transaction::model::RecordTransactionReq;
use abt_core::wms::enums::TransactionType;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_stock_out::{StockOutCreatePath, StockOutListPath, StockOutItemRowPath};
use crate::utils::{RequestContext, empty_as_none};
use abt_macros::require_permission;

// ── Query Params ──


#[derive(Debug, Deserialize)]
pub struct ItemRowParams {
 pub product_id: i64,
}

// ── Handlers ──

#[require_permission("INVENTORY", "create")]
pub async fn get_stock_out_create(
 _path: StockOutCreatePath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let warehouse_svc = state.warehouse_service();

 let warehouses = warehouse_svc
 .list(&service_ctx, &mut conn, abt_core::wms::warehouse::model::WarehouseFilter::default(), 1, 200)
 .await
 .map(|r| r.items)
 .unwrap_or_default();

 let content = stock_out_create_content(&warehouses, &claims.display_name);
 let page_html = admin_page(
 is_htmx, "新建出库单", &claims, "inventory", StockOutCreatePath::PATH, "库存管理", None, content, &nav_filter,
 );
 Ok(Html(page_html.into_string()))
}

/// HTMX: search products for the modal

/// HTMX: return a single item row fragment for a given product_id
#[require_permission("INVENTORY", "create")]
pub async fn get_item_row(
 ctx: RequestContext,
 Query(params): Query<ItemRowParams>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.product_service();
 let product = svc.get(&service_ctx, &mut conn, params.product_id).await?;
 Ok(Html(item_row_fragment(&product).into_string()))
}

// ── Form Data ──

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct StockOutCreateForm {
 pub source_type: String,
 pub source_ref: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub warehouse_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub zone_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub bin_id: Option<i64>,
 pub remark: Option<String>,
 pub items_json: String,
}

#[derive(Debug, Deserialize)]
struct StockOutItemWeb {
 product_id: String,
 quantity: String,
 unit_cost: Option<String>,
}

#[require_permission("INVENTORY", "create")]
pub async fn create_stock_out(
 _path: StockOutCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<StockOutCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.inventory_transaction_service();

 let warehouse_id = form.warehouse_id
 .ok_or_else(|| DomainError::validation("请选择来源仓库"))?;

 let web_items: Vec<StockOutItemWeb> = serde_json::from_str(&form.items_json)
 .map_err(|e| DomainError::validation(format!("无效物料数据: {e}")))?;

 if web_items.is_empty() {
 return Err(DomainError::validation("请至少添加一个物料").into());
 }

 let transaction_type = match form.source_type.as_str() {
 "shipping" | "sales" => TransactionType::SalesShipment,
 "requisition" | "material" => TransactionType::MaterialIssue,
 _ => TransactionType::SalesShipment,
 };

 let source_type = form.source_type.as_str();

 let remark = form.remark.filter(|s| !s.is_empty());

 // 出库单号：通过 DocumentSequenceService 生成规范编号（CK-YYYY-MM-SEQ）
 let doc_number = state.document_sequence_service()
 .next_number(&service_ctx, &mut conn, DocumentType::StockShipment)
 .await?;

 // Record one transaction per line item
 for item in &web_items {
 let product_id: i64 = item.product_id.parse()
 .map_err(|_| DomainError::validation("无效产品ID"))?;
 let quantity: Decimal = item.quantity.parse()
 .map_err(|_| DomainError::validation("无效数量"))?;
 let unit_cost: Option<Decimal> = item.unit_cost.as_ref()
 .and_then(|s| s.parse().ok());

 if quantity <= Decimal::ZERO {
 return Err(DomainError::validation("出库数量必须大于0").into());
 }

 // Check available stock
 let available = svc.query_available(&service_ctx, &mut conn, product_id, Some(warehouse_id)).await?;
 if quantity > available {
 return Err(DomainError::business_rule(
 format!("库存不足：产品ID {} 需要 {}，可用 {}", product_id, quantity, available),
 ).into());
 }

 let req = RecordTransactionReq {
 doc_number: Some(doc_number.clone()),
 delivery_no: None,
 source_doc_number: None,
 transaction_type,
 product_id,
 warehouse_id,
 zone_id: form.zone_id,
 bin_id: form.bin_id,
 batch_no: None,
 // record() 的 quantity 是有符号 delta（入库正 / 出库负，参考 transfer 调用方取负）。
 // 出库必须传负数，否则台账会反向增加（历史 bug）。
 quantity: -quantity,
 unit_cost,
 source_type: source_type.to_string(),
 source_id: 0,
 remark: remark.clone(),
 };

 svc.record(&service_ctx, &mut conn, req).await?;
 }

 let redirect = StockOutListPath.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn stock_out_create_content(
 warehouses: &[abt_core::wms::warehouse::model::Warehouse],
 operator_name: &str,
) -> Markup {
 html! {
 div {
 // ── Back Link ──
 a href="/admin/wms/stock-out" class="inline-flex items-center gap-2 mb-4 text-sm text-muted no-underline hover:text-accent transition-colors" {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回出库列表"
 }

 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "新建出库单" }
 }

 // ── Type Switch ──
 div class="flex gap-3 mb-6" {
 div id="type-card-sales" onclick="wmsStockOutSelectType('sales')" class="type-card flex-1 flex flex-col items-center gap-2 rounded-lg p-5 border-2 cursor-pointer transition-colors border-danger bg-danger-bg" {
 (icon::upload_icon("w-7 h-7"))
 span class="text-base font-semibold text-fg" { "销售出库" }
 span class="text-xs text-muted text-center" { "SALES_SHIPMENT" br; "关联发货申请 / 销售订单" br; "消耗 SOFT 预留" }
 }
 div id="type-card-material" onclick="wmsStockOutSelectType('material')" class="type-card flex-1 flex flex-col items-center gap-2 rounded-lg p-5 border-2 cursor-pointer transition-colors border-border bg-bg" {
 (icon::clipboard_document_icon("w-7 h-7"))
 span class="text-base font-semibold text-fg" { "生产领料" }
 span class="text-xs text-muted text-center" { "MATERIAL_ISSUE" br; "关联工单 / 领料单" br; "消耗 HARD 预留" }
 }
 }

 form id="stockOutForm" class="space-y-5" hx-post=(StockOutCreatePath::PATH) hx-swap="none"
 onsubmit="return wmsStockOutCollectItems()" {
 // ── Source Section ──
 div class="bg-bg border border-border rounded p-6" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
 (icon::link_icon("w-4 h-4"))
 "来源关联"
 }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "来源类型" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="source_type" {
 option value="shipping" { "发货申请 (SH)" }
 option value="requisition" { "领料单 (MR)" }
 option value="manual" { "手工录入" }
 }
 }
 div class="flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "来源单号 " span class="text-danger" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="source_ref" placeholder="选择来源单号" readonly;
 }
 div class="flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "客户/工单" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" placeholder="选择来源后自动填充" readonly class="bg-surface";
 }
 div class="flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "预留类型" }
 input id="reservation-type-input" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" value="SOFT 预留（发货消耗）" readonly class="bg-surface text-danger";
 }
 }
 }

 // ── Warehouse Section ──
 div class="bg-bg border border-border rounded p-6" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
 (icon::building_icon("w-4 h-4"))
 "出库信息"
 }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "来源仓库 " span class="text-danger" { "*" } }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="warehouse_id" {
 option value="" { "请选择仓库" }
 @for wh in warehouses {
 option value=(wh.id) { (wh.name) }
 }
 }
 }
 div class="flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "来源库区" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="zone_id" {
 option value="" { "按拣货策略分配" }
 }
 }
 div class="flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "拣货策略" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="pick_strategy" {
 option value="fifo" selected { "FIFO 先进先出" }
 option value="fefo" { "FEFO 先到期先出" }
 option value="shortest" { "最短路径" }
 option value="full_pallet" { "整托优先" }
 }
 }
 div class="flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "操作员" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" value=(operator_name) readonly class="bg-surface";
 }
 }
 }

 // ── Pick Strategy Tip ──
 div class="flex items-center rounded-md mb-6 gap-3 px-4 py-3 bg-[rgba(250,173,20,0.05)] border border-[rgba(250,173,20,0.15)]" {
 (icon::circle_alert_icon("w-4 h-4 text-warn shrink-0"))
 span class="text-sm text-fg-2" {
 "拣货策略："
 strong { "FIFO 先进先出" }
 " — 系统优先拣选最早入库批次的物料，确保库存周转。对于有效期管理物料建议使用 FEFO。"
 }
 }

 // ── Line Items ──
 div class="bg-bg border border-border rounded p-6" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
 (icon::box_icon("w-4 h-4"))
 "出库物料明细"
 span id="stockout-item-count" class="ml-auto text-xs font-normal text-muted" { "共 0 项" }
 }
 table class="data-table" {
 thead {
 tr {
 th class="w-10" { "序号" }
 th { "产品编码" }
 th { "产品名称" }
 th { "规格型号" }
 th class="w-[100px]" { "出库数量 " span class="text-danger" { "*" } }
 th class="w-[90px]" { "单位" }
 th class="w-[110px]" { "单位成本" }
 th class="w-[110px]" { "小计" }
 th class="w-10" { }
 }
 }
 tbody id="stockout-item-tbody" {
 // JS-managed dynamic rows
 }
 }
 div class="mt-4" {
 button type="button" class="flex items-center justify-center gap-2 w-full text-accent text-sm font-medium cursor-pointer"
 _="on click add .is-open to #stockout-product-modal" {
 (icon::plus_icon("w-3.5 h-3.5"))
 "添加物料"
 }
 }
 }

 // ── Reservation Info ──
 div class="mt-4 p-4 rounded-md border border-border-soft bg-[linear-gradient(135deg,rgba(250,173,20,0.04),rgba(255,77,79,0.04))]" {
 h4 class="flex items-center gap-2 mb-3 text-sm font-semibold text-fg-2" {
 (icon::lock_icon("w-4 h-4"))
 "库存预留 & 可用性检查"
 }
 div class="grid grid-cols-3 gap-4" {
 div class="text-center p-3 rounded-md bg-bg" {
 div class="text-[11px] text-muted mb-0.5" { "预留类型" }
 div id="reservation-type-badge" class="text-base font-semibold font-mono text-danger" { "SOFT" }
 }
 div class="text-center p-3 rounded-md bg-bg" {
 div class="text-[11px] text-muted mb-0.5" { "已预留量" }
 div class="text-lg font-semibold font-mono text-warn" { "—" }
 }
 div class="text-center p-3 rounded-md bg-bg" {
 div class="text-[11px] text-muted mb-0.5" { "出库后释放" }
 div class="text-base font-semibold font-mono text-success" { "→ available_qty" }
 }
 }
 }

 // ── Summary ──
 div class="bg-bg border border-border rounded p-6" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
 (icon::clipboard_list_icon("w-4 h-4"))
 "出库汇总"
 }
 div class="grid grid-cols-4 gap-6" {
 div class="text-center bg-surface p-4 rounded-md" {
 div class="text-[11px] text-muted mb-1" { "物料种类" }
 div id="stockout-summary-kinds" class="text-xl font-semibold font-mono text-fg" { "0" }
 }
 div class="text-center bg-surface p-4 rounded-md" {
 div class="text-[11px] text-muted mb-1" { "出库总量" }
 div id="stockout-summary-qty" class="text-xl font-semibold font-mono text-fg" { "0" }
 }
 div class="text-center p-4 rounded-md bg-danger-bg border border-[rgba(255,77,79,0.15)]" {
 div class="text-[11px] text-danger mb-1" { "出库总金额" }
 div id="stockout-summary-amount" class="text-xl font-semibold font-mono text-danger" { "¥0.00" }
 }
 div class="text-center bg-surface p-4 rounded-md" {
 div class="text-[11px] text-muted mb-1" { "拣货策略" }
 div class="text-sm font-semibold text-fg" { "FIFO" }
 }
 }
 }

 // ── Remark ──
 div class="bg-bg border border-border rounded p-6" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
 (icon::edit_icon("w-4 h-4"))
 "备注"
 }
 textarea class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none resize-y min-h-[80px] transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]" name="remark" placeholder="输入备注信息…" rows="3" { }
 }

 // hidden input for items JSON
 input type="hidden" name="items_json" id="stockout-items-json" value="[]" {}
 // ── Action Bar ──
 div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft" {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href="/admin/wms/stock-out" { "取消" }
 div class="flex gap-3" {
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" { "保存草稿" }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-danger text-accent-on border-none hover:bg-danger-700 text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(220,38,38,0.2)]" {
 (icon::upload_icon("w-4 h-4"))
 "确认出库"
 }
 }
 }
 }
 }

            (crate::components::product_picker::product_picker_modal_with_search("stockout-product-modal", StockOutItemRowPath::PATH, "stockout-item-tbody"))

 // ── Line Item JS ──
 (maud::PreEscaped(r#"<script>
 // Line item calculations
 function wmsStockOutCalcRow(row) {
 var qtyInput = row.querySelector('input[name="quantity"]');
 var costInput = row.querySelector('input[name="unit_cost"]');
 var totalCell = row.querySelector('.line-subtotal');
 var qty = parseFloat(qtyInput.value) || 0;
 var cost = parseFloat(costInput.value) || 0;
 var subtotal = qty * cost;
 totalCell.textContent = subtotal > 0 ? '¥' + subtotal.toFixed(2) : '—';
 wmsStockOutCalcSummary();
 }

 function wmsStockOutCalcSummary() {
 var tbody = document.getElementById('stockout-item-tbody');
 var rows = tbody.querySelectorAll('tr');
 var kinds = rows.length;
 var totalQty = 0;
 var totalAmount = 0;
 rows.forEach(function(row) {
 var qty = parseFloat(row.querySelector('input[name="quantity"]').value) || 0;
 var cost = parseFloat(row.querySelector('input[name="unit_cost"]').value) || 0;
 totalQty += qty;
 totalAmount += qty * cost;
 });
 document.getElementById('stockout-summary-kinds').textContent = kinds;
 document.getElementById('stockout-summary-qty').textContent = totalQty;
 document.getElementById('stockout-summary-amount').textContent = '¥' + totalAmount.toFixed(2);
 document.getElementById('stockout-item-count').textContent = '共 ' + kinds + ' 项';
 }

 // Collect items for form submission
 function wmsStockOutCollectItems() {
 var tbody = document.getElementById('stockout-item-tbody');
 var rows = tbody.querySelectorAll('tr');
 var items = [];
 rows.forEach(function(row) {
 items.push({
 product_id: row.querySelector('input[name="product_id"]').value,
 quantity: row.querySelector('input[name="quantity"]').value || '0',
 unit_cost: row.querySelector('input[name="unit_cost"]').value || null
 });
 });
 document.getElementById('stockout-items-json').value = JSON.stringify(items);
 if (items.length === 0) {
 alert('请至少添加一个物料');
 return false;
 }
 return true;
 }

 // Renumber rows
 function wmsStockOutRenumber() {
 var tbody = document.getElementById('stockout-item-tbody');
 var rows = tbody.querySelectorAll('tr');
 rows.forEach(function(row, i) {
 row.querySelector('.line-num').textContent = i + 1;
 });
 wmsStockOutCalcSummary();
 }

 // Type switch: toggle card visual state and update form fields
 function wmsStockOutSelectType(type) {
 var sales = document.getElementById('type-card-sales');
 var material = document.getElementById('type-card-material');
 var selectEl = document.querySelector('select[name=source_type]');
 var resInput = document.getElementById('reservation-type-input');
 var resBadge = document.getElementById('reservation-type-badge');
 function setOn(card, on) {
 card.classList.remove('border-border', 'bg-bg', 'border-danger', 'bg-danger-bg');
 if (on) { card.classList.add('border-danger', 'bg-danger-bg'); }
 else { card.classList.add('border-border', 'bg-bg'); }
 }
 if (type === 'sales') {
 setOn(sales, true); setOn(material, false);
 selectEl.value = 'shipping';
 resInput.value = 'SOFT 预留（发货消耗）';
 resBadge.textContent = 'SOFT';
 } else {
 setOn(material, true); setOn(sales, false);
 selectEl.value = 'requisition';
 resInput.value = 'HARD 预留（生产领料）';
 resBadge.textContent = 'HARD';
 }
 }
 </script>"#))
 }
}

const CELL_INPUT: &str =
 "w-full px-2 py-[5px] border border-border rounded-sm text-[13px] bg-white text-fg \
  outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]";

/// Single item row fragment
fn item_row_fragment(product: &abt_core::master_data::product::model::Product) -> Markup {
 html! {
 tr oninput="wmsStockOutCalcRow(this)" {
 td class="line-num text-muted text-xs text-center" { }
 td class="font-mono tabular-nums text-sm text-fg" { (product.product_code) }
 td class="text-sm text-fg" { (product.pdt_name) }
 td class="text-sm text-fg-2" { (product.meta.specification) }
 td { input class=(format!("{CELL_INPUT} w-[90px] text-right font-mono")) type="number" step="any" name="quantity" placeholder="0" {} }
 td class="text-center text-sm text-fg-2" { (product.unit) }
 td { input class=(format!("{CELL_INPUT} w-[100px] text-right font-mono")) type="number" step="any" name="unit_cost" placeholder="0.00" {} }
 td class="line-subtotal text-right font-mono font-semibold whitespace-nowrap text-sm" { "—" }
 td { button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:text-danger" title="删除行"
 _="on click remove closest <tr/> then call wmsStockOutRenumber()" {
 (icon::x_icon("w-3.5 h-3.5"))
 } }
 input type="hidden" name="product_id" value=(product.product_id) {}
 }
 }
}
