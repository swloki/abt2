use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::*;
use abt_core::shared::types::PaginatedResult;

use abt_macros::require_permission;

use crate::components::{confirm_dialog, detail::detail_row, icon};
use crate::components::pagination::htmx_pagination;
use crate::layout::page::admin_page;
use crate::routes::bom::BomDetailPath;
use crate::routes::product::{ProductDeletePath, ProductDetailPath, ProductEditPath, ProductListPath, ProductUpdatePath, ProductUsageTablePath};
use crate::utils::RequestContext;

// ── Handlers ──

#[require_permission("PRODUCT", "read")]
pub async fn get_product_detail(
    path: ProductDetailPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.product_service();

    let product = svc.get(&service_ctx, &mut conn, path.id).await?;

    let content = product_detail_page(&product);
    let detail_path_str = ProductDetailPath { id: path.id }.to_string();
    let page_html = admin_page(
        is_htmx,
        &format!("{} - 产品详情", product.pdt_name),
        &claims,
        "md",
        &detail_path_str,
        "主数据管理",
        Some(&product.product_code),
        content,
    );

    Ok(Html(page_html.into_string()))
}

// ── Usage Table Query ──

#[derive(Debug, Deserialize)]
pub struct UsageTableParams {
    pub page: Option<u32>,
}

#[require_permission("PRODUCT", "read")]
pub async fn get_product_usage_table(
    path: ProductUsageTablePath,
    ctx: RequestContext,
    Query(params): Query<UsageTableParams>,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.product_service();

    let page = params.page.unwrap_or(1);
    let usage = svc.check_product_usage(
        &service_ctx,
        &mut conn,
        path.id,
        UsageQuery { page, page_size: 10 },
    ).await?;

    Ok(Html(usage_table_fragment(path.id, &usage).into_string()))
}

#[require_permission("PRODUCT", "update")]
pub async fn get_product_edit(
    path: ProductEditPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.product_service();

    let product = svc.get(&service_ctx, &mut conn, path.id).await?;
    let title = format!("{} - 编辑产品", product.pdt_name);
    let edit_path_str = ProductEditPath { id: path.id }.to_string();
    let content = product_edit_page(&product);
    let page_html = admin_page(
        is_htmx,
        &title,
        &claims,
        "md",
        &edit_path_str,
        "主数据管理",
        Some(&title),
        content,
    );

    Ok(Html(page_html.into_string()))
}

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub struct ProductEditForm {
    pub name: String,
    pub unit: String,
    pub specification: String,
    pub acquire_channel: Option<String>,
    pub external_code: Option<String>,
    pub owner_department_id: Option<String>,
    pub old_code: Option<String>,
    pub remark: Option<String>,
}

