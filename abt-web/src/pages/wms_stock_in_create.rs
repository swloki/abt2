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
use abt_core::shared::types::{DomainError, PageParams};
use abt_core::shared::enums::DocumentType;
use abt_core::shared::document_sequence::DocumentSequenceService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_stock_in::{StockInCreatePath, StockInListPath, StockInItemRowPath, StockInPoSearchPath};
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

 let supplier_svc = state.supplier_service();
 let suppliers = supplier_svc
 .list(&service_ctx, &mut conn, abt_core::master_data::supplier::model::SupplierQuery::default(), PageParams::new(1, 500))
 .await
 .map(|r| r.items)
 .unwrap_or_default();

 let content = stock_in_create_content(&warehouses, &all_zones, &all_bins, &suppliers, &claims.display_name);
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
 _ => return Ok(Html(source_items_fragment(&[], 0, None).into_string())),
 };

 // 取来源单号（per-item source_doc_number；采购入库多 PO 主流程）
 let source_doc: Option<String> = if params.source_type == "purchase" {
 state.purchase_order_service()
 .get(&service_ctx, &mut conn, source_id).await.ok().map(|o| o.doc_number)
 } else {
 None
 };

 // Fetch line items from the source document (product_id, 订单/申报量, 已收量)
 let items: Vec<(i64, Decimal, Decimal)> = match params.source_type.as_str() {
 "purchase" => {
 let po_svc = state.purchase_order_service();
 po_svc.list_items(&service_ctx, &mut conn, source_id)
 .await?
 .into_iter()
 .map(|it| (it.product_id, it.quantity, it.received_qty))
 .collect()
 }
 "arrival" => {
 let an_svc = state.arrival_notice_service();
 an_svc.list_items(&service_ctx, &mut conn, source_id)
 .await?
 .into_iter()
 .map(|it| (it.product_id, it.declared_qty, it.received_qty))
 .collect()
 }
 _ => Vec::new(),
 };

 // Resolve product details for each item
 let mut rows: Vec<(abt_core::master_data::product::model::Product, Decimal, Decimal)> = Vec::new();
 for (product_id, qty, received) in &items {
 match product_svc.get(&service_ctx, &mut conn, *product_id).await {
 Ok(p) => rows.push((p, *qty, *received)),
 Err(_) => continue,
 }
 }

 Ok(Html(source_items_fragment(&rows, source_id, source_doc.as_deref()).into_string()))
}

// ── PO 多选搜索（入库来源单选择）──

#[derive(Debug, Deserialize)]
pub struct PoSearchParams {
 #[serde(default, deserialize_with = "empty_as_none")]
 pub doc_number: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub product_code: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub supplier_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub status: Option<i16>,
}

/// HTMX: 多条件搜索可入库的采购订单（单号/产品编码/供应商/状态）
#[require_permission("INVENTORY", "create")]
pub async fn search_purchase_orders(
 ctx: RequestContext,
 Query(params): Query<PoSearchParams>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let po_svc = state.purchase_order_service();
 let supplier_svc = state.supplier_service();

 let query = PurchaseOrderQuery {
 doc_number: params.doc_number,
 product_code: params.product_code,
 supplier_id: params.supplier_id,
 status: params.status.and_then(abt_core::purchase::enums::PurchaseOrderStatus::from_i16),
 ..Default::default()
 };

 let result = po_svc
 .list(&service_ctx, &mut conn, query, PageParams::new(1, 50))
 .await
 .map(|r| r.items)
 .unwrap_or_default();

 let names = resolve_supplier_names_map(
 &supplier_svc, &service_ctx, &mut conn,
 result.iter().map(|o| o.supplier_id).collect(),
 ).await;

 Ok(Html(po_search_results_fragment(&result, &names).into_string()))
}

