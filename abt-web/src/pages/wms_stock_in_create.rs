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
use abt_core::wms::inventory::InventoryService;
use abt_core::mes::work_order::WorkOrderService;
use abt_core::wms::inventory_transaction::model::RecordTransactionReq;
use abt_core::wms::enums::{ArrivalStatus, TransactionType};
use abt_core::master_data::product::ProductService;
use abt_core::shared::types::{DomainError, PageParams};
use abt_core::shared::enums::DocumentType;
use abt_core::shared::document_sequence::DocumentSequenceService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_stock_in::{StockInCreatePath, StockInListPath, StockInItemRowPath, StockInConfirmPosPath, StockInConfirmWoPath};
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
 let RequestContext { claims, .. } = ctx;

 let content = stock_in_create_content();
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
 let product = state.product_service().get(&service_ctx, &mut conn, params.product_id).await?;
 let warehouses = state.warehouse_service()
 .list(&service_ctx, &mut conn, abt_core::wms::warehouse::model::WarehouseFilter::default(), 1, 200)
 .await.map(|r| r.items).unwrap_or_default();
 Ok(Html(item_row_fragment(&product, &warehouses).into_string()))
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
 let warehouses = state.warehouse_service()
 .list(&service_ctx, &mut conn, abt_core::wms::warehouse::model::WarehouseFilter::default(), 1, 200)
 .await.map(|r| r.items).unwrap_or_default();

 let source_id = match params.source_id {
 Some(id) if id > 0 => id,
 _ => return Ok(Html(source_items_fragment(&[], 0, None, &warehouses).into_string())),
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

 Ok(Html(source_items_fragment(&rows, source_id, source_doc.as_deref(), &warehouses).into_string()))
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

// ── PO 确认（多选确认 → 一次性渲染所有 PO 折叠卡片 + 明细）──

struct PoCardData {
 po_id: i64,
 doc_number: String,
 supplier_name: String,
 status_label: String,
 items: Vec<(abt_core::master_data::product::model::Product, Decimal, Decimal)>,
}

/// HTMX: 确认选中的采购订单 → 渲染所有 PO 折叠卡片（含明细行）替换 #po-cards。
/// HX-Trigger 触发 closePoPicker（关弹窗+清勾选）与 poCardsUpdated（填库位+重编号+汇总）。
#[require_permission("INVENTORY", "create")]
pub async fn confirm_purchase_orders(
 ctx: RequestContext,
 body: axum::body::Bytes,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let po_svc = state.purchase_order_service();
 let supplier_svc = state.supplier_service();

 // 解析 urlencoded body：po_id=1&po_id=423（HTMX 单值或多值都兼容；
 // 不用 axum::Form，因为 serde_urlencoded 对单值 po_id 无法反序列化为 Vec）
 let body_str = std::str::from_utf8(&body).unwrap_or("");
 let mut seen = std::collections::HashSet::new();
 let po_ids: Vec<i64> = body_str.split('&')
 .filter_map(|kv| {
 let mut it = kv.splitn(2, '=');
 let (k, v) = (it.next()?, it.next()?);
 if k == "po_id" { v.parse::<i64>().ok() } else { None }
 })
 .filter(|id| *id > 0 && seen.insert(*id))
 .collect();

 // 逐个取 PO（doc_number / supplier_id / status）
 let mut pos: Vec<abt_core::purchase::order::model::PurchaseOrder> = Vec::new();
 for id in &po_ids {
 let o = po_svc.get(&service_ctx, &mut conn, *id).await?;
 pos.push(o);
 }

 // 批量解析供应商名
 let names = resolve_supplier_names_map(
 &supplier_svc, &service_ctx, &mut conn,
 pos.iter().map(|o| o.supplier_id).collect(),
 ).await;

 // 每个 PO 加载明细 → 组装卡片数据
 let mut cards: Vec<PoCardData> = Vec::new();
 for o in &pos {
 let items = load_po_items(&state, &service_ctx, &mut conn, o.id).await?;
 cards.push(PoCardData {
 po_id: o.id,
 doc_number: o.doc_number.clone(),
 supplier_name: names.get(&o.supplier_id).cloned().unwrap_or_else(|| "-".into()),
 status_label: po_status_label(&o.status).to_string(),
 items,
 });
 }

 let warehouses = state.warehouse_service()
 .list(&service_ctx, &mut conn, abt_core::wms::warehouse::model::WarehouseFilter::default(), 1, 200)
 .await.map(|r| r.items).unwrap_or_default();
 let html = po_cards_fragment(&cards, &warehouses).into_string();
 // HX-Trigger-After-Settle：事件在 swap+settle 后触发，此时 #po-cards 已填充明细行，
 // poCardsUpdated 监听才能正确重编号/汇总/填库位（HX-Trigger 在 swap 前触发，#po-cards 尚空）
 Ok(([("HX-Trigger-After-Settle", r#"{"closePoPicker":"","poCardsUpdated":""}"#)], Html(html)))
}

// ── 工单确认（生产入库选工单 → 渲染工单明细卡片：完工产品 + 完工量-已入库）──

/// HTMX: 确认选中的工单 → 渲染工单明细卡片（完工产品 + 完工量-已入库）替换 #po-cards
#[require_permission("INVENTORY", "create")]
pub async fn confirm_work_order(
 ctx: RequestContext,
 body: axum::body::Bytes,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let wo_svc = state.work_order_service();
 let inv_svc = state.inventory_transaction_service();
 let product_svc = state.product_service();
 let warehouse_svc = state.warehouse_service();

 // 解析 work_order_id（Bytes，同 confirm-pos 兼容单值/多值）
 let body_str = std::str::from_utf8(&body).unwrap_or("");
 let wo_id: i64 = body_str.split('&')
 .filter_map(|kv| {
 let mut it = kv.splitn(2, '=');
 let (k, v) = (it.next()?, it.next()?);
 if k == "work_order_id" { v.parse().ok() } else { None }
 })
 .next()
 .ok_or_else(|| DomainError::validation("未选择工单"))?;

 let wo = wo_svc.find_by_id(&service_ctx, &mut conn, wo_id).await?;
 let product = product_svc.get(&service_ctx, &mut conn, wo.product_id).await?;
 // 已入库 = find_by_source("work_order", wo_id) 累加 quantity
 let received: Decimal = inv_svc
 .find_by_source(&service_ctx, &mut conn, "work_order", wo_id).await.unwrap_or_default()
 .iter()
 .map(|t| t.quantity)
 .sum();

 let card = PoCardData {
 po_id: wo.id,
 doc_number: wo.doc_number.clone(),
 supplier_name: product.pdt_name.clone(),
 status_label: wo_status_label(&wo.status).to_string(),
 items: vec![(product, wo.completed_qty, received)],
 };
 let warehouses = warehouse_svc
 .list(&service_ctx, &mut conn, abt_core::wms::warehouse::model::WarehouseFilter::default(), 1, 200)
 .await.map(|r| r.items).unwrap_or_default();
 let html = po_cards_fragment(&[card], &warehouses).into_string();
 Ok(([("HX-Trigger-After-Settle", r#"{"closeWoPicker":"","poCardsUpdated":""}"#)], Html(html)))
}

/// 工单状态中文标签
fn wo_status_label(s: &abt_core::mes::WorkOrderStatus) -> &'static str {
 use abt_core::mes::WorkOrderStatus;
 match s {
 WorkOrderStatus::Draft => "草稿",
 WorkOrderStatus::Planned => "已计划",
 WorkOrderStatus::Released => "已下达",
 WorkOrderStatus::InProduction => "进行中",
 WorkOrderStatus::Closed => "已关闭",
 WorkOrderStatus::Cancelled => "已取消",
 }
}

/// 加载某采购订单的明细行（product, 订单量, 已收量）—— confirm 与 get_source_items 共享
async fn load_po_items(
 state: &crate::state::AppState,
 ctx: &abt_core::shared::types::ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 po_id: i64,
) -> Result<Vec<(abt_core::master_data::product::model::Product, Decimal, Decimal)>> {
 let po_svc = state.purchase_order_service();
 let product_svc = state.product_service();
 let mut rows = Vec::new();
 for it in po_svc.list_items(ctx, db, po_id).await? {
 match product_svc.get(ctx, db, it.product_id).await {
 Ok(p) => rows.push((p, it.quantity, it.received_qty)),
 Err(_) => continue,
 }
 }
 Ok(rows)
}

// ── 库位建议（按产品+仓库，SameMerge 优先推荐同物料已有库位）──

#[derive(Debug, Deserialize)]
pub struct SuggestBinsParams {
 pub product_id: i64,
 pub warehouse_id: i64,
}

/// HTMX: 根据产品+仓库推荐入库库位。有该产品库存的库位排前（同物料合并），用户也可任选。
#[require_permission("INVENTORY", "create")]
pub async fn suggest_bins(
 ctx: RequestContext,
 Query(params): Query<SuggestBinsParams>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let warehouse_svc = state.warehouse_service();
 let inventory_svc = state.inventory_service();

 // 仓库下所有可用库位
 let bins = warehouse_svc
 .list_bins_by_warehouse(&service_ctx, &mut conn, abt_core::wms::warehouse::model::ListBinsByWarehouseParams {
 warehouse_id: params.warehouse_id,
 keyword: None,
 is_active: Some(true),
 page: 1,
 page_size: 500,
 })
 .await
 .map(|r| r.items)
 .unwrap_or_default();

 // 产品在该仓库的库存分布（bin_id → 现有量）
 let inv = inventory_svc.get_by_product(&service_ctx, &mut conn, params.product_id).await.unwrap_or_default();
 let mut stock_by_bin: std::collections::HashMap<i64, Decimal> = std::collections::HashMap::new();
 for v in inv.into_iter().filter(|v| v.warehouse_id == params.warehouse_id) {
 *stock_by_bin.entry(v.bin_id).or_insert(Decimal::ZERO) += v.quantity;
 }

 // 组装（bin, 现有库存），有库存的排前（SameMerge 推荐）
 let mut rows: Vec<(abt_core::wms::warehouse::model::Bin, Option<Decimal>)> = bins
 .into_iter()
 .map(|b| {
 let id = b.id;
 (b, stock_by_bin.get(&id).copied().filter(|q| *q > Decimal::ZERO))
 })
 .collect();
 rows.sort_by_key(|(_, q)| q.is_none());

 Ok(Html(suggest_bins_fragment(&rows).into_string()))
}

/// 库位建议列表（每个 bin 一个按钮，点击 wmsStockInPickBin 填回当前行）
fn suggest_bins_fragment(rows: &[(abt_core::wms::warehouse::model::Bin, Option<Decimal>)]) -> Markup {
 html! {
    @if rows.is_empty() {
        div class="text-center text-muted py-10" {
            (icon::link_icon("w-8 h-8"))
            p class="mt-2 text-sm" { "该仓库暂无可用库位" }
            p class="text-xs mt-1" { "请先在仓库管理中创建库位，或选择其他仓库" }
        }
    } @else {
        @for (bin, qty) in rows {
            @let suggested = qty.is_some();
            button
                type="button"
                class=({
                    format!(
                        "w-full flex items-center justify-between gap-3 px-4 py-3 border-b border-border-soft last:border-b-0 text-left transition-colors {}",
                        if suggested {
                            "bg-accent-bg/40 hover:bg-accent-bg"
                        } else {
                            "hover:bg-surface"
                        },
                    )
                })
                data-bin-id=(bin.id)
                data-bin-label=(format!("{} {}", bin.code, bin.name))
                _="on click call wmsStockInPickBin(@data-bin-id, @data-bin-label)"
            {
                div class="flex-1 min-w-0" {
                    div class="text-sm font-medium text-fg truncate" { (bin.code) " " (bin.name) }
                    @if let Some(q) = qty {
                        div class="text-xs text-success flex items-center gap-1 mt-0.5" {
                            (icon::check_circle_icon("w-3 h-3"))
                            "已有该物料库存 "
                            (crate::utils::fmt_qty(*q))
                            " · 推荐同物料合并"
                        }
                    } @else {
                        div class="text-xs text-muted mt-0.5" { "空库位" }
                    }
                }
            }
        }
    }
}
}

/// PO 搜索结果行（带 checkbox 多选，data-* 供 JS 读取）
fn po_search_results_fragment(
 orders: &[abt_core::purchase::order::model::PurchaseOrder],
 supplier_names: &HashMap<i64, String>,
) -> Markup {
 html! {
    @if orders.is_empty() {
        div class="text-center text-muted py-10" {
            (icon::link_icon("w-8 h-8"))
            p class="mt-2 text-sm" { "未找到匹配的采购订单" }
        }
    } @else {
        @for o in orders {
            @let sl = po_status_label(&o.status);
            @let sup = supplier_names
                .get(&o.supplier_id)
                .cloned()
                .unwrap_or_else(|| "-".into());
            label
                class="flex items-center gap-3 px-3 py-2 hover:bg-surface cursor-pointer border-b border-border-soft last:border-b-0 transition-colors duration-100"
            {
                input
                    type="checkbox"
                    name="po_id"
                    value=(o.id)
                    class="po-pick-cb cursor-pointer accent-accent w-4 h-4 shrink-0"
                    data-id=(o.id)
                    data-doc=(o.doc_number)
                    data-supplier=(sup.as_str())
                    data-status=(sl);
                div class="flex-1 min-w-0" {
                    div class="text-sm font-medium text-fg truncate" { (o.doc_number) }
                    div class="text-xs text-muted truncate" {
                        (sup.as_str())
                        " · "
                        (sl)
                        " · "
                        (o.order_date.format("%Y-%m-%d").to_string())
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
 pub delivery_no: Option<String>,
 pub remark: Option<String>,
 pub items_json: String,
}

#[derive(Debug, Deserialize)]
struct StockInItemWeb {
 product_id: String,
 quantity: String,
 bin_id: Option<String>,
 /// 该行物料的目标仓库（每行独立）
 #[serde(default, deserialize_with = "empty_as_none")]
 warehouse_id: Option<String>,
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
 // 手动物料无来源单 → 0；有来源的明细行自带 per-item source_id（PO/工单）
 let source_id: i64 = 0;
 // 入库单号：通过 DocumentSequenceService 生成规范编号（RK-YYYY-MM-SEQ）
 let doc_number = state.document_sequence_service()
 .next_number(&service_ctx, &mut conn, DocumentType::StockReceipt)
 .await?;
 // 来源单号：记录来源单据的单号（如采购单号 PO-xxx、来料通知单号 AN-xxx）
 let source_doc_number = form.source_ref
 .as_ref()
 .filter(|s| !s.is_empty())
 .cloned();

 let warehouse_svc = state.warehouse_service();

 // 每行物料独立仓库 + 库位，逐条记录一笔库存事务
 for item in &web_items {
 let product_id: i64 = item.product_id.parse()
 .map_err(|_| DomainError::validation("无效产品ID"))?;
 let quantity: Decimal = item.quantity.parse()
 .map_err(|_| DomainError::validation("无效数量"))?;
 let bin_id: Option<i64> = item.bin_id.as_ref()
 .and_then(|s| s.parse().ok());
 // 每行目标仓库（必填）；缺省库区时按该仓库自动取默认库区
 let warehouse_id: i64 = item.warehouse_id.as_deref()
 .and_then(|s| s.parse().ok())
 .ok_or_else(|| DomainError::validation("请为每行物料选择目标仓库"))?;
 let zone_id = warehouse_svc
 .get_or_create_default_zone(&service_ctx, &mut conn, warehouse_id)
 .await
 .ok()
 .map(|z| z.id);
 let default_bin_id: Option<i64> = if let Some(zid) = zone_id {
 warehouse_svc
 .list_bins(&service_ctx, &mut conn, zid, None, 1, 1)
 .await
 .ok()
 .and_then(|r| r.items.first().map(|b| b.id))
 } else {
 None
 };
 // 来源：每条物料优先自带所属来源单（多 PO 场景），缺省回退全局（生产入库等）
 let item_source_id: i64 = item.source_id.as_deref()
 .and_then(|s| s.parse().ok())
 .unwrap_or(source_id);
 // 关联单号必填：每行物料必须有来源单号（PO 单号 / 工单号），手动物料手填关联单号
 let item_source_doc = item.source_doc_number.clone()
 .filter(|s| !s.is_empty())
 .or_else(|| source_doc_number.clone())
 .ok_or_else(|| DomainError::validation("每行物料必须填写关联单号"))?;

 if quantity <= Decimal::ZERO {
 return Err(DomainError::validation("入库数量必须大于0").into());
 }

 let req = RecordTransactionReq {
 doc_number: Some(doc_number.clone()),
 delivery_no: form.delivery_no.clone(),
 source_doc_number: Some(item_source_doc),
 transaction_type,
 product_id,
 warehouse_id,
 zone_id,
 bin_id: bin_id.or(default_bin_id),
 batch_no: None,
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

fn stock_in_create_content() -> Markup {
 html! {
    div {
        // ── Back Link ──
        a   href="/admin/wms/stock-in"
            class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
        { (icon::chevron_left_icon("w-4 h-4")) "返回入库列表" }
        // ── Page Header ──
        div class="flex items-center justify-between mb-6" {
            h1 class="text-xl font-bold text-fg tracking-tight" { "新建入库单" }
        }

        form
            id="stockInForm"
            class="space-y-3"
            hx-post=(StockInCreatePath::PATH)
            hx-swap="none"
            onsubmit="return wmsStockInCollectItems()"
        {
            // ── Strategy Tip ──
            div class="flex items-center rounded-md mb-6 gap-3 px-4 py-3 bg-[rgba(82,196,26,0.05)] border border-[rgba(82,196,26,0.15)]"
            {
                (icon::check_circle_icon("w-4 h-4 text-success shrink-0"))
                span class="text-sm text-fg-2" {
                    "当前仓库上架策略："
                    strong { "同物料合并 (SAME_MERGE)" }
                    " — 系统将自动分配至同物料已有库位，库位满时按就近原则分配。"
                }
            }
            // ── 来源关联与入库明细（合并 card：选 PO → 同区立即出明细）──
            div class="bg-bg border border-border rounded p-4" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-3 pb-2 border-b border-border-soft"
                { (icon::link_icon("w-[18px] h-[18px]")) "来源关联与入库明细" }
                // 子区1：来源选择
                div {
                    div class="text-xs font-medium text-fg-2 mb-2" { "来源选择" }
                    div class="grid grid-cols-2 gap-3 gap-x-4" {
                        div class="flex flex-col" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            {
                                "入库类型 "
                                span class="text-danger" { "*" }
                            }
                            select
                                id="txn-type"
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                                name="transaction_type"
                                required
                                _="on change call wmsStockInToggleSourceBtn()"
                            {
                                option value="" disabled selected { "选择入库类型" }
                                option value="PurchaseReceipt" { "采购入库" }
                                option value="ProductionReceipt" { "生产入库" }
                            }
                        }
                        div id="delivery-no-field" class="flex flex-col" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            { "送货单号" }
                            input
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                                type="text"
                                name="delivery_no"
                                placeholder="送货单号";
                        }
                    }
                    // 工单来源 hidden：选工单后 fill value + trigger change → HTMX POST confirm-wo 渲染工单明细
                    input
                        type="hidden"
                        id="wo-id-hidden"
                        name="work_order_id"
                        value=""
                        hx-post=(StockInConfirmWoPath::PATH)
                        hx-trigger="change"
                        hx-target="#po-cards"
                        hx-swap="innerHTML" {}
                    ;
                    input type="hidden" id="wo-display" value="" {}
                    ;
                    div class="mt-2" {
                        // 入库类型联动：采购入库→选择采购订单 / 生产入库→选择生产工单（两按钮切换显隐）
                        button
                            id="po-btn"
                            type="button"
                            class="inline-flex items-center justify-center gap-2 w-full px-3 py-2 rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-colors"
                            _="on click add .is-open to #po-picker"
                        { (icon::plus_icon("w-4 h-4")) "选择采购订单" }
                        button
                            id="wo-btn"
                            type="button"
                            class="hidden inline-flex items-center justify-center gap-2 w-full px-3 py-2 rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-colors"
                            _="on click add .is-open to #wo-picker-modal"
                        { (icon::plus_icon("w-4 h-4")) "选择生产工单" }
                    }
                    div class="mt-3 text-xs text-muted" id="po-selected-hint" {
                        "未选择采购订单；也可在下方手动添加物料（如生产入库）"
                    }
                    // 全局来源：source_type 固定采购；source_ref 为来源单号全局兜底（多 PO 场景每行自带 source_id/source_doc_number）
                    input type="hidden" name="source_type" value="purchase" {}
                    ;
                    input type="hidden" name="source_ref" value="" {}
                    ;
                }
                // 分隔线
                div class="border-t border-border-soft my-5" {}
                // 子区2：入库明细
                div {
                    div class="flex items-center gap-2 mb-3" {
                        span class="text-xs font-medium text-fg-2" { "入库明细" }
                        span
                            id="stockin-item-count"
                            class="ml-auto text-xs font-normal text-muted"
                        { "共 0 项" }
                    }
                    // 采购订单折叠卡片容器（confirm 端点渲染，每个 PO 一卡含明细；poCardsUpdated 事件由 JS 原生监听填库位+汇总）
                    div id="po-cards" class="space-y-3" {}
                    // 手动物料表（无 PO 来源 / 生产入库 / 补充明细）
                    div class="overflow-x-auto" {
                        table class="data-table" {
                            thead {
                                tr {
                                    th class="w-10" { "序号" }
                                    th { "产品编码" }
                                    th { "产品名称" }
                                    th class="w-[140px]" {
                                        "关联单号 "
                                        span class="text-danger" { "*" }
                                    }
                                    th class="w-[180px]" {
                                        "目标仓库 "
                                        span class="text-danger" { "*" }
                                    }
                                    th class="w-[130px]" {
                                        "入库数量 "
                                        span class="text-danger" { "*" }
                                    }
                                    th class="w-[180px]" { "目标库位" }
                                    th class="w-10" {}
                                }
                            }
                            tbody id="stockin-item-tbody" {
                                // JS-managed dynamic rows（product_picker 手动添加）
                            }
                        }
                    }
                    div class="mt-4" {
                        button
                            type="button"
                            class="flex items-center justify-center gap-2 w-full text-accent text-sm font-medium cursor-pointer"
                            _="on click add .is-open to #product-modal"
                        { (icon::plus_icon("w-3.5 h-3.5")) "手动添加物料" }
                    }
                }
            }
            // ── Summary ──
            div class="bg-bg border border-border rounded p-4" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-3 pb-2 border-b border-border-soft"
                { (icon::clipboard_list_icon("w-[18px] h-[18px]")) "入库汇总" }
                div class="grid grid-cols-3 gap-6" {
                    div class="text-center bg-surface p-4 rounded-md" {
                        div class="text-[11px] text-muted mb-1" { "物料种类" }
                        div id="stockin-summary-kinds"
                            class="font-mono tabular-nums font-semibold text-xl text-fg"
                        { "0" }
                    }
                    div class="text-center bg-surface p-4 rounded-md" {
                        div class="text-[11px] text-muted mb-1" { "入库总量" }
                        div id="stockin-summary-qty"
                            class="font-mono tabular-nums font-semibold text-xl text-fg"
                        { "0" }
                    }
                    div class="text-center bg-surface p-4 rounded-md" {
                        div class="text-[11px] text-muted mb-1" { "上架策略" }
                        div class="font-semibold text-sm text-fg" { "同物料合并" }
                    }
                }
            }
            // ── Remark ──
            div class="bg-bg border border-border rounded p-4" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-3 pb-2 border-b border-border-soft"
                { (icon::edit_icon("w-[18px] h-[18px]")) "备注" }
                textarea
                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none resize-y min-h-[80px] transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                    name="remark"
                    placeholder="输入备注信息…"
                    rows="3" {}
            }
            // hidden input for items JSON
            input type="hidden" name="items_json" id="stockin-items-json" value="[]" {}
            // ── Action Bar ──
            div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft"
            {
                a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    href=(format!("{}?restore=true", StockInListPath::PATH))
                { "取消" }
                div class="flex gap-3" {
                    button
                        type="button"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    { "保存草稿" }
                    button
                        type="submit"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                    { (icon::check_circle_icon("w-4 h-4")) "确认入库" }
                }
            }
        }
    }

    ({
        crate::components::product_picker::product_picker_modal_with_search(
            "product-modal",
            StockInItemRowPath::PATH,
            "stockin-item-tbody",
        )
    })
    // ── 采购订单多选弹窗（可复用组件，采购入库用）──
    ({
        crate::components::purchase_order_picker::purchase_order_picker_modal(
            "po-picker",
            StockInConfirmPosPath::PATH,
        )
    })
    // ── 工单选择弹窗（可复用组件，生产入库用；选中 fill #wo-id-hidden + trigger change → HTMX POST confirm-wo）──
    ({
        crate::components::work_order_picker::work_order_picker_modal(
            "wo-picker-modal",
            "wo-id-hidden",
            "wo-display",
        )
    })
    // ── 库位选择弹窗（按产品+上架策略 SameMerge 推荐，由 wmsStockInOpenBinPicker 触发）──
    div id="bin-picker"
        class="fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto"
        _="on click[me is event.target] remove .is-open"
    {
        div class="modal bg-bg rounded-xl w-[520px] max-h-[80vh] flex flex-col overflow-hidden shadow-xl"
        {
            div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0"
            {
                h2 { "选择入库库位" }
                button
                    type="button"
                    class="bg-transparent border-none cursor-pointer text-xl text-muted p-1"
                    _="on click remove .is-open from #bin-picker"
                { "×" }
            }
            div id="bin-picker-results" class="overflow-y-auto flex-1 min-h-0" {
                div class="text-center text-muted py-10 text-sm" { "点击物料行的「自动分配」加载推荐库位…" }
            }
        }
    }
    // ── Page-specific JS（库位级联 + PO 多选 + 库位选择 + 明细收集）──
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
                    button
                        type="button"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)] icon:w-4 icon:h-4"
                        data-doc=(o.doc_number)
                        data-supplier=(o.supplier_name)
                        data-source-id=(o.id)
                        onclick="wmsStockInPickSource(this)"
                    { "选择" }
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
fn item_row_fragment(
 product: &abt_core::master_data::product::model::Product,
 warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
 manual_item_row(product, warehouses)
}

/// PO 来源明细行集合（每个 PO 折叠卡片内，带订单/已收/待入库 + per-item source）
fn source_items_fragment(
 items: &[(abt_core::master_data::product::model::Product, Decimal, Decimal)],
 source_id: i64,
 source_doc: Option<&str>,
 warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
 html! {
    @for (product, order_qty, received) in items {
        @let pending = (*order_qty - received).max(Decimal::ZERO);
        ({
            po_detail_row(
                product,
                *order_qty,
                *received,
                pending,
                source_id,
                source_doc,
                warehouses,
            )
        })
    }
}
}

/// 采购订单状态中文标签（搜索结果与 PO 卡片复用）
fn po_status_label(s: &abt_core::purchase::enums::PurchaseOrderStatus) -> &'static str {
 use abt_core::purchase::enums::PurchaseOrderStatus;
 match s {
 PurchaseOrderStatus::Draft => "草稿",
 PurchaseOrderStatus::PendingApproval => "待审批",
 PurchaseOrderStatus::Confirmed => "已确认",
 PurchaseOrderStatus::PartiallyReceived => "部分到货",
 PurchaseOrderStatus::Received => "已到货",
 PurchaseOrderStatus::Closed => "已关闭",
 PurchaseOrderStatus::Cancelled => "已取消",
 }
}

/// 所有 PO 折叠卡片集合（confirm 端点返回，替换 #po-cards）
fn po_cards_fragment(
 cards: &[PoCardData],
 warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
 html! {
    @for c in cards { (po_card_fragment(c, warehouses)) }
    @if cards.is_empty() {
        div class="text-center text-muted py-6 text-sm" { "未选择采购订单；可在下方手动添加物料" }
    }
}
}

/// 单个 PO 折叠卡片（后端渲染，替代 JS 拼 DOM）：header 可折叠 + body 含明细表（复用 po_detail_row）
fn po_card_fragment(
 card: &PoCardData,
 warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
 html! {
    div class="po-card bg-surface border border-border-soft rounded-md [&.is-collapsed_.po-card-body]:hidden [&.is-collapsed_.po-toggle]:-rotate-90"
        data-po-id=(card.po_id)
    {
        div class="po-card-header flex items-center gap-3 px-4 py-3 border-b border-border-soft cursor-pointer hover:bg-surface/60"
            _="on click[not (event.target matches <button/>)] toggle .is-collapsed on closest .po-card"
        {
            span
                class="po-toggle text-muted text-xs transition-transform duration-150 inline-block"
            { "▼" }
            span class="text-sm font-semibold text-fg" { (card.doc_number) }
            span class="text-xs text-muted" { (card.supplier_name) " · " (card.status_label) }
            button
                type="button"
                class="ml-auto text-xs text-muted hover:text-danger"
                _="on click remove closest .po-card then trigger poCardsUpdated on body"
            { "删除" }
        }
        div class="po-card-body p-3 overflow-x-auto" {
            table class="data-table" {
                thead {
                    tr {
                        th class="w-10" { "序号" }
                        th { "产品" }
                        th class="w-[180px]" {
                            "目标仓库 "
                            span class="text-danger" { "*" }
                        }
                        th class="w-[130px]" {
                            "入库数量 "
                            span class="text-danger" { "*" }
                        }
                        th class="w-[180px]" { "目标库位" }
                        th class="w-10" {}
                    }
                }
                tbody {
                    @for (product, order_qty, received) in &card.items {
                        @let pending = (*order_qty - *received).max(Decimal::ZERO);
                        ({
                            po_detail_row(
                                product,
                                *order_qty,
                                *received,
                                pending,
                                card.po_id,
                                Some(card.doc_number.as_str()),
                                warehouses,
                            )
                        })
                    }
                }
            }
        }
    }
}
}

/// 手动物料行（每行独立选仓库 + 库位）
fn manual_item_row(
 product: &abt_core::master_data::product::model::Product,
 warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
 html! {
    tr class="item-row" oninput="wmsStockInCalcRow(this)" {
        td class="line-num text-muted text-xs text-center" {}
        td class="font-mono tabular-nums text-sm text-fg" { (product.product_code) }
        td class="text-sm text-fg" {
            div class="truncate max-w-[200px]" title=(product.pdt_name) { (product.pdt_name) }
        }
        td {
            input
                class=(CELL_INPUT)
                type="text"
                name="source_doc_number"
                placeholder="关联单号"
                required {}
        }
        td {
            select class=(format!("{CELL_INPUT} row-wh-select")) name="warehouse_id" required {
                option value="" disabled selected { "选择仓库" }
                @for wh in warehouses {
                    option value=(wh.id) { (wh.name) }
                }
            }
        }
        td {
            input
                class=(format!("{CELL_INPUT} w-[90px] text-right font-mono"))
                type="number"
                step="any"
                name="quantity"
                placeholder="0" {}
        }
        td {
            input type="hidden" name="bin_id" {}
            ;
            button
                type="button"
                class="bin-picker-btn w-full px-2 py-[5px] border border-border rounded-sm text-[13px] bg-white text-fg-2 hover:border-accent hover:text-accent transition-colors text-left truncate"
                _="on click call wmsStockInOpenBinPicker(me)"
            {
                span class="bin-label" { "自动分配" }
            }
        }
        td {
            button
                type="button"
                class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:text-danger"
                title="删除行"
                _="on click remove closest <tr/> then call wmsStockInCalcSummary()"
            { (icon::x_icon("w-3.5 h-3.5")) }
        }
        input type="hidden" name="product_id" value=(product.product_id) {}
    }
}
}

/// PO 明细行（带待入库余量校验，per-item source + 每行独立选仓库/库位）
fn po_detail_row(
 product: &abt_core::master_data::product::model::Product,
 order_qty: Decimal,
 received: Decimal,
 pending: Decimal,
 source_id: i64,
 source_doc: Option<&str>,
 warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
 html! {
    tr class="item-row" oninput="wmsStockInCalcRow(this)" {
        td class="line-num text-muted text-xs text-center" {}
        td {
            div class="font-mono tabular-nums text-sm text-fg" { (product.product_code) }
            div class="text-sm text-fg truncate max-w-[200px]" title=(product.pdt_name) {
                (product.pdt_name)
            }
            div class="text-[11px] text-muted pending-hint" data-pending=(pending.to_string()) {
                "订单 "
                (crate::utils::fmt_qty(order_qty))
                " · 已收 "
                (crate::utils::fmt_qty(received))
                " · 待入库 "
                (crate::utils::fmt_qty(pending))
            }
        }
        td {
            select class=(format!("{CELL_INPUT} row-wh-select")) name="warehouse_id" required {
                option value="" disabled selected { "选择仓库" }
                @for wh in warehouses {
                    option value=(wh.id) { (wh.name) }
                }
            }
        }
        td {
            input
                class=(format!("{CELL_INPUT} w-[90px] text-right font-mono"))
                type="number"
                step="any"
                name="quantity"
                placeholder="0"
                value=(pending.to_string())
                data-pending=(pending.to_string())
                oninput="wmsStockInValidateRow(this)" {}
        }
        td {
            input type="hidden" name="bin_id" {}
            ;
            button
                type="button"
                class="bin-picker-btn w-full px-2 py-[5px] border border-border rounded-sm text-[13px] bg-white text-fg-2 hover:border-accent hover:text-accent transition-colors text-left truncate"
                _="on click call wmsStockInOpenBinPicker(me)"
            {
                span class="bin-label" { "自动分配" }
            }
        }
        td {
            button
                type="button"
                class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:text-danger"
                title="删除行"
                _="on click remove closest <tr/> then call wmsStockInCalcSummary()"
            { (icon::x_icon("w-3.5 h-3.5")) }
        }
        input type="hidden" name="product_id" value=(product.product_id) {}
        input type="hidden" name="source_id" value=(source_id) {}
        input type="hidden" name="source_doc_number" value=(source_doc.unwrap_or("")) {}
    }
}
}

