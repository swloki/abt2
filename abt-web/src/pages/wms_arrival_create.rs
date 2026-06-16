use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use chrono::Local;
use maud::{html, Markup};
use serde::Deserialize;
use rust_decimal::Decimal;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::purchase::order::model::PurchaseOrderQuery;
use abt_core::purchase::enums::PurchaseOrderStatus;
use abt_core::master_data::supplier::SupplierStatus;
use abt_core::shared::types::{DomainError, PageParams};
use abt_core::wms::arrival_notice::{ArrivalNoticeService, CreateArrivalNoticeItemReq, CreateArrivalNoticeReq};
use abt_core::wms::warehouse::model::WarehouseFilter;
use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_arrival::*;
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
pub async fn get_arrival_create(
    _path: ArrivalCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let supplier_svc = state.supplier_service();
    let warehouse_svc = state.warehouse_service();

    let suppliers = supplier_svc
        .list(&service_ctx, &mut conn, SupplierQuery { name: None, status: Some(SupplierStatus::Qualified), category: None }, PageParams::new(1, 200))
        .await?;

    let warehouses = warehouse_svc
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
        .await?;

    let content = arrival_create_page(&suppliers.items, &warehouses.items, &claims.display_name);
    let page_html = admin_page(
        is_htmx,
        "新建来料通知",
        &claims,
        "inventory",
        ArrivalCreatePath::PATH,
        "库存管理",
        Some("新建来料通知"),
        content, &nav_filter,    );
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

/// HTMX: return a single item row fragment
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

// ── PO Import Handlers ──

#[derive(Debug, Deserialize)]
pub struct PoSearchParams {
    pub keyword: Option<String>,
}

/// HTMX: search confirmed POs for import
#[require_permission("INVENTORY", "create")]
pub async fn get_po_pick(
    ctx: RequestContext,
    Query(params): Query<PoSearchParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_order_service();
    let supplier_svc = state.supplier_service();

    let query = PurchaseOrderQuery::default();
    let kw = params.keyword.as_deref().unwrap_or("").trim().to_lowercase();

    let orders = svc
        .list(&service_ctx, &mut conn, query, PageParams::new(1, 50))
        .await
        .map(|r| r.items)
        .unwrap_or_default();

    // 过滤：只显示 Confirmed 状态 + 关键词匹配 doc_number
    let filtered: Vec<_> = orders.into_iter()
        .filter(|o| o.status == PurchaseOrderStatus::Confirmed)
        .filter(|o| kw.is_empty() || o.doc_number.to_lowercase().contains(&kw))
        .collect();

    // 批量获取供应商名
    let supplier_ids: Vec<i64> = filtered.iter().map(|o| o.supplier_id).collect();
    let suppliers = supplier_svc
        .list(&service_ctx, &mut conn, SupplierQuery::default(), PageParams::new(1, 200))
        .await
        .map(|r| r.items)
        .unwrap_or_default();
    let supplier_map: std::collections::HashMap<i64, String> = suppliers
        .into_iter()
        .map(|s| (s.id, s.name))
        .collect();

    Ok(Html(po_list_fragment(&filtered, &supplier_map).into_string()))
}

/// HTMX: return PO items as arrival item rows
#[require_permission("INVENTORY", "create")]
pub async fn get_po_items(
    path: ArrivalPoItemsPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let po_svc = state.purchase_order_service();
    let product_svc = state.product_service();

    let items = po_svc.list_items(&service_ctx, &mut conn, path.po_id).await?;
    let po = po_svc.get(&service_ctx, &mut conn, path.po_id).await?;

    // 批量获取产品信息
    let mut product_map = std::collections::HashMap::new();
    for item in &items {
        if !product_map.contains_key(&item.product_id) {
            if let Ok(p) = product_svc.get(&service_ctx, &mut conn, item.product_id).await {
                product_map.insert(item.product_id, p);
            }
        }
    }

    Ok(Html(po_items_fragment(&items, &product_map, po.supplier_id).into_string()))
}

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub struct ArrivalCreateForm {
    #[serde(deserialize_with = "empty_as_none")]
    pub purchase_order_id: Option<i64>,
    #[serde(deserialize_with = "empty_as_none")]
    pub supplier_id: Option<i64>,
    pub arrival_date: String,
    #[serde(deserialize_with = "empty_as_none")]
    pub warehouse_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub zone_id: Option<i64>,
    pub delivery_note: Option<String>,
    pub remark: Option<String>,
    pub items_json: String,
}

#[derive(Debug, Deserialize)]
struct ArrivalItemWeb {
    product_id: String,
    declared_qty: String,
    batch_no: Option<String>,
    #[serde(default)]
    order_item_id: Option<String>,
}

#[require_permission("INVENTORY", "create")]
pub async fn create_arrival(
    _path: ArrivalCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ArrivalCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.arrival_notice_service();

    let arrival_date = chrono::NaiveDate::parse_from_str(&form.arrival_date, "%Y-%m-%d")
        .map_err(|e| DomainError::validation(format!("无效的到货日期: {e}")))?;

    let web_items: Vec<ArrivalItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("物料明细数据无效: {e}")))?;

    if web_items.is_empty() {
        return Err(DomainError::validation("请添加至少一条物料明细").into());
    }

    let items: Vec<CreateArrivalNoticeItemReq> = web_items.into_iter().map(|it| {
        let product_id: i64 = it.product_id.parse()
            .map_err(|_| DomainError::validation("无效产品ID")).unwrap_or(0);
        let declared_qty: Decimal = it.declared_qty.parse()
            .map_err(|_| DomainError::validation("无效数量")).unwrap_or(Decimal::ZERO);
        let order_item_id = it.order_item_id
            .filter(|s| !s.is_empty())
            .and_then(|s| s.parse::<i64>().ok());
        CreateArrivalNoticeItemReq {
            order_item_id,
            product_id,
            declared_qty,
            batch_no: it.batch_no.filter(|s| !s.is_empty()),
        }
    }).collect();

    let req = CreateArrivalNoticeReq {
        purchase_order_id: form.purchase_order_id,
        supplier_id: form.supplier_id.ok_or_else(|| {
            DomainError::validation("请选择供应商")
        })?,
        arrival_date,
        warehouse_id: form.warehouse_id.ok_or_else(|| DomainError::validation("请选择仓库"))?,
        zone_id: form.zone_id,
        delivery_note: form.delivery_note.filter(|s| !s.is_empty()),
        remark: form.remark.filter(|s| !s.is_empty()).unwrap_or_default(),
        items,
    };

    let id = svc.create(&service_ctx, &mut conn, req).await?;

    let redirect = format!("{}/{}", ArrivalListPath::PATH, id);
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn arrival_create_page(
    suppliers: &[abt_core::master_data::supplier::model::Supplier],
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
    operator_name: &str,
) -> Markup {
    html! {
        div {
            a href=(format!("{}?restore=true", ArrivalListPath::PATH)) class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回来料通知列表"
            }

            div class="flex items-center justify-between mb-6" style="margin-bottom:var(--space-5)" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "新建来料通知" }
                div class="flex gap-3" {
                    span style="font-size:var(--text-xs);color:var(--muted);display:flex;align-items:center;gap:var(--space-2)" {
                        (icon::clock_icon("w-3.5 h-3.5"))
                        "操作员: " (operator_name)
                    }
                }
            }

            form hx-post=(ArrivalCreatePath::PATH) hx-swap="none" id="arrivalForm"
                onsubmit="return arrivalCollectItems()" {
                // ── 供应商信息 ──
                div class="bg-bg border border-border rounded p-6" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
                        (icon::building_icon("w-4 h-4"))
                        "供应商信息"
                    }
                    div class="wms-grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                        div class="form-group" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "供应商 " span class="required" { "*" } }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" id="supplier-select" name="supplier_id" required {
                                option value="" { "请选择供应商" }
                                @for s in suppliers {
                                    option value=(s.id) { (s.name) }
                                }
                            }
                        }
                        div class="form-group" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "联系人" }
                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" readonly tabindex="-1" placeholder="自动填充";
                        }
                        div class="form-group" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "联系电话" }
                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" readonly tabindex="-1" placeholder="自动填充";
                        }
                        div class="form-group" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "来源采购单" }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="purchase_order_id" {
                                option value="" { "请选择采购单（可选）" }
                            }
                        }
                    }
                }

                // ── 到货信息 ──
                div class="bg-bg border border-border rounded p-6" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
                        (icon::truck_icon("w-4 h-4"))
                        "到货信息"
                    }
                    div class="wms-grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                        div class="form-group" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "到货仓库 " span class="required" { "*" } }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="warehouse_id" required {
                                option value="" { "请选择仓库" }
                                @for w in warehouses {
                                    option value=(w.id) { (w.name) }
                                }
                            }
                        }
                        div class="form-group" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "到货库区" }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="zone_id" {
                                option value="" { "请选择库区" }
                            }
                        }
                        div class="form-group" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "到货日期 " span class="required" { "*" } }
                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="date" name="arrival_date" required value=(Local::now().format("%Y-%m-%d"));
                        }
                        div class="form-group" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "送货单号" }
                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="delivery_note" placeholder="请输入送货单号";
                        }
                    }
                }

                // ── 物料明细 ──
                div class="bg-bg border border-border rounded p-6" style="padding:0;overflow:hidden" {
                    div style="padding:var(--space-6) var(--space-6) var(--space-4)" {
                        div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
                            (icon::box_icon("w-4 h-4"))
                            "物料明细"
                            span id="arrival-item-count" style="margin-left:auto;font-size:var(--text-xs);font-weight:400;color:var(--muted)" { "共 0 项" }
                        }
                    }
                    div style="overflow-x:auto" {
                        table class="line-items-table" {
                            thead {
                                tr {
                                    th style="width:40px;text-align:center" { "行号" }
                                    th style="min-width:140px" { "产品编码" }
                                    th style="min-width:200px" { "产品名称" }
                                    th style="min-width:160px" { "规格" }
                                    th style="width:100px;text-align:right" { "申报数量 " span class="required" { "*" } }
                                    th style="width:140px" { "批次号" }
                                    th style="width:40px" { "操作" }
                                }
                            }
                            tbody id="arrival-item-tbody" {
                                // JS-managed dynamic rows
                            }
                        }
                    }
                    div class="p-3 flex items-center gap-2" {
                        button type="button" class="inline-flex items-center gap-2 rounded-sm text-accent text-sm cursor-pointer"
                            _="on click add .is-open to #product-modal" {
                            (icon::plus_icon("w-4 h-4"))
                            "添加物料"
                        }
                        button type="button" class="inline-flex items-center gap-2 rounded-sm text-accent text-sm cursor-pointer" style="margin-left:var(--space-3)"
                            _="on click add .is-open to #po-modal" {
                            (icon::download_icon("w-4 h-4"))
                            "从采购订单导入"
                        }
                    }
                }

                // ── 备注 ──
                div class="bg-bg border border-border rounded p-6" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
                        (icon::edit_icon("w-4 h-4"))
                        "备注"
                    }
                    textarea class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="remark" rows="3" placeholder="请输入备注信息" style="resize:vertical;width:100%;min-height:80px" {}
                }

                input type="hidden" name="purchase_order_id" id="arrival-po-id" value="" {}
                input type="hidden" name="items_json" id="arrival-items-json" value="[]" {}

                // ── Action Bar ──
                div class="action-bar" {
                    a href=(format!("{}?restore=true", ArrivalListPath::PATH)) class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-white text-fg border border-border hover:bg-surface" { "取消" }
                    button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-accent text-accent-on border-none hover:bg-accent-hover" {
                        (icon::check_circle_icon("w-4 h-4"))
                        "提交来料通知"
                    }
                }
            }
        }

        // ── Product Search Modal ──
        div id="product-modal" class="fixed z-[1000] grid place-items-center opacity-0"
            _="on click[me is event.target] remove .is-open" {
            div class="modal bg-bg rounded-xl w-[680px] flex flex-col overflow-hidden opacity-0-lg" onclick="event.stopPropagation()" {
                div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                    h2 { "选择物料" }
                    button type="button" style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                        _="on click remove .is-open from #product-modal" { "×" }
                }
                div class="overflow-y-auto flex-1 min-h-0 p-6" style="padding:0" hx-disinherit="hx-select" {
                    div class="flex gap-4 p-4 border-b" {
                        div class="flex-1 flex flex-col gap-[4px]" {
                            label class="text-[12px] font-medium text-fg-2" { "产品名称" }
                            input class="product-w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="name" placeholder="输入产品名称…"
                                hx-get=(ArrivalProductsPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-sync="this:replace"
                                hx-target="#arrival-product-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                        }
                        div class="flex-1 flex flex-col gap-[4px]" {
                            label class="text-[12px] font-medium text-fg-2" { "产品编码" }
                            input class="product-w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="code" placeholder="输入产品编码…"
                                hx-get=(ArrivalProductsPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-sync="this:replace"
                                hx-target="#arrival-product-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                        }
                        button type="button" class="border border-border rounded-sm bg-bg text-fg-2 text-sm cursor-pointer whitespace-nowrap"
                            hx-get=(ArrivalProductsPath::PATH)
                            hx-target="#arrival-product-results"
                            hx-swap="innerHTML"
                            _="on click set (.product-search-input)'s value to '' then trigger keyup on .product-search-input" {
                            "清除"
                        }
                    }
                    div id="arrival-product-results" {
                        div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
                            (icon::package_icon("w-8 h-8"))
                            p style="margin:var(--space-2) 0 0;font-size:var(--text-sm)" { "输入关键词搜索物料" }
                        }
                    }
                }
            }
        }

        // ── PO Import Modal ──
        div id="po-modal" class="fixed z-[1000] grid place-items-center opacity-0"
            _="on click[me is event.target] remove .is-open" {
            div class="modal bg-bg rounded-xl w-[680px] flex flex-col overflow-hidden opacity-0-lg" onclick="event.stopPropagation()" {
                div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                    h2 { "从采购订单导入" }
                    button type="button" style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                        _="on click remove .is-open from #po-modal" { "×" }
                }
                div class="overflow-y-auto flex-1 min-h-0 p-6" style="padding:0" hx-disinherit="hx-select" {
                    div class="flex gap-4 p-4 border-b" {
                        div class="flex-1 flex flex-col gap-[4px]" {
                            label class="text-[12px] font-medium text-fg-2" { "采购订单号" }
                            input class="product-w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" id="po-search-input" name="keyword" placeholder="输入PO编号搜索…"
                                hx-get=(ArrivalPoPickPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-sync="this:replace"
                                hx-target="#po-search-results"
                                hx-swap="innerHTML"
                                hx-include="#po-search-input" {}
                        }
                        button type="button" class="border border-border rounded-sm bg-bg text-fg-2 text-sm cursor-pointer whitespace-nowrap"
                            hx-get=(ArrivalPoPickPath::PATH)
                            hx-target="#po-search-results"
                            hx-swap="innerHTML"
                            _="on click set #po-search-input's value to '' then trigger keyup on #po-search-input" {
                            "清除"
                        }
                    }
                    div id="po-search-results" {
                        div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
                            (icon::package_icon("w-8 h-8"))
                            p style="margin:var(--space-2) 0 0;font-size:var(--text-sm)" { "输入PO编号搜索已确认的采购订单" }
                        }
                    }
                }
            }
        }

        // ── JS ──
        (maud::PreEscaped(r#"<script>
        function arrivalCalcSummary() {
            var tbody = document.getElementById('arrival-item-tbody');
            var rows = tbody.querySelectorAll('tr');
            document.getElementById('arrival-item-count').textContent = '共 ' + rows.length + ' 项';
        }

        function arrivalCollectItems() {
            var tbody = document.getElementById('arrival-item-tbody');
            var rows = tbody.querySelectorAll('tr');
            var items = [];
            rows.forEach(function(row) {
                var oiInput = row.querySelector('input[name="order_item_id"]');
                items.push({
                    product_id: row.querySelector('input[name="product_id"]').value,
                    declared_qty: row.querySelector('input[name="declared_qty"]').value || '0',
                    batch_no: row.querySelector('input[name="batch_no"]').value || null,
                    order_item_id: oiInput ? oiInput.value : null
                });
            });
            document.getElementById('arrival-items-json').value = JSON.stringify(items);
            if (items.length === 0) {
                alert('请至少添加一个物料');
                return false;
            }
            return true;
        }

        function arrivalRenumber() {
            var tbody = document.getElementById('arrival-item-tbody');
            var rows = tbody.querySelectorAll('tr');
            rows.forEach(function(row, i) {
                row.querySelector('.line-num').textContent = i + 1;
            });
            arrivalCalcSummary();
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
                    div class="flex items-center justify-between p-3 border-b" {
                        div class="product-select-info" {
                            div class="text-sm font-medium text-fg" { (p.pdt_name) }
                            div class="text-[12px] text-muted flex items-center gap-[6px] flex-wrap" {
                                span class="bg-surface rounded-sm" { (p.product_code) }
                                span class="product-select-sep" { "·" }
                                span { (p.meta.specification) }
                                span class="product-select-sep" { "·" }
                                span { (p.unit) }
                            }
                        }
                        button type="button" class="btn inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative-sm bg-accent text-accent-on border-none hover:bg-accent-hover"
                            hx-get=(format!("{}?product_id={}", ArrivalItemRowPath::PATH, p.product_id))
                            hx-target="#arrival-item-tbody"
                            hx-swap="beforeend"
                            _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .is-open from #product-modal then wait 50ms then call arrivalRenumber()" {
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
        tr {
            td class="text-muted text-xs text-center" { }
            td class="mono" { (product.product_code) }
            td { (product.pdt_name) }
            td style="color:var(--fg-2);font-size:var(--text-sm)" { (product.meta.specification) }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" min="0.01" step="any" name="declared_qty" placeholder="0" style="width:90px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="batch_no" placeholder="批次号" style="width:120px;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center" title="删除行"
                _="on click remove closest <tr/> then call arrivalRenumber()" {
                (icon::x_icon("w-3.5 h-3.5"))
            } }
            input type="hidden" name="product_id" value=(product.product_id) {}
        }
    }
}

/// PO search results fragment for import modal
fn po_list_fragment(
    orders: &[abt_core::purchase::order::model::PurchaseOrder],
    supplier_map: &std::collections::HashMap<i64, String>,
) -> Markup {
    html! {
        @if orders.is_empty() {
            div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
                (icon::package_icon("w-8 h-8"))
                p style="margin:var(--space-2) 0 0;font-size:var(--text-sm)" { "未找到已确认的采购订单" }
            }
        } @else {
            div class="product-select-list" {
                @for o in orders {
                    div class="flex items-center justify-between p-3 border-b" {
                        div class="product-select-info" {
                            div class="text-sm font-medium text-fg" { (o.doc_number) }
                            div class="text-[12px] text-muted flex items-center gap-[6px] flex-wrap" {
                                span class="bg-surface rounded-sm" { (supplier_map.get(&o.supplier_id).cloned().unwrap_or_else(|| "-".into())) }
                                span class="product-select-sep" { "·" }
                                span { (o.order_date.format("%Y-%m-%d")) }
                                span class="product-select-sep" { "·" }
                                span { "¥" (o.total_amount) }
                            }
                        }
                        button type="button" class="btn inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative-sm bg-accent text-accent-on border-none hover:bg-accent-hover"
                            hx-get=(ArrivalPoItemsPath { po_id: o.id }.to_string())
                            hx-target="#arrival-item-tbody"
                            hx-swap="beforeend"
                            _=(format!("on click set #arrival-po-id's value to '{}' then set #supplier-select's value to '{}' then remove .is-open from #po-modal end on htmx:afterRequest[detail.xhr.status < 400] wait 50ms then call arrivalRenumber()", o.id, o.supplier_id)) {
                            "导入"
                        }
                    }
                }
            }
        }
    }
}

/// PO items rendered as arrival item rows (appended to tbody)
fn po_items_fragment(
    items: &[abt_core::purchase::order::model::PurchaseOrderItem],
    product_map: &std::collections::HashMap<i64, abt_core::master_data::product::model::Product>,
    _supplier_id: i64,
) -> Markup {
    html! {
        @for item in items {
            @if let Some(product) = product_map.get(&item.product_id) {
                tr {
                    td class="text-muted text-xs text-center" { }
                    td class="mono" { (product.product_code) }
                    td { (product.pdt_name) }
                    td style="color:var(--fg-2);font-size:var(--text-sm)" { (product.meta.specification) }
                    td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" min="0.01" step="any" name="declared_qty" value=(item.quantity) style="width:90px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
                    td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="batch_no" placeholder="批次号" style="width:120px;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
                    td { button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center" title="删除行"
                        _="on click remove closest <tr/> then call arrivalRenumber()" {
                        (icon::x_icon("w-3.5 h-3.5"))
                    } }
                    input type="hidden" name="product_id" value=(item.product_id) {}
                    input type="hidden" name="order_item_id" value=(item.id) {}
                }
            }
        }
    }
}
