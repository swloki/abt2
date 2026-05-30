use axum::extract::Query;
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::master_data::bom::model::*;
use abt_core::master_data::bom::{BomCategoryService, BomCommandService, BomNodeService};
use abt_core::master_data::product::model::ProductQuery;
use abt_core::master_data::product::ProductService;
use abt_core::shared::types::PageParams;
use abt_macros::require_permission;
use rust_decimal::Decimal;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::bom::{BomCreatePath, BomDetailPath, BomListPath, BomProductsPath};
use crate::utils::RequestContext;

// ── Query Params ──

#[derive(Debug, Deserialize)]
pub struct ProductSearchParams {
    pub name: Option<String>,
    pub code: Option<String>,
}

// ── Form request ──

#[derive(Debug, Deserialize)]
pub struct BomCreateForm {
    pub bom_name: String,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub bom_category_id: Option<i64>,
    pub remark: Option<String>,
    pub action: String,
    // Root node fields
    pub root_product_id: i64,
    pub root_quantity: String,
    pub root_unit: Option<String>,
    pub root_loss_rate: Option<String>,
    pub root_position: Option<String>,
    pub root_work_center: Option<String>,
    pub root_node_remark: Option<String>,
}

// ── Handlers ──

#[require_permission("BOM", "create")]
pub async fn get_bom_create(
    _path: BomCreatePath,
    ctx: RequestContext,
    headers: HeaderMap,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;

    // Load BOM categories for select dropdown
    let cat_svc = state.bom_category_service();
    let categories = cat_svc
        .list(
            &service_ctx,
            &mut conn,
            BomCategoryQuery::default(),
            PageParams::new(1, 200),
        )
        .await?;

    let content = bom_create_page(&categories.items);
    let page_html = admin_page(
        &headers,
        "新建物料清单",
        &claims,
        "md",
        BomCreatePath::PATH,
        "主数据管理",
        Some("新建物料清单"),
        content,
    );

    Ok(Html(page_html.into_string()))
}

/// HTMX: search products → return HTML fragment for root node selector
#[require_permission("PRODUCT", "read")]
pub async fn get_bom_products(
    ctx: RequestContext,
    Query(params): Query<ProductSearchParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.product_service();

    let filter = ProductQuery {
            name: params.name,
            code: params.code,
            status: None,
            owner_department_id: None,
            category_id: None,
        };
    let result = svc
        .list(&service_ctx, &mut conn, filter, PageParams::new(1, 20))
        .await?;

    Ok(Html(product_list_fragment(&result.items).into_string()))
}

