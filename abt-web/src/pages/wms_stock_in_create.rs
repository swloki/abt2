use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use rust_decimal::Decimal;
use std::collections::{BTreeMap, HashMap};

use abt_core::shared::idempotency::IdempotencyService;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::purchase::order::model::PurchaseOrderQuery;
use abt_core::master_data::supplier::SupplierService;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::wms::inventory_transaction::InventoryTransactionService;
use abt_core::mes::work_order::WorkOrderService;
use abt_core::wms::inventory_transaction::model::RecordTransactionReq;
use abt_core::wms::picking::{model::{PoReceiveRow, ReceivePurchaseReq}, PickingService};
use abt_core::wms::enums::TransactionType;
use abt_core::master_data::product::ProductService;
use abt_core::shared::types::{context::ServiceContext, DomainError, PageParams, PgExecutor};
use abt_core::shared::enums::DocumentType;
use abt_core::shared::document_sequence::DocumentSequenceService;

use crate::components::icon;
use crate::components::bin_search::warehouse_bin_cell;
use crate::errors::Result;
use crate::state::AppState;
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
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let warehouses = state.warehouse_service()
  .list(&service_ctx, &mut conn, abt_core::wms::warehouse::model::WarehouseFilter::default(), 1, 200)
  .await.map(|r| r.items).unwrap_or_default();

 let content = stock_in_create_content(StockInCreatePath::PATH, "", true, &warehouses, true);
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
 Query(_params): Query<SourcePickParams>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let supplier_svc = state.supplier_service();

 let options: Vec<SourceOption> = {
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
 pub posting_date: Option<String>,
pub remark: Option<String>,
 pub items_json: String,
 pub idempotency_key: String,
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

/// 提取的业务逻辑（tx + 幂等 + 来料通知/直接入库闭环），供独立页 POST 与作业中心 drawer POST 共用。
/// **PO 收货闭环编排（handle_purchase_stock_in）原样保留，不改一行**（CLAUDE.md「业务闭环单据不可绕过」）。
pub async fn do_create_stock_in(
    state: &crate::state::AppState,
    service_ctx: &abt_core::shared::types::ServiceContext,
    form: StockInCreateForm,
) -> Result<()> {
    let web_items: Vec<StockInItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("无效物料数据: {e}")))?;
    if web_items.is_empty() {
        return Err(DomainError::validation("请至少添加一个物料").into());
    }
    let transaction_type = match form.transaction_type.as_str() {
        "ProductionReceipt" => TransactionType::ProductionReceipt,
        _ => TransactionType::PurchaseReceipt,
    };
   let remark = form.remark.clone().filter(|s| !s.is_empty());
    // posting_date 合并进 remark（暂存方案：后续 inventory_transactions 加列后改为独立字段）
    let remark = if let Some(date) = form.posting_date.as_ref().filter(|s| !s.is_empty()) {
        let user_remark = remark.as_deref().unwrap_or("");
        if user_remark.is_empty() {
            Some(format!("入库日期: {date}"))
        } else {
            Some(format!("入库日期: {date} | {user_remark}"))
        }
    } else {
        remark
    };
   // 多步写（来料通知编排 + 库存入库）必须事务包裹：半失败整体回滚，避免「库存已入但 PO 回写/台账未动」残留
    let mut tx = state.pool.begin().await.map_err(|e| DomainError::Internal(e.into()))?;
    // 幂等防护：try_claim 在事务内，业务失败回滚则记录也回滚（允许重试）
    if !state.idempotency_service().try_claim(service_ctx, &mut tx, &form.idempotency_key).await? {
        return Ok(());  // 幂等命中：tx drop rollback，claim 回滚
    }
    if form.source_type == "purchase" {
        // 采购入库走「来料通知」回写闭环（ArrivalAcceptedHandler 回写 PO received_qty/状态 + 立应付台账）
        handle_purchase_stock_in(state, service_ctx, &mut tx, &web_items, transaction_type, &form, remark.as_deref()).await?;
    } else {
        // arrival/work_order/manual：直接 record() 入库
        handle_direct_stock_in(state, service_ctx, &mut tx, &web_items, transaction_type, &form, remark.as_deref()).await?;
    }
    tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

