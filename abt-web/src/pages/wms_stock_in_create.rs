use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use rust_decimal::Decimal;
use std::collections::HashMap;

use abt_core::wms::arrival_notice::ArrivalNoticeService;
use abt_core::wms::arrival_notice::model::ArrivalNoticeFilter;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::purchase::order::model::PurchaseOrderQuery;
use abt_core::master_data::supplier::SupplierService;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::wms::inventory_transaction::InventoryTransactionService;
use abt_core::wms::inventory_transaction::model::RecordTransactionReq;
use abt_core::wms::enums::{ArrivalStatus, TransactionType};
use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::shared::types::{DomainError, PageParams};
use abt_core::shared::enums::DocumentType;
use abt_core::shared::document_sequence::DocumentSequenceService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_stock_in::{StockInCreatePath, StockInListPath, StockInProductsPath, StockInItemRowPath, StockInSourcePickPath, StockInSourceItemsPath};
use crate::utils::{RequestContext, empty_as_none};
use abt_macros::require_permission;

// ── Query Params ──


#[derive(Debug, Deserialize)]
pub struct ItemRowParams {
 pub product_id: i64,
}

// ── Handlers ──

#[require_permission("INVENTORY", "create")]
pub async fn get_stock_in_create(
 _path: StockInCreatePath,
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

 // Load zones for all warehouses
 let mut all_zones: Vec<(i64, Vec<abt_core::wms::warehouse::model::Zone>)> = Vec::new();
 for wh in &warehouses {
 if let Ok(zs) = warehouse_svc.list_zones(&service_ctx, &mut conn, wh.id).await {
 all_zones.push((wh.id, zs));
 }
 }

 // Load bins for all zones
 let mut all_bins: Vec<(i64, Vec<abt_core::wms::warehouse::model::Bin>)> = Vec::new();
 for (_, zones) in &all_zones {
 for z in zones {
 if let Ok(result) = warehouse_svc.list_bins(&service_ctx, &mut conn, z.id, None, 1, 200).await {
 all_bins.push((z.id, result.items));
 }
 }
 }

 let content = stock_in_create_content(&warehouses, &all_zones, &all_bins, &claims.display_name);
 let page_html = admin_page(
 is_htmx, "新建入库单", &claims, "inventory", StockInCreatePath::PATH, "库存管理", None, content, &nav_filter,
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

// ── Source Pick (来料通知 / 采购订单 来源选择) ──

#[derive(Debug, Deserialize)]
pub struct SourcePickParams {
 pub source_type: String,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub keyword: Option<String>,
}

struct SourceOption {
 id: i64,
 doc_number: String,
 supplier_name: String,
 extra: String,
}

/// HTMX: list arrival notices or purchase orders for source selection
#[require_permission("INVENTORY", "create")]
pub async fn get_source_pick(
 ctx: RequestContext,
 Query(params): Query<SourcePickParams>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let kw = params.keyword.as_deref().unwrap_or("").trim().to_string();
 let supplier_svc = state.supplier_service();

 let options: Vec<SourceOption> = if params.source_type == "arrival" {
 let svc = state.arrival_notice_service();
 let filter = ArrivalNoticeFilter {
 doc_number: if kw.is_empty() { None } else { Some(kw.clone()) },
 ..Default::default()
 };
 let notices = svc
 .list(&service_ctx, &mut conn, filter, 1, 50)
 .await
 .map(|r| r.items)
 .unwrap_or_default();
 let names = resolve_supplier_names_map(
 &supplier_svc, &service_ctx, &mut conn,
 notices.iter().map(|n| n.supplier_id).collect(),
 ).await;
 notices.into_iter()
 .filter(|n| matches!(n.status, ArrivalStatus::Accepted | ArrivalStatus::PartiallyAccepted))
 .map(|n| SourceOption {
 id: n.id,
 doc_number: n.doc_number,
 supplier_name: names.get(&n.supplier_id).cloned().unwrap_or_else(|| "-".into()),
 extra: n.arrival_date.to_string(),
 }).collect()
 } else {
 let svc = state.purchase_order_service();
 let result = svc
 .list(&service_ctx, &mut conn, PurchaseOrderQuery::default(), PageParams::new(1, 50))
 .await
 .map(|r| r.items)
 .unwrap_or_default();
 let names = resolve_supplier_names_map(
 &supplier_svc, &service_ctx, &mut conn,
 result.iter().map(|o| o.supplier_id).collect(),
 ).await;
 result.into_iter()
 .filter(|o| {
 use abt_core::purchase::enums::PurchaseOrderStatus;
 matches!(o.status, PurchaseOrderStatus::PartiallyReceived | PurchaseOrderStatus::Received)
 })
 .map(|o| SourceOption {
 id: o.id,
 doc_number: o.doc_number,
 supplier_name: names.get(&o.supplier_id).cloned().unwrap_or_else(|| "-".into()),
 extra: o.order_date.to_string(),
 }).collect()
 };

 Ok(Html(source_pick_fragment(&options).into_string()))
}

// ── Source Items (来源单据明细自动填充) ──

#[derive(Debug, Deserialize)]
pub struct SourceItemsParams {
 pub source_type: String,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub source_id: Option<i64>,
}

/// HTMX: 根据来源单据 ID 返回物料明细行（采购订单 / 来料通知）
#[require_permission("INVENTORY", "create")]
pub async fn get_source_items(
 ctx: RequestContext,
 Query(params): Query<SourceItemsParams>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let product_svc = state.product_service();

 let source_id = match params.source_id {
 Some(id) if id > 0 => id,
 _ => return Ok(Html(source_items_fragment(&[]).into_string())),
 };

 // Fetch line items from the source document
 let items: Vec<(i64, Decimal)> = match params.source_type.as_str() {
 "purchase" => {
 let po_svc = state.purchase_order_service();
 po_svc.list_items(&service_ctx, &mut conn, source_id)
 .await?
 .into_iter()
 .map(|it| (it.product_id, it.quantity))
 .collect()
 }
 "arrival" => {
 let an_svc = state.arrival_notice_service();
 an_svc.list_items(&service_ctx, &mut conn, source_id)
 .await?
 .into_iter()
 .map(|it| (it.product_id, it.declared_qty))
 .collect()
 }
 _ => Vec::new(),
 };

 // Resolve product details for each item
 let mut rows: Vec<(abt_core::master_data::product::model::Product, Decimal)> = Vec::new();
 for (product_id, qty) in &items {
 match product_svc.get(&service_ctx, &mut conn, *product_id).await {
 Ok(p) => rows.push((p, *qty)),
 Err(_) => continue,
 }
 }

 Ok(Html(source_items_fragment(&rows).into_string()))
}

async fn resolve_supplier_names_map<S: SupplierService>(
 svc: &S,
 ctx: &abt_core::shared::types::ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 ids: Vec<i64>,
) -> HashMap<i64, String> {
 if ids.is_empty() {
 return HashMap::new();
 }
 svc.list(ctx, db, abt_core::master_data::supplier::model::SupplierQuery::default(), PageParams::new(1, 500))
 .await
 .map(|r| r.items.into_iter().filter(|s| ids.contains(&s.id)).map(|s| (s.id, s.name)).collect())
 .unwrap_or_default()
}
// ── Form Data ──


#[derive(Debug, Deserialize)]
pub struct StockInCreateForm {
 pub transaction_type: String,
 pub source_type: String,
 pub source_ref: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub source_id: Option<i64>,
 pub delivery_no: Option<String>,
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
struct StockInItemWeb {
 product_id: String,
 batch_no: Option<String>,
 quantity: String,
 bin_id: Option<String>,
}

#[require_permission("INVENTORY", "create")]
pub async fn create_stock_in(
 _path: StockInCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<StockInCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.inventory_transaction_service();

 let warehouse_id = form.warehouse_id
 .ok_or_else(|| DomainError::validation("请选择目标仓库"))?;

 let web_items: Vec<StockInItemWeb> = serde_json::from_str(&form.items_json)
 .map_err(|e| DomainError::validation(format!("无效物料数据: {e}")))?;

 if web_items.is_empty() {
 return Err(DomainError::validation("请至少添加一个物料").into());
 }

 let transaction_type = match form.transaction_type.as_str() {
 "ProductionReceipt" => TransactionType::ProductionReceipt,
 _ => TransactionType::PurchaseReceipt,
 };

 let source_type = match form.source_type.as_str() {
 "arrival" => "arrival_notice",
 "purchase" => "purchase_order",
 other => other,
 };

 let remark = form.remark.filter(|s| !s.is_empty());
 let source_id: i64 = form.source_id.unwrap_or(0);
 // 入库单号：通过 DocumentSequenceService 生成规范编号（RK-YYYY-MM-SEQ）
 let doc_number = state.document_sequence_service()
 .next_number(&service_ctx, &mut conn, DocumentType::StockReceipt)
 .await?;
 // 来源单号：记录来源单据的单号（如采购单号 PO-xxx、来料通知单号 AN-xxx）
 let source_doc_number = form.source_ref
 .as_ref()
 .filter(|s| !s.is_empty())
 .cloned();

 // 问题三修复：未选库区/库位时自动解析默认值，确保库存台账更新
 let warehouse_svc = state.warehouse_service();
 let zone_id = match form.zone_id {
 Some(zid) => Some(zid),
 None => warehouse_svc
 .get_or_create_default_zone(&service_ctx, &mut conn, warehouse_id)
 .await
 .ok()
 .map(|z| z.id),
 };
 let default_bin_id: Option<i64> = if let Some(zid) = zone_id {
 warehouse_svc
 .list_bins(&service_ctx, &mut conn, zid, None, 1, 1)
 .await
 .ok()
 .and_then(|r| r.items.first().map(|b| b.id))
 } else {
 None
 };

 // Record one transaction per line item
 for item in &web_items {
 let product_id: i64 = item.product_id.parse()
 .map_err(|_| DomainError::validation("无效产品ID"))?;
 let quantity: Decimal = item.quantity.parse()
 .map_err(|_| DomainError::validation("无效数量"))?;
 let bin_id: Option<i64> = item.bin_id.as_ref()
 .and_then(|s| s.parse().ok());

 if quantity <= Decimal::ZERO {
 return Err(DomainError::validation("入库数量必须大于0").into());
 }

 let req = RecordTransactionReq {
 doc_number: Some(doc_number.clone()),
 delivery_no: form.delivery_no.clone(),
 source_doc_number: source_doc_number.clone(),
 transaction_type,
 product_id,
 warehouse_id,
 zone_id,
 bin_id: bin_id.or(form.bin_id).or(default_bin_id),
 batch_no: item.batch_no.clone(),
 quantity,
 unit_cost: None,
 source_type: source_type.to_string(),
 source_id,
 remark: remark.clone(),
 };

 svc.record(&service_ctx, &mut conn, req).await?;
 }

 let redirect = StockInListPath.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn stock_in_create_content(
 warehouses: &[abt_core::wms::warehouse::model::Warehouse],
 all_zones: &[(i64, Vec<abt_core::wms::warehouse::model::Zone>)],
 all_bins: &[(i64, Vec<abt_core::wms::warehouse::model::Bin>)],
 operator_name: &str,
) -> Markup {
 html! {
 div {
 // ── Back Link ──
 a href="/admin/wms/stock-in" class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" {
 (icon::chevron_left_icon("w-4 h-4"))
 "返回入库列表"
 }

 // ── Page Header ──
 div class="flex items-center justify-between mb-6" class="mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "新建入库单" }
 }

 // ── Type Switch ──
 div class="flex gap-3" {
 div class="flex-1 flex flex-col items-center gap-2 p-5 rounded-lg bg-bg cursor-pointer active" _="on click take .active from .type-btn then put 'PurchaseReceipt' into #stockin-txn-type's value" {
 (icon::download_icon("w-7 h-7"))
 span class="type-label" { "采购入库" }
 span class="type-desc" { "PURCHASE_RECEIPT" br; "关联来料通知 / 采购订单" }
 }
 div class="flex-1 flex flex-col items-center gap-2 p-5 rounded-lg bg-bg cursor-pointer" _="on click take .active from .type-btn then put 'ProductionReceipt' into #stockin-txn-type's value" {
 (icon::box_icon("w-7 h-7"))
 span class="type-label" { "生产入库" }
 span class="type-desc" { "PRODUCTION_RECEIPT" br; "关联工单完工报工" }
 }
 }

 form id="stockInForm" hx-post=(StockInCreatePath::PATH) hx-swap="none"
 onsubmit="return wmsStockInCollectItems()" {
 input type="hidden" id="stockin-txn-type" name="transaction_type" value="PurchaseReceipt" {};
 // ── Source Section ──
 div class="bg-bg border border-border rounded p-6" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" {
 (icon::link_icon("w-[18px] h-[18px]"))
 "来源关联"
 }
 div class="wms-grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-group" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "来源类型" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="source_type" id="source-type-select"
 _="on change[my value is 'manual'] put '可选：输入来源单号' into #source-ref-input's @placeholder then hide #source-ref-pick-btn then hide #source-ref-required then hide #source-supplier-group then set #source-ref-input's value to '' then set #source-id-input's value to '' then set #source-supplier-input's value to '' then set #stockin-item-tbody's innerHTML to '' then call wmsStockInCalcSummary()
 on change[my value is not 'manual'] put '选择来源单号或直接输入' into #source-ref-input's @placeholder then show #source-ref-pick-btn then show #source-ref-required then show #source-supplier-group then set #source-ref-input's value to '' then set #source-id-input's value to '' then set #stockin-item-tbody's innerHTML to '' then call wmsStockInCalcSummary()" {
 option value="arrival" selected { "来料通知 (AN)" }
 option value="purchase" { "采购订单 (PO)" }
 option value="manual" { "手工录入" }
 }
 }
 div class="form-group" id="source-ref-group" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "来源单号 " span class="required" id="source-ref-required" { "*" } }
 div class="flex gap-2" {
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" id="source-ref-input" name="source_ref"
 placeholder="选择来源单号或直接输入" class="flex-1" {};
 input type="hidden" id="source-id-input" name="source_id" {};
 button type="button" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" id="source-ref-pick-btn"
 _="on click set #source-pick-type's value to #source-type-select's value then add .is-open to #source-modal then call wmsOpenSourceModal()" { "选择" }
 }
 }
 div class="form-group" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "送货单号" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="delivery_no" placeholder="输入送货单号";
 }
 div class="form-group" id="source-supplier-group" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "供应商" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" id="source-supplier-input"
 placeholder="选择来源后自动填充" readonly class="bg-surface";
 }
 }
 }

 // ── Warehouse Section ──
 div class="bg-bg border border-border rounded p-6" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" {
 (icon::building_icon("w-[18px] h-[18px]"))
 "入库信息"
 }
 div class="wms-grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-group" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "目标仓库 " span class="required" { "*" } }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="warehouse_id" id="warehouse-select"
 onchange="wmsUpdateZones()" {
 option value="" disabled selected { "请选择仓库" }
 @for wh in warehouses {
 option value=(wh.id) { (wh.name) }
 }
 }
 }
 div class="form-group" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "目标库区" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="zone_id" id="zone-select"
 onchange="wmsUpdateBins()" {
 option value="" { "请先选择仓库" }
 @for (wh_id, zones) in all_zones {
 @for z in zones {
 option value=(z.id) data-wh=(wh_id) style="display:none" { (z.code) " " (z.name) }
 }
 }
 }
 }
 div class="form-group" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "目标库位" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="bin_id" id="bin-select" {
 option value="" { "请先选择库区" }
 @for (zone_id, bins) in all_bins {
 @for b in bins {
 option value=(b.id) data-zone=(zone_id) style="display:none" { (b.code) " " (b.name) }
 }
 }
 }
 }
 div class="form-group" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "操作员" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" value=(operator_name) readonly class="bg-surface";
 }
 }
 }

 // ── Strategy Tip ──
 div class="flex items-center" class="rounded-md mb-6 gap-3" class="px-4 py-3" style="background:rgba(82,196,26,0.05);border:1px solid rgba(82,196,26,0.15)" {
 (icon::check_circle_icon("w-4 h-4"))
 span class="text-fg-2" class="text-sm" {
 "当前仓库上架策略："
 strong { "同物料合并 (SAME_MERGE)" }
 " — 系统将自动分配至同物料已有库位，库位满时按就近原则分配。"
 }
 }

 // ── Line Items ──
 div class="bg-bg border border-border rounded p-6" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" {
 (icon::box_icon("w-[18px] h-[18px]"))
 "入库物料明细"
 span id="stockin-item-count" class="font-normal text-muted" class="ml-auto text-xs" { "共 0 项" }
 }
 table class="data-table" {
 thead {
 tr {
 th class="w-10" { "序号" }
 th { "产品编码" }
 th { "产品名称" }
 th { "规格型号" }
 th { "批次号" }
 th class="w-[100px]" { "入库数量 " span class="required" { "*" } }
 th { "目标库位" }
 th class="w-10" { }
 }
 }
 tbody id="stockin-item-tbody" {
 // JS-managed dynamic rows
 }
 }
 div class="mt-4" {
 button type="button" class="flex items-center justify-center gap-2 w-full text-[#2563eb] text-sm font-medium cursor-pointer"
 _="on click add .is-open to #product-modal" {
 (icon::plus_icon("w-3.5 h-3.5"))
 "添加物料"
 }
 }
 }

 // ── Summary ──
 div class="bg-bg border border-border rounded p-6" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" {
 (icon::clipboard_list_icon("w-[18px] h-[18px]"))
 "入库汇总"
 }
 div class="grid" class="grid-cols-3" style="gap:var(--space-6)" {
 div class="text-center bg-surface" class="p-4 rounded-md" {
 div class="text-[11px] text-muted" class="mb-1" { "物料种类" }
 div id="stockin-summary-kinds" class="font-mono tabular-nums" class="font-semibold" class="text-xl" { "0" }
 }
 div class="text-center bg-surface" class="p-4 rounded-md" {
 div class="text-[11px] text-muted" class="mb-1" { "入库总量" }
 div id="stockin-summary-qty" class="font-mono tabular-nums" class="font-semibold" class="text-xl" { "0" }
 }
 div class="text-center bg-surface" class="p-4 rounded-md" {
 div class="text-[11px] text-muted" class="mb-1" { "上架策略" }
 div class="font-semibold" class="text-sm" { "同物料合并" }
 }
 }
 }

 // ── Remark ──
 div class="bg-bg border border-border rounded p-6" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" {
 (icon::edit_icon("w-[18px] h-[18px]"))
 "备注"
 }
 textarea class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="remark" placeholder="输入备注信息…" rows="3" class="w-full resize-y" class="min-h-[80px]" style="padding:var(--space-2) var(--space-3)" { }
 }

 // hidden input for items JSON
 input type="hidden" name="items_json" id="stockin-items-json" value="[]" {}

 // ── Action Bar ──
 div class="flex items-center justify-end gap-3 pt-4 [border-top:1px_solid_var(--border-soft)]" {
 a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{}?restore=true", StockInListPath::PATH)) { "取消" }
 div class="flex gap-3" {
 button type="button" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" { "保存草稿" }
 button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
 (icon::check_circle_icon("w-4 h-4"))
 "确认入库"
 }
 }
 }
 }
 }

            (crate::components::product_picker::product_picker_modal_with_search("product-modal", StockInItemRowPath::PATH, "stockin-item-tbody"))

 // ── Source Pick Modal ──
 div id="source-modal" class="fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto" data-source-path=(StockInSourcePickPath::PATH)
 _="on click[me is event.target] remove .is-open" {
 div class="modal bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl" _="on click halt the event" {
 div class="px-6 py-5 [border-bottom:1px_solid_var(--border-soft)] flex justify-between items-center shrink-0" {
 h2 { "选择来源单据" }
 button type="button" class="bg-transparent border-none cursor-pointer text-xl text-muted p-1"
 _="on click remove .is-open from #source-modal" { "×" }
 }
 div class="overflow-y-auto flex-1 min-h-0 p-6" class="p-0" hx-disinherit="hx-select" {
 input type="hidden" id="source-pick-type" name="source_type" value="arrival" {}
 div class="flex gap-4 p-4 [border-bottom:1px_solid_var(--border-soft)]" {
 div class="flex-1 flex flex-col gap-[4px]" {
 label class="text-[12px] font-medium text-fg-2" { "来源单号" }
 input id="source-search-input" class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword" placeholder="输入单号关键词…"
 hx-get=(StockInSourcePickPath::PATH)
 hx-trigger="keyup changed delay:300ms"
 hx-sync="this:replace"
 hx-target="#stockin-source-results"
 hx-swap="innerHTML"
 hx-include="#source-pick-type" {}
 }
 button type="button" class="border border-border rounded-sm bg-bg text-fg-2 text-sm cursor-pointer whitespace-nowrap"
 hx-get=(StockInSourcePickPath::PATH)
 hx-target="#stockin-source-results"
 hx-swap="innerHTML"
 hx-include="#source-pick-type"
 _="on click set #source-search-input's value to '' then trigger keyup on #source-search-input" {
 "清除"
 }
 }
 div id="stockin-source-results" {
 div class="text-center text-muted py-12" {
 (icon::link_icon("w-8 h-8"))
 p class="mt-2 text-sm" { "输入关键词搜索来源单据" }
 }
 }
 }
 }
 }

 // ── Cascade + Line Item JS ──
 (maud::PreEscaped(r#"<script>
 // Warehouse → Zone cascade
 function wmsUpdateZones() {
 var whId = document.getElementById('warehouse-select').value;
 var zoneSelect = document.getElementById('zone-select');
 var options = zoneSelect.querySelectorAll('option[data-wh]');
 var firstOpt = zoneSelect.querySelector('option:not([data-wh])');
 options.forEach(function(opt) {
 opt.style.display = (!whId || opt.dataset.wh === whId) ? '' : 'none';
 });
 zoneSelect.value = '';
 if (firstOpt) firstOpt.textContent = whId ? '请选择库区' : '请先选择仓库';
 wmsUpdateBins();
 }

 // Zone → Bin cascade
 function wmsUpdateBins() {
 var zoneId = document.getElementById('zone-select').value;
 var binSelect = document.getElementById('bin-select');
 var options = binSelect.querySelectorAll('option[data-zone]');
 var firstOpt = binSelect.querySelector('option:not([data-zone])');
 options.forEach(function(opt) {
 opt.style.display = (!zoneId || opt.dataset.zone === zoneId) ? '' : 'none';
 });
 binSelect.value = '';
 if (firstOpt) firstOpt.textContent = zoneId ? '按上架策略分配' : '请先选择库区';
 }

 // Line item calculations
 function wmsStockInCalcRow(row) {
 wmsStockInCalcSummary();
 }

 function wmsStockInCalcSummary() {
 var tbody = document.getElementById('stockin-item-tbody');
 var rows = tbody.querySelectorAll('tr');
 var kinds = rows.length;
 var totalQty = 0;
 rows.forEach(function(row) {
 var qty = parseFloat(row.querySelector('input[name="quantity"]').value) || 0;
 totalQty += qty;
 });
 document.getElementById('stockin-summary-kinds').textContent = kinds;
 document.getElementById('stockin-summary-qty').textContent = totalQty;
 document.getElementById('stockin-item-count').textContent = '共 ' + kinds + ' 项';
 }

 // Collect items for form submission
 function wmsStockInCollectItems() {
 var tbody = document.getElementById('stockin-item-tbody');
 var rows = tbody.querySelectorAll('tr');
 var items = [];
 rows.forEach(function(row) {
 items.push({
 product_id: row.querySelector('input[name="product_id"]').value,
 batch_no: row.querySelector('input[name="batch_no"]').value || null,
 quantity: row.querySelector('input[name="quantity"]').value || '0',

 bin_id: row.querySelector('input[name="item_bin_id"]')?.value || null
 });
 });
 document.getElementById('stockin-items-json').value = JSON.stringify(items);
 if (items.length === 0) {
 alert('请至少添加一个物料');
 return false;
 }
 return true;
 }

 // Renumber rows
 function wmsStockInRenumber() {
 var tbody = document.getElementById('stockin-item-tbody');
 var rows = tbody.querySelectorAll('tr');
 rows.forEach(function(row, i) {
 row.querySelector('.line-num').textContent = i + 1;
 });
 wmsStockInCalcSummary();
 }

 // Open source pick modal — capture current source_type and load list
 function wmsOpenSourceModal() {
 var modal = document.getElementById('source-modal');
 var type = document.getElementById('source-type-select').value;
 document.getElementById('source-pick-type').value = type;
 modal.classList.add('is-open');
 var path = modal.dataset.sourcePath;
 htmx.ajax('GET', path + '?source_type=' + encodeURIComponent(type), {target: '#stockin-source-results', swap: 'innerHTML'});
 }

 // Pick a source row — backfill ref / supplier / source_id, then auto-load items
 function wmsStockInPickSource(btn) {
 document.getElementById('source-ref-input').value = btn.dataset.doc;
 document.getElementById('source-supplier-input').value = btn.dataset.supplier;
 document.getElementById('source-id-input').value = btn.dataset.sourceId;
 document.querySelector('#source-modal').classList.remove('is-open');
 wmsStockInLoadSourceItems();
 }

 // Auto-load line items from the selected source document
 function wmsStockInLoadSourceItems() {
 var sourceType = document.getElementById('source-type-select').value;
 var sourceId = document.getElementById('source-id-input').value;
 if (sourceType === 'manual' || !sourceId || sourceId === '0') return;
 // Clear existing manually-added items
 document.getElementById('stockin-item-tbody').innerHTML = '';
 htmx.ajax('GET', '/admin/wms/stock-in/create/source-items', {
 target: '#stockin-item-tbody',
 swap: 'innerHTML',
 values: { source_type: sourceType, source_id: sourceId }
 }).then(function() { setTimeout(wmsStockInRenumber, 50); });
 }
 </script>"#))
 }
}

