use std::collections::HashMap;

use axum::http::HeaderMap;
use axum::response::Html;
use maud::{Markup, html};
use rust_decimal::Decimal;

use abt_core::master_data::bom::{BomCommandService, BomCostService, BomQueryService};
use abt_core::master_data::bom::model::*;
use abt_core::master_data::product::ProductService;

use abt_macros::require_permission;

use crate::components::{confirm_dialog, icon};
use crate::layout::page::admin_page;
use crate::routes::bom::{BomCostDrawerPath, BomDeletePath, BomDetailPath, BomEditPath, BomLaborCostDrawerPath, BomListPath, BomPublishPath};
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
    path: BomPublishPath,
    ctx: RequestContext,
) -> crate::errors::Result<impl axum::response::IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let query_svc = state.bom_query_service();
    let bom = query_svc.get(&service_ctx, &mut conn, path.id).await?;

    let cmd_svc = state.bom_command_service();
    if bom.status == BomStatus::Published {
        cmd_svc.unpublish(&service_ctx, &mut conn, path.id).await?;
    } else {
        cmd_svc.publish(&service_ctx, &mut conn, path.id).await?;
    }

    let redirect = BomDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("BOM", "read")]
pub async fn get_cost_drawer(
    path: BomCostDrawerPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let cost_svc = state.bom_cost_service();
    let report = cost_svc.get_cost_report(&service_ctx, &mut conn, path.id, None).await?;

    Ok(Html(cost_drawer_content(&report).into_string()))
}

#[require_permission("BOM", "read")]
pub async fn get_labor_cost_drawer(
    path: BomLaborCostDrawerPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let cost_svc = state.bom_cost_service();
    let report = cost_svc.get_labor_cost_report(&service_ctx, &mut conn, path.id).await?;
    let bom_svc = state.bom_query_service();
    let bom = bom_svc.get(&service_ctx, &mut conn, path.id).await?;

    Ok(Html(labor_cost_drawer_content(&bom.bom_name, &report).into_string()))
}

// ── Components ──