#[require_permission("INVENTORY", "create")]
pub async fn create_stock_in(
 _path: StockInCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<StockInCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;
 do_create_stock_in(&state, &service_ctx, form).await?;
 let redirect = StockInListPath.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

/// arrival/work_order/manual 来源：保持原有直接 record() 入库逻辑（仅改为事务内执行）。
async fn handle_direct_stock_in(
 state: &AppState,
 ctx: &ServiceContext,
 db: PgExecutor<'_>,
 web_items: &[StockInItemWeb],
 transaction_type: TransactionType,
 form: &StockInCreateForm,
 remark: Option<&str>,
) -> Result<()> {
 let inv_svc = state.inventory_transaction_service();
 let warehouse_svc = state.warehouse_service();
 let source_type = match form.source_type.as_str() {
  "arrival" => "arrival_notice",
  "purchase" => "purchase_order",
  other => other,
 };
 // 入库单号：RK-YYYY-MM-SEQ
 let doc_number = state
  .document_sequence_service()
  .next_number(ctx, db, DocumentType::StockReceipt)
  .await?;
 let source_doc = form.source_ref.as_ref().filter(|s| !s.is_empty()).cloned();

 for item in web_items {
  // 来源：每条物料优先自带所属来源单（多 PO 场景），缺省回退全局（生产/手动物料）
  let item_source_id: i64 = item.source_id.as_deref().and_then(|s| s.parse().ok()).unwrap_or(0);
  let item_source_doc = item
   .source_doc_number
   .clone()
   .filter(|s| !s.is_empty())
   .or_else(|| source_doc.clone())
   .ok_or_else(|| DomainError::validation("每行物料必须填写关联单号"))?;

  record_stock_in_item(
   &inv_svc, &warehouse_svc, ctx, db, item, transaction_type, &doc_number, &form.delivery_no,
   source_type, item_source_id, &item_source_doc, remark,
  )
  .await?;
 }
 Ok(())
}

/// 采购入库闭环：按 PO 分组，每组建来料通知 + 收货 + 检验通过（触发 ArrivalAcceptedHandler
/// 回写 PO received_qty/状态 + 立应付台账），再逐行 record 库存（source 关联来料通知）。
async fn handle_purchase_stock_in(
 state: &AppState,
 ctx: &ServiceContext,
 db: PgExecutor<'_>,
 web_items: &[StockInItemWeb],
 _transaction_type: TransactionType,
 form: &StockInCreateForm,
 remark: Option<&str>,
) -> Result<()> {
 // 1. 按 PO id 分组
 let mut groups: BTreeMap<i64, Vec<&StockInItemWeb>> = BTreeMap::new();
 for item in web_items {
  let po_id: i64 = item
   .source_id
   .as_deref()
   .and_then(|s| s.parse().ok())
   .ok_or_else(|| DomainError::validation("采购入库每行必须指定来源采购订单"))?;
  groups.entry(po_id).or_default().push(item);
 }

 // 2. 每个 PO：调 PurchaseStockInService 直收入库闭环（record→回写PO received_qty/状态→立应付→成本）。
 //    幂等由上层 create_stock_in 的 try_claim(form.idempotency_key) 保证，service 内传 None 跳过。
 let svc = state.picking_service();
 for (po_id, items) in &groups {
  let po_rows: Vec<PoReceiveRow> = items
   .iter()
   .map(|it| -> Result<PoReceiveRow> {
    Ok(PoReceiveRow {
     order_item_id: 0, // service 内按 product_id 解析（stock-in/create 多 PO 前端只传 product_id）
     product_id: it.product_id.parse().map_err(|_| DomainError::validation("无效产品ID"))?,
     received_qty: it.quantity.parse().map_err(|_| DomainError::validation("无效数量"))?,
     batch_no: None,
     warehouse_id: it
      .warehouse_id
      .as_deref()
      .and_then(|s| s.parse().ok())
      .ok_or_else(|| DomainError::validation("请为每行物料选择目标仓库"))?,
     bin_id: it.bin_id.as_ref().and_then(|s| s.parse().ok()),
    })
   })
   .collect::<Result<Vec<_>>>()?;

  svc.receive_purchase(
   ctx,
   db,
   ReceivePurchaseReq {
    po_id: *po_id,
    rows: po_rows,
    delivery_note: form.delivery_no.clone(),
    remark: remark.map(|s| s.to_string()),
    idempotency_key: None,
   },
  )
  .await?;
 }
 Ok(())
}

/// 记一笔入库库存事务（标量核心）。record_stock_in_item 与 stock_in_from_notice 共享。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn record_stock_in_txn(
 inv_svc: &impl InventoryTransactionService,
 warehouse_svc: &impl WarehouseService,
 ctx: &ServiceContext,
 db: PgExecutor<'_>,
 product_id: i64,
 quantity: Decimal,
 warehouse_id: i64,
 bin_id: Option<i64>,
 transaction_type: TransactionType,
 doc_number: &str,
 delivery_no: &Option<String>,
 source_type: &str,
 source_id: i64,
 source_doc: &str,
 remark: Option<&str>,
) -> Result<()> {
 if quantity <= Decimal::ZERO {
  return Err(DomainError::validation("入库数量必须大于0").into());
 }
 let zone_id = warehouse_svc.get_or_create_default_zone(ctx, db, warehouse_id).await.ok().map(|z| z.id);
 let default_bin_id: Option<i64> = if let Some(zid) = zone_id {
  warehouse_svc
   .list_bins(ctx, db, zid, None, 1, 1)
   .await
   .ok()
   .and_then(|r| r.items.first().map(|b| b.id))
 } else {
  None
 };

 inv_svc
  .record(
   ctx, db,
   RecordTransactionReq {
    doc_number: Some(doc_number.to_string()),
    delivery_no: delivery_no.clone(),
    source_doc_number: Some(source_doc.to_string()),
    transaction_type,
    product_id,
    warehouse_id,
    zone_id,
    bin_id: bin_id.or(default_bin_id),
    batch_no: None,
    quantity,
    unit_cost: None,
    source_type: source_type.to_string(),
    source_id,
    remark: remark.map(|s| s.to_string()),
   },
  )
  .await?;
 Ok(())
}