/// PO 搜索结果行（带 checkbox 多选，data-* 供 JS 读取）
fn po_search_results_fragment(
 orders: &[abt_core::purchase::order::model::PurchaseOrder],
 supplier_names: &HashMap<i64, String>,
) -> Markup {
 use abt_core::purchase::enums::PurchaseOrderStatus;
 let status_label = |s: &PurchaseOrderStatus| -> &'static str {
 match s {
 PurchaseOrderStatus::Draft => "草稿",
 PurchaseOrderStatus::PendingApproval => "待审批",
 PurchaseOrderStatus::Confirmed => "已确认",
 PurchaseOrderStatus::PartiallyReceived => "部分到货",
 PurchaseOrderStatus::Received => "已到货",
 PurchaseOrderStatus::Closed => "已关闭",
 PurchaseOrderStatus::Cancelled => "已取消",
 }
 };
 html! {
 @if orders.is_empty() {
 div class="text-center text-muted py-10" {
 (icon::link_icon("w-8 h-8"))
 p class="mt-2 text-sm" { "未找到匹配的采购订单" }
 }
 } @else {
 @for o in orders {
 @let sl = status_label(&o.status);
 @let sup = supplier_names.get(&o.supplier_id).cloned().unwrap_or_else(|| "-".into());
 label class="flex items-center gap-3 px-3 py-2 hover:bg-surface cursor-pointer border-b border-border-soft last:border-b-0 transition-colors duration-100" {
 input type="checkbox" class="po-pick-cb cursor-pointer accent-accent w-4 h-4 shrink-0"
 data-id=(o.id) data-doc=(o.doc_number) data-supplier=(sup.as_str()) data-status=(sl);
 div class="flex-1 min-w-0" {
 div class="text-sm font-medium text-fg truncate" { (o.doc_number) }
 div class="text-xs text-muted truncate" {
 (sup.as_str()) " · " (sl) " · " (o.order_date.format("%Y-%m-%d").to_string())
 }
 }
 }
 }
 }
 }
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
 /// 该物料所属来源单 ID（多 PO 场景每条自带；缺省回退全局 source_id）
 #[serde(default, deserialize_with = "empty_as_none")]
 source_id: Option<String>,
 /// 该物料所属来源单号（多 PO 场景每条自带；缺省回退全局 source_ref）
 #[serde(default, deserialize_with = "empty_as_none")]
 source_doc_number: Option<String>,
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
 // 来源：每条物料优先自带所属来源单（多 PO 场景），缺省回退全局（生产入库等）
 let item_source_id: i64 = item.source_id.as_deref()
 .and_then(|s| s.parse().ok())
 .unwrap_or(source_id);
 let item_source_doc = item.source_doc_number.clone()
 .filter(|s| !s.is_empty())
 .or_else(|| source_doc_number.clone());

 if quantity <= Decimal::ZERO {
 return Err(DomainError::validation("入库数量必须大于0").into());
 }

 let req = RecordTransactionReq {
 doc_number: Some(doc_number.clone()),
 delivery_no: form.delivery_no.clone(),
 source_doc_number: item_source_doc,
 transaction_type,
 product_id,
 warehouse_id,
 zone_id,
 bin_id: bin_id.or(form.bin_id).or(default_bin_id),
 batch_no: item.batch_no.clone(),
 quantity,
 unit_cost: None,
 source_type: source_type.to_string(),
 source_id: item_source_id,
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
 suppliers: &[abt_core::master_data::supplier::model::Supplier],
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
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "新建入库单" }
 }

 // ── Type Switch ──
 div class="flex gap-3 mb-6" {
 div class="type-btn flex-1 flex flex-col items-center gap-2 p-5 rounded-lg border cursor-pointer transition-colors border-border-soft text-muted hover:border-border [&.active]:border-accent [&.active]:bg-accent-bg [&.active]:text-accent active"
 _="on click take .active from .type-btn then put 'PurchaseReceipt' into #stockin-txn-type's value" {
 (icon::download_icon("w-7 h-7"))
 span class="text-sm font-semibold" { "采购入库" }
 span class="text-xs" { "PURCHASE_RECEIPT" br; "关联来料通知 / 采购订单" }
 }
 div class="type-btn flex-1 flex flex-col items-center gap-2 p-5 rounded-lg border cursor-pointer transition-colors border-border-soft text-muted hover:border-border [&.active]:border-accent [&.active]:bg-accent-bg [&.active]:text-accent"
 _="on click take .active from .type-btn then put 'ProductionReceipt' into #stockin-txn-type's value" {
 (icon::box_icon("w-7 h-7"))
 span class="text-sm font-semibold" { "生产入库" }
 span class="text-xs" { "PRODUCTION_RECEIPT" br; "关联工单完工报工" }
 }
 }

 form id="stockInForm" class="space-y-5" hx-post=(StockInCreatePath::PATH) hx-swap="none"
 onsubmit="return wmsStockInCollectItems()" {
 input type="hidden" id="stockin-txn-type" name="transaction_type" value="PurchaseReceipt" {};
 // ── Source Section ──
 div class="bg-bg border border-border rounded p-6" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
 (icon::link_icon("w-[18px] h-[18px]"))
 "来源关联"
 }
 div class="grid grid-cols-2 gap-4 gap-x-6" {
 div class="flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "送货单号" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="delivery_no" placeholder="一张送货单可关联多个采购订单";
 }
 div class="flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "采购订单来源" }
 button type="button" class="inline-flex items-center justify-center gap-2 px-3 py-2 rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-colors"
 _="on click add .is-open to #po-picker then call wmsStockInOpenPoPicker()" {
 (icon::plus_icon("w-4 h-4"))
 "选择采购订单"
 }
 }
 }
 div class="mt-3 text-xs text-muted" id="po-selected-hint" { "未选择采购订单；也可在下方「入库物料明细」手动添加物料（如生产入库）" }
 // 全局来源回退（per-item 未带 source_id 时使用，如手动物料）
 input type="hidden" name="source_type" value="purchase" {};
 input type="hidden" name="source_id" value="0" {};
 input type="hidden" name="source_ref" value="" {};
 }

 // ── Warehouse Section ──
 div class="bg-bg border border-border rounded p-6" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
 (icon::building_icon("w-[18px] h-[18px]"))
 "入库信息"
 }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "目标仓库 " span class="text-danger" { "*" } }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="warehouse_id" id="warehouse-select"
 onchange="wmsUpdateZones()" {
 option value="" disabled selected { "请选择仓库" }
 @for wh in warehouses {
 option value=(wh.id) { (wh.name) }
 }
 }
 }
 div class="flex flex-col" {
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
 div class="flex flex-col" {
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
 div class="flex flex-col" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "操作员" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" value=(operator_name) readonly class="bg-surface cursor-not-allowed";
 }
 }
 }

 // ── Strategy Tip ──
 div class="flex items-center rounded-md mb-6 gap-3 px-4 py-3 bg-[rgba(82,196,26,0.05)] border border-[rgba(82,196,26,0.15)]" {
 (icon::check_circle_icon("w-4 h-4 text-success shrink-0"))
 span class="text-sm text-fg-2" {
 "当前仓库上架策略："
 strong { "同物料合并 (SAME_MERGE)" }
 " — 系统将自动分配至同物料已有库位，库位满时按就近原则分配。"
 }
 }

 // ── Line Items ──
 div class="bg-bg border border-border rounded p-6" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
 (icon::box_icon("w-[18px] h-[18px]"))
 "入库物料明细"
 span id="stockin-item-count" class="ml-auto text-xs font-normal text-muted" { "共 0 项" }
 }
 // 采购订单折叠卡片容器（JS 按多选结果渲染，每个 PO 一卡，含该 PO 物料明细）
 div id="po-cards" class="space-y-3" { }
 // 手动物料表（无 PO 来源 / 生产入库 / 补充明细）
 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th class="w-10" { "序号" }
 th { "产品编码" }
 th { "产品名称" }
 th { "规格型号" }
 th { "批次号" }
 th class="w-[100px]" { "入库数量 " span class="text-danger" { "*" } }
 th { "目标库位" }
 th class="w-10" { }
 }
 }
 tbody id="stockin-item-tbody" {
 // JS-managed dynamic rows（product_picker 手动添加）
 }
 }
 }
 div class="mt-4" {
 button type="button" class="flex items-center justify-center gap-2 w-full text-accent text-sm font-medium cursor-pointer"
 _="on click add .is-open to #product-modal" {
 (icon::plus_icon("w-3.5 h-3.5"))
 "手动添加物料"
 }
 }
 }

 // ── Summary ──
 div class="bg-bg border border-border rounded p-6" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
 (icon::clipboard_list_icon("w-[18px] h-[18px]"))
 "入库汇总"
 }
 div class="grid grid-cols-3 gap-6" {
 div class="text-center bg-surface p-4 rounded-md" {
 div class="text-[11px] text-muted mb-1" { "物料种类" }
 div id="stockin-summary-kinds" class="font-mono tabular-nums font-semibold text-xl text-fg" { "0" }
 }
 div class="text-center bg-surface p-4 rounded-md" {
 div class="text-[11px] text-muted mb-1" { "入库总量" }
 div id="stockin-summary-qty" class="font-mono tabular-nums font-semibold text-xl text-fg" { "0" }
 }
 div class="text-center bg-surface p-4 rounded-md" {
 div class="text-[11px] text-muted mb-1" { "上架策略" }
 div class="font-semibold text-sm text-fg" { "同物料合并" }
 }
 }
 }

 // ── Remark ──
 div class="bg-bg border border-border rounded p-6" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
 (icon::edit_icon("w-[18px] h-[18px]"))
 "备注"
 }
 textarea class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none resize-y min-h-[80px] transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]" name="remark" placeholder="输入备注信息…" rows="3" { }
 }

 // hidden input for items JSON
 input type="hidden" name="items_json" id="stockin-items-json" value="[]" {}

 // ── Action Bar ──
 div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft" {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{}?restore=true", StockInListPath::PATH)) { "取消" }
 div class="flex gap-3" {
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" { "保存草稿" }
 button type="submit" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
 (icon::check_circle_icon("w-4 h-4"))
 "确认入库"
 }
 }
 }
 }
 }

            (crate::components::product_picker::product_picker_modal_with_search("product-modal", StockInItemRowPath::PATH, "stockin-item-tbody"))

 // ── PO 多选弹窗 ──
 div id="po-picker" class="fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto"
 _="on click[me is event.target] remove .is-open" {
 div class="modal bg-bg rounded-xl w-[780px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl" {
 div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
 h2 { "选择采购订单（可多选）" }
 button type="button" class="bg-transparent border-none cursor-pointer text-xl text-muted p-1"
 _="on click remove .is-open from #po-picker" { "×" }
 }
 div class="px-6 py-4 border-b border-border-soft shrink-0 grid grid-cols-2 gap-3" {
 div class="flex flex-col gap-1" {
 label class="text-xs font-medium text-fg-2" { "采购单号" }
 input id="po-filter-doc" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="doc_number" placeholder="单号关键词…"
 hx-get=(StockInPoSearchPath::PATH) hx-trigger="keyup changed delay:300ms" hx-sync="this:replace"
 hx-target="#po-search-results" hx-swap="innerHTML" hx-include="#po-filter-doc,#po-filter-code,#po-filter-supplier,#po-filter-status" {}
 }
 div class="flex flex-col gap-1" {
 label class="text-xs font-medium text-fg-2" { "产品编码" }
 input id="po-filter-code" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="product_code" placeholder="产品编码关键词…"
 hx-get=(StockInPoSearchPath::PATH) hx-trigger="keyup changed delay:300ms" hx-sync="this:replace"
 hx-target="#po-search-results" hx-swap="innerHTML" hx-include="#po-filter-doc,#po-filter-code,#po-filter-supplier,#po-filter-status" {}
 }
 div class="flex flex-col gap-1" {
 label class="text-xs font-medium text-fg-2" { "供应商" }
 select id="po-filter-supplier" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" name="supplier_id"
 hx-get=(StockInPoSearchPath::PATH) hx-trigger="change" hx-target="#po-search-results" hx-swap="innerHTML" hx-include="#po-filter-doc,#po-filter-code,#po-filter-supplier,#po-filter-status" {
 option value="" { "全部供应商" }
 @for s in suppliers {
 option value=(s.id) { (s.name) }
 }
 }
 }
 div class="flex flex-col gap-1" {
 label class="text-xs font-medium text-fg-2" { "状态" }
 select id="po-filter-status" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" name="status"
 hx-get=(StockInPoSearchPath::PATH) hx-trigger="change" hx-target="#po-search-results" hx-swap="innerHTML" hx-include="#po-filter-doc,#po-filter-code,#po-filter-supplier,#po-filter-status" {
 option value="" { "全部状态" }
 option value="3" { "部分到货" }
 option value="4" { "已到货" }
 option value="2" { "已确认" }
 }
 }
 }
 div id="po-search-results" class="overflow-y-auto flex-1 min-h-0"
 hx-get=(StockInPoSearchPath::PATH) hx-trigger="intersect once" hx-swap="innerHTML" hx-include="#po-filter-doc,#po-filter-code,#po-filter-supplier,#po-filter-status" {
 div class="text-center text-muted py-10" { "加载中…" }
 }
 div class="px-6 py-4 border-t border-border-soft flex items-center justify-between shrink-0" {
 span id="po-selected-count" class="text-sm text-muted" { "已选 0 个采购订单" }
 div class="flex gap-3" {
 button type="button" class="inline-flex items-center gap-2 py-2 px-4 rounded-sm bg-white text-fg-2 border border-border hover:bg-surface text-sm font-medium cursor-pointer transition-colors"
 _="on click remove .is-open from #po-picker" { "取消" }
 button type="button" class="inline-flex items-center gap-2 py-2 px-4 rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-colors"
 _="on click call wmsStockInConfirmPoPicker()" { "确认选择" }
 }
 }
 }
 }

 // ── Page-specific JS（库位级联 + PO 多选弹窗 + 明细收集）──
 script src="/wms-stock-in-create.js" {}
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
div class="flex items-center justify-between p-3 border-b border-border-soft" {
 div class="flex-1 min-w-0" {
 div class="text-sm font-medium text-fg" { (o.doc_number) }
 div class="text-xs text-muted flex items-center gap-[6px] flex-wrap" {
 span { (o.supplier_name) }
 span class="text-border" { "·" }
 span { (o.extra) }
 }
 }
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)] icon:w-4 icon:h-4"
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