/// Source pick (来料通知/采购订单) results fragment
fn source_pick_fragment(options: &[SourceOption]) -> Markup {
 html! {
 @if options.is_empty() {
 div class="text-center text-muted py-12" {
 (icon::link_icon("w-8 h-8"))
 p class="mt-2 text-sm" { "未找到匹配的来源单据" }
 }
 } @else {
 div class="py-2" {
 @for o in options {
div class="flex items-center justify-between p-3 [border-bottom:1px_solid_var(--border-soft)]" {
 div class="flex-1 min-w-0" {
 div class="text-sm font-medium text-fg" { (o.doc_number) }
 div class="text-[12px] text-muted flex items-center gap-[6px] flex-wrap" {
 span { (o.supplier_name) }
 span class="text-border" { "·" }
 span { (o.extra) }
 }
 }
 button type="button" class="btn inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)] [&_svg]:w-4 [&_svg]:h-4"
 data-doc=(o.doc_number)
 data-supplier=(o.supplier_name)
 data-source-id=(o.id)
 onclick="wmsStockInPickSource(this)" {
 "选择"
 }
 }
 }
 }
 }
 }
}

/// Single item row fragment
fn item_row_fragment(product: &abt_core::master_data::product::model::Product) -> Markup {
 html! {
 tr oninput="wmsStockInCalcRow(this)" {
 td class="text-muted text-xs text-center" { }
 td class="font-mono tabular-nums" { (product.product_code) }
 td { (product.pdt_name) }
 td class="text-fg-2" class="text-sm" { (product.meta.specification) }
 td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="batch_no" placeholder="批次号" class="w-full text-[13px]" class="rounded-sm" class="px-2 py-[5px] border border-border" {} }
 td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" min="0.01" step="any" name="quantity" placeholder="0" class="w-[90px] text-right text-[13px] font-mono" class="rounded-sm" class="px-2 py-[5px] border border-border" {} }
 td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="item_bin_id" placeholder="自动" class="w-[80px] text-[13px] bg-surface" class="rounded-sm" class="px-2 py-[5px] border border-border" {} }
 td { button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center" title="删除行"
 _="on click remove closest <tr/> then call wmsStockInRenumber()" {
 (icon::x_icon("w-3.5 h-3.5"))
 } }
 input type="hidden" name="product_id" value=(product.product_id) {}
 }
 }
}

