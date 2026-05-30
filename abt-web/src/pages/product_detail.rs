use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse};
use maud::{Markup, html};

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::*;

use abt_macros::require_permission;

use crate::components::{confirm_dialog, detail::detail_row, icon};
use crate::layout::page::admin_page;
use crate::routes::product::{ProductDeletePath, ProductDetailPath, ProductListPath, ProductUpdatePath};
use crate::utils::RequestContext;

// ── Handlers ──

#[require_permission("PRODUCT", "read")]
pub async fn get_product_detail(
    path: ProductDetailPath,
    ctx: RequestContext,
    headers: HeaderMap,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.product_service();

    let product = svc.get(&service_ctx, &mut conn, path.id).await?;

    let usage = svc.check_product_usage(
        &service_ctx,
        &mut conn,
        path.id,
        UsageQuery { page: 1, page_size: 50 },
    ).await?;

    let content = product_detail_page(&product, &usage.items);
    let detail_path_str = ProductDetailPath { id: path.id }.to_string();
    let page_html = admin_page(
        &headers,
        &format!("{} - 产品详情", product.pdt_name),
        &claims,
        "md-product",
        &detail_path_str,
        "主数据管理",
        Some(&product.product_code),
        content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("PRODUCT", "update")]
pub async fn update_product(
    path: ProductUpdatePath,
    _ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    Ok(axum::response::Redirect::to(
        &ProductDetailPath { id: path.id }.to_string(),
    ))
}

// ── Components ──

fn product_detail_page(product: &Product, usage_entries: &[UsageEntry]) -> Markup {
    let list_path = ProductListPath;
    let update_path = ProductUpdatePath { id: product.product_id };
    let delete_path = ProductDeletePath { id: product.product_id };

    let (status_label, status_class) = status_display(product.status);

    html! {
        div x-data="{ deleteOpen: false }" {
            // ── Detail Top ──
            div class="detail-top" {
                div class="customer-identity" {
                    div class="customer-avatar" style="background:var(--color-primary-light,#e0e7ff)" {
                        (icon::box_icon("w-5 h-5"))
                    }
                    div {
                        h1 class="customer-name" {
                            (product.pdt_name)
                            " "
                            span class=(format!("status-pill {status_class}")) { (status_label) }
                        }
                        div class="customer-meta" {
                            span { "编码: " (product.product_code) }
                            span { "单位: " (product.unit) }
                            @if let Some(dt) = product.created_at {
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
                    a class="btn btn-primary" href=(update_path) {
                        (icon::edit_icon("w-4 h-4"))
                        " 编辑"
                    }
                }
            }

            // ── 3-Column Detail Grid ──
            div class="detail-grid" {
                // ── Left: 基本信息 ──
                div class="detail-card" {
                    div class="detail-card-title" { "基本信息" }
                    (detail_row("产品编码", html! { span class="mono" { (product.product_code) } }))
                    (detail_row("产品名称", html! { (product.pdt_name) }))
                    (detail_row("规格型号", html! {
                        @if product.meta.specification.is_empty() { "—" } @else { (&product.meta.specification) }
                    }))
                    (detail_row("单位", html! { (product.unit) }))
                    (detail_row("获取途径", html! {
                        @if product.meta.acquire_channel.is_empty() { "—" } @else { (&product.meta.acquire_channel) }
                    }))
                    (detail_row("状态", html! {
                        span class=(format!("status-pill {status_class}")) { (status_label) }
                    }))
                    (detail_row("创建时间", html! {
                        @if let Some(dt) = product.created_at { (dt.format("%Y-%m-%d %H:%M")) } @else { "—" }
                    }))
                    (detail_row("更新时间", html! {
                        @if let Some(dt) = product.updated_at { (dt.format("%Y-%m-%d %H:%M")) } @else { "—" }
                    }))
                }

                // ── Center: 分类与归属 ──
                div class="detail-card" {
                    div class="detail-card-title" { "分类与归属" }
                    (detail_row("外部编码", html! {
                        (product.external_code.as_deref().unwrap_or("—"))
                    }))
                    (detail_row("旧编码", html! {
                        (product.meta.old_code.as_deref().unwrap_or("—"))
                    }))
                    (detail_row("归属部门", html! {
                        @if let Some(_dept_id) = product.owner_department_id { "—" } @else { "—" }
                    }))
                }

                // ── Right: 规格参数 ──
                div class="detail-card" {
                    div class="detail-card-title" { "规格参数" }
                    @if product.meta.specification.is_empty() {
                        div class="empty-state" { "暂无规格参数" }
                    } @else {
                        @for line in product.meta.specification.lines() {
                            div class="detail-row" {
                                span class="detail-value" style="white-space:pre-wrap;word-break:break-all" {
                                    (line)
                                }
                            }
                        }
                    }
                }
            }

            // ── BOM Usage Table (full width) ──
            div class="detail-card" style="margin-top:var(--space-5)" {
                div class="detail-card-title" { "BOM 引用" }
                @if usage_entries.is_empty() {
                    div class="empty-state" { "该产品未被任何 BOM 引用" }
                } @else {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "来源类型" }
                                th { "来源 ID" }
                                th { "来源名称" }
                            }
                        }
                        tbody {
                            @for entry in usage_entries {
                                tr {
                                    td { (entry.source_type) }
                                    td class="mono" { (entry.source_id) }
                                    td { (entry.source_name) }
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
                &format!("确定要删除产品 <strong>{}</strong> 吗？此操作不可撤销。", product.pdt_name),
                "确认删除",
                "delete-product-form",
                html! {
                    form id="delete-product-form" class="hidden"
                        hx-post=(delete_path.to_string())
                        hx-target="closest div[x-data]" {}
                },
            ))
        }
    }
}

fn status_display(status: ProductStatus) -> (&'static str, &'static str) {
    match status {
        ProductStatus::Active => ("在用", "status-accepted"),
        ProductStatus::Inactive => ("停用", "status-draft"),
        ProductStatus::Obsolete => ("作废", "status-rejected"),
    }
}