const CELL_INPUT: &str =
 "w-full px-2 py-[5px] border border-border rounded-sm text-[13px] bg-white text-fg \
  outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]";

/// 手动物料行（product_picker 添加，无 PO 来源）
fn item_row_fragment(product: &abt_core::master_data::product::model::Product) -> Markup {
 manual_item_row(product)
}

/// PO 来源明细行集合（每个 PO 折叠卡片内，带订单/已收/待入库 + per-item source）
fn source_items_fragment(
 items: &[(abt_core::master_data::product::model::Product, Decimal, Decimal)],
 source_id: i64,
 source_doc: Option<&str>,
) -> Markup {
 html! {
 @for (product, order_qty, received) in items {
 @let pending = (*order_qty - received).max(Decimal::ZERO);
 (po_detail_row(product, *order_qty, *received, pending, source_id, source_doc))
 }
 }
}

/// 手动物料行（无待入库校验，库位下拉由 JS 按目标库区填充）
fn manual_item_row(product: &abt_core::master_data::product::model::Product) -> Markup {
 html! {
 tr class="item-row" oninput="wmsStockInCalcRow(this)" {
 td class="line-num text-muted text-xs text-center" { }
 td class="font-mono tabular-nums text-sm text-fg" { (product.product_code) }
 td class="text-sm text-fg" { (product.pdt_name) }
 td class="text-sm text-fg-2" { (product.meta.specification) }
 td { input class=(CELL_INPUT) type="text" name="batch_no" placeholder="批次号" {} }
 td { input class=(format!("{CELL_INPUT} w-[90px] text-right font-mono")) type="number" step="any" name="quantity" placeholder="0" {} }
 td { select class=(format!("{CELL_INPUT} row-bin-select")) name="bin_id" {} }
 td { button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:text-danger" title="删除行"
 _="on click remove closest <tr/> then call wmsStockInCalcSummary()" {
 (icon::x_icon("w-3.5 h-3.5"))
 } }
 input type="hidden" name="product_id" value=(product.product_id) {}
 }
 }
}