/// Multiple item rows with pre-filled quantities (from PO / arrival notice)
fn source_items_fragment(items: &[(abt_core::master_data::product::model::Product, Decimal)]) -> Markup {
 html! {
 @for (product, qty) in items {
 tr oninput="wmsStockInCalcRow(this)" {
 td class="text-muted text-xs text-center" { }
 td class="font-mono tabular-nums" { (product.product_code) }
 td { (product.pdt_name) }
 td class="text-fg-2" class="text-sm" { (product.meta.specification) }
 td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="batch_no" placeholder="批次号" class="w-full text-[13px]" class="rounded-sm" class="px-2 py-[5px] border border-border" {} }
 td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" min="0.01" step="any" name="quantity" value=(qty.to_string()) class="w-[90px] text-right text-[13px] font-mono" class="rounded-sm" class="px-2 py-[5px] border border-border" {} }
 td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="item_bin_id" placeholder="自动" class="w-[80px] text-[13px] bg-surface" class="rounded-sm" class="px-2 py-[5px] border border-border" {} }
 td { button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center" title="删除行"
 _="on click remove closest <tr/> then call wmsStockInRenumber()" {
 (icon::x_icon("w-3.5 h-3.5"))
 } }
 input type="hidden" name="product_id" value=(product.product_id) {}
 }
 }
 }
}
