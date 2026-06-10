use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use rust_decimal::Decimal;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::shared::types::{DomainError, PageParams};
use abt_core::wms::form_conversion::FormConversionService;
use abt_core::wms::form_conversion::model::{CreateConversionReq, CreateConversionItemReq};
use abt_core::wms::enums::ConversionDir;
use abt_core::wms::warehouse::model::WarehouseFilter;
use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_conversion::*;
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
pub async fn get_conversion_create(
    _path: ConversionCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let warehouse_svc = state.warehouse_service();

    let warehouses = warehouse_svc
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
        .await
        .map(|r| r.items)
        .unwrap_or_default();

    let content = conversion_create_page(&warehouses);
    let page_html = admin_page(
        is_htmx,
        "新建形态转换单",
        &claims,
        "inventory",
        ConversionCreatePath::PATH,
        "库存管理",
        None,
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
struct ConversionItemWeb {
    product_id: String,
    quantity: String,
    unit_cost: Option<String>,
    batch_no: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConversionCreateForm {
    #[serde(deserialize_with = "empty_as_none")]
    pub warehouse_id: Option<i64>,
    pub conversion_date: String,
    pub remark: Option<String>,
    pub consume_json: String,
    pub produce_json: String,
}

#[require_permission("INVENTORY", "create")]
pub async fn create_conversion(
    _path: ConversionCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ConversionCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.form_conversion_service();

    let consume_items: Vec<ConversionItemWeb> = serde_json::from_str(&form.consume_json)
        .map_err(|e| DomainError::validation(format!("消耗物料数据无效: {e}")))?;
    let produce_items: Vec<ConversionItemWeb> = serde_json::from_str(&form.produce_json)
        .map_err(|e| DomainError::validation(format!("产出物料数据无效: {e}")))?;

    if consume_items.is_empty() && produce_items.is_empty() {
        return Err(DomainError::validation("请至少添加一行消耗物料或产出物料").into());
    }

    let mut items: Vec<CreateConversionItemReq> = consume_items.into_iter().map(|it| {
        let product_id: i64 = it.product_id.parse().unwrap_or(0);
        let quantity: Decimal = it.quantity.parse().unwrap_or(Decimal::ZERO);
        let unit_cost: Decimal = it.unit_cost.as_ref().and_then(|s| s.parse().ok()).unwrap_or(Decimal::ZERO);
        CreateConversionItemReq {
            direction: ConversionDir::Consume,
            product_id,
            quantity,
            unit_cost,
            batch_no: it.batch_no.filter(|s| !s.is_empty()),
        }
    }).collect();

    items.extend(produce_items.into_iter().map(|it| {
        let product_id: i64 = it.product_id.parse().unwrap_or(0);
        let quantity: Decimal = it.quantity.parse().unwrap_or(Decimal::ZERO);
        let unit_cost: Decimal = it.unit_cost.as_ref().and_then(|s| s.parse().ok()).unwrap_or(Decimal::ZERO);
        CreateConversionItemReq {
            direction: ConversionDir::Produce,
            product_id,
            quantity,
            unit_cost,
            batch_no: it.batch_no.filter(|s| !s.is_empty()),
        }
    }));

    let conversion_date = chrono::NaiveDate::parse_from_str(&form.conversion_date, "%Y-%m-%d")
        .map_err(|_| DomainError::validation("无效的转换日期格式"))?;

    let warehouse_id = form.warehouse_id
        .ok_or_else(|| DomainError::validation("请选择转换仓库"))?;

    let req = CreateConversionReq {
        warehouse_id,
        conversion_date,
        remark: form.remark.filter(|s| !s.is_empty()).unwrap_or_default(),
        items,
    };

    let _id = svc.create(&service_ctx, &mut conn, req).await?;

    let redirect = ConversionListPath.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn conversion_create_page(
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
    html! {
        div {
            a href=(ConversionListPath::PATH) class="back-link" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回形态转换列表"
            }

            div class="page-header" {
                h1 class="page-title" { "新建形态转换单" }
            }

            div class="workflow-steps" {
                div class="wf-step current" { span class="wf-dot" {} "草稿" }
                div class="wf-line" {}
                div class="wf-step" { span class="wf-dot" {} "已完成" }
            }

            form hx-post=(ConversionCreatePath::PATH) hx-swap="none" id="conversionForm"
                onsubmit="return conversionCollectItems()" {
                // ── Basic Info ──
                div class="wms-form-section" {
                    h3 class="form-section-title" { "转换信息" }
                    div class="wms-form-grid" {
                        div class="form-field" {
                            label class="form-label" { "仓库 " span class="required" { "*" } }
                            select class="form-select" name="warehouse_id" required {
                                option value="" { "请选择仓库" }
                                @for w in warehouses {
                                    option value=(w.id) { (w.name) }
                                }
                            }
                        }
                        div class="form-field" {
                            label class="form-label" { "转换日期 " span class="required" { "*" } }
                            input class="form-input" type="date" name="conversion_date" required {}
                        }
                        div class="form-field" style="grid-column:span 2" {
                            label class="form-label" { "备注" }
                            input class="form-input" type="text" name="remark" {}
                        }
                    }
                }

                // ── Consume Items ──
                div class="wms-form-section" {
                    h3 class="form-section-title" {
                        "消耗物料 "
                        span style="display:inline-flex;align-items:center;padding:3px 10px;border-radius:9999px;font-size:12px;font-weight:600;background:#fff2f0;color:var(--danger)" { "消耗" }
                        span id="consume-item-count" style="margin-left:auto;font-size:var(--text-xs);font-weight:400;color:var(--muted)" { "共 0 项" }
                    }
                    div class="data-card" {
                        table class="data-table" {
                            thead {
                                tr {
                                    th style="width:40px" { "行号" }
                                    th { "产品编码" }
                                    th { "产品名称" }
                                    th { "规格" }
                                    th style="width:100px" { "数量 " span class="required" { "*" } }
                                    th style="width:110px" { "单位成本" }
                                    th style="width:120px" { "批次号" }
                                    th style="width:40px" { }
                                }
                            }
                            tbody id="consume-item-tbody" { }
                        }
                    }
                    button type="button" class="add-row-btn"
                        onclick="conversionOpenModal('consume')" {
                        (icon::plus_icon("w-3.5 h-3.5"))
                        "添加消耗行"
                    }
                }

                // ── Produce Items ──
                div class="wms-form-section" {
                    h3 class="form-section-title" {
                        "产出物料 "
                        span style="display:inline-flex;align-items:center;padding:3px 10px;border-radius:9999px;font-size:12px;font-weight:600;background:#f0fff0;color:var(--success)" { "产出" }
                        span id="produce-item-count" style="margin-left:auto;font-size:var(--text-xs);font-weight:400;color:var(--muted)" { "共 0 项" }
                    }
                    div class="data-card" {
                        table class="data-table" {
                            thead {
                                tr {
                                    th style="width:40px" { "行号" }
                                    th { "产品编码" }
                                    th { "产品名称" }
                                    th { "规格" }
                                    th style="width:100px" { "数量 " span class="required" { "*" } }
                                    th style="width:110px" { "单位成本" }
                                    th style="width:120px" { "批次号" }
                                    th style="width:40px" { }
                                }
                            }
                            tbody id="produce-item-tbody" { }
                        }
                    }
                    button type="button" class="add-row-btn"
                        onclick="conversionOpenModal('produce')" {
                        (icon::plus_icon("w-3.5 h-3.5"))
                        "添加产出行"
                    }
                }

                input type="hidden" name="consume_json" id="consume-json" value="[]" {}
                input type="hidden" name="produce_json" id="produce-json" value="[]" {}
                div id="conversion-item-target" style="display:none" { }

                // ── Actions ──
                div class="create-action-bar" {
                    a href=(ConversionListPath::PATH) class="btn btn-default" { "取消" }
                    button type="submit" class="btn btn-primary" { "提交" }
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
                                hx-get=(ConversionProductsPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-target="#conversion-product-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                        }
                        div class="product-search-field" {
                            label class="product-search-label" { "产品编码" }
                            input class="product-search-input" type="text" name="code" placeholder="输入产品编码…"
                                hx-get=(ConversionProductsPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-target="#conversion-product-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                        }
                        button type="button" class="product-search-clear"
                            hx-get=(ConversionProductsPath::PATH)
                            hx-target="#conversion-product-results"
                            hx-swap="innerHTML"
                            onclick="hsSetAndTrigger('.product-search-input','','keyup')" {
                            "清除"
                        }
                    }
                    div id="conversion-product-results" {
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
        var conversionTarget = 'consume'; // 'consume' or 'produce'

        function conversionOpenModal(target) {
            conversionTarget = target;
            me('#product-modal').classAdd('is-open');
        }

        function conversionRenumber(tbodyId) {
            var tbody = document.getElementById(tbodyId);
            var rows = tbody.querySelectorAll('tr');
            rows.forEach(function(row, i) {
                row.querySelector('.line-num').textContent = i + 1;
            });
            var countId = tbodyId === 'consume-item-tbody' ? 'consume-item-count' : 'produce-item-count';
            document.getElementById(countId).textContent = '共 ' + rows.length + ' 项';
        }

        function conversionCollectItems() {
            var consumeTbody = document.getElementById('consume-item-tbody');
            var produceTbody = document.getElementById('produce-item-tbody');
            var consumeItems = [];
            var produceItems = [];

            consumeTbody.querySelectorAll('tr').forEach(function(row) {
                consumeItems.push({
                    product_id: row.querySelector('input[name="product_id"]').value,
                    quantity: row.querySelector('input[name="quantity"]').value || '0',
                    unit_cost: row.querySelector('input[name="unit_cost"]').value || null,
                    batch_no: row.querySelector('input[name="batch_no"]').value || null
                });
            });
            produceTbody.querySelectorAll('tr').forEach(function(row) {
                produceItems.push({
                    product_id: row.querySelector('input[name="product_id"]').value,
                    quantity: row.querySelector('input[name="quantity"]').value || '0',
                    unit_cost: row.querySelector('input[name="unit_cost"]').value || null,
                    batch_no: row.querySelector('input[name="batch_no"]').value || null
                });
            });

            document.getElementById('consume-json').value = JSON.stringify(consumeItems);
            document.getElementById('produce-json').value = JSON.stringify(produceItems);

            if (consumeItems.length === 0 && produceItems.length === 0) {
                alert('请至少添加一行消耗物料或产出物料');
                return false;
            }
            return true;
        }

        function conversionAfterAdd() {
            var target = document.getElementById('conversion-item-target');
            var tbodyId = conversionTarget === 'consume' ? 'consume-item-tbody' : 'produce-item-tbody';
            var tbody = document.getElementById(tbodyId);
            while (target.firstChild) {
                tbody.appendChild(target.firstChild);
            }
            hsRemove(null, '#product-modal', 'is-open');
            conversionRenumber(tbodyId);
        }

        function conversionRemoveRow(btn) {
            hsRemoveClosestEl(btn, 'tr');
            conversionRenumber('consume-item-tbody');
            conversionRenumber('produce-item-tbody');
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
                            hx-get=(format!("{}?product_id={}", ConversionItemRowPath::PATH, p.product_id))
                            hx-target="#conversion-item-target"
                            hx-swap="beforeend"
                            hx-on::after-request="conversionAfterAdd()" {
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
            td { input class="form-input num-input" type="number" min="0.01" step="any" name="quantity" placeholder="0" style="width:90px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { input class="form-input num-input" type="number" step="0.01" name="unit_cost" placeholder="0.00" style="width:100px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { input class="form-input" type="text" name="batch_no" placeholder="批次号" style="width:100px;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { button type="button" class="btn-remove-row" title="删除行"
                onclick="conversionRemoveRow(this)" {
                (icon::x_icon("w-3.5 h-3.5"))
            } }
            input type="hidden" name="product_id" value=(product.product_id) {}
        }
    }
}