#[require_permission("PRODUCT", "update")]
pub async fn update_product(
    path: ProductUpdatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ProductEditForm>,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.product_service();

    let owner_department_id = form
        .owner_department_id
        .as_deref()
        .and_then(|s| if s.is_empty() { None } else { s.parse::<i64>().ok() });

    let req = UpdateProductReq {
        name: Some(form.name),
        unit: Some(form.unit),
        external_code: form.external_code.filter(|s| !s.is_empty()),
        owner_department_id,
        meta: Some(ProductMeta {
            specification: form.specification,
            acquire_channel: form
                .acquire_channel
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "采购".to_string()),
            old_code: form.old_code.filter(|s| !s.is_empty()),
            remark: form.remark.filter(|s| !s.is_empty()),
        }),
    };

    svc.update(&service_ctx, &mut conn, path.id, req).await?;

    let redirect = ProductDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn product_detail_page(product: &Product) -> Markup {
    let list_path = ProductListPath;
    let edit_path = ProductEditPath { id: product.product_id };
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
                    a class="btn btn-primary" href=(edit_path) {
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
                    (detail_row("备注", html! {
                        @if let Some(ref r) = product.meta.remark {
                            span style="white-space:pre-wrap" { (r) }
                        } @else {
                            "—"
                        }
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

            // ── 使用情况（BOM 引用）── HTMX lazy load + pagination
            @let usage_table_path = ProductUsageTablePath { id: product.product_id };
            div class="detail-card" style="margin-top:var(--space-5)"
                hx-get=(usage_table_path.to_string())
                hx-trigger="load" {}

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

// ── Edit Page ──

fn product_edit_page(product: &Product) -> Markup {
    let update_path = ProductUpdatePath { id: product.product_id };
    let detail_path = ProductDetailPath { id: product.product_id };

    let acquire_val = &product.meta.acquire_channel;
    let external_code_val = product.external_code.as_deref().unwrap_or("");
    let old_code_val = product.meta.old_code.as_deref().unwrap_or("");
    let remark_val = product.meta.remark.as_deref().unwrap_or("");

    html! {
        div {
            // ── Page Header ──
            div class="page-header" {
                a class="back-link" href=(detail_path) {
                    (icon::arrow_left_icon("w-4 h-4"))
                    "返回产品详情"
                }
                h1 class="page-title" { "编辑产品" }
            }

            form id="product-edit-form"
                  hx-post=(update_path)
                  hx-swap="none" {

                // ── Section: 基本信息 ──
                div class="data-card" style="margin-bottom:var(--space-4)" {
                    div class="form-section-title" { "基本信息" }
                    div class="form-grid" {
                        div class="form-field" {
                            label { "产品名称 " span style="color:var(--danger)" { "*" } }
                            input type="text" name="name" required placeholder="请输入产品名称" value=(product.pdt_name) {}
                        }
                        div class="form-field" {
                            label { "产品编码" }
                            input type="text" value=(product.product_code) readonly
                                style="background:var(--surface);color:var(--muted)" {}
                        }
                        div class="form-field" {
                            label { "规格型号 " span style="color:var(--danger)" { "*" } }
                            input type="text" name="specification" required placeholder="请输入规格型号" value=(product.meta.specification) {}
                        }
                        div class="form-field" {
                            label { "计量单位 " span style="color:var(--danger)" { "*" } }
                            input type="text" name="unit" required placeholder="请输入计量单位" value=(product.unit) {}
                        }
                        div class="form-field" {
                            label { "获取途径" }
                            select name="acquire_channel" {
                                option value="采购" selected[acquire_val == "采购"] { "采购" }
                                option value="自制" selected[acquire_val == "自制"] { "自制" }
                                option value="委外" selected[acquire_val == "委外"] { "委外" }
                            }
                        }
                        div class="form-field" {
                            label { "外部编码" }
                            input type="text" name="external_code" placeholder="请输入外部编码" value=(external_code_val) {}
                        }
                    }
                }

                // ── Section: 分类与归属 ──
                div class="data-card" style="margin-bottom:var(--space-4)" {
                    div class="form-section-title" { "分类与归属" }
                    div class="form-grid" {
                        div class="form-field" {
                            label { "归属部门" }
                            select name="owner_department_id" {
                                option value="" { "-- 请选择 --" }
                            }
                        }
                        div class="form-field" {
                            label { "旧编码" }
                            input type="text" name="old_code" placeholder="请输入旧编码" value=(old_code_val) {}
                        }
                    }
                }

                // ── Section: 其他信息 ──
                div class="data-card" style="margin-bottom:var(--space-4)" {
                    div class="form-section-title" { "其他信息" }
                    div class="form-grid" {
                        div class="form-field field-full" {
                            label { "备注" }
                            textarea name="remark" placeholder="请输入备注信息…"
                                style="width:100%;min-height:80px;resize:vertical" {
                                (remark_val)
                            }
                        }
                    }
                }

                // ── Action Bar ──
                div class="create-action-bar" {
                    a class="btn btn-default" href=(detail_path) { "取消" }
                    button type="submit" class="btn btn-primary" {
                        "保存修改"
                    }
                }
            }
        }
    }
}

// ── Usage Table Fragment (HTMX) ──

fn usage_table_fragment(product_id: i64, result: &PaginatedResult<UsageEntry>) -> Markup {
    let usage_path = ProductUsageTablePath { id: product_id };
    let base_path = usage_path.to_string();

    html! {
        div class="detail-card-title" {
            span { "使用情况（BOM 引用）" }
            span style="font-size:var(--text-xs);color:var(--muted);font-weight:400" { "该产品被以下 BOM 引用" }
        }
        @if result.items.is_empty() {
            div class="empty-state" { "该产品未被任何 BOM 引用" }
        } @else {
            table class="usage-table" {
                thead {
                    tr {
                        th { "BOM 名称" }
                        th { "BOM 编码" }
                        th { "版本" }
                        th { "用量" }
                        th { "BOM 状态" }
                        th { "更新日期" }
                    }
                }
                tbody {
                    @for entry in &result.items {
                        @let bom_detail_path = BomDetailPath { id: entry.source_id };
                        @let (status_label, status_class) = match entry.bom_status {
                            Some(1) => ("草稿", "status-draft"),
                            Some(2) => ("已生效", "status-accepted"),
                            _ => ("未知", "status-draft"),
                        };
                        tr {
                            td {
                                strong { (entry.source_name) }
                            }
                            td {
                                a href=(bom_detail_path.to_string()) style="color:var(--accent);text-decoration:none" {
                                    @if let Some(code) = &entry.parent_product_code {
                                        (code)
                                    } @else {
                                        "BOM-" (entry.source_id)
                                    }
                                }
                            }
                            td class="mono" {
                                @if let Some(v) = entry.bom_version {
                                    "V" (v)
                                } @else {
                                    "—"
                                }
                            }
                            td {
                                @if let Some(qty) = entry.quantity {
                                    (qty)
                                    " "
                                    @if let Some(unit) = &entry.node_unit {
                                        (unit)
                                    } @else {
                                        "pcs"
                                    }
                                    "/套"
                                } @else {
                                    "—"
                                }
                            }
                            td {
                                span class=(format!("status-pill {status_class}")) { (status_label) }
                            }
                            td class="mono" {
                                @if let Some(dt) = entry.bom_updated_at {
                                    (dt.format("%Y-%m-%d"))
                                } @else {
                                    "—"
                                }
                            }
                        }
                    }
                }
            }
            (htmx_pagination(&base_path, result.total, result.page, result.total_pages, "closest .detail-card", "innerHTML"))
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

