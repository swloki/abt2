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

    let mut bom = bom_svc.get(&service_ctx, &mut conn, path.id).await?;

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


    // Filter out nodes whose products no longer exist (and their descendants)
    crate::pages::bom_detail::filter_invalid_nodes(&mut bom.bom_detail.nodes, &product_map);
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
    // Build ancestors map for collapse: each node → ordered list of ancestor node IDs
    let ancestors_map = build_ancestors_map(&bom.bom_detail.nodes);

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

                    // Level filter
                    select x-model="layerFilter" class="bom-level-filter" {
                        option value="0" { "全部层级" }
                        @for lv in 1..=max_level {
                            option value=(lv) { "层级 " (lv) }
                        }
                    }

                    button type="button" class="bom-level-filter"
                        x-on:click="toggleAllCollapse()"
                        x-text="allCollapsed ? '全部展开' : '全部折叠'" {
                    }
                }

                // Right side: publish/unpublish, add/save-as, labor cost
                div class="bom-toolbar-right" {
                    @if !is_draft && is_owner {
                        button class="btn btn-sm btn-warning-ghost"
                            x-on:click="publishOpen = true" {
                            (icon::return_arrow_icon("w-4 h-4"))
                            " 取消发布"
                        }
                    } @else if is_draft {
                        button class="btn btn-sm btn-success"
                            x-on:click="publishOpen = true"
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

            // ── Node Table ──
            div class="data-card" style="padding:0;overflow:hidden" {
                @if bom.bom_detail.nodes.is_empty() {
                    div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
                        "暂无组件数据，请点击上方按钮添加根节点"
                    }
                } @else {
                    div style="overflow-x:auto" {
                        table class="bom-table" style="table-layout:fixed;min-width:900px" {
                            thead {
                                tr {
                                    th style="width:40px" { "编号" }
                                    th style="width:40px" { "层级" }
                                    th style="width:120px" { "产品编码" }
                                    th class="bom-col-name" { "产品" }
                                    th style="width:100px" { "工作中心" }
                                    th style="width:80px" { "数量" }
                                    th style="width:60px" { "单位" }
                                    th style="width:50px" { "损耗率" }
                                    th style="width:100px" { "位置" }
                                    th style="width:90px" { "备注" }
                                    th style="width:120px" { "操作" }
                                }
                            }
                            tbody id="bom-sortable-tbody" {
                                @for (idx, node) in bom.bom_detail.nodes.iter().enumerate() {
                                    @let depth = *depth_map.get(&node.id).unwrap_or(&0);
                                    @let level = depth + 1;
                                    @let has_children = parent_ids.contains(&node.id);
                                    @let product = product_map.get(&node.product_id);
                                    @let ancestors = ancestors_map.get(&node.id).map(|v| v.as_slice()).unwrap_or(&[]);
                                    (bom_node_row(idx, level, has_children, node, product.map(|v| &**v), ancestors))
                                }
                            }
                        }
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

            // ── Publish / Unpublish Confirm Dialog ──
            @if !is_draft && is_owner {
                (crate::components::confirm_dialog::confirm_dialog(
                    "publishOpen",
                    "确认取消发布",
                    "确定要取消发布此 BOM 吗？取消后可重新编辑。",
                    "确认取消发布",
                    "publish-bom-form",
                    html! {
                        form id="publish-bom-form" class="hidden"
                            hx-post=(publish_path.to_string())
                            hx-swap="none" {}
                    },
                ))
            } @else if is_draft {
                (crate::components::confirm_dialog::confirm_dialog(
                    "publishOpen",
                    "确认发布",
                    "确定要发布此 BOM 吗？发布后将无法修改。",
                    "确认发布",
                    "publish-bom-form",
                    html! {
                        form id="publish-bom-form" class="hidden"
                            hx-post=(publish_path.to_string())
                            hx-swap="none" {}
                    },
                ))
            }

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
            script src="/bom-edit.js?v=20260531" {}
        }
    }
}

fn bom_node_row(
    index: usize,
    level: usize,
    has_children: bool,
    node: &BomNode,
    product: Option<&abt_core::master_data::product::model::Product>,
    ancestors: &[i64],
) -> Markup {
    let code = node.product_code.as_deref().or_else(|| product.map(|p| p.product_code.as_str())).unwrap_or("—");
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
    let ancestors_str = ancestors.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
    let show_expr = format!("isNodeVisible({}, '{}')", level, ancestors_str);
    let indent_px = (level - 1) * 24;
    html! {
        tr class=(row_class) x-show=(show_expr) draggable="true"
            data-node-id=(node.id) data-parent-id=(node.parent_id) data-level=(level) {
            td {
                div style="display:flex;align-items:center;justify-content:center;gap:4px" {
                    @if has_children {
                        button type="button" class="bom-collapse-btn"
                            x-on:click=(format!("toggleCollapse({})", node.id))
                            x-bind:class=(format!("{{'bom-collapsed': collapsedNodes[{}]}}", node.id)) {
                            (icon::chevron_down_icon("bom-collapse-icon"))
                        }
                    } @else {
                        span style="display:inline-block;width:20px" {}
                    }
                    span { (index + 1) }
                }
            }
            td style="text-align:center" { (level) }
            td class="mono" { (code) }
            td class="bom-col-name" style={"padding-left:" (indent_px) "px"} { (name) }
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

fn build_ancestors_map(nodes: &[BomNode]) -> HashMap<i64, Vec<i64>> {
    let mut ancestors_map: HashMap<i64, Vec<i64>> = HashMap::with_capacity(nodes.len());
    for node in nodes {
        if node.parent_id == 0 {
            ancestors_map.insert(node.id, Vec::new());
        } else if let Some(parent_ancestors) = ancestors_map.get(&node.parent_id).cloned() {
            let mut ancestors = parent_ancestors;
            ancestors.push(node.parent_id);
            ancestors_map.insert(node.id, ancestors);
        } else {
            ancestors_map.insert(node.id, vec![node.parent_id]);
        }
    }
    ancestors_map
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