fn bom_detail_page(
    bom: &Bom,
    product_map: &HashMap<i64, &abt_core::master_data::product::model::Product>,
) -> Markup {
    let list_path = BomListPath;
    let delete_path = BomDeletePath { id: bom.bom_id };
    let publish_path = BomPublishPath { id: bom.bom_id };
    let cost_drawer_path = BomCostDrawerPath { id: bom.bom_id };
    let labor_drawer_path = BomLaborCostDrawerPath { id: bom.bom_id };
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
        div x-data="{ deleteOpen: false, publishOpen: false, costOpen: false, laborOpen: false }" {
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
                    button class="btn btn-default"
                        hx-get=(cost_drawer_path.to_string())
                        hx-target="#cost-drawer-body"
                        hx-swap="innerHTML"
                        x-on:click="costOpen = true" {
                        (icon::currency_icon("w-4 h-4"))
                        " 查看成本"
                    }
                    button class="btn btn-default"
                        hx-get=(labor_drawer_path.to_string())
                        hx-target="#labor-drawer-body"
                        hx-swap="innerHTML"
                        x-on:click="laborOpen = true" {
                        (icon::bolt_icon("w-4 h-4"))
                        " 查看人工成本"
                    }
                    a class="btn btn-primary" href=(BomEditPath { id: bom.bom_id }) {
                        (icon::edit_icon("w-4 h-4"))
                        " 编辑"
                    }
                    @if is_draft {
                        button class="btn btn-primary"
                            x-on:click="publishOpen = true" {
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

            // ── BOM结构 ──
            div class="detail-card" {
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

            // ── Publish Confirm Dialog ──
            @if is_draft {
                (confirm_dialog::confirm_dialog(
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

            // ── Cost Drawer (wider: 1000px) ──
            div class="drawer-overlay"
                x-bind:class="{ 'open': costOpen }"
                x-on:click="if(event.target===this) costOpen = false" {
                div class="drawer" style="max-width:1000px;width:100%" x-on:click="event.stopPropagation()" {
                    div class="drawer-head" {
                        h2 { (icon::currency_icon("w-5 h-5")) " BOM成本报告" }
                        button style="background:none;border:none;cursor:pointer;font-size:22px;color:var(--muted);padding:4px;line-height:1"
                            x-on:click="costOpen = false" { "×" }
                    }
                    div class="drawer-body" {
                        div id="cost-drawer-body" {
                            div style="text-align:center;padding:40px;color:var(--muted)" { "加载中..." }
                        }
                    }
                    div class="drawer-foot" {
                        button type="button" class="btn btn-default"
                            x-on:click="costOpen = false" { "关闭" }
                    }
                }
            }

            // ── Labor Cost Drawer (wider: 800px) ──
            div class="drawer-overlay"
                x-bind:class="{ 'open': laborOpen }"
                x-on:click="if(event.target===this) laborOpen = false" {
                div class="drawer" style="max-width:800px;width:100%" x-on:click="event.stopPropagation()" {
                    div class="drawer-head" {
                        h2 { (icon::bolt_icon("w-5 h-5")) " BOM 人工成本" }
                        button style="background:none;border:none;cursor:pointer;font-size:22px;color:var(--muted);padding:4px;line-height:1"
                            x-on:click="laborOpen = false" { "×" }
                    }
                    div class="drawer-body" {
                        div id="labor-drawer-body" {
                            div style="text-align:center;padding:40px;color:var(--muted)" { "加载中..." }
                        }
                    }
                    div class="drawer-foot" {
                        button type="button" class="btn btn-default"
                            x-on:click="laborOpen = false" { "关闭" }
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


// ── Cost Drawer Content ──

fn format_currency(d: Decimal) -> String {
    let val = d.round_dp(6);
    format!("¥{}", val)
}

fn format_amount(unit_price: Decimal, quantity: Decimal) -> String {
    format_currency(unit_price * quantity)
}

fn cost_drawer_content(report: &BomCostReport) -> Markup {
    let has_warnings = !report.warnings.is_empty();
    let has_labor_cost_issue = !report.labor_costs.is_empty()
        && report.labor_costs.iter().all(|item| item.unit_price == Decimal::ZERO);
    let has_cost_issue = has_warnings || has_labor_cost_issue;

    let material_total: Decimal = report.material_costs.iter()
        .filter_map(|item| item.unit_price.map(|p| p * item.quantity))
        .sum();
    let labor_total: Decimal = report.labor_costs.iter()
        .map(|item| item.unit_price * item.quantity)
        .sum();

    html! {
        // Warning banner
        @if has_warnings {
            div class="cost-warning-banner" x-data="{ warnOpen: false }" {
                button type="button" class="cost-warning-toggle"
                    x-on:click="warnOpen = !warnOpen" {
                    div class="warning-left" {
                        (icon::circle_alert_icon("w-4 h-4"))
                        span { "部分材料缺失单价（共 " (report.warnings.len()) " 项）" }
                    }
                    (icon::chevron_down_icon("w-4 h-4"))
                }
                div class="cost-warning-list" x-show="warnOpen" {
                    ul style="list-style:none;margin:0;padding:0" {
                        @for w in &report.warnings {
                            li { "- " (w) }
                        }
                    }
                }
            }
        }

        // Product code
        div class="cost-product-code" {
            p { "产品编码：" span { (report.product_code) } }
        }

        // Summary cards
        div class="cost-summary-grid" {
            div class="cost-summary-card primary" {
                div class="card-label" { "材料成本" }
                div class="card-value" { (format_currency(material_total)) }
                div class="card-sub" { (report.material_costs.len()) " 项材料" }
            }
            div class={"cost-summary-card " (if has_labor_cost_issue { "danger" } else { "" })} {
                div class="card-label" { "人工成本" }
                div class="card-value" { (format_currency(labor_total)) }
                div class="card-sub" {
                    (report.labor_costs.len()) " 道工序"
                    @if has_labor_cost_issue { "（单价为0）" }
                }
            }
            div class={"cost-summary-card " (if has_cost_issue { "total-warn" } else { "total-ok" })} {
                div class="card-label" { "总成本" }
                @if has_cost_issue {
                    div class="card-value" { "-" }
                    div class="card-sub" {
                        @if has_warnings && has_labor_cost_issue {
                            "材料缺失单价，人工成本为0"
                        } @else if has_warnings {
                            "存在缺失单价"
                        } @else {
                            "人工成本为0"
                        }
                    }
                } @else {
                    div class="card-value" { (format_currency(material_total + labor_total)) }
                    div class="card-sub" { "已完成计算" }
                }
            }
        }

        // Material cost table
        div style="margin-bottom:24px" {
            div class="cost-section-title" { "【材料成本】" }
            table class="cost-drawer-table" {
                thead {
                    tr {
                        th style="min-width:160px" { "产品名称" }
                        th { "产品编码" }
                        th class="text-right" { "数量" }
                        th class="text-right" { "单价" }
                        th class="text-right" { "小计" }
                    }
                }
                tbody {
                    @for item in &report.material_costs {
                        @let is_missing = item.unit_price.is_none();
                        tr class=(if is_missing { "row-danger" } else { "" }) {
                            td class="cell-name" style="font-weight:500"
                                title=(item.product_name) {
                                (item.product_name)
                            }
                            td class="font-mono" style="color:#6b7280" { (item.product_code) }
                            td class="text-right font-mono" { (item.quantity) }
                            td class="text-right" {
                                @match item.unit_price {
                                    Some(price) => {
                                        span class="font-mono" { (format_currency(price)) }
                                    }
                                    None => {
                                        span class="missing-price" { "缺失" }
                                    }
                                }
                            }
                            td class="text-right" style="font-weight:500" {
                                @match item.unit_price {
                                    Some(price) => {
                                        span class="font-mono" style="color:#2563eb" {
                                            (format_amount(price, item.quantity))
                                        }
                                    }
                                    None => {
                                        span class="missing-price" { "-" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div class="cost-drawer-footer bg-blue" {
                span class="footer-label" { "材料成本合计:" }
                span class="footer-value blue" { (format_currency(material_total)) }
            }
        }

        // Labor cost table
        div style="margin-bottom:24px" {
            div class="cost-section-title" { "【人工成本】" }
            table class="cost-drawer-table" {
                thead {
                    tr {
                        th { "工序名称" }
                        th class="text-right" { "单价" }
                        th class="text-right" { "数量" }
                        th class="text-right" { "小计" }
                    }
                }
                tbody {
                    @if report.labor_costs.is_empty() {
                        tr {
                            td colspan="4" style="text-align:center;padding:32px;color:#9ca3af" { "暂无人工成本数据" }
                        }
                    } @else {
                        @for item in &report.labor_costs {
                            @let is_zero = item.unit_price == Decimal::ZERO;
                            tr class=(if is_zero { "row-danger" } else { "" }) {
                                td style="font-weight:500" { (item.name) }
                                td class="text-right" {
                                    @if is_zero {
                                        span style="color:#ef4444;font-weight:500" { "¥0.000000" }
                                    } @else {
                                        span class="font-mono" { (format_currency(item.unit_price)) }
                                    }
                                }
                                td class="text-right font-mono" { (item.quantity) }
                                td class="text-right" style="font-weight:500" {
                                    @if is_zero {
                                        span style="color:#ef4444" { (format_amount(item.unit_price, item.quantity)) }
                                    } @else {
                                        span class="font-mono" style="color:#2563eb" {
                                            (format_amount(item.unit_price, item.quantity))
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div class={"cost-drawer-footer " (if has_labor_cost_issue { "bg-red" } else { "bg-blue" })} {
                span class={"footer-label" } { "人工成本合计:" }
                span class={"footer-value " (if has_labor_cost_issue { "red" } else { "blue" })} {
                    (format_currency(labor_total))
                }
                @if has_labor_cost_issue {
                    span style="font-size:11px;color:#ef4444;margin-left:4px" { "（所有工序单价为0）" }
                }
            }
        }

        // Total footer
        div class="cost-drawer-footer bg-gray" style="padding:14px 16px" {
            @if has_cost_issue {
                span style="font-size:13px;font-weight:500;color:#d97706" {
                    @if has_warnings && has_labor_cost_issue {
                        "请补全材料单价并设置人工成本"
                    } @else if has_warnings {
                        "请补全所有材料单价"
                    } @else {
                        "请设置人工成本单价"
                    }
                }
            } @else {
                span class="footer-label" { "总成本:" }
                span class="footer-value dark" style="font-size:18px" {
                    (format_currency(material_total + labor_total))
                }
            }
        }
    }
}

fn labor_cost_drawer_content(bom_name: &str, report: &BomLaborCostReport) -> Markup {
    let has_issue = !report.items.is_empty()
        && report.items.iter().all(|item| item.unit_price == Decimal::ZERO);

    html! {
        // Product code
        div class="cost-product-code" {
            p { "BOM：" span style="font-weight:500" { (bom_name) } }
        }

        // Labor cost summary card
        div class="labor-summary-card" {
            div class="card-label" { "人工成本合计" }
            div class="card-value" { (format_currency(report.total_cost)) }
            div class="card-sub" {
                (report.items.len()) " 道工序"
                @if has_issue { "（所有工序单价为0）" }
            }
        }

        // Detail table
        div style="margin-bottom:24px" {
            div class="cost-section-title" { "【人工成本明细】" }
            table class="cost-drawer-table" {
                thead {
                    tr {
                        th { "工序名称" }
                        th class="text-right" { "单价" }
                        th class="text-right" { "数量" }
                        th class="text-right" { "小计" }
                        th { "备注" }
                    }
                }
                tbody {
                    @if report.items.is_empty() {
                        tr {
                            td colspan="5" style="text-align:center;padding:32px;color:#9ca3af" { "暂无人工成本数据" }
                        }
                    } @else {
                        @for item in &report.items {
                            @let is_zero = item.unit_price == Decimal::ZERO;
                            tr class=(if is_zero { "row-danger" } else { "" }) {
                                td style="font-weight:500" { (item.name) }
                                td class="text-right" {
                                    @if is_zero {
                                        span style="color:#ef4444;font-weight:500" { "¥0.000000" }
                                    } @else {
                                        span class="font-mono" { (format_currency(item.unit_price)) }
                                    }
                                }
                                td class="text-right font-mono" { (item.quantity) }
                                td class="text-right" style="font-weight:500" {
                                    @if is_zero {
                                        span style="color:#ef4444" { (format_amount(item.unit_price, item.quantity)) }
                                    } @else {
                                        span class="font-mono" style="color:#2563eb" {
                                            (format_amount(item.unit_price, item.quantity))
                                        }
                                    }
                                }
                                td style="color:#6b7280" {
                                    @if item.remark.is_empty() { "—" } @else { (item.remark) }
                                }
                            }
                        }
                    }
                }
            }
            div class={"cost-drawer-footer " (if has_issue { "bg-red" } else { "bg-blue" })} {
                span class="footer-label" { "人工成本合计:" }
                span class={"footer-value " (if has_issue { "red" } else { "blue" })} {
                    (format_currency(report.total_cost))
                }
                @if has_issue {
                    span style="font-size:11px;color:#ef4444;margin-left:4px" { "（所有工序单价为0）" }
                }
            }
        }
    }
}
