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
use abt_core::wms::enums::TransactionType;
use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::shared::types::{DomainError, PageParams};

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_stock_in::{StockInCreatePath, StockInListPath, StockInProductsPath, StockInItemRowPath, StockInSourcePickPath};
use crate::utils::{RequestContext, empty_as_none};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize)]
pub struct ProductSearchParams {
    pub name: Option<String>,
    pub code: Option<String>,
}

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
#[require_permission("PRODUCT", "read")]
pub async fn get_products(
    ctx: RequestContext,
    Query(params): Query<ProductSearchParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.product_service();

    let filter = ProductQuery {
        name: params.name.filter(|s| !s.is_empty()),
        code: params.code.filter(|s| !s.is_empty()),
        status: None,
        owner_department_id: None,
        category_id: None,
    };
    let result = svc.list(&service_ctx, &mut conn, filter, PageParams::new(1, 20)).await?;

    Ok(Html(product_list_fragment(&result.items).into_string()))
}

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
        notices.into_iter().map(|n| SourceOption {
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
        result.into_iter().map(|o| SourceOption {
            id: o.id,
            doc_number: o.doc_number,
            supplier_name: names.get(&o.supplier_id).cloned().unwrap_or_else(|| "-".into()),
            extra: o.order_date.to_string(),
        }).collect()
    };

    Ok(Html(source_pick_fragment(&options).into_string()))
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

#[allow(dead_code)]
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
    unit_cost: Option<String>,
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

    let source_id: i64 = form.source_id.unwrap_or(0);
    let remark = form.remark.filter(|s| !s.is_empty());

    // Record one transaction per line item
    for item in &web_items {
        let product_id: i64 = item.product_id.parse()
            .map_err(|_| DomainError::validation("无效产品ID"))?;
        let quantity: Decimal = item.quantity.parse()
            .map_err(|_| DomainError::validation("无效数量"))?;
        let unit_cost: Option<Decimal> = item.unit_cost.as_ref()
            .and_then(|s| s.parse().ok());
        let bin_id: Option<i64> = item.bin_id.as_ref()
            .and_then(|s| s.parse().ok());

        if quantity <= Decimal::ZERO {
            return Err(DomainError::validation("入库数量必须大于0").into());
        }

        let req = RecordTransactionReq {
            doc_number: None,
            transaction_type,
            product_id,
            warehouse_id,
            zone_id: form.zone_id,
            bin_id: bin_id.or(form.bin_id),
            batch_no: item.batch_no.clone(),
            quantity,
            unit_cost,
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
            a href="/admin/wms/stock-in" class="back-link" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回入库列表"
            }

            // ── Page Header ──
            div class="page-header" style="margin-bottom:var(--space-6)" {
                h1 class="page-title" { "新建入库单" }
                div class="page-actions" {
                    button class="btn btn-default" type="button" { "保存草稿" }
                    button class="btn btn-primary" type="submit" form="stockInForm" {
                        (icon::check_circle_icon("w-4 h-4"))
                        "确认入库"
                    }
                }
            }

            // ── Type Switch ──
            div class="type-switch" {
                div class="type-btn active" {
                    (icon::download_icon("w-7 h-7"))
                    span class="type-label" { "采购入库" }
                    span class="type-desc" { "PURCHASE_RECEIPT" br; "关联来料通知 / 采购订单" }
                    (maud::PreEscaped(r#"<script>me().on('click',e=>{any('.type-btn').classRemove('active');me(e).classAdd('active');me('#stockin-txn-type').value='PurchaseReceipt'})</script>"#))
                }
                div class="type-btn" {
                    (icon::box_icon("w-7 h-7"))
                    span class="type-label" { "生产入库" }
                    span class="type-desc" { "PRODUCTION_RECEIPT" br; "关联工单完工报工" }
                    (maud::PreEscaped(r#"<script>me().on('click',e=>{any('.type-btn').classRemove('active');me(e).classAdd('active');me('#stockin-txn-type').value='ProductionReceipt'})</script>"#))
                }
            }

            form id="stockInForm" hx-post=(StockInCreatePath::PATH) hx-swap="none"
                onsubmit="return wmsStockInCollectItems()" {
                input type="hidden" id="stockin-txn-type" name="transaction_type" value="PurchaseReceipt" {};
                // ── Source Section ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::link_icon("w-[18px] h-[18px]"))
                        "来源关联"
                    }
                    div class="wms-form-grid" {
                        div class="form-group" {
                            label class="form-label" { "来源类型" }
                            select class="form-select" name="source_type" id="source-type-select"
                                onchange="wmsUpdateSourceType()" {
                                option value="arrival" selected { "来料通知 (AN)" }
                                option value="purchase" { "采购订单 (PO)" }
                                option value="manual" { "手工录入" }
                            }
                        }
                        div class="form-group" id="source-ref-group" {
                            label class="form-label" { "来源单号 " span class="required" { "*" } }
                            div style="display:flex;gap:var(--space-2)" {
                                input class="form-input" type="text" id="source-ref-input" name="source_ref"
                                    placeholder="点击右侧选择来源单号" readonly style="flex:1;background:var(--surface)" {};
                                input type="hidden" id="source-id-input" name="source_id" {};
                                button type="button" class="btn btn-default"
                                    onclick="wmsOpenSourceModal()" { "选择" }
                            }
                        }
                        div class="form-group" {
                            label class="form-label" { "送货单号" }
                            input class="form-input" type="text" name="delivery_no" placeholder="输入送货单号";
                        }
                        div class="form-group" id="source-supplier-group" {
                            label class="form-label" { "供应商" }
                            input class="form-input" type="text" id="source-supplier-input"
                                placeholder="选择来源后自动填充" readonly style="background:var(--surface)";
                        }
                    }
                }

                // ── Warehouse Section ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::building_icon("w-[18px] h-[18px]"))
                        "入库信息"
                    }
                    div class="wms-form-grid" {
                        div class="form-group" {
                            label class="form-label" { "目标仓库 " span class="required" { "*" } }
                            select class="form-select" name="warehouse_id" id="warehouse-select"
                                onchange="wmsUpdateZones()" {
                                option value="" disabled selected { "请选择仓库" }
                                @for wh in warehouses {
                                    option value=(wh.id) { (wh.name) }
                                }
                            }
                        }
                        div class="form-group" {
                            label class="form-label" { "目标库区" }
                            select class="form-select" name="zone_id" id="zone-select"
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
                            label class="form-label" { "目标储位" }
                            select class="form-select" name="bin_id" id="bin-select" {
                                option value="" { "请先选择库区" }
                                @for (zone_id, bins) in all_bins {
                                    @for b in bins {
                                        option value=(b.id) data-zone=(zone_id) style="display:none" { (b.code) " " (b.name) }
                                    }
                                }
                            }
                        }
                        div class="form-group" {
                            label class="form-label" { "操作员" }
                            input class="form-input" type="text" value=(operator_name) readonly style="background:var(--surface)";
                        }
                    }
                }

                // ── Strategy Tip ──
                div style="padding:var(--space-3) var(--space-4);background:rgba(82,196,26,0.05);border:1px solid rgba(82,196,26,0.15);border-radius:var(--radius-md);margin-bottom:var(--space-6);display:flex;align-items:center;gap:var(--space-3)" {
                    (icon::check_circle_icon("w-4 h-4"))
                    span style="font-size:var(--text-sm);color:var(--fg-2)" {
                        "当前仓库上架策略："
                        strong { "同物料合并 (SAME_MERGE)" }
                        " — 系统将自动分配至同物料已有储位，储位满时按就近原则分配。"
                    }
                }

                // ── Line Items ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::box_icon("w-[18px] h-[18px]"))
                        "入库物料明细"
                        span id="stockin-item-count" style="margin-left:auto;font-size:var(--text-xs);font-weight:400;color:var(--muted)" { "共 0 项" }
                    }
                    table class="detail-table" {
                        thead {
                            tr {
                                th style="width:40px" { "序号" }
                                th { "产品编码" }
                                th { "产品名称" }
                                th { "规格型号" }
                                th { "批次号" }
                                th style="width:100px" { "入库数量 " span class="required" { "*" } }
                                th style="width:110px" { "单位成本" }
                                th style="width:110px" { "小计" }
                                th { "目标储位" }
                                th style="width:40px" { }
                            }
                        }
                        tbody id="stockin-item-tbody" {
                            // JS-managed dynamic rows
                        }
                    }
                    div style="margin-top:var(--space-4)" {
                        button type="button" class="add-row-btn"
                            onclick="me('#product-modal').classAdd('is-open')" {
                            (icon::plus_icon("w-3.5 h-3.5"))
                            "添加物料"
                        }
                    }
                }

                // ── Summary ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::clipboard_list_icon("w-[18px] h-[18px]"))
                        "入库汇总"
                    }
                    div style="display:grid;grid-template-columns:repeat(4,1fr);gap:var(--space-6)" {
                        div style="text-align:center;padding:var(--space-4);background:var(--surface);border-radius:var(--radius-md)" {
                            div style="font-size:11px;color:var(--muted);margin-bottom:var(--space-1)" { "物料种类" }
                            div id="stockin-summary-kinds" class="font-mono" style="font-size:var(--text-xl);font-weight:600" { "0" }
                        }
                        div style="text-align:center;padding:var(--space-4);background:var(--surface);border-radius:var(--radius-md)" {
                            div style="font-size:11px;color:var(--muted);margin-bottom:var(--space-1)" { "入库总量" }
                            div id="stockin-summary-qty" class="font-mono" style="font-size:var(--text-xl);font-weight:600" { "0" }
                        }
                        div style="text-align:center;padding:var(--space-4);background:var(--accent-bg);border-radius:var(--radius-md);border:1px solid rgba(22,119,255,0.15)" {
                            div style="font-size:11px;color:var(--accent);margin-bottom:var(--space-1)" { "入库总金额" }
                            div id="stockin-summary-amount" class="font-mono" style="font-size:var(--text-xl);font-weight:600;color:var(--accent)" { "¥0.00" }
                        }
                        div style="text-align:center;padding:var(--space-4);background:var(--surface);border-radius:var(--radius-md)" {
                            div style="font-size:11px;color:var(--muted);margin-bottom:var(--space-1)" { "上架策略" }
                            div style="font-size:var(--text-sm);font-weight:600" { "同物料合并" }
                        }
                    }
                }

                // ── Remark ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::edit_icon("w-[18px] h-[18px]"))
                        "备注"
                    }
                    textarea class="form-input" name="remark" placeholder="输入备注信息…" rows="3" style="width:100%;min-height:80px;padding:var(--space-2) var(--space-3);resize:vertical" { }
                }

                // hidden input for items JSON
                input type="hidden" name="items_json" id="stockin-items-json" value="[]" {}
            }
        }

        // ── Product Search Modal ──
        div id="product-modal" class="modal-overlay"
            onclick="hsBackdropClose(this,event,'is-open')" {
            div class="modal modal-lg" onclick="event.stopPropagation()" {
                div class="modal-head" {
                    h2 { "选择物料" }
                    button type="button" style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                        onclick="hsRemove(null,'#product-modal','is-open')" { "×" }
                }
                div class="modal-body" style="padding:0" hx-disinherit="hx-select" {
                    div class="product-search-bar" {
                        div class="product-search-field" {
                            label class="product-search-label" { "产品名称" }
                            input class="product-search-input" type="text" name="name" placeholder="输入产品名称…"
                                hx-get=(StockInProductsPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-target="#stockin-product-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                        }
                        div class="product-search-field" {
                            label class="product-search-label" { "产品编码" }
                            input class="product-search-input" type="text" name="code" placeholder="输入产品编码…"
                                hx-get=(StockInProductsPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-target="#stockin-product-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                        }
                        button type="button" class="product-search-clear"
                            hx-get=(StockInProductsPath::PATH)
                            hx-target="#stockin-product-results"
                            hx-swap="innerHTML"
                            onclick="hsSetAndTrigger('.product-search-input','','keyup')" {
                            "清除"
                        }
                    }
                    div id="stockin-product-results" {
                        div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
                            (icon::package_icon("w-8 h-8"))
                            p style="margin:var(--space-2) 0 0;font-size:var(--text-sm)" { "输入关键词搜索物料" }
                        }
                    }
                }
            }
        }

        // ── Source Pick Modal ──
        div id="source-modal" class="modal-overlay" data-source-path=(StockInSourcePickPath::PATH)
            onclick="hsBackdropClose(this,event,'is-open')" {
            div class="modal modal-lg" onclick="event.stopPropagation()" {
                div class="modal-head" {
                    h2 { "选择来源单据" }
                    button type="button" style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                        onclick="hsRemove(null,'#source-modal','is-open')" { "×" }
                }
                div class="modal-body" style="padding:0" hx-disinherit="hx-select" {
                    input type="hidden" id="source-pick-type" value="arrival" {}
                    div class="product-search-bar" {
                        div class="product-search-field" {
                            label class="product-search-label" { "来源单号" }
                            input class="product-search-input" type="text" name="keyword" placeholder="输入单号关键词…"
                                hx-get=(StockInSourcePickPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-target="#stockin-source-results"
                                hx-swap="innerHTML"
                                hx-include="#source-pick-type" {}
                        }
                        button type="button" class="product-search-clear"
                            hx-get=(StockInSourcePickPath::PATH)
                            hx-target="#stockin-source-results"
                            hx-swap="innerHTML"
                            hx-include="#source-pick-type"
                            onclick="hsSetAndTrigger('#source-modal .product-search-input','','keyup')" {
                            "清除"
                        }
                    }
                    div id="stockin-source-results" {
                        div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
                            (icon::link_icon("w-8 h-8"))
                            p style="margin:var(--space-2) 0 0;font-size:var(--text-sm)" { "输入关键词搜索来源单据" }
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
            var qtyInput = row.querySelector('input[name="quantity"]');
            var costInput = row.querySelector('input[name="unit_cost"]');
            var totalCell = row.querySelector('.line-subtotal');
            var qty = parseFloat(qtyInput.value) || 0;
            var cost = parseFloat(costInput.value) || 0;
            var subtotal = qty * cost;
            totalCell.textContent = subtotal > 0 ? '¥' + subtotal.toFixed(2) : '—';
            wmsStockInCalcSummary();
        }

        function wmsStockInCalcSummary() {
            var tbody = document.getElementById('stockin-item-tbody');
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
            document.getElementById('stockin-summary-kinds').textContent = kinds;
            document.getElementById('stockin-summary-qty').textContent = totalQty;
            document.getElementById('stockin-summary-amount').textContent = '¥' + totalAmount.toFixed(2);
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
                    unit_cost: row.querySelector('input[name="unit_cost"]').value || null,
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

        // Source type cascade: 手工录入隐藏来源单号+供应商
        function wmsUpdateSourceType() {
            var type = document.getElementById('source-type-select').value;
            var refGroup = document.getElementById('source-ref-group');
            var supplierGroup = document.getElementById('source-supplier-group');
            if (type === 'manual') {
                refGroup.style.display = 'none';
                supplierGroup.style.display = 'none';
                document.getElementById('source-ref-input').value = '';
                document.getElementById('source-supplier-input').value = '';
                document.getElementById('source-id-input').value = '';
            } else {
                refGroup.style.display = '';
                supplierGroup.style.display = '';
            }
        }

        // Open source pick modal — capture current source_type and load list
        function wmsOpenSourceModal() {
            var modal = document.getElementById('source-modal');
            var type = document.getElementById('source-type-select').value;
            document.getElementById('source-pick-type').value = type;
            me(modal).classAdd('is-open');
            var path = modal.dataset.sourcePath;
            htmx.ajax('GET', path + '?source_type=' + encodeURIComponent(type), {target: '#stockin-source-results', swap: 'innerHTML'});
        }

        // Pick a source row — backfill ref / supplier / source_id
        function wmsStockInPickSource(btn) {
            document.getElementById('source-ref-input').value = btn.dataset.doc;
            document.getElementById('source-supplier-input').value = btn.dataset.supplier;
            document.getElementById('source-id-input').value = btn.dataset.sourceId;
            hsRemove(null, '#source-modal', 'is-open');
        }
        </script>"#))
    }
}

/// Product search results fragment
fn product_list_fragment(products: &[abt_core::master_data::product::model::Product]) -> Markup {
    html! {
        @if products.is_empty() {
            div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
                (icon::package_icon("w-8 h-8"))
                p style="margin:var(--space-2) 0 0;font-size:var(--text-sm)" { "未找到匹配的产品" }
            }
        } @else {
            div class="product-select-list" {
                @for p in products {
                    div class="product-select-item" {
                        div class="product-select-info" {
                            div class="product-select-name" { (p.pdt_name) }
                            div class="product-select-meta" {
                                span class="product-select-code" { (p.product_code) }
                                span class="product-select-sep" { "·" }
                                span { (p.meta.specification) }
                                span class="product-select-sep" { "·" }
                                span { (p.unit) }
                            }
                        }
                        button type="button" class="btn btn-sm btn-primary"
                            hx-get=(format!("{}?product_id={}", StockInItemRowPath::PATH, p.product_id))
                            hx-target="#stockin-item-tbody"
                            hx-swap="beforeend"
                            hx-on::after-request="hsRemove(null,'#product-modal','is-open');setTimeout(wmsStockInRenumber,50)" {
                            "选择"
                        }
                    }
                }
            }
        }
    }
}

/// Source pick (来料通知/采购订单) results fragment
fn source_pick_fragment(options: &[SourceOption]) -> Markup {
    html! {
        @if options.is_empty() {
            div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
                (icon::link_icon("w-8 h-8"))
                p style="margin:var(--space-2) 0 0;font-size:var(--text-sm)" { "未找到匹配的来源单据" }
            }
        } @else {
            div class="product-select-list" {
                @for o in options {
                    div class="product-select-item" {
                        div class="product-select-info" {
                            div class="product-select-name" { (o.doc_number) }
                            div class="product-select-meta" {
                                span { (o.supplier_name) }
                                span class="product-select-sep" { "·" }
                                span { (o.extra) }
                            }
                        }
                        button type="button" class="btn btn-sm btn-primary"
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
            td class="line-num" { }
            td class="mono" { (product.product_code) }
            td { (product.pdt_name) }
            td style="color:var(--fg-2);font-size:var(--text-sm)" { (product.meta.specification) }
            td { input class="form-input" type="text" name="batch_no" placeholder="批次号" style="width:100%;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { input class="form-input num-input" type="number" min="0.01" step="any" name="quantity" placeholder="0" style="width:90px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { input class="form-input num-input" type="number" step="any" name="unit_cost" placeholder="0.00" style="width:100px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td class="line-subtotal" style="text-align:right;font-family:var(--font-mono);font-weight:600;white-space:nowrap" { "—" }
            td { input class="form-input" type="text" name="item_bin_id" placeholder="自动" style="width:80px;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm);background:var(--surface)" {} }
            td { button type="button" class="btn-remove-row" title="删除行"
                onclick="hsRemoveClosestEl(this,'tr');setTimeout(wmsStockInRenumber,50)" {
                (icon::x_icon("w-3.5 h-3.5"))
            } }
            input type="hidden" name="product_id" value=(product.product_id) {}
        }
    }
}
