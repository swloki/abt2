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
        CreateArrivalNoticeItemReq {
            order_item_id: None,
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
            a href=(ArrivalListPath::PATH) class="back-link" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回来料通知列表"
            }

            div class="page-header" style="margin-bottom:var(--space-5)" {
                h1 class="page-title" { "新建来料通知" }
                div class="page-actions" {
                    span style="font-size:var(--text-xs);color:var(--muted);display:flex;align-items:center;gap:var(--space-2)" {
                        (icon::clock_icon("w-3.5 h-3.5"))
                        "操作员: " (operator_name)
                    }
                }
            }

            form hx-post=(ArrivalCreatePath::PATH) hx-swap="none" id="arrivalForm"
                onsubmit="return arrivalCollectItems()" {
                // ── 供应商信息 ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::building_icon("w-4 h-4"))
                        "供应商信息"
                    }
                    div class="wms-form-grid" {
                        div class="form-group" {
                            label class="form-label" { "供应商 " span class="required" { "*" } }
                            select class="form-select" name="supplier_id" required {
                                option value="" { "请选择供应商" }
                                @for s in suppliers {
                                    option value=(s.id) { (s.name) }
                                }
                            }
                        }
                        div class="form-group" {
                            label class="form-label" { "联系人" }
                            input class="form-input" type="text" readonly tabindex="-1" placeholder="自动填充";
                        }
                        div class="form-group" {
                            label class="form-label" { "联系电话" }
                            input class="form-input" type="text" readonly tabindex="-1" placeholder="自动填充";
                        }
                        div class="form-group" {
                            label class="form-label" { "来源采购单" }
                            select class="form-select" name="purchase_order_id" {
                                option value="" { "请选择采购单（可选）" }
                            }
                        }
                    }
                }

                // ── 到货信息 ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::truck_icon("w-4 h-4"))
                        "到货信息"
                    }
                    div class="wms-form-grid" {
                        div class="form-group" {
                            label class="form-label" { "到货仓库 " span class="required" { "*" } }
                            select class="form-select" name="warehouse_id" required {
                                option value="" { "请选择仓库" }
                                @for w in warehouses {
                                    option value=(w.id) { (w.name) }
                                }
                            }
                        }
                        div class="form-group" {
                            label class="form-label" { "到货库区" }
                            select class="form-select" name="zone_id" {
                                option value="" { "请选择库区" }
                            }
                        }
                        div class="form-group" {
                            label class="form-label" { "到货日期 " span class="required" { "*" } }
                            input class="form-input" type="date" name="arrival_date" required value=(Local::now().format("%Y-%m-%d"));
                        }
                        div class="form-group" {
                            label class="form-label" { "送货单号" }
                            input class="form-input" type="text" name="delivery_note" placeholder="请输入送货单号";
                        }
                    }
                }

                // ── 物料明细 ──
                div class="wms-form-section" style="padding:0;overflow:hidden" {
                    div style="padding:var(--space-6) var(--space-6) var(--space-4)" {
                        div class="form-section-title" {
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
                    div class="add-row-bar" {
                        button type="button" class="btn-add-row"
                            onclick="me('#product-modal').classAdd('is-open')" {
                            (icon::plus_icon("w-4 h-4"))
                            "添加物料"
                        }
                    }
                }

                // ── 备注 ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::edit_icon("w-4 h-4"))
                        "备注"
                    }
                    textarea class="form-input" name="remark" rows="3" placeholder="请输入备注信息" style="resize:vertical;width:100%;min-height:80px" {}
                }

                input type="hidden" name="items_json" id="arrival-items-json" value="[]" {}

                // ── Action Bar ──
                div class="action-bar" {
                    a href=(ArrivalListPath::PATH) class="btn btn-default" { "取消" }
                    button type="submit" class="btn btn-primary" {
                        (icon::check_circle_icon("w-4 h-4"))
                        "提交来料通知"
                    }
                }
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
                                hx-get=(ArrivalProductsPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-target="#arrival-product-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                        }
                        div class="product-search-field" {
                            label class="product-search-label" { "产品编码" }
                            input class="product-search-input" type="text" name="code" placeholder="输入产品编码…"
                                hx-get=(ArrivalProductsPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-target="#arrival-product-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                        }
                        button type="button" class="product-search-clear"
                            hx-get=(ArrivalProductsPath::PATH)
                            hx-target="#arrival-product-results"
                            hx-swap="innerHTML"
                            onclick="hsSetAndTrigger('.product-search-input','','keyup')" {
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
                items.push({
                    product_id: row.querySelector('input[name="product_id"]').value,
                    declared_qty: row.querySelector('input[name="declared_qty"]').value || '0',
                    batch_no: row.querySelector('input[name="batch_no"]').value || null
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
                            hx-get=(format!("{}?product_id={}", ArrivalItemRowPath::PATH, p.product_id))
                            hx-target="#arrival-item-tbody"
                            hx-swap="beforeend"
                            hx-on::after-request="hsRemove(null,'#product-modal','is-open');setTimeout(arrivalRenumber,50)" {
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
            td class="line-num" { }
            td class="mono" { (product.product_code) }
            td { (product.pdt_name) }
            td style="color:var(--fg-2);font-size:var(--text-sm)" { (product.meta.specification) }
            td { input class="form-input num-input" type="number" min="0.01" step="any" name="declared_qty" placeholder="0" style="width:90px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { input class="form-input" type="text" name="batch_no" placeholder="批次号" style="width:120px;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { button type="button" class="btn-remove-row" title="删除行"
                onclick="hsRemoveClosestEl(this,'tr');setTimeout(arrivalRenumber,50)" {
                (icon::x_icon("w-3.5 h-3.5"))
            } }
            input type="hidden" name="product_id" value=(product.product_id) {}
        }
    }
}