/// PO 明细行（带待入库余量校验，per-item source_id/source_doc_number）
fn po_detail_row(
 product: &abt_core::master_data::product::model::Product,
 order_qty: Decimal,
 received: Decimal,
 pending: Decimal,
 source_id: i64,
 source_doc: Option<&str>,
) -> Markup {
 html! {
 tr class="item-row" oninput="wmsStockInCalcRow(this)" {
 td class="line-num text-muted text-xs text-center" { }
 td {
 div class="font-mono tabular-nums text-sm text-fg" { (product.product_code) }
 div class="text-sm text-fg" { (product.pdt_name) }
 div class="text-[11px] text-muted pending-hint" data-pending=(pending.to_string()) {
 "订单 " (crate::utils::fmt_qty(order_qty)) " · 已收 " (crate::utils::fmt_qty(received)) " · 待入库 " (crate::utils::fmt_qty(pending))
 }
 }
 td { input class=(CELL_INPUT) type="text" name="batch_no" placeholder="批次号" {} }
 td {
 input class=(format!("{CELL_INPUT} w-[90px] text-right font-mono")) type="number" step="any"
 name="quantity" placeholder="0" value=(pending.to_string())
 data-pending=(pending.to_string()) oninput="wmsStockInValidateRow(this)" {}
 }
 td { select class=(format!("{CELL_INPUT} row-bin-select")) name="bin_id" {} }
 td { button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:text-danger" title="删除行"
 _="on click remove closest <tr/> then call wmsStockInCalcSummary()" {
 (icon::x_icon("w-3.5 h-3.5"))
 } }
 input type="hidden" name="product_id" value=(product.product_id) {}
 input type="hidden" name="source_id" value=(source_id) {}
 input type="hidden" name="source_doc_number" value=(source_doc.unwrap_or("")) {}
 }
 }
}

