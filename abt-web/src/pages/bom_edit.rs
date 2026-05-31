use std::collections::{HashMap, HashSet};

use axum::extract::Query;
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use rust_decimal::Decimal;
use serde::Deserialize;

use abt_core::master_data::bom::model::*;
use abt_core::master_data::bom::{
    BomCategoryService, BomCommandService, BomNodeService, BomQueryService,
};
use abt_core::master_data::product::model::ProductQuery;
use abt_core::master_data::product::ProductService;
use abt_core::shared::types::PageParams;

use abt_macros::require_permission;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::bom::{
    BomEditPath, BomListPath, BomNodeMovePath, BomNodePath, BomNodesPath, BomProductsPath,
    BomPublishPath, BomSaveAsPath, BomUpdateCategoryPath,
};
use crate::utils::RequestContext;

// ── Query Params ──

#[derive(Debug, Deserialize)]
pub struct ProductSearchParams {
    pub name: Option<String>,
    pub code: Option<String>,
}

// ── Form requests ──

#[derive(Debug, Deserialize)]
pub struct AddNodeForm {
    pub product_id: i64,
    pub parent_id: i64,
    pub quantity: String,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub loss_rate: Option<String>,
    #[serde(default)]
    pub position: Option<String>,
    #[serde(default)]
    pub work_center: Option<String>,
    #[serde(default)]
    pub remark: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNodeForm {
    pub quantity: Option<String>,
    #[serde(default)]
    pub loss_rate: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub position: Option<String>,
    #[serde(default)]
    pub work_center: Option<String>,
    #[serde(default)]
    pub remark: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SaveAsForm {
    pub new_name: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCategoryForm {
    pub bom_category_id: Option<i64>,
}

// ── Handlers ──

#[require_permission("BOM", "update")]
pub async fn get_bom_edit(
    path: BomEditPath,
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

    let bom_svc = state.bom_query_service();
    let product_svc = state.product_service();
    let category_svc = state.bom_category_service();

    let bom = bom_svc.get(&service_ctx, &mut conn, path.id).await?;

    // Resolve product names for all nodes
    let product_ids: Vec<i64> = bom.bom_detail.nodes.iter().map(|n| n.product_id).collect();
    let products = if product_ids.is_empty() {
        Vec::new()
    } else {
        product_svc
            .get_by_ids(&service_ctx, &mut conn, product_ids)
            .await
            .unwrap_or_default()
    };
    let product_map: HashMap<i64, &abt_core::master_data::product::model::Product> =
        products.iter().map(|p| (p.product_id, p)).collect();

    // Load BOM categories
    let categories = category_svc
        .list(
            &service_ctx,
            &mut conn,
            BomCategoryQuery::default(),
            PageParams::new(1, 200),
        )
        .await
        .map(|r| r.items)
        .unwrap_or_default();

    let content = bom_edit_page(&bom, &product_map, &categories, claims.sub);
    let edit_path_str = BomEditPath { id: path.id }.to_string();
    let page_html = admin_page(
        &headers,
        &format!("{} - 编辑 BOM", bom.bom_name),
        &claims,
        "md",
        &edit_path_str,
        "主数据管理",
        Some(&bom.bom_name),
        content,
    );

    Ok(Html(page_html.into_string()))
}

/// HTMX: search products → return HTML fragment
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
        .list(
            &service_ctx,
            &mut conn,
            filter,
            abt_core::shared::types::PageParams::new(1, 20),
        )
        .await?;

    Ok(Html(product_list_fragment(&result.items).into_string()))
}

/// POST: add a node to BOM
#[require_permission("BOM", "update")]
pub async fn add_node(
    path: BomNodesPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<AddNodeForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    let node_svc = state.bom_node_service();
    let quantity: Decimal = form.quantity.parse().unwrap_or(Decimal::ONE);
    let loss_rate: Decimal = form
        .loss_rate
        .as_deref()
        .and_then(|s| s.parse().ok())
        .unwrap_or(Decimal::ZERO);

    // Determine order: max existing order among siblings + 1
    let bom_svc = state.bom_query_service();
    let bom = bom_svc.get(&service_ctx, &mut conn, path.id).await?;
    let max_order = bom
        .bom_detail
        .nodes
        .iter()
        .filter(|n| n.parent_id == form.parent_id)
        .map(|n| n.order)
        .max()
        .unwrap_or(0);

    node_svc
        .add_node(
            &service_ctx,
            &mut conn,
            path.id,
            NewBomNode {
                product_id: form.product_id,
                quantity,
                parent_id: form.parent_id,
                loss_rate,
                order: max_order + 1,
                unit: form.unit.filter(|s| !s.is_empty()),
                remark: form.remark.filter(|s| !s.is_empty()),
                position: form.position.filter(|s| !s.is_empty()),
                work_center: form.work_center.filter(|s| !s.is_empty()),
                properties: None,
            },
        )
        .await?;

    let redirect = BomEditPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

/// POST: update a node
#[require_permission("BOM", "update")]
pub async fn update_node(
    path: BomNodePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<UpdateNodeForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    let node_svc = state.bom_node_service();

    let quantity: Option<Decimal> = form
        .quantity
        .as_deref()
        .and_then(|s| s.parse().ok());
    let loss_rate: Option<Decimal> = form
        .loss_rate
        .as_deref()
        .and_then(|s| s.parse().ok());

    node_svc
        .update_node(
            &service_ctx,
            &mut conn,
            path.id,
            path.node_id,
            UpdateBomNodeReq {
                quantity,
                loss_rate,
                order: None,
                unit: form.unit.filter(|s| !s.is_empty()),
                remark: form.remark.filter(|s| !s.is_empty()),
                position: form.position.filter(|s| !s.is_empty()),
                work_center: form.work_center.filter(|s| !s.is_empty()),
                properties: None,
            },
            0,
        )
        .await?;

    let redirect = BomEditPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

/// DELETE: delete a node
#[require_permission("BOM", "update")]
pub async fn delete_node(
    path: BomNodePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    let node_svc = state.bom_node_service();
    node_svc
        .delete_node(&service_ctx, &mut conn, path.id, path.node_id)
        .await?;

    let redirect = BomEditPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

/// POST: move a node (drag-and-drop reorder)
#[derive(Debug, Deserialize)]
pub struct MoveNodeForm {
    pub new_parent_id: i64,
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    pub before_sibling_id: Option<i64>,
}

fn deserialize_optional_i64<'de, D>(de: D) -> std::result::Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(de)?;
    match opt {
        None => Ok(None),
        Some(ref s) if s.is_empty() => Ok(None),
        Some(s) => s.parse::<i64>().map(Some).map_err(serde::de::Error::custom),
    }
}

#[require_permission("BOM", "update")]
pub async fn move_node(
    path: BomNodeMovePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<MoveNodeForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    let node_svc = state.bom_node_service();
    node_svc
        .move_node(
            &service_ctx,
            &mut conn,
            path.id,
            path.node_id,
            form.new_parent_id,
            form.before_sibling_id,
        )
        .await?;

    let redirect = BomEditPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

/// POST: update BOM category
#[require_permission("BOM", "update")]
pub async fn update_category(
    path: BomUpdateCategoryPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<UpdateCategoryForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    let cmd_svc = state.bom_command_service();
    cmd_svc
        .update(
            &service_ctx,
            &mut conn,
            path.id,
            UpdateBomReq {
                name: None,
                bom_category_id: form.bom_category_id,
            },
            0,
        )
        .await?;

    let redirect = BomEditPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

/// POST: save BOM as new copy
#[require_permission("BOM", "create")]
pub async fn save_as(
    path: BomSaveAsPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<SaveAsForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    let cmd_svc = state.bom_command_service();
    let new_id = cmd_svc
        .save_as(&service_ctx, &mut conn, path.id, form.new_name)
        .await?;

    let redirect = BomEditPath { id: new_id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn bom_edit_page(
    bom: &Bom,
    product_map: &HashMap<i64, &abt_core::master_data::product::model::Product>,
    categories: &[BomCategory],
    current_user_id: i64,
) -> Markup {
    let list_path = BomListPath;
    let publish_path = BomPublishPath { id: bom.bom_id };
    let node_count = bom.bom_detail.nodes.len();

    let (status_label, status_class) = bom_status_display(bom.status);
    let is_draft = bom.status == BomStatus::Draft;
    let is_owner = bom.created_by.map(|id| id == current_user_id).unwrap_or(false);

    // Build depth map and parent set
    let depth_map = build_depth_map(&bom.bom_detail.nodes);
    let parent_ids: HashSet<i64> = bom
        .bom_detail
        .nodes
        .iter()
        .filter(|n| n.parent_id != 0)
        .map(|n| n.parent_id)
        .collect();

    // Max level for filter
    let max_level = depth_map.values().copied().max().map(|d| d + 1).unwrap_or(0);
    let save_as_click = format!("saveAsName = '{}_副本'; saveAsOpen = true", bom.bom_name.replace('\'', "\\'"));
    html! {
        div x-data="bomEdit()" x-init="initSortable()" {
            // ── Toolbar ──
            div class="bom-toolbar" {
                // Left side: back, category, view toggle, level filter
                div class="bom-toolbar-left" {
                    a class="btn btn-sm btn-default" href=(list_path) {
                        (icon::arrow_left_icon("w-4 h-4"))
                        " 返回列表"
                    }

                    // Category selector
                    @if !categories.is_empty() {
                        div class="bom-category-select" {
                            select name="bom_category_id"
                                hx-post=(BomUpdateCategoryPath { id: bom.bom_id }.to_string())
                                hx-trigger="change"
                                hx-swap="none"
                                hx-confirm="确定要更改分类吗？" {
                                option value="" selected[bom.bom_category_id.is_none()] { "未分类" }
                                @for cat in categories {
                                    option value=(cat.bom_category_id)
                                        selected[bom.bom_category_id == Some(cat.bom_category_id)] {
                                        (cat.bom_category_name)
                                    }
                                }
                            }
                        }
                    }

                    // View toggle: table | tree
                    div class="bom-view-toggle" {
                        button type="button" class="bom-view-btn"
                            x-bind:class="{ 'active': viewMode === 'table' }"
                            x-on:click="viewMode = 'table'" title="表格视图" {
                            (table_icon("w-4 h-4"))
                        }
                        button type="button" class="bom-view-btn"
                            x-bind:class="{ 'active': viewMode === 'tree' }"
                            x-on:click="viewMode = 'tree'" title="树形视图" {
                            (tree_icon("w-4 h-4"))
                        }
                    }

                    // Level filter (table mode only)
                    div x-show="viewMode === 'table'" x-cloak {
                        select x-model="layerFilter" class="bom-level-filter" {
                            option value="0" { "全部层级" }
                            @for lv in 1..=max_level {
                                option value=(lv) { "层级 " (lv) }
                            }
                        }
                    }
                }

                // Right side: publish/unpublish, add/save-as, labor cost
                div class="bom-toolbar-right" {
                    @if !is_draft && is_owner {
                        button class="btn btn-sm btn-warning-ghost"
                            hx-post=(publish_path.to_string())
                            hx-target="body"
                            hx-swap="outerHTML"
                            hx-confirm="确定要取消发布此 BOM 吗？" {
                            (icon::return_arrow_icon("w-4 h-4"))
                            " 取消发布"
                        }
                    } @else if is_draft {
                        button class="btn btn-sm btn-success"
                            hx-post=(publish_path.to_string())
                            hx-target="body"
                            hx-swap="outerHTML"
                            disabled[node_count == 0]
                            title="请先添加物料" {
                            (icon::rocket_icon("w-4 h-4"))
                            " 发布"
                        }
                    }

                    @if node_count == 0 {
                        button type="button" class="btn btn-sm btn-primary"
                            x-on:click="addModalOpen = true; addParentId = 0" {
                            (icon::plus_icon("w-4 h-4"))
                            " 添加根节点"
                        }
                    } @else {
                        button type="button" class="btn btn-sm btn-success"
                            x-on:click=(save_as_click) {
                            (icon::copy_icon("w-4 h-4"))
                            " 另存为"
                        }
                    }

                    a class="btn btn-sm btn-labor-cost" href=(format!("/admin/labor/bom-cost/{}", bom.bom_id)) {
                        (icon::currency_icon("w-4 h-4"))
                        " 人工成本"
                    }
                }
            }

            // ── Title ──
            h1 class="page-title" style="display:flex;align-items:center;gap:var(--space-2);margin-bottom:var(--space-4)" {
                (bom.bom_name)
                span class=(format!("status-pill {status_class}")) { (status_label) }
            }

            // ── Node Table (table view) ──
            div x-show="viewMode === 'table'" {
                div class="data-card" style="padding:0;overflow:hidden" {
                    @if bom.bom_detail.nodes.is_empty() {
                        div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
                            "暂无组件数据，请点击上方按钮添加根节点"
                        }
                    } @else {
                        div style="overflow-x:auto" {
                            table class="bom-table" style="min-width:900px" {
                                thead {
                                    tr {
                                        th style="width:32px" { "" }
                                        th style="width:40px" { "编号" }
                                        th style="width:40px" { "层级" }
                                        th style="width:120px" { "产品编码" }
                                        th { "产品" }
                                        th style="width:100px" { "工作中心" }
                                        th style="width:80px" { "数量" }
                                        th style="width:60px" { "单位" }
                                        th style="width:80px" { "损耗率" }
                                        th style="width:100px" { "位置" }
                                        th { "备注" }
                                        th style="width:120px" { "操作" }
                                    }
                                }
                                tbody id="bom-sortable-tbody" {
                                    @for (idx, node) in bom.bom_detail.nodes.iter().enumerate() {
                                        @let depth = *depth_map.get(&node.id).unwrap_or(&0);
                                        @let level = depth + 1;
                                        @let has_children = parent_ids.contains(&node.id);
                                        @let product = product_map.get(&node.product_id);
                                        (bom_node_row(idx, level, has_children, node, product.map(|v| &**v)))
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Tree View ──
            div x-show="viewMode === 'tree'" x-cloak {
                div class="data-card" style="padding:var(--space-5)" {
                    div style="display:flex;gap:var(--space-2);margin-bottom:var(--space-4)" {
                        span style="font-size:var(--text-xs);color:var(--muted);line-height:32px" { "展开：" }
                        button type="button" class="btn btn-sm btn-default" x-on:click="$el.closest('.data-card').querySelectorAll('.bom-tree-branch').forEach(function(el){ Alpine.$data(el).open = true })" { "全部" }
                        button type="button" class="btn btn-sm btn-default" x-on:click="$el.closest('.data-card').querySelectorAll('.bom-tree-branch').forEach(function(el){ Alpine.$data(el).open = false })" { "折叠" }
                    }
                    @if bom.bom_detail.nodes.is_empty() {
                        div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
                            "暂无组件数据"
                        }
                    } @else {
                        (bom_tree_view(&bom.bom_detail.nodes, &depth_map, &parent_ids, product_map))
                    }
                }
            }

            // ── Add Node Modal ──
            div class="modal-overlay"
                x-bind:class="{ 'is-open': addModalOpen }"
                x-on:click="addModalOpen = false" {
                div class="modal modal-lg" x-on:click="event.stopPropagation()" {
                    div class="modal-head" {
                        h2 { "添加物料" }
                        button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                            x-on:click="addModalOpen = false" { "×" }
                    }
                    div class="modal-body" style="padding:0" {
                        div class="product-search-bar" {
                            div class="product-search-field" {
                                label class="product-search-label" { "产品名称" }
                                input class="product-search-input" type="text" name="name" placeholder="输入产品名称…"
                                    hx-get=(BomProductsPath::PATH)
                                    hx-trigger="keyup changed delay:300ms"
                                    hx-target="#bom-edit-product-results"
                                    hx-swap="innerHTML"
                                    hx-include=".product-search-bar" {}
                            }
                            div class="product-search-field" {
                                label class="product-search-label" { "产品编码" }
                                input class="product-search-input" type="text" name="code" placeholder="输入产品编码…"
                                    hx-get=(BomProductsPath::PATH)
                                    hx-trigger="keyup changed delay:300ms"
                                    hx-target="#bom-edit-product-results"
                                    hx-swap="innerHTML"
                                    hx-include=".product-search-bar" {}
                            }
                            button type="button" class="product-search-clear"
                                hx-get=(BomProductsPath::PATH)
                                hx-target="#bom-edit-product-results"
                                hx-swap="innerHTML"
                                onclick="document.querySelectorAll('.product-search-input').forEach(function(i){i.value=''})" {
                                "清除"
                            }
                        }
                        div id="bom-edit-product-results" style="max-height:320px;overflow-y:auto"
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

            // ── Edit Node Modal ──
            div class="modal-overlay"
                x-bind:class="{ 'is-open': editModalOpen }"
                x-on:click="editModalOpen = false" {
                div class="modal" x-on:click="event.stopPropagation()" {
                    div class="modal-head" {
                        h2 { "编辑节点" }
                        button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                            x-on:click="editModalOpen = false" { "×" }
                    }
                    div class="modal-body" {
                        form id="bom-edit-node-form" hx-post="" hx-swap="none" {
                            div class="form-grid" {
                                div class="form-field" {
                                    label { "数量 " span style="color:var(--danger)" { "*" } }
                                    input type="number" name="quantity" step="0.01" min="0.01" required
                                        x-model="editNode.quantity" {}
                                }
                                div class="form-field" {
                                    label { "损耗率%" }
                                    input type="number" name="loss_rate" step="0.1" min="0"
                                        x-model="editNode.loss_rate" {}
                                }
                                div class="form-field" {
                                    label { "单位" }
                                    input type="text" name="unit"
                                        x-model="editNode.unit" {}
                                }
                                div class="form-field" {
                                    label { "工作中心" }
                                    input type="text" name="work_center"
                                        x-model="editNode.work_center" {}
                                }
                                div class="form-field" {
                                    label { "位置" }
                                    input type="text" name="position"
                                        x-model="editNode.position" {}
                                }
                                div class="form-field field-full" {
                                    label { "备注" }
                                    input type="text" name="remark"
                                        x-model="editNode.remark" {}
                                }
                            }
                            div class="modal-foot" style="padding:var(--space-4) 0 0;border-top:1px solid var(--border-soft)" {
                                button type="button" class="btn btn-default" x-on:click="editModalOpen = false" { "取消" }
                                button type="submit" class="btn btn-primary" { "保存" }
                            }
                        }
                    }
                }
            }

            // ── Delete Confirm ──
            (crate::components::confirm_dialog::confirm_dialog(
                "deleteOpen",
                "确认删除",
                "确定要删除该节点及其所有子节点吗？此操作不可撤销。",
                "确认删除",
                "bom-node-delete-form",
                html! {
                    form id="bom-node-delete-form" style="display:none"
                        hx-delete=""
                        hx-swap="none" {}
                },
            ))

            // ── Save As Modal ──
            div class="modal-overlay"
                x-bind:class="{ 'is-open': saveAsOpen }"
                x-on:click="saveAsOpen = false" {
                div class="modal" x-on:click="event.stopPropagation()" {
                    div class="modal-head" {
                        h2 { "另存为" }
                        button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                            x-on:click="saveAsOpen = false" { "×" }
                    }
                    div class="modal-body" {
                        form hx-post=(BomSaveAsPath { id: bom.bom_id }.to_string())
                            hx-swap="none" {
                            div class="form-field" {
                                label { "新 BOM 名称 " span style="color:var(--danger)" { "*" } }
                                input type="text" name="new_name" required
                                    x-model="saveAsName" placeholder="输入新的 BOM 名称" {}
                            }
                            div class="modal-foot" style="padding:var(--space-4) 0 0;border-top:1px solid var(--border-soft)" {
                                button type="button" class="btn btn-default" x-on:click="saveAsOpen = false" { "取消" }
                                button type="submit" class="btn btn-success" { "确认另存为" }
                            }
                        }
                    }
                }
            }

            // ── Alpine.js component ──
            script {
                (maud::PreEscaped(format!(r#"
                function bomEdit() {{
                    return {{
                        viewMode: 'table',
                        layerFilter: 0,

                        addModalOpen: false,
                        addParentId: 0,
                        addProductId: 0,
                        addProductCode: '',
                        addProductName: '',
                        addProductUnit: '',

                        editModalOpen: false,
                        editNodeId: 0,
                        editNode: {{
                            quantity: '',
                            loss_rate: '',
                            unit: '',
                            work_center: '',
                            position: '',
                            remark: ''
                        }},

                        saveAsOpen: false,
                        saveAsName: '',
                        deleteOpen: false,

                        selectAddProduct(product) {{
                            this.addProductId = product.product_id;
                            this.addProductCode = product.product_code;
                            this.addProductName = product.product_name;
                            this.addProductUnit = product.unit || '';
                            this.submitAddNode();
                        }},

                        submitAddNode() {{
                            if (!this.addProductId) return;
                            var bomId = window.location.pathname.split('/')[4];
                            var fields = {{
                                product_id: this.addProductId,
                                parent_id: this.addParentId,
                                quantity: '1',
                                unit: this.addProductUnit
                            }};
                            htmx.ajax('POST', '/admin/md/boms/' + bomId + '/nodes', {{
                                values: fields,
                                swap: 'none',
                                headers: {{'HX-Request': 'true'}}
                            }}).then(() => {{
                                this.addModalOpen = false;
                            }});
                        }},

                        openEdit(nodeId, quantity, lossRate, unit, workCenter, position, remark) {{
                            this.editNodeId = nodeId;
                            this.editNode = {{
                                quantity: quantity,
                                loss_rate: lossRate,
                                unit: unit,
                                work_center: workCenter,
                                position: position,
                                remark: remark
                            }};
                            var bomId = window.location.pathname.split('/')[4];
                            var form = document.getElementById('bom-edit-node-form');
                            form.action = '/admin/md/boms/' + bomId + '/nodes/' + nodeId;
                            form.setAttribute('hx-post', form.action);
                            this.editModalOpen = true;
                        }},

                        openDelete(nodeId) {{
                            var bomId = window.location.pathname.split('/')[4];
                            var form = document.getElementById('bom-node-delete-form');
                            form.action = '/admin/md/boms/' + bomId + '/nodes/' + nodeId;
                            form.setAttribute('hx-delete', form.action);
                            this.deleteOpen = true;
                        }},

                        openAddChild(parentId) {{
                            this.addParentId = parentId;
                            this.addProductId = 0;
                            this.addModalOpen = true;
                        }},

                        initSortable() {{
                            var bomId = window.location.pathname.split('/')[4];
                            var tbody = document.getElementById('bom-sortable-tbody');
                            if (!tbody) return;

                            var dragNodeId = null;
                            var descendantIds = new Set();

                            function getDescendants(nodeId) {{
                                var ids = new Set([nodeId]);
                                var changed = true;
                                while (changed) {{
                                    changed = false;
                                    tbody.querySelectorAll('tr[data-node-id]').forEach(function(r) {{
                                        var pid = Number(r.dataset.parentId);
                                        var nid = Number(r.dataset.nodeId);
                                        if (ids.has(pid) && !ids.has(nid)) {{
                                            ids.add(nid);
                                            changed = true;
                                        }}
                                    }});
                                }}
                                return ids;
                            }}

                            function clearIndicators() {{
                                tbody.querySelectorAll('.bom-drop-top,.bom-drop-bottom,.bom-drop-child').forEach(function(r) {{
                                    r.classList.remove('bom-drop-top', 'bom-drop-bottom', 'bom-drop-child');
                                }});
                            }}

                            tbody.addEventListener('dragstart', function(e) {{
                                var row = e.target.closest('tr[data-node-id]');
                                if (!row) return;
                                dragNodeId = Number(row.dataset.nodeId);
                                descendantIds = getDescendants(dragNodeId);
                                tbody.querySelectorAll('tr[data-node-id]').forEach(function(r) {{
                                    if (descendantIds.has(Number(r.dataset.nodeId))) {{
                                        r.classList.add('bom-dragging');
                                    }}
                                }});
                                e.dataTransfer.effectAllowed = 'move';
                                e.dataTransfer.setData('text/plain', String(dragNodeId));
                            }});

                            tbody.addEventListener('dragend', function() {{
                                tbody.querySelectorAll('.bom-dragging').forEach(function(r) {{
                                    r.classList.remove('bom-dragging');
                                }});
                                clearIndicators();
                                dragNodeId = null;
                                descendantIds = new Set();
                            }});

                            tbody.addEventListener('dragover', function(e) {{
                                e.preventDefault();
                                e.dataTransfer.dropEffect = 'move';
                                clearIndicators();
                                var row = e.target.closest('tr[data-node-id]');
                                if (!row) return;
                                var tid = Number(row.dataset.nodeId);
                                if (descendantIds.has(tid)) return;
                                var rect = row.getBoundingClientRect();
                                var y = e.clientY - rect.top;
                                if (y < rect.height * 0.25) {{
                                    row.classList.add('bom-drop-top');
                                }} else if (y > rect.height * 0.75) {{
                                    row.classList.add('bom-drop-bottom');
                                }} else {{
                                    row.classList.add('bom-drop-child');
                                }}
                            }});

                            tbody.addEventListener('dragleave', function(e) {{
                                var row = e.target.closest('tr[data-node-id]');
                                if (row) row.classList.remove('bom-drop-top','bom-drop-bottom','bom-drop-child');
                            }});

                            tbody.addEventListener('drop', function(e) {{
                                e.preventDefault();
                                clearIndicators();
                                if (!dragNodeId) return;
                                var row = e.target.closest('tr[data-node-id]');
                                if (!row) return;
                                var tid = Number(row.dataset.nodeId);
                                if (descendantIds.has(tid)) return;

                                var rect = row.getBoundingClientRect();
                                var y = e.clientY - rect.top;
                                var targetPid = Number(row.dataset.parentId);
                                var newParentId, beforeSiblingId;

                                if (y < rect.height * 0.25) {{
                                    // Insert before target, same parent
                                    newParentId = targetPid;
                                    beforeSiblingId = tid;
                                }} else if (y > rect.height * 0.75) {{
                                    // Insert after target
                                    // Check if target has children → become a child of target
                                    var allRows = Array.from(tbody.querySelectorAll('tr[data-node-id]'));
                                    var tIdx = allRows.indexOf(row);
                                    var nextIsChild = tIdx + 1 < allRows.length && Number(allRows[tIdx + 1].dataset.parentId) === tid;
                                    if (nextIsChild) {{
                                        newParentId = tid;
                                        beforeSiblingId = '';
                                    }} else {{
                                        // Same parent, find next sibling
                                        newParentId = targetPid;
                                        var nextSibling = '';
                                        for (var i = tIdx + 1; i < allRows.length; i++) {{
                                            if (Number(allRows[i].dataset.parentId) === targetPid) {{
                                                nextSibling = Number(allRows[i].dataset.nodeId);
                                                break;
                                            }}
                                        }}
                                        beforeSiblingId = nextSibling || '';
                                    }}
                                }} else {{
                                    // Middle → become child of target
                                    newParentId = tid;
                                    beforeSiblingId = '';
                                }}

                                htmx.ajax('POST', '/admin/md/boms/' + bomId + '/nodes/' + dragNodeId + '/move', {{
                                    values: {{
                                        new_parent_id: newParentId,
                                        before_sibling_id: beforeSiblingId
                                    }},
                                    swap: 'none',
                                    headers: {{'HX-Request': 'true'}}
                                }}).then(function() {{
                                    window.location.reload();
                                }});
                            }});
                        }}
                    }};
                }}
                "#)))
            }
        }
    }
}

fn bom_node_row(
    index: usize,
    level: usize,
    has_children: bool,
    node: &BomNode,
    product: Option<&abt_core::master_data::product::model::Product>,
) -> Markup {
    let code = node.product_code.as_deref().unwrap_or("—");
    let name = product.map(|p| p.pdt_name.as_str()).unwrap_or("—");
    let unit = node.unit.as_deref().unwrap_or("—");
    let position = node.position.as_deref().filter(|s| !s.is_empty()).unwrap_or("—");
    let work_center = node
        .work_center
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("—");
    let remark = node.remark.as_deref().filter(|s| !s.is_empty()).unwrap_or("");
    let loss_rate = if node.loss_rate == Decimal::ZERO {
        "—".to_string()
    } else {
        format!("{}%", node.loss_rate)
    };

    let row_class = if level == 1 {
        "bom-row-level-0"
    } else if has_children {
        "bom-row-level-1"
    } else {
        "bom-row-level-default"
    };

    let js_args = format!(
        "{}, '{}', '{}', '{}', '{}', '{}', '{}'",
        node.id,
        node.quantity,
        node.loss_rate,
        unit.replace('\'', "\\'"),
        work_center.replace('\'', "\\'"),
        position.replace('\'', "\\'"),
        remark.replace('\'', "\\'")
    );

    let show_expr = format!("layerFilter == 0 || layerFilter == {}", level);

    html! {
        tr class=(row_class) x-show=(show_expr) draggable="true"
            data-node-id=(node.id) data-parent-id=(node.parent_id) data-level=(level) {
            td class="bom-drag-handle" title="拖动排序" {
                (icon::dots_vertical_icon("w-3.5 h-3.5"))
            }
            td style="text-align:center" { (index + 1) }
            td style="text-align:center" { (level) }
            td class="mono" { (code) }
            td { (name) }
            td { (work_center) }
            td class="mono" style="text-align:right" { (node.quantity) }
            td { (unit) }
            td style="text-align:right" { (loss_rate) }
            td { (position) }
            td style="color:var(--muted)" { (remark) }
            td {
                div style="display:flex;gap:var(--space-1)" {
                    button type="button" class="row-action-btn" title="添加子节点"
                        x-on:click=(format!("openAddChild({})", node.id)) {
                        (icon::plus_icon("w-3.5 h-3.5"))
                    }
                    button type="button" class="row-action-btn" title="编辑"
                        x-on:click=(format!("openEdit({})", js_args)) {
                        (icon::edit_icon("w-3.5 h-3.5"))
                    }
                    button type="button" class="row-action-btn text-danger" title="删除"
                        x-on:click=(format!("openDelete({})", node.id)) {
                        (icon::trash_icon("w-3.5 h-3.5"))
                    }
                }
            }
        }
    }
}

/// Tree view with per-node expand/collapse via Alpine.js
fn bom_tree_view(
    nodes: &[BomNode],
    _depth_map: &HashMap<i64, usize>,
    parent_ids: &HashSet<i64>,
    product_map: &HashMap<i64, &abt_core::master_data::product::model::Product>,
) -> Markup {
    // Build children map: parent_id -> Vec<&BomNode>
    let mut children_map: HashMap<i64, Vec<&BomNode>> = HashMap::new();
    let mut roots: Vec<&BomNode> = Vec::new();
    for node in nodes {
        if node.parent_id == 0 {
            roots.push(node);
        } else {
            children_map.entry(node.parent_id).or_default().push(node);
        }
    }

    fn render_node(
        node: &BomNode,
        children_map: &HashMap<i64, Vec<&BomNode>>,
        parent_ids: &HashSet<i64>,
        product_map: &HashMap<i64, &abt_core::master_data::product::model::Product>,
    ) -> Markup {
        let product = product_map.get(&node.product_id);
        let name = product.map(|p| p.pdt_name.as_str()).unwrap_or("—");
        let code = node.product_code.as_deref().unwrap_or("—");
        let has_children = parent_ids.contains(&node.id);
        let children: &[&BomNode] = children_map.get(&node.id).map(|v| v.as_slice()).unwrap_or_default();

        html! {
            div class="bom-tree-branch" x-data="{ open: true }" {
                div class="bom-tree-row" {
                    @if has_children {
                        button type="button" class="bom-tree-toggle"
                            x-on:click="open = !open"
                            x-bind:class="{ 'is-collapsed': !open }" {
                            (icon::chevron_right_icon("w-3.5 h-3.5"))
                        }
                    } @else {
                        span class="bom-tree-dot" {}
                    }
                    span class="bom-tree-code" { (code) }
                    span class="bom-tree-name" { (name) }
                    span class="bom-tree-qty" { (node.quantity) }
                    @if let Some(u) = node.unit.as_deref().filter(|s| !s.is_empty()) {
                        span class="bom-tree-unit" { (u) }
                    }
                }
                div x-show="open" x-transition {
                    @for child in children {
                        div class="bom-tree-indent" {
                            (render_node(child, children_map, parent_ids, product_map))
                        }
                    }
                }
            }
        }
    }

    html! {
        div class="bom-tree" {
            @for root in roots {
                (render_node(root, &children_map, parent_ids, product_map))
            }
        }
    }
}

// ── Helpers ──

fn bom_status_display(status: BomStatus) -> (&'static str, &'static str) {
    match status {
        BomStatus::Draft => ("草稿", "status-bom-draft"),
        BomStatus::Published => ("已发布", "status-bom-published"),
    }
}

fn build_depth_map(nodes: &[BomNode]) -> HashMap<i64, usize> {
    let mut depth_map: HashMap<i64, usize> = HashMap::with_capacity(nodes.len());
    for node in nodes {
        let depth = if node.parent_id == 0 {
            0
        } else {
            depth_map.get(&node.parent_id).copied().unwrap_or(0) + 1
        };
        depth_map.insert(node.id, depth);
    }
    depth_map
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
                            x-on:click="selectAddProduct(JSON.parse($el.dataset.product))" {
                            "选择"
                        }
                    }
                }
            }
        }
    }
}

// ── Custom icons for view toggle ──

fn table_icon(c: &str) -> Markup {
    icon::svg(
        r#"<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 9h18M3 15h18M9 3v18M15 3v18"/>"#,
        c,
    )
}

fn tree_icon(c: &str) -> Markup {
    icon::svg(
        r#"<path d="M12 3v6M12 9h6M12 9H6M12 15v6M12 15h4M12 15H8"/>"#,
        c,
    )
}
