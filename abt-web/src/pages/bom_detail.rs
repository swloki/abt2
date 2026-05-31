use std::collections::HashMap;

use axum::http::HeaderMap;
use axum::response::Html;
use maud::{Markup, html};
use rust_decimal::Decimal;

use abt_core::master_data::bom::BomQueryService;
use abt_core::master_data::bom::model::*;
use abt_core::master_data::product::ProductService;

use abt_macros::require_permission;

use crate::components::{confirm_dialog, detail::detail_row, icon};
use crate::layout::page::admin_page;
use crate::routes::bom::{BomDeletePath, BomDetailPath, BomEditPath, BomListPath, BomPublishPath};
use crate::utils::RequestContext;

// ── Handlers ──

#[require_permission("BOM", "read")]
pub async fn get_bom_detail(
    path: BomDetailPath,
    ctx: RequestContext,
    headers: HeaderMap,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    let bom_svc = state.bom_query_service();
    let product_svc = state.product_service();

    let bom = bom_svc.get(&service_ctx, &mut conn, path.id).await?;

    // Resolve product names & specs for all nodes
    let product_ids: Vec<i64> = bom.bom_detail.nodes.iter().map(|n| n.product_id).collect();
    let products = if product_ids.is_empty() {
        Vec::new()
    } else {
        product_svc.get_by_ids(&service_ctx, &mut conn, product_ids).await.unwrap_or_default()
    };
    let product_map: HashMap<i64, &abt_core::master_data::product::model::Product> =
        products.iter().map(|p| (p.product_id, p)).collect();

    let content = bom_detail_page(&bom, &product_map);
    let detail_path_str = BomDetailPath { id: path.id }.to_string();
    let page_html = admin_page(
        &headers,
        &format!("{} - BOM 详情", bom.bom_name),
        &claims,
        "md",
        &detail_path_str,
        "主数据管理",
        Some(&bom.bom_name),
        content,
    );

    Ok(Html(page_html.into_string()))
}


#[require_permission("BOM", "update")]
pub async fn publish_bom(
    _path: BomPublishPath,
    _ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    Ok(Html("<p>Publish BOM placeholder</p>".into()))
}

// ── Components ──