/// 解析单行 web 物料（StockInItemWeb）→ 调 record_stock_in_txn。handle_direct_stock_in 用。
#[allow(clippy::too_many_arguments)]
async fn record_stock_in_item(
 inv_svc: &impl InventoryTransactionService,
 warehouse_svc: &impl WarehouseService,
 ctx: &ServiceContext,
 db: PgExecutor<'_>,
 item: &StockInItemWeb,
 transaction_type: TransactionType,
 doc_number: &str,
 delivery_no: &Option<String>,
 source_type: &str,
 source_id: i64,
 source_doc: &str,
 remark: Option<&str>,
) -> Result<()> {
 let product_id: i64 = item.product_id.parse().map_err(|_| DomainError::validation("无效产品ID"))?;
 let quantity: Decimal = item.quantity.parse().map_err(|_| DomainError::validation("无效数量"))?;
 let bin_id: Option<i64> = item.bin_id.as_ref().and_then(|s| s.parse().ok());
 let warehouse_id: i64 = item
  .warehouse_id
  .as_deref()
  .and_then(|s| s.parse().ok())
  .ok_or_else(|| DomainError::validation("请为每行物料选择目标仓库"))?;
 record_stock_in_txn(
  inv_svc, warehouse_svc, ctx, db, product_id, quantity, warehouse_id, bin_id, transaction_type,
  doc_number, delivery_no, source_type, source_id, source_doc, remark,
 )
 .await
}

// ── Components ──

