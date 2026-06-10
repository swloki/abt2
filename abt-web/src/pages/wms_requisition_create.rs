use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use chrono::Local;
use maud::{html, Markup};
use serde::Deserialize;
use rust_decimal::Decimal;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::shared::types::{DomainError, PageParams};
use abt_core::wms::material_requisition::{CreateManualReq, CreateManualItemReq, MaterialRequisitionService};
use abt_core::wms::warehouse::model::WarehouseFilter;
use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_requisition::*;
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
pub async fn get_requisition_create(
    _path: RequisitionCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let warehouse_svc = state.warehouse_service();

    let warehouses = warehouse_svc
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
        .await?;

    let content = requisition_create_page(&warehouses.items);
    let page_html = admin_page(
        is_htmx,
        "新建领料单",
        &claims,
        "inventory",
        RequisitionCreatePath::PATH,
        "库存管理",
        Some("新建领料单"),
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
struct RequisitionItemWeb {
    product_id: String,
    requested_qty: String,
}

#[derive(Debug, Deserialize)]
pub struct RequisitionCreateForm {
    #[serde(default, deserialize_with = "empty_as_none")]
    pub work_order_id: Option<i64>,
    #[serde(deserialize_with = "empty_as_none")]
    pub warehouse_id: Option<i64>,
    pub requisition_date: String,
    pub items_json: String,
}

#[require_permission("INVENTORY", "create")]
pub async fn create_requisition(
    _path: RequisitionCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<RequisitionCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.material_requisition_service();

    let requisition_date = chrono::NaiveDate::parse_from_str(&form.requisition_date, "%Y-%m-%d")
        .map_err(|e| DomainError::validation(format!("Invalid date: {e}")))?;

    let warehouse_id = form.warehouse_id
        .ok_or_else(|| DomainError::validation("Please select a warehouse"))?;

    // If work_order_id provided, use create_for_work_order
    if let Some(wo_id) = form.work_order_id {
        if wo_id > 0 {
            let _id = svc.create_for_work_order(&service_ctx, &mut conn, wo_id).await
                .map_err(|e| {
                    if matches!(e, DomainError::NotFound(_)) {
                        DomainError::validation(format!("工单 {} 不存在", wo_id))
                    } else {
                        e
                    }
                })?;
            let redirect = RequisitionListPath.to_string();
            return Ok(([("HX-Redirect", redirect)], Html(String::new())));
        }
    }

    // Otherwise, manual create with items
    let web_items: Vec<RequisitionItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("Invalid item data: {e}")))?;

    let items: Vec<CreateManualItemReq> = web_items.into_iter().map(|it| {
        let product_id: i64 = it.product_id.parse().unwrap_or(0);
        let requested_qty: Decimal = it.requested_qty.parse().unwrap_or(Decimal::ZERO);
        CreateManualItemReq { product_id, requested_qty }
    }).collect();

    let req = CreateManualReq {
        warehouse_id,
        requisition_date,
        remark: None,
        items,
    };

    let _id = svc.create_manual(&service_ctx, &mut conn, req).await?;

    let redirect = RequisitionListPath.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn requisition_create_page(
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
    html! {
        div {
            a href=(RequisitionListPath::PATH) class="back-link" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回领料单列表"
            }

            div class="page-header" style="margin-bottom:var(--space-5)" {
                h1 class="page-title" { "新建领料单" }
            }

            div class="workflow-steps" {
                div class="wf-step current" { span class="wf-dot" {} "草稿" }
                div class="wf-line" {}
                div class="wf-step" { span class="wf-dot" {} "已确认" }
                div class="wf-line" {}
                div class="wf-step" { span class="wf-dot" {} "已发料" }
            }

            form hx-post=(RequisitionCreatePath::PATH) hx-swap="none" id="requisitionForm"
                onsubmit="return reqCollectItems()" {
                // Basic info
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::clipboard_document_icon("w-4 h-4"))
                        "领料信息"
                    }
                    div class="wms-form-grid" {
                        div class="form-group" {
                            label class="form-label" { "领料仓库 " span class="required" { "*" } }
                            select class="form-select" name="warehouse_id" required {
                                option value="" { "请选择仓库" }
                                @for w in warehouses {
                                    option value=(w.id) { (w.name) }
                                }
                            }
                        }
                        div class="form-group" {
                            label class="form-label" { "领料日期 " span class="required" { "*" } }
                            input class="form-input" type="date" name="requisition_date" required value=(Local::now().format("%Y-%m-%d")) {}
                        }
                        div class="form-group" {
                            label class="form-label" { "关联工单（可选）" }
                            input class="form-input" type="number" name="work_order_id" placeholder="留空为手动创建";
                        }
                        div class="form-group" {
                            label class="form-label" { "操作员" }
                            input class="form-input" type="text" readonly style="background:var(--surface)" value="admin";
                        }
                    }
                }

                // Line items
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::box_icon("w-4 h-4"))
                        "领料明细"
                        span id="req-item-count" style="margin-left:auto;font-size:var(--text-xs);font-weight:400;color:var(--muted)" { "共 0 项" }
                    }
                    div style="overflow-x:auto" {
                        table class="line-items-table" {
                            thead {
                                tr {
                                    th style="width:40px;text-align:center" { "行号" }
                                    th style="min-width:140px" { "产品编码" }
                                    th style="min-width:200px" { "产品名称" }
                                    th style="min-width:160px" { "规格" }
                                    th style="width:110px;text-align:right" { "请求数量 " span class="required" { "*" } }
                                    th style="width:40px" { }
                                }
                            }
                            tbody id="req-item-tbody" { }
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

                input type="hidden" name="items_json" id="req-items-json" value="[]" {}

                // Actions
                div class="action-bar" {
                    a href=(RequisitionListPath::PATH) class="btn btn-default" { "取消" }
                    button type="submit" class="btn btn-primary" {
                        (icon::check_circle_icon("w-4 h-4"))
                        "提交领料单"
                    }
                }
            }
        }

        // Product Search Modal
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
                                hx-get=(RequisitionProductsPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-target="#req-product-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                        }
                        div class="product-search-field" {
                            label class="product-search-label" { "产品编码" }
                            input class="product-search-input" type="text" name="code" placeholder="输入产品编码…"
                                hx-get=(RequisitionProductsPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-target="#req-product-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                        }
                        button type="button" class="product-search-clear"
                            hx-get=(RequisitionProductsPath::PATH)
                            hx-target="#req-product-results"
                            hx-swap="innerHTML"
                            onclick="hsSetAndTrigger('.product-search-input','','keyup')" {
                            "清除"
                        }
                    }
                    div id="req-product-results" {
                        div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
                            (icon::package_icon("w-8 h-8"))
                            p style="margin:var(--space-2) 0 0;font-size:var(--text-sm)" { "输入关键词搜索物料" }
                        }
                    }
                }
            }
        }

        // JS
        (maud::PreEscaped(r#"<script>
        function reqCalcSummary() {
            var tbody = document.getElementById('req-item-tbody');
            var rows = tbody.querySelectorAll('tr');
            document.getElementById('req-item-count').textContent = '共 ' + rows.length + ' 项';
        }

        function reqRenumber() {
            var tbody = document.getElementById('req-item-tbody');
            var rows = tbody.querySelectorAll('tr');
            rows.forEach(function(row, i) {
                row.querySelector('.line-num').textContent = i + 1;
            });
            reqCalcSummary();
        }

        function reqCollectItems() {
            var tbody = document.getElementById('req-item-tbody');
            var rows = tbody.querySelectorAll('tr');
            var items = [];
            rows.forEach(function(row) {
                items.push({
                    product_id: row.querySelector('input[name="product_id"]').value,
                    requested_qty: row.querySelector('input[name="requested_qty"]').value || '0'
                });
            });
            document.getElementById('req-items-json').value = JSON.stringify(items);
            if (items.length === 0) {
                alert('请至少添加一个物料');
                return false;
            }
            return true;
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
                            hx-get=(format!("{}?product_id={}", RequisitionItemRowPath::PATH, p.product_id))
                            hx-target="#req-item-tbody"
                            hx-swap="beforeend"
                            hx-on::after-request="hsRemove(null,'#product-modal','is-open');setTimeout(reqRenumber,50)" {
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
            td { input class="form-input num-input" type="number" min="0.01" step="any" name="requested_qty" placeholder="0" style="width:100px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { button type="button" class="btn-remove-row" title="删除行"
                onclick="hsRemoveClosestEl(this,'tr');setTimeout(reqRenumber,50)" {
                (icon::x_icon("w-3.5 h-3.5"))
            } }
            input type="hidden" name="product_id" value=(product.product_id) {}
        }
    }
}