fn bom_detail_page(
    bom: &Bom,
    product_map: &HashMap<i64, &abt_core::master_data::product::model::Product>,
) -> Markup {
    let list_path = BomListPath;
    let delete_path = BomDeletePath { id: bom.bom_id };
    let publish_path = BomPublishPath { id: bom.bom_id };
    let node_count = bom.bom_detail.nodes.len();
    let depth_map = build_depth_map(&bom.bom_detail.nodes);

    // Build set of parent IDs to know which nodes have children
    let parent_ids: std::collections::HashSet<i64> = bom.bom_detail.nodes.iter()
        .filter(|n| n.parent_id != 0)
        .map(|n| n.parent_id)
        .collect();

    let (status_label, status_class) = bom_status_display(bom.status);
    let is_draft = bom.status == BomStatus::Draft;

    html! {
        div x-data="{ deleteOpen: false }" {
            // ── Detail Top ──
            div class="detail-top" {
                div class="customer-identity" {
                    div class="customer-avatar" style="background:var(--color-primary-light,#e0e7ff)" {
                        (icon::clipboard_list_icon("w-5 h-5"))
                    }
                    div {
                        h1 class="customer-name" {
                            (bom.bom_name)
                            " "
                            span class="tag-key" { "v" (bom.version) }
                            " "
                            span class=(format!("status-pill {status_class}")) { (status_label) }
                        }
                        div class="customer-meta" {
                            span { "节点: " (node_count) }
                            @if let Some(cat_id) = bom.bom_category_id {
                                span { "分类ID: " (cat_id) }
                            }
                            span { "创建: " (bom.create_at.format("%Y-%m-%d")) }
                        }
                    }
                }
                div class="page-actions" {
                    a class="btn btn-default" href=(list_path) {
                        (icon::arrow_left_icon("w-4 h-4"))
                        " 返回列表"
                    }
                    a class="btn btn-primary" href=(BomEditPath { id: bom.bom_id }) {
                        (icon::edit_icon("w-4 h-4"))
                        " 编辑"
                    }
                    @if is_draft {
                        button class="btn btn-primary"
                            hx-post=(publish_path.to_string())
                            hx-target="body"
                            hx-swap="outerHTML" {
                            (icon::check_circle_icon("w-4 h-4"))
                            " 发布"
                        }
                    }
                    button class="btn btn-danger-ghost" x-on:click="deleteOpen = true" {
                        (icon::trash_icon("w-4 h-4"))
                        " 删除"
                    }
                }
            }

            // ── Workflow Steps ──
            div class="detail-card" style="margin-bottom:var(--space-5)" {
                div class="workflow-steps" {
                    (workflow_step("草稿", bom.status == BomStatus::Draft, bom.status == BomStatus::Draft))
                    (workflow_connector(bom.status == BomStatus::Published))
                    (workflow_step("已发布", bom.status == BomStatus::Published, bom.status == BomStatus::Published))
                }
            }

            // ── 基本信息 ──
            div class="detail-card" {
                div class="detail-card-title" { "基本信息" }
                div class="detail-grid" style="grid-template-columns:repeat(3,1fr)" {
                    (detail_row("BOM名称", html! { (bom.bom_name) }))
                    (detail_row("BOM编码", html! { span class="mono" { (bom.bom_id) } }))
                    @if let Some(cat_id) = bom.bom_category_id {
                        (detail_row("BOM分类", html! { "ID: " (cat_id) }))
                    } @else {
                        (detail_row("BOM分类", html! { "—" }))
                    }
                    (detail_row("状态", html! {
                        span class=(format!("status-pill {status_class}")) { (status_label) }
                    }))
                    (detail_row("版本", html! { "v" (bom.version) }))
                    @if let Some(dt) = bom.published_at {
                        (detail_row("发布时间", html! { (dt.format("%Y-%m-%d %H:%M")) }))
                    } @else {
                        (detail_row("发布时间", html! { "—" }))
                    }
                    @if let Some(_uid) = bom.created_by {
                        (detail_row("创建人", html! { "ID: " (_uid) }))
                    } @else {
                        (detail_row("创建人", html! { "—" }))
                    }
                    (detail_row("创建时间", html! { (bom.create_at.format("%Y-%m-%d %H:%M")) }))
                    @if let Some(dt) = bom.update_at {
                        (detail_row("更新时间", html! { (dt.format("%Y-%m-%d %H:%M")) }))
                    } @else {
                        (detail_row("更新时间", html! { "—" }))
                    }
                }
            }

            // ── BOM结构 ──
            div class="detail-card" style="margin-top:var(--space-5)" {
                div class="detail-card-title" {
                    span { "BOM结构" }
                    span style="color:var(--text-tertiary);font-weight:400;font-size:12px" {
                        "（共 " (node_count) " 个节点）"
                    }
                }
                @if bom.bom_detail.nodes.is_empty() {
                    div class="empty-state" { "暂无BOM节点" }
                } @else {
                    table class="bom-table" {
                        thead {
                            tr {
                                th style="width:40px" { "编号" }
                                th style="width:40px" { "层级" }
                                th style="width:120px" { "产品编码" }
                                th { "产品" }
                                th style="width:100px" { "工作中心" }
                                th style="width:80px" { "数量" }
                                th style="width:60px" { "单位" }
                                th style="width:80px" { "损耗率" }
                                th { "备注" }
                            }
                        }
                        tbody {
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


            // ── Delete Confirm Dialog ──
            (confirm_dialog::confirm_dialog(
                "deleteOpen",
                "确认删除",
                &format!("确定要删除 BOM <strong>{}</strong> 吗？此操作不可撤销。", bom.bom_name),
                "确认删除",
                "delete-bom-form",
                html! {
                    form id="delete-bom-form" class="hidden"
                        hx-post=(delete_path.to_string())
                        hx-target="body"
                        hx-swap="outerHTML" {}
                },
            ))
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


/// Build a map from node id → depth. Root nodes (parent_id == 0) have depth 0,
/// others have parent_depth + 1.
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
    let work_center = node.work_center.as_deref().filter(|s| !s.is_empty()).unwrap_or("—");
    let remark = node.remark.as_deref().filter(|s| !s.is_empty()).unwrap_or("");
    let loss_rate = if node.loss_rate == Decimal::ZERO {
        "—".to_string()
    } else {
        format!("{}%", node.loss_rate)
    };

    // Row background class (matching old code getNodeRowStyle)
    let row_class = if level == 1 {
        "bom-row-level-0"
    } else if has_children {
        "bom-row-level-1"
    } else {
        "bom-row-level-default"
    };

    html! {
        tr class=(row_class) {
            td style="text-align:center" { (index + 1) }
            td style="text-align:center" { (level) }
            td class="mono" { (code) }
            td { (name) }
            td { (work_center) }
            td class="mono" style="text-align:right" { (node.quantity) }
            td { (unit) }
            td style="text-align:right" { (loss_rate) }
            td style="color:var(--muted)" { (remark) }
        }
    }
}

fn workflow_step(label: &str, active: bool, completed: bool) -> Markup {
    let state_class = if active { "workflow-step-active" } else if completed { "workflow-step-completed" } else { "workflow-step-pending" };
    html! {
        div class=(format!("workflow-step {state_class}")) {
            div class="workflow-step-dot" {}
            span { (label) }
        }
    }
}

fn workflow_connector(completed: bool) -> Markup {
    let cls = if completed { "workflow-connector-done" } else { "workflow-connector" };
    html! {
        div class=(cls) {}
    }
}
