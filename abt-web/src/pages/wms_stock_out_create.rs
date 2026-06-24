use std::collections::HashMap;

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use rust_decimal::Decimal;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::customer::CustomerService;
use abt_core::shared::types::{DomainError, PageParams};
use abt_core::shared::enums::DocumentType;
use abt_core::shared::document_sequence::DocumentSequenceService;
use abt_core::wms::outbound::ShippingRequestService;
use abt_core::wms::outbound::model::ShippingStatus;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::wms::inventory_transaction::InventoryTransactionService;
use abt_core::wms::inventory_transaction::model::RecordTransactionReq;
use abt_core::wms::inventory::InventoryService;
use abt_core::wms::material_requisition::MaterialRequisitionService;
use abt_core::wms::enums::{RequisitionStatus, TransactionType};

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_stock_out::{
    StockOutCreatePath, StockOutListPath, StockOutItemRowPath, StockOutConfirmShippingPath,
    StockOutConfirmReqPath,
};
use crate::utils::{RequestContext, empty_as_none};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize)]
pub struct ItemRowParams {
    pub product_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct SuggestBinsParams {
    pub product_id: i64,
    pub warehouse_id: i64,
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

    let customer_svc = state.customer_service();
    let customers = customer_svc
        .list(&service_ctx, &mut conn, abt_core::master_data::customer::model::CustomerQuery::default(), PageParams::new(1, 500))
        .await
        .map(|r| r.items)
        .unwrap_or_default();

    let content = stock_out_create_content(&customers);
    let page_html = admin_page(
        is_htmx, "新建出库单", &claims, "inventory", StockOutCreatePath::PATH, "库存管理", None, content, &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

/// HTMX: 手动物料行（product_picker 添加）
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
    Ok(Html(manual_item_row(&product, &warehouses).into_string()))
}

// ── 来源确认（发货申请 多选 / 领料单 单选 → 渲染折叠卡片明细到 #source-cards）──

struct SourceCardData {
    source_id: i64,
    /// "shipping" | "requisition" —— 写入每行 hidden source_type
    source_type: &'static str,
    doc_number: String,
    /// 客户名（发货申请）/ 关联工单号（领料单）
    party_label: String,
    status_label: String,
    /// (product, 申请量, 已完成量, 默认仓库)
    items: Vec<(abt_core::master_data::product::model::Product, Decimal, Decimal, Option<i64>)>,
}

/// HTMX: 确认选中的发货申请 → 渲染所有发货申请折叠卡片（含明细行）替换 #source-cards。
#[require_permission("INVENTORY", "create")]
pub async fn confirm_shipping(
    ctx: RequestContext,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.shipping_service();
    let customer_svc = state.customer_service();
    let product_svc = state.product_service();

    // 解析 urlencoded body：shipping_id=1&shipping_id=2（兼容单值/多值）
    let body_str = std::str::from_utf8(&body).unwrap_or("");
    let mut seen = std::collections::HashSet::new();
    let ids: Vec<i64> = body_str.split('&')
        .filter_map(|kv| {
            let mut it = kv.splitn(2, '=');
            let (k, v) = (it.next()?, it.next()?);
            if k == "shipping_id" { v.parse::<i64>().ok() } else { None }
        })
        .filter(|id| *id > 0 && seen.insert(*id))
        .collect();

    // 客户名批量解析
    let mut customer_ids: Vec<i64> = Vec::new();
    let mut shippings: Vec<abt_core::wms::outbound::model::ShippingRequest> = Vec::new();
    for id in &ids {
        if let Ok(s) = svc.find_by_id(&service_ctx, &mut conn, *id).await {
            customer_ids.push(s.customer_id);
            shippings.push(s);
        }
    }
    let names = resolve_customer_names_map(&customer_svc, &service_ctx, &mut conn, customer_ids).await;

    let mut cards: Vec<SourceCardData> = Vec::new();
    for s in &shippings {
        let items_result = svc.list_items(&service_ctx, &mut conn, s.id).await.unwrap_or_default();
        let mut rows: Vec<(abt_core::master_data::product::model::Product, Decimal, Decimal, Option<i64>)> = Vec::new();
        for it in items_result {
            if let Ok(p) = product_svc.get(&service_ctx, &mut conn, it.product_id).await {
                rows.push((p, it.requested_qty, it.shipped_qty, Some(it.warehouse_id)));
            }
        }
        cards.push(SourceCardData {
            source_id: s.id,
            source_type: "shipping",
            doc_number: s.doc_number.clone(),
            party_label: names.get(&s.customer_id).cloned().unwrap_or_else(|| "-".into()),
            status_label: shipping_status_label(&s.status).to_string(),
            items: rows,
        });
    }

    let warehouses = state.warehouse_service()
        .list(&service_ctx, &mut conn, abt_core::wms::warehouse::model::WarehouseFilter::default(), 1, 200)
        .await.map(|r| r.items).unwrap_or_default();
    let html = source_cards_fragment(&cards, &warehouses).into_string();
    Ok(([("HX-Trigger-After-Settle", r#"{"closeShippingPicker":"","sourceCardsUpdated":""}"#)], Html(html)))
}

/// HTMX: 确认选中的领料单（单选）→ 渲染领料单明细卡片替换 #source-cards
#[require_permission("INVENTORY", "create")]
pub async fn confirm_requisition(
    ctx: RequestContext,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.material_requisition_service();
    let product_svc = state.product_service();

    let body_str = std::str::from_utf8(&body).unwrap_or("");
    let requisition_id: i64 = body_str.split('&')
        .filter_map(|kv| {
            let mut it = kv.splitn(2, '=');
            let (k, v) = (it.next()?, it.next()?);
            if k == "requisition_id" { v.parse().ok() } else { None }
        })
        .next()
        .ok_or_else(|| DomainError::validation("未选择领料单"))?;

    let mr = svc.get(&service_ctx, &mut conn, requisition_id).await?;
    let items_result = svc.list_items(&service_ctx, &mut conn, requisition_id).await?;
    let mut rows: Vec<(abt_core::master_data::product::model::Product, Decimal, Decimal, Option<i64>)> = Vec::new();
    for it in items_result {
        if let Ok(p) = product_svc.get(&service_ctx, &mut conn, it.product_id).await {
            // 领料单整单一个仓库
            rows.push((p, it.requested_qty, it.issued_qty, Some(mr.warehouse_id)));
        }
    }

    let card = SourceCardData {
        source_id: mr.id,
        source_type: "material_requisition",
        doc_number: mr.doc_number.clone(),
        party_label: format!("工单 #{}", mr.work_order_id),
        status_label: requisition_status_label(&mr.status).to_string(),
        items: rows,
    };

    let warehouses = state.warehouse_service()
        .list(&service_ctx, &mut conn, abt_core::wms::warehouse::model::WarehouseFilter::default(), 1, 200)
        .await.map(|r| r.items).unwrap_or_default();
    let html = source_cards_fragment(&[card], &warehouses).into_string();
    Ok(([("HX-Trigger-After-Settle", r#"{"closeMrPicker":"","sourceCardsUpdated":""}"#)], Html(html)))
}

// ── 库位建议（按产品+仓库，仅有库存的库位）──

/// HTMX: 根据产品+仓库推荐出库库位（仅返回有该产品库存的库位）
#[require_permission("INVENTORY", "create")]
pub async fn suggest_bins(
    ctx: RequestContext,
    Query(params): Query<SuggestBinsParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let warehouse_svc = state.warehouse_service();
    let inventory_svc = state.inventory_service();

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
    let mut stock_by_bin: HashMap<i64, Decimal> = HashMap::new();
    for v in inv.into_iter().filter(|v| v.warehouse_id == params.warehouse_id) {
        *stock_by_bin.entry(v.bin_id).or_insert(Decimal::ZERO) += v.quantity;
    }

    // 仅保留有库存的库位，按库存量降序
    let mut rows: Vec<(abt_core::wms::warehouse::model::Bin, Decimal)> = bins
        .into_iter()
        .filter_map(|b| {
            let id = b.id;
            stock_by_bin.get(&id).copied().filter(|q| *q > Decimal::ZERO).map(|q| (b, q))
        })
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1));

    Ok(Html(suggest_bins_fragment(&rows).into_string()))
}

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub struct StockOutCreateForm {
    pub transaction_type: String,
    pub remark: Option<String>,
    pub items_json: String,
}

#[derive(Debug, Deserialize)]
struct StockOutItemWeb {
    product_id: String,
    quantity: String,
    unit_cost: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    warehouse_id: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    bin_id: Option<String>,
    /// 该行所属来源单 ID（来源明细自带；手动物料为 0）
    #[serde(default, deserialize_with = "empty_as_none")]
    source_id: Option<String>,
    /// 该行所属来源单号（来源明细自带；手动物料用户手填）
    #[serde(default, deserialize_with = "empty_as_none")]
    source_doc_number: Option<String>,
    /// 该行 source_type：shipping / material_requisition / manual
    #[serde(default, deserialize_with = "empty_as_none")]
    source_type: Option<String>,
}

#[require_permission("INVENTORY", "create")]
pub async fn create_stock_out(
    _path: StockOutCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<StockOutCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.inventory_transaction_service();
    let warehouse_svc = state.warehouse_service();

    let web_items: Vec<StockOutItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("无效物料数据: {e}")))?;

    if web_items.is_empty() {
        return Err(DomainError::validation("请至少添加一个物料").into());
    }

    let transaction_type = match form.transaction_type.as_str() {
        "MaterialIssue" => TransactionType::MaterialIssue,
        _ => TransactionType::SalesShipment,
    };
    // 手动物料无 per-item source_type 时回退；行内明细自带 source_type
    let global_source_type = "manual";
    let remark = form.remark.filter(|s| !s.is_empty());

    // 出库单号：通过 DocumentSequenceService 生成规范编号（CK-YYYY-MM-SEQ）
    let doc_number = state.document_sequence_service()
        .next_number(&service_ctx, &mut conn, DocumentType::StockShipment)
        .await?;

    // 每行物料独立仓库 + 库位，逐条记录一笔出库事务
    for item in &web_items {
        let product_id: i64 = item.product_id.parse()
            .map_err(|_| DomainError::validation("无效产品ID"))?;
        let quantity: Decimal = item.quantity.parse()
            .map_err(|_| DomainError::validation("无效数量"))?;

        if quantity <= Decimal::ZERO {
            return Err(DomainError::validation("出库数量必须大于0").into());
        }

        // 每行来源仓库（必填）
        let warehouse_id: i64 = item.warehouse_id.as_deref()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| DomainError::validation("请为每行物料选择来源仓库"))?;

        let bin_id: Option<i64> = item.bin_id.as_ref()
            .and_then(|s| s.parse().ok());

        let zone_id = match warehouse_svc
            .get_or_create_default_zone(&service_ctx, &mut conn, warehouse_id).await.ok().map(|z| z.id)
        {
            Some(zid) => Some(zid),
            None => None,
        };

        let unit_cost: Option<Decimal> = item.unit_cost.as_ref().and_then(|s| s.parse().ok());

        // 来源：每条物料优先自带所属来源单，缺省回退全局
        let source_id: i64 = item.source_id.as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let source_doc_number = item.source_doc_number.clone()
            .filter(|s| !s.is_empty());
        let source_type = item.source_type.clone()
            .unwrap_or_else(|| global_source_type.to_string());

        // 可用量校验
        let available = svc.query_available(&service_ctx, &mut conn, product_id, Some(warehouse_id)).await?;
        if quantity > available {
            return Err(DomainError::business_rule(
                format!("库存不足：产品ID {} 需要 {}，可用 {}", product_id, quantity, available),
            ).into());
        }

        // record() 的 quantity 是有符号 delta（入库正 / 出库负）。
        // 出库必须传负数，否则台账会反向增加（历史 bug）。
        let req = RecordTransactionReq {
            doc_number: Some(doc_number.clone()),
            delivery_no: None,
            source_doc_number,
            transaction_type,
            product_id,
            warehouse_id,
            zone_id,
            bin_id,
            batch_no: None,
            quantity: -quantity,
            unit_cost,
            source_type,
            source_id,
            remark: remark.clone(),
        };

        svc.record(&service_ctx, &mut conn, req).await?;
    }

    let redirect = StockOutListPath.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn stock_out_create_content(
    customers: &[abt_core::master_data::customer::model::Customer],
) -> Markup {
    html! {
        div {
            // ── Back Link ──
            a   href="/admin/wms/stock-out"
                class="inline-flex items-center gap-2 mb-4 text-sm text-muted no-underline hover:text-accent transition-colors"
            { (icon::chevron_left_icon("w-4 h-4")) "返回出库列表" }
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "新建出库单" }
            }

            form
                id="stockOutForm"
                class="space-y-3"
                hx-post=(StockOutCreatePath::PATH)
                hx-swap="none"
                onsubmit="return wmsStockOutCollectItems()"
            {
                // ── 独立出库警示 ──
                div class="flex items-center rounded-md mb-4 gap-3 px-4 py-3 bg-[rgba(250,173,20,0.05)] border border-[rgba(250,173,20,0.15)]"
                {
                    (icon::circle_alert_icon("w-4 h-4 text-warn shrink-0"))
                    span class="text-sm text-fg-2" {
                        "本页为"
                        strong { "独立出库登记" }
                        "，直接扣减库存台账。若已通过"
                        strong { "发货申请（发货）" }
                        "或"
                        strong { "领料单（发料）" }
                        "完成出库，请勿在此重复登记，否则会造成库存双重扣减。"
                    }
                }
                // ── 来源关联与出库明细（合并 card：选来源 → 同区立即出明细）──
                div class="bg-bg border border-border rounded p-4" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-3 pb-2 border-b border-border-soft"
                    { (icon::link_icon("w-[18px] h-[18px]")) "来源关联与出库明细" }
                    // 子区1：来源选择
                    div {
                        div class="text-xs font-medium text-fg-2 mb-2" { "来源选择" }
                        div class="flex gap-3 items-end" {
                            div class="flex flex-col w-[200px]" {
                                label
                                    class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                                {
                                    "出库类型 "
                                    span class="text-danger" { "*" }
                                }
                                select
                                    id="txn-type"
                                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                                    name="transaction_type"
                                    required
                                    _="on change call wmsStockOutToggleSourceBtn()"
                                {
                                    option value="SalesShipment" selected { "销售出库" }
                                    option value="MaterialIssue" { "生产领料" }
                                }
                            }
                            // 出库类型联动：销售出库→选择发货申请 / 生产领料→选择领料单（两按钮切换显隐）
                            button
                                id="shipping-btn"
                                type="button"
                                class="inline-flex items-center justify-center gap-2 px-[18px] py-2 rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-colors whitespace-nowrap"
                                _="on click add .is-open to #shipping-picker"
                            { (icon::plus_icon("w-4 h-4")) "选择发货申请" }
                            button
                                id="mr-btn"
                                type="button"
                                class="hidden inline-flex items-center justify-center gap-2 px-[18px] py-2 rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-colors whitespace-nowrap"
                                _="on click add .is-open to #mr-picker-modal"
                            { (icon::plus_icon("w-4 h-4")) "选择领料单" }
                        }
                        // 领料单来源 hidden：选领料单后 fill value + trigger change → HTMX POST confirm-requisition 渲染明细
                        input
                            type="hidden"
                            id="mr-id-hidden"
                            name="requisition_id"
                            value=""
                            hx-post=(StockOutConfirmReqPath::PATH)
                            hx-trigger="change"
                            hx-target="#source-cards"
                            hx-swap="innerHTML" {}
                        ;
                        input type="hidden" id="mr-display" value="" {}
                        ;
                        div class="mt-3 text-xs text-muted" id="source-selected-hint" {
                            "未选择来源单据；也可在下方手动添加物料"
                        }
                    }
                    // 分隔线
                    div class="border-t border-border-soft my-5" {}
                    // 子区2：出库明细
                    div {
                        div class="flex items-center gap-2 mb-3" {
                            span class="text-xs font-medium text-fg-2" { "出库明细" }
                            span
                                id="stockout-item-count"
                                class="ml-auto text-xs font-normal text-muted"
                            { "共 0 项" }
                        }
                        // 来源单据折叠卡片容器（confirm 端点渲染，每个来源一卡含明细）
                        div id="source-cards" class="space-y-3" {}
                        // 手动物料表（无来源 / 补充明细）
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
                                        th class="w-[160px]" {
                                            "来源仓库 "
                                            span class="text-danger" { "*" }
                                        }
                                        th class="w-[110px]" {
                                            "出库数量 "
                                            span class="text-danger" { "*" }
                                        }
                                        th class="w-[100px]" { "单位成本" }
                                        th class="w-[100px]" { "小计" }
                                        th class="w-[150px]" { "来源库位" }
                                        th class="w-10" {}
                                    }
                                }
                                tbody id="stockout-item-tbody" {
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
                    { (icon::clipboard_list_icon("w-[18px] h-[18px]")) "出库汇总" }
                    div class="grid grid-cols-3 gap-6" {
                        div class="text-center bg-surface p-4 rounded-md" {
                            div class="text-[11px] text-muted mb-1" { "物料种类" }
                            div id="stockout-summary-kinds"
                                class="font-mono tabular-nums font-semibold text-xl text-fg"
                            { "0" }
                        }
                        div class="text-center bg-surface p-4 rounded-md" {
                            div class="text-[11px] text-muted mb-1" { "出库总量" }
                            div id="stockout-summary-qty"
                                class="font-mono tabular-nums font-semibold text-xl text-fg"
                            { "0" }
                        }
                        div class="text-center p-4 rounded-md bg-danger-bg border border-[rgba(255,77,79,0.15)]"
                        {
                            div class="text-[11px] text-danger mb-1" { "出库总金额" }
                            div id="stockout-summary-amount"
                                class="font-mono tabular-nums font-semibold text-xl text-danger"
                            { "¥0.00" }
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
                input type="hidden" name="items_json" id="stockout-items-json" value="[]" {}
                // ── Action Bar ──
                div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft"
                {
                    a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                        href="/admin/wms/stock-out"
                    { "取消" }
                    div class="flex gap-3" {
                        button
                            type="button"
                            class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                        { "保存草稿" }
                        button
                            type="submit"
                            class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-danger text-accent-on border-none hover:bg-danger-700 text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(220,38,38,0.2)]"
                        { (icon::upload_icon("w-4 h-4")) "确认出库" }
                    }
                }
            }

            ({
                crate::components::product_picker::product_picker_modal_with_search(
                    "product-modal",
                    StockOutItemRowPath::PATH,
                    "stockout-item-tbody",
                )
            })
            // ── 发货申请多选弹窗（销售出库用）──
            ({
                crate::components::shipping_request_picker::shipping_request_picker_modal(
                    "shipping-picker",
                    StockOutConfirmShippingPath::PATH,
                    customers,
                )
            })
            // ── 领料单选择弹窗（生产领料用；选中 fill #mr-id-hidden + trigger change → HTMX POST confirm-requisition 渲染明细）──
            ({
                crate::components::material_requisition_picker::material_requisition_picker_modal(
                    "mr-picker-modal",
                    "mr-id-hidden",
                    "mr-display",
                )
            })
            // ── 库位选择弹窗（按产品+仓库，仅有库存库位，由 wmsStockOutOpenBinPicker 触发）──
            div id="bin-picker"
                class="fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto"
                _="on click[me is event.target] remove .is-open"
            {
                div class="modal bg-bg rounded-xl w-[520px] max-h-[80vh] flex flex-col overflow-hidden shadow-xl"
                {
                    div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0"
                    {
                        h2 { "选择出库库位" }
                        button
                            type="button"
                            class="bg-transparent border-none cursor-pointer text-xl text-muted p-1"
                            _="on click remove .is-open from #bin-picker"
                        { "×" }
                    }
                    div id="bin-picker-results" class="overflow-y-auto flex-1 min-h-0" {
                        div class="text-center text-muted py-10 text-sm" {
                            "点击物料行的「自动分配」加载该产品有库存的库位…"
                        }
                    }
                }
            }
            // ── Page-specific JS（类型联动 + 来源卡片 + 库位选择 + 明细收集）──
            script src="/wms-stock-out-create.js" {}
        }
    }
}

const CELL_INPUT: &str =
    "w-full px-2 py-[5px] border border-border rounded-sm text-[13px] bg-white text-fg \
     outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]";

/// 所有来源折叠卡片集合（confirm 端点返回，替换 #source-cards）
fn source_cards_fragment(
    cards: &[SourceCardData],
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
    html! {
        @for c in cards { (source_card_fragment(c, warehouses)) }
        @if cards.is_empty() {
            div class="text-center text-muted py-6 text-sm" { "未选择来源单据；可在下方手动添加物料" }
        }
    }
}

/// 单个来源折叠卡片：header 可折叠 + body 含明细表（复用 source_detail_row）
fn source_card_fragment(
    card: &SourceCardData,
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
    html! {
        div class="source-card bg-surface border border-border-soft rounded-md [&.is-collapsed_.source-card-body]:hidden [&.is-collapsed_.source-toggle]:-rotate-90"
            data-source-id=(card.source_id)
        {
            div class="source-card-header flex items-center gap-3 px-4 py-3 border-b border-border-soft cursor-pointer hover:bg-surface/60"
                _="on click[not (event.target matches <button/>)] toggle .is-collapsed on closest .source-card"
            {
                span
                    class="source-toggle text-muted text-xs transition-transform duration-150 inline-block"
                { "▼" }
                span class="text-sm font-semibold text-fg" { (card.doc_number) }
                span class="text-xs text-muted" { (card.party_label) " · " (card.status_label) }
                button
                    type="button"
                    class="ml-auto text-xs text-muted hover:text-danger"
                    _="on click remove closest .source-card then trigger sourceCardsUpdated on body"
                { "删除" }
            }
            div class="source-card-body p-3 overflow-x-auto" {
                table class="data-table" {
                    thead {
                        tr {
                            th class="w-10" { "序号" }
                            th { "产品" }
                            th class="w-[160px]" {
                                "来源仓库 "
                                span class="text-danger" { "*" }
                            }
                            th class="w-[110px]" {
                                "出库数量 "
                                span class="text-danger" { "*" }
                            }
                            th class="w-[100px]" { "单位成本" }
                            th class="w-[100px]" { "小计" }
                            th class="w-[150px]" { "来源库位" }
                            th class="w-10" {}
                        }
                    }
                    tbody {
                        @for (product, req_qty, done_qty, default_wh) in &card.items {
                            ({
                                source_detail_row(
                                    &SourceRowInfo {
                                        product,
                                        req_qty: *req_qty,
                                        done_qty: *done_qty,
                                        source_id: card.source_id,
                                        source_type: card.source_type,
                                        source_doc: Some(card.doc_number.as_str()),
                                        default_warehouse_id: *default_wh,
                                    },
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

/// 手动物料行（每行独立选仓库 + 库位 + 关联单号）
fn manual_item_row(
    product: &abt_core::master_data::product::model::Product,
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
    html! {
        tr class="item-row" oninput="wmsStockOutCalcRow(this)" {
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
                input
                    class=(format!("{CELL_INPUT} w-[90px] text-right font-mono"))
                    type="number"
                    step="any"
                    name="unit_cost"
                    placeholder="0.00" {}
            }
            td class="line-subtotal text-right font-mono font-semibold whitespace-nowrap text-sm" {
                "—"
            }
            td {
                input type="hidden" name="bin_id" {}
                ;
                button
                    type="button"
                    class="bin-picker-btn w-full px-2 py-[5px] border border-border rounded-sm text-[13px] bg-white text-fg-2 hover:border-accent hover:text-accent transition-colors text-left truncate"
                    _="on click call wmsStockOutOpenBinPicker(me)"
                {
                    span class="bin-label" { "自动分配" }
                }
            }
            td {
                button
                    type="button"
                    class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:text-danger"
                    title="删除行"
                    _="on click remove closest <tr/> then call wmsStockOutCalcSummary()"
                { (icon::x_icon("w-3.5 h-3.5")) }
            }
            input type="hidden" name="product_id" value=(product.product_id) {}
            ;
            input type="hidden" name="source_id" value="0" {}
            ;
            input type="hidden" name="source_type" value="manual" {}
            ;
        }
    }
}

/// 来源明细行上下文（打包来源 + 数量信息，避免函数参数过多）
struct SourceRowInfo<'a> {
    product: &'a abt_core::master_data::product::model::Product,
    req_qty: Decimal,
    done_qty: Decimal,
    source_id: i64,
    source_type: &'a str,
    source_doc: Option<&'a str>,
    default_warehouse_id: Option<i64>,
}

/// 来源明细行（带待出库余量校验，per-item source + 每行独立选仓库/库位 + 单位成本/小计）
fn source_detail_row(
    info: &SourceRowInfo,
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
    let pending = (info.req_qty - info.done_qty).max(Decimal::ZERO);
    let product = info.product;
    let req_qty = info.req_qty;
    let done_qty = info.done_qty;
    let source_id = info.source_id;
    let source_type = info.source_type;
    let source_doc = info.source_doc;
    let default_warehouse_id = info.default_warehouse_id;
    html! {
        tr class="item-row" oninput="wmsStockOutCalcRow(this)" {
            td class="line-num text-muted text-xs text-center" {}
            td {
                div class="font-mono tabular-nums text-sm text-fg" { (product.product_code) }
                div class="text-sm text-fg truncate max-w-[200px]" title=(product.pdt_name) {
                    (product.pdt_name)
                }
                div class="text-[11px] text-muted pending-hint" data-pending=(pending.to_string()) {
                    "申请 "
                    (crate::utils::fmt_qty(req_qty))
                    " · 已出 "
                    (crate::utils::fmt_qty(done_qty))
                    " · 待出库 "
                    (crate::utils::fmt_qty(pending))
                }
            }
            td {
                select class=(format!("{CELL_INPUT} row-wh-select")) name="warehouse_id" required {
                    option value="" disabled { "选择仓库" }
                    @for wh in warehouses {
                        @let is_default = default_warehouse_id == Some(wh.id);
                        option value=(wh.id) selected[is_default] { (wh.name) }
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
                    oninput="wmsStockOutValidateRow(this)" {}
            }
            td {
                input
                    class=(format!("{CELL_INPUT} w-[90px] text-right font-mono"))
                    type="number"
                    step="any"
                    name="unit_cost"
                    placeholder="0.00" {}
            }
            td class="line-subtotal text-right font-mono font-semibold whitespace-nowrap text-sm" {
                "—"
            }
            td {
                input type="hidden" name="bin_id" {}
                ;
                button
                    type="button"
                    class="bin-picker-btn w-full px-2 py-[5px] border border-border rounded-sm text-[13px] bg-white text-fg-2 hover:border-accent hover:text-accent transition-colors text-left truncate"
                    _="on click call wmsStockOutOpenBinPicker(me)"
                {
                    span class="bin-label" { "自动分配" }
                }
            }
            td {
                button
                    type="button"
                    class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:text-danger"
                    title="删除行"
                    _="on click remove closest <tr/> then call wmsStockOutCalcSummary()"
                { (icon::x_icon("w-3.5 h-3.5")) }
            }
            input type="hidden" name="product_id" value=(product.product_id) {}
            ;
            input type="hidden" name="source_id" value=(source_id) {}
            ;
            input type="hidden" name="source_type" value=(source_type) {}
            ;
            input type="hidden" name="source_doc_number" value=(source_doc.unwrap_or("")) {}
            ;
        }
    }
}

/// 库位建议列表（仅有库存的 bin，点击 wmsStockOutPickBin 填回当前行）
fn suggest_bins_fragment(rows: &[(abt_core::wms::warehouse::model::Bin, Decimal)]) -> Markup {
    html! {
        @if rows.is_empty() {
            div class="text-center text-muted py-10" {
                (icon::link_icon("w-8 h-8"))
                p class="mt-2 text-sm" { "该产品在此仓库无库存" }
                p class="text-xs mt-1" { "请检查仓库选择，或先入库该产品" }
            }
        } @else {
            @for (bin, qty) in rows {
                button
                    type="button"
                    class="w-full flex items-center justify-between gap-3 px-4 py-3 border-b border-border-soft last:border-b-0 text-left transition-colors hover:bg-surface"
                    data-bin-id=(bin.id)
                    data-bin-label=(format!("{} {}", bin.code, bin.name))
                    _="on click call wmsStockOutPickBin(@data-bin-id, @data-bin-label)"
                {
                    div class="flex-1 min-w-0" {
                        div class="text-sm font-medium text-fg truncate" { (bin.code) " " (bin.name) }
                        div class="text-xs text-success flex items-center gap-1 mt-0.5" {
                            (icon::check_circle_icon("w-3 h-3"))
                            "现有库存 "
                            (crate::utils::fmt_qty(*qty))
                        }
                    }
                }
            }
        }
    }
}

// ── 辅助：状态标签 ──

fn shipping_status_label(s: &ShippingStatus) -> &'static str {
    match s {
        ShippingStatus::Draft => "草稿",
        ShippingStatus::Confirmed => "已确认",
        ShippingStatus::Picking => "拣货中",
        ShippingStatus::Shipped => "已发货",
        ShippingStatus::Cancelled => "已取消",
    }
}

fn requisition_status_label(s: &RequisitionStatus) -> &'static str {
    match s {
        RequisitionStatus::Draft => "草稿",
        RequisitionStatus::Confirmed => "已确认",
        RequisitionStatus::Issued => "已发料",
        RequisitionStatus::Cancelled => "已取消",
        RequisitionStatus::PartiallyIssued => "部分发料",
    }
}

async fn resolve_customer_names_map<S: CustomerService>(
    svc: &S,
    ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    ids: Vec<i64>,
) -> HashMap<i64, String> {
    if ids.is_empty() {
        return HashMap::new();
    }
    svc.list(ctx, db, abt_core::master_data::customer::model::CustomerQuery::default(), PageParams::new(1, 500))
        .await
        .map(|r| r.items.into_iter().filter(|c| ids.contains(&c.id)).map(|c| (c.id, c.name)).collect())
        .unwrap_or_default()
}