/// POST: create BOM from form submission (HTMX)
#[require_permission("BOM", "create")]
pub async fn post_bom_create(
    _path: BomCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<BomCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    // 1. Create the BOM header
    let cmd_svc = state.bom_command_service();
    let bom_id = cmd_svc
        .create(
            &service_ctx,
            &mut conn,
            CreateBomReq {
                name: form.bom_name,
                bom_category_id: form.bom_category_id,
            },
        )
        .await?;

    // 2. Add root node (parent_id = 0)
    let node_svc = state.bom_node_service();
    let quantity: Decimal = form
        .root_quantity
        .parse()
        .unwrap_or(Decimal::ONE);
    let loss_rate: Decimal = form
        .root_loss_rate
        .as_deref()
        .and_then(|s| s.parse().ok())
        .unwrap_or(Decimal::ZERO);

    node_svc
        .add_node(
            &service_ctx,
            &mut conn,
            bom_id,
            NewBomNode {
                product_id: form.root_product_id,
                quantity,
                parent_id: 0,
                loss_rate,
                order: 1,
                unit: form.root_unit.filter(|s| !s.is_empty()),
                remark: form.root_node_remark.filter(|s| !s.is_empty()),
                position: form.root_position.filter(|s| !s.is_empty()),
                work_center: form.root_work_center.filter(|s| !s.is_empty()),
                properties: None,
            },
        )
        .await?;

    // 3. If action == "publish", publish the BOM
    if form.action == "publish" {
        cmd_svc.publish(&service_ctx, &mut conn, bom_id).await?;
    }

    let redirect = BomDetailPath { id: bom_id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn bom_create_page(categories: &[BomCategory]) -> Markup {
    html! {
        div x-data="bomForm()" {
            // ── Page Header ──
            div class="page-header" {
                a class="back-link" href=(BomListPath::PATH) {
                    (icon::arrow_left_icon("w-4 h-4"))
                    "返回物料清单列表"
                }
                h1 class="page-title" { "新建物料清单" }
            }

            form id="bom-form"
                  hx-post=(BomCreatePath::PATH)
                  hx-swap="none" {
                input type="hidden" name="root_product_id" x-model="rootNode.product_id" required;

                // ── Section: 基本信息 ──
                div class="data-card" style="margin-bottom:var(--space-4)" {
                    div class="form-section-title" { "基本信息" }
                    div class="form-grid" {
                        div class="form-field" {
                            label { "BOM名称 " span style="color:var(--danger)" { "*" } }
                            input type="text" name="bom_name" required placeholder="请输入BOM名称" {}
                        }
                        div class="form-field" {
                            label { "BOM分类" }
                            select name="bom_category_id" {
                                option value="" { "-- 请选择 --" }
                                @for cat in categories {
                                    option value=(cat.bom_category_id) { (cat.bom_category_name) }
                                }
                            }
                        }
                        div class="form-field field-full" {
                            label { "备注" }
                            textarea name="remark" placeholder="请输入备注信息…"
                                style="width:100%;min-height:80px;resize:vertical" {}
                        }
                    }
                }

                // ── Section: 物料节点 (root node only for v1) ──
                div class="data-card" style="padding:0;overflow:hidden;margin-bottom:var(--space-4)" {
                    div style="padding:var(--space-5) var(--space-5) var(--space-3);display:flex;justify-content:space-between;align-items:center" {
                        span class="form-section-title" style="margin:0;padding:0;border:none" { "物料节点" }
                        button type="button" class="btn btn-sm btn-primary"
                            x-on:click="productModalOpen = true" {
                            (icon::plus_icon("w-3.5 h-3.5"))
                            "选择根节点物料"
                        }
                    }
                    div style="overflow-x:auto" {
                        table class="data-table" style="min-width:900px" {
                            thead {
                                tr {
                                    th style="width:60px;text-align:center" { "排序" }
                                    th { "物料编码" }
                                    th { "物料名称" }
                                    th { "规格型号" }
                                    th style="width:100px" { "用量" }
                                    th style="width:70px" { "单位" }
                                    th style="width:90px" { "损耗率%" }
                                    th style="width:100px" { "位置" }
                                    th style="width:100px" { "工作中心" }
                                    th { "备注" }
                                    th style="width:50px" { }
                                }
                            }
                            tbody {
                                tr x-show="rootNode.product_id > 0" {
                                    td class="line-num" { "1" }
                                    td class="mono" x-text="rootNode.product_code" {}
                                    td x-text="rootNode.product_name" {}
                                    td x-text="rootNode.specification" {}
                                    td {
                                        input class="form-input" type="number" name="root_quantity"
                                            x-model="rootNode.quantity" min="0.01" step="0.01"
                                            style="width:80px;text-align:right;padding:4px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {}
                                    }
                                    td {
                                        input class="form-input" type="text" name="root_unit"
                                            x-model="rootNode.unit" readonly
                                            style="width:60px;text-align:center;padding:4px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm);background:var(--surface)" {}
                                    }
                                    td {
                                        input class="form-input" type="number" name="root_loss_rate"
                                            x-model="rootNode.loss_rate" min="0" step="0.1"
                                            style="width:70px;text-align:right;padding:4px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {}
                                    }
                                    td {
                                        input class="form-input" type="text" name="root_position"
                                            x-model="rootNode.position"
                                            style="width:90px;padding:4px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {}
                                    }
                                    td {
                                        input class="form-input" type="text" name="root_work_center"
                                            x-model="rootNode.work_center"
                                            style="width:90px;padding:4px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {}
                                    }
                                    td {
                                        input class="form-input" type="text" name="root_node_remark"
                                            x-model="rootNode.remark"
                                            style="width:100%;padding:4px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {}
                                    }
                                    td {
                                        button type="button" class="btn-remove-row"
                                            x-on:click="clearRootNode()" title="清除" {
                                            (icon::x_icon("w-3.5 h-3.5"))
                                        }
                                    }
                                }
                                tr x-show="rootNode.product_id === 0 || rootNode.product_id === null" {
                                    td colspan="11" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "请选择根节点物料"
                                    }
                                }
                            }
                        }
                    }
                }

                // ── Action Bar ──
                div class="create-action-bar" {
                    a class="btn btn-default" href=(BomListPath::PATH) { "取消" }
                    div style="display:flex;gap:var(--space-3)" {
                        button type="submit" class="btn btn-default"
                            x-on:click="document.querySelector('input[name=action]').value = 'draft'"
                            x-bind:disabled="!rootNode.product_id" {
                            "保存草稿"
                        }
                        button type="submit" class="btn btn-primary"
                            x-on:click="document.querySelector('input[name=action]').value = 'publish'"
                            x-bind:disabled="!rootNode.product_id" {
                            "发布"
                        }
                    }
                    input type="hidden" name="action" value="draft" {}
                }
            }

            // ── Product Selection Modal ──
            div class="modal-overlay"
                x-bind:class="{ 'is-open': productModalOpen }"
                x-on:click="productModalOpen = false" {
                div class="modal modal-lg" x-on:click="event.stopPropagation()" {
                    div class="modal-head" {
                        h2 { "选择物料" }
                        button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                            x-on:click="productModalOpen = false" { "×" }
                    }
                    div class="modal-body" style="padding:0" {
                        div class="product-search-bar" {
                            div class="product-search-field" {
                                label class="product-search-label" { "产品名称" }
                                input class="product-search-input" type="text" name="name" placeholder="输入产品名称…"
                                    hx-get=(BomProductsPath::PATH)
                                    hx-trigger="keyup changed delay:300ms"
                                    hx-target="#bom-product-search-results"
                                    hx-swap="innerHTML"
                                    hx-include=".product-search-bar" {}
                            }
                            div class="product-search-field" {
                                label class="product-search-label" { "产品编码" }
                                input class="product-search-input" type="text" name="code" placeholder="输入产品编码…"
                                    hx-get=(BomProductsPath::PATH)
                                    hx-trigger="keyup changed delay:300ms"
                                    hx-target="#bom-product-search-results"
                                    hx-swap="innerHTML"
                                    hx-include=".product-search-bar" {}
                            }
                            button type="button" class="product-search-clear"
                                hx-get=(BomProductsPath::PATH)
                                hx-target="#bom-product-search-results"
                                hx-swap="innerHTML"
                                onclick="document.querySelectorAll('.product-search-input').forEach(function(i){i.value=''})" {
                                "清除"
                            }
                        }
                        div id="bom-product-search-results" style="max-height:320px;overflow-y:auto"
                            hx-get=(BomProductsPath::PATH)
                            hx-trigger="intersect once"
                            hx-swap="innerHTML" {
                            div style="display:flex;align-items:center;justify-content:center;padding:var(--space-8);color:var(--muted)" {
                                "加载中…"
                            }
                        }
                    }
                }
            }

            // ── Alpine.js component ──
            script {
                (maud::PreEscaped(r#"
                function bomForm() {
                    return {
                        productModalOpen: false,
                        rootNode: {
                            product_id: 0,
                            product_code: '',
                            product_name: '',
                            specification: '',
                            unit: '',
                            quantity: '1',
                            loss_rate: '0',
                            position: '',
                            work_center: '',
                            remark: ''
                        },
                        selectRootProduct(product) {
                            this.rootNode.product_id = product.product_id;
                            this.rootNode.product_code = product.product_code;
                            this.rootNode.product_name = product.product_name;
                            this.rootNode.specification = product.specification || '';
                            this.rootNode.unit = product.unit || '';
                            this.productModalOpen = false;
                        },
                        clearRootNode() {
                            this.rootNode.product_id = 0;
                            this.rootNode.product_code = '';
                            this.rootNode.product_name = '';
                            this.rootNode.specification = '';
                            this.rootNode.unit = '';
                            this.rootNode.quantity = '1';
                            this.rootNode.loss_rate = '0';
                            this.rootNode.position = '';
                            this.rootNode.work_center = '';
                            this.rootNode.remark = '';
                        }
                    };
                }
                "#))
            }
        }
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
                    @let product_json = serde_json::json!({
                        "product_id": p.product_id,
                        "product_code": &p.product_code,
                        "product_name": &p.pdt_name,
                        "specification": &p.meta.specification,
                        "unit": &p.unit,
                    }).to_string();
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
                            data-product=(product_json)
                            x-on:click="selectRootProduct(JSON.parse($el.dataset.product))" {
                            "选择"
                        }
                    }
                }
            }
        }
    }
}