pub fn stock_in_create_content(
    post_path: &str,
    after_request_hs: &str,
    show_header: bool,
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
    with_picker: bool,
) -> Markup {
    html! {
        @if show_header {
            a href=(StockInListPath::PATH)
                class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
            { (icon::chevron_left_icon("w-4 h-4")) "返回入库列表" }
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "新建入库单" }
            }
        }
        form
            id="stockInForm"
            class="space-y-5 p-6"
            hx-post=(post_path)
            hx-swap="none"
            hx-disabled-elt="#stockin-submit-btn"
            onsubmit="return wmsStockInCollectItems()"
            _=(after_request_hs)
        {
            input type="hidden" name="idempotency_key" _="on load call wcGenIdempotencyKey(me)" {}
            // ── 顶部：segmented control + 入库日期 ──
            div class="flex items-center justify-between gap-4" {
                div class="inline-flex bg-surface rounded-sm p-[3px] gap-[2px]" {
                    button
                        type="button"
                        id="seg-purchase"
                        class="seg-btn active inline-flex items-center gap-1.5 px-4 py-[7px] rounded-[4px] text-[13px] font-medium cursor-pointer transition-all duration-150 border-none bg-transparent text-muted act:bg-bg act:text-accent act:font-semibold act:shadow-[0_1px_3px_rgba(15,23,42,0.08)]"
                        _="on click set #txn-type's value to 'PurchaseReceipt' then take .active from .seg-btn then remove .hidden from #po-btn then add .hidden to #wo-btn then remove .hidden from #delivery-no-field then set #po-selected-hint's innerHTML to '未选择采购订单，也可在下方手动添加物料' then set #po-cards's innerHTML to '' then call wmsStockInCalcSummary()"
                    { (icon::truck_icon("w-[15px] h-[15px]")) "采购入库" }
                    button
                        type="button"
                        id="seg-production"
                        class="seg-btn inline-flex items-center gap-1.5 px-4 py-[7px] rounded-[4px] text-[13px] font-medium cursor-pointer transition-all duration-150 border-none bg-transparent text-muted act:bg-bg act:text-accent act:font-semibold act:shadow-[0_1px_3px_rgba(15,23,42,0.08)]"
                        _="on click set #txn-type's value to 'ProductionReceipt' then take .active from .seg-btn then add .hidden to #po-btn then remove .hidden from #wo-btn then add .hidden to #delivery-no-field then set #po-selected-hint's innerHTML to '未选择生产工单，也可在下方手动添加物料' then set #po-cards's innerHTML to '' then call wmsStockInCalcSummary()"
                    { (icon::package_icon("w-[15px] h-[15px]")) "生产入库" }
                }
                input type="hidden" id="txn-type" name="transaction_type" value="PurchaseReceipt" {}
                div class="flex items-center gap-2" {
                    (icon::calendar_icon("w-[15px] h-[15px] text-muted"))
                    input
                        type="date"
                        name="posting_date"
                        id="posting-date"
                        class="w-[140px] px-3 py-[7px] border border-border rounded-sm text-[13px] bg-white text-fg outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]" {}
                }
            }
            // ── 送货单号（采购入库用，生产入库时隐藏）──
            div id="delivery-no-field" {
                input
                    type="text"
                    name="delivery_no"
                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                    placeholder="送货单号（可选）" {}
            }
            // 工单来源 hidden
            input
                type="hidden"
                id="wo-id-hidden"
                name="work_order_id"
                value=""
                hx-post=(StockInConfirmWoPath::PATH)
                hx-trigger="change"
                hx-target="#po-cards"
                hx-swap="innerHTML" {}
            input type="hidden" id="wo-display" value="" {}
            input type="hidden" name="source_type" value="purchase" {}
            input type="hidden" name="source_ref" value="" {}
            // ── 来源关联 ──
            div {
                button
                    id="po-btn"
                    type="button"
                    class="flex items-center justify-center gap-2 w-full px-4 py-2.5 rounded-md border border-dashed border-accent bg-accent-bg text-accent text-[13px] font-medium cursor-pointer transition-all duration-150 hover:bg-[rgba(37,99,235,0.1)]"
                    _="on click add .is-open to #po-picker"
                { (icon::plus_icon("w-4 h-4")) "选择采购订单" }
                button
                    id="wo-btn"
                    type="button"
                    class="hidden flex items-center justify-center gap-2 w-full px-4 py-2.5 rounded-md border border-dashed border-accent bg-accent-bg text-accent text-[13px] font-medium cursor-pointer transition-all duration-150 hover:bg-[rgba(37,99,235,0.1)]"
                    _="on click add .is-open to #wo-picker-modal"
                { (icon::plus_icon("w-4 h-4")) "选择生产工单" }
                div class="mt-2 text-xs text-muted text-center" id="po-selected-hint" {
                    "未选择采购订单，也可在下方手动添加物料"
                }
            }
            // 分隔线
            div class="h-px bg-border-soft" {}
            // ── 入库明细 ──
            div {
                div class="flex items-center gap-2 mb-3" {
                    (icon::clipboard_list_icon("w-4 h-4 text-accent"))
                    span class="text-[13px] font-semibold text-fg" { "入库明细" }
                    span id="stockin-item-count" class="ml-auto text-xs text-muted" { "共 0 项" }
                }
                div id="po-cards" class="space-y-3" {}
                div class="overflow-x-auto" {
                    table class="data-table" {
                        thead {
                            tr {
                                th class="w-10" { "序号" }
                                th { "产品" }
                                th class="w-[180px]" {
                                    "仓库 / 库位 "
                                    span class="text-danger" { "*" }
                                }
                                th class="w-[100px] text-right" {
                                    "入库数量 "
                                    span class="text-danger" { "*" }
                                }
                                th class="w-10" {}
                            }
                        }
                        tbody id="stockin-item-tbody" {}
                    }
                }
                button
                    type="button"
                    class="flex items-center justify-center gap-2 w-full py-2.5 mt-3 border border-dashed border-border rounded-md text-accent text-sm font-medium cursor-pointer transition-all duration-150 hover:border-accent hover:bg-accent-bg"
                    _="on click add .is-open to #product-modal"
                { (icon::plus_icon("w-3.5 h-3.5")) "手动添加物料" }
            }
            // ── 备注 ──
            div {
                div class="flex items-center gap-2 mb-2" {
                    (icon::edit_icon("w-4 h-4 text-accent"))
                    span class="text-[13px] font-semibold text-fg" { "备注" }
                }
                textarea
                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none resize-y min-h-[64px] transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                    name="remark"
                    placeholder="输入备注信息…"
                    rows="3" {}
            }
            input type="hidden" name="items_json" id="stockin-items-json" value="[]" {}
            // ── Action Bar ──
            div class="sticky bottom-0 flex items-center justify-end gap-3 -mx-6 px-6 py-4 bg-bg border-t border-border-soft"
            {
                @if show_header {
                    a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                        href=(format!("{}?restore=true", StockInListPath::PATH))
                    { "取消" }
                } @else {
                    button
                        type="button"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                        _="on click remove .open from closest .drawer-overlay"
                    { "取消" }
                }
                button
                    type="submit"
                    id="stockin-submit-btn"
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                { (icon::check_circle_icon("w-4 h-4")) "确认入库" }
            }
        }
        ({
            crate::components::product_picker::product_picker_modal_with_search(
                "product-modal",
                StockInItemRowPath::PATH,
                "stockin-item-tbody",
            )
        })
        ({
            crate::components::purchase_order_picker::purchase_order_picker_modal(
                "po-picker",
                StockInConfirmPosPath::PATH,
            )
        })
        ({
            crate::components::work_order_picker::work_order_picker_modal(
                "wo-picker-modal",
                "wo-id-hidden",
                "wo-display",
            )
       })
        // ── 库位选择弹窗（独立页渲染；作业中心 drawer 复用时由页面级 shell 提供，避免重复 id）──
        @if with_picker {
            (crate::components::bin_search::bin_picker_modal("bin-picker-modal", warehouses))
        }
       script src=(crate::layout::page::cache_url("/wms-stock-in-create.js")) {}
    }
}
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
                            "仓库 / 库位 "
                            span class="text-danger" { "*" }
                        }
                        th class="w-[130px]" {
                            "入库数量 "
                            span class="text-danger" { "*" }
                        }
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
    let bid = format!("manual-bin-{}", product.product_id);
    html! {
        tr class="item-row" oninput="wmsStockInCalcRow(this)" {
            td class="line-num text-muted text-xs text-center" {}
            // 产品（编码 + 名称 + 关联单号）
            td {
                div class="font-mono tabular-nums text-sm text-fg" { (product.product_code) }
                div class="text-sm text-fg truncate max-w-[200px]" title=(product.pdt_name) {
                    (product.pdt_name)
                }
                input
                    class="w-full mt-1 px-2 py-[3px] border border-border rounded-sm text-xs bg-white text-fg-2 outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                    type="text"
                    name="source_doc_number"
                    placeholder="关联单号 *"
                    required {}
            }
            // 仓库 / 库位（公共控件）
            td class="align-top" {
                ({
                   warehouse_bin_cell(
                       &bid,
                       product.product_id,
                       warehouses,
                       "",
                       "inbound",
                   )
                })
            }
            // 入库数量
            td {
                input
                    class=(format!("{CELL_INPUT} w-[90px] text-right font-mono"))
                    type="number"
                    step="any"
                    name="quantity"
                    placeholder="0" {}
            }
            // 删除
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
    let bid = format!("po-bin-{source_id}-{}", product.product_id);
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
            td class="align-top" {
                ({
                    warehouse_bin_cell(
                       &bid,
                       product.product_id,
                       warehouses,
                       "",
                       "inbound",
                   )
                })
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
