use axum::http::HeaderMap;
use axum::response::Html;
use maud::{Markup, html};

use abt_core::master_data::routing::RoutingService;
use abt_core::master_data::routing::model::*;

use abt_macros::require_permission;

use crate::components::{confirm_dialog, detail::detail_row, icon};
use crate::layout::page::admin_page;
use crate::routes::routing::{RoutingDeletePath, RoutingDetailPath, RoutingListPath};
use crate::utils::RequestContext;

// ── Handlers ──

#[require_permission("ROUTING", "read")]
pub async fn get_routing_detail(
    path: RoutingDetailPath,
    ctx: RequestContext,
    headers: HeaderMap,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.routing_service();

    let detail = svc.get_detail(&service_ctx, &mut conn, path.id).await?;

    // Resolve associated BOMs
    let boms = svc.list_boms_by_routing(&service_ctx, &mut conn, path.id).await.unwrap_or_default();

    let content = routing_detail_page(&detail, &boms);
    let detail_path_str = RoutingDetailPath { id: path.id }.to_string();
    let page_html = admin_page(
        &headers,
        &format!("{} - 工艺路线详情", detail.routing.name),
        &claims,
        "md-routing",
        &detail_path_str,
        "主数据管理",
        Some(&detail.routing.name),
        content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("ROUTING", "update")]
pub async fn update_routing(
    _path: RoutingDetailPath,
    _ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    Ok(Html("<p>Update routing placeholder</p>".into()))
}

// ── Components ──

fn routing_detail_page(
    detail: &RoutingDetail,
    boms: &[BomRouting],
) -> Markup {
    let routing = &detail.routing;
    let steps = &detail.steps;
    let list_path = RoutingListPath;
    let delete_path = RoutingDeletePath { id: routing.id };

    let required_count = steps.iter().filter(|s| s.is_required).count();
    let step_count = steps.len();
    let bom_count = boms.len();

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
                            (routing.name)
                        }
                        div class="customer-meta" {
                            span { "工序: " (step_count) }
                            span { "必经: " (required_count) }
                            span { "关联BOM: " (bom_count) }
                            @if let Some(dt) = routing.created_at {
                                span { "创建: " (dt.format("%Y-%m-%d")) }
                            }
                        }
                    }
                }
                div class="page-actions" {
                    a class="btn btn-default" href=(list_path) {
                        (icon::arrow_left_icon("w-4 h-4"))
                        " 返回列表"
                    }
                    a class="btn btn-default" href="#" {
                        (icon::edit_icon("w-4 h-4"))
                        " 编辑"
                    }
                    a class="btn btn-default" href="#" {
                        (icon::copy_icon("w-4 h-4"))
                        " 复制"
                    }
                    button class="btn btn-danger-ghost" x-on:click="deleteOpen = true" {
                        (icon::trash_icon("w-4 h-4"))
                        " 删除"
                    }
                }
            }

            // ── 基本信息 ──
            div class="detail-card" {
                div class="detail-card-title" { "基本信息" }
                div class="detail-grid" style="grid-template-columns:repeat(3,1fr)" {
                    (detail_row("编码", html! { span class="mono" { (routing.id) } }))
                    (detail_row("名称", html! { (routing.name) }))
                    (detail_row("描述", html! { (routing.description.as_deref().unwrap_or("—")) }))
                    (detail_row("创建人", html! {
                        @match routing.operator_id {
                            Some(uid) => { "ID: " (uid) }
                            None => { "—" }
                        }
                    }))
                    @if let Some(dt) = routing.created_at {
                        (detail_row("创建时间", html! { (dt.format("%Y-%m-%d %H:%M")) }))
                    } @else {
                        (detail_row("创建时间", html! { "—" }))
                    }
                    @if let Some(dt) = routing.updated_at {
                        (detail_row("更新时间", html! { (dt.format("%Y-%m-%d %H:%M")) }))
                    } @else {
                        (detail_row("更新时间", html! { "—" }))
                    }
                }
            }

            // ── 工序流程 ──
            div class="detail-card" style="margin-top:var(--space-5)" {
                div class="detail-card-title" {
                    span { "工序流程" }
                    span style="color:var(--text-tertiary);font-weight:400;font-size:12px" {
                        "（共 " (step_count) " 道工序）"
                    }
                }
                @if steps.is_empty() {
                    div class="empty-state" { "暂无工序步骤" }
                } @else {
                    table class="data-table" {
                        thead {
                            tr {
                                th style="width:60px" { "序号" }
                                th style="width:120px" { "工序代码" }
                                th { "工序名称" }
                                th style="width:80px" { "是否必经" }
                                th { "备注" }
                            }
                        }
                        tbody {
                            @for step in steps {
                                tr {
                                    td class="mono" { (step.step_order) }
                                    td class="mono" { (step.process_code) }
                                    td { (step.process_code) }
                                    td {
                                        @if step.is_required {
                                            span class="status-pill status-accepted" { "必经" }
                                        } @else {
                                            span class="status-pill status-draft" { "选检" }
                                        }
                                    }
                                    td { (step.remark.as_deref().unwrap_or("—")) }
                                }
                            }
                        }
                    }
                }
            }

            // ── 关联BOM ──
            div class="detail-card" style="margin-top:var(--space-5)" {
                div class="detail-card-title" { "关联BOM" }
                @if boms.is_empty() {
                    div class="empty-state" { "暂无关联BOM" }
                } @else {
                    table class="data-table" {
                        thead {
                            tr {
                                th style="width:60px" { "ID" }
                                th { "产品编码" }
                                th style="width:160px" { "关联时间" }
                            }
                        }
                        tbody {
                            @for bom in boms {
                                tr {
                                    td class="mono" { (bom.id) }
                                    td class="mono" { (bom.product_code) }
                                    td {
                                        @if let Some(dt) = bom.created_at {
                                            (dt.format("%Y-%m-%d %H:%M"))
                                        } @else {
                                            "—"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Delete Confirm Dialog ──
            (confirm_dialog::confirm_dialog(
                "deleteOpen",
                "确认删除",
                &format!("确定要删除工艺路线 <strong>{}</strong> 吗？此操作不可撤销。", routing.name),
                "确认删除",
                "delete-routing-form",
                html! {
                    form id="delete-routing-form" class="hidden"
                        hx-post=(delete_path.to_string())
                        hx-target="body"
                        hx-swap="outerHTML" {}
                },
            ))
        }
    }
}

// ── Helpers ──
