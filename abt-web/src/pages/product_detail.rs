use axum::response::{Html, IntoResponse};
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::{*, AcquireChannel};

use abt_macros::require_permission;

use crate::components::{detail::detail_row, icon};
use crate::layout::page::admin_page;
use crate::routes::product::{ProductDeletePath, ProductDetailPath, ProductEditPath, ProductListPath, ProductUpdatePath};
use crate::utils::RequestContext;

// ── Handlers ──

#[require_permission("PRODUCT", "read")]
pub async fn get_product_detail(
    path: ProductDetailPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
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
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("PRODUCT", "update")]
pub async fn get_product_edit(
    path: ProductEditPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
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
        content, &nav_filter,    );

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

    // 将中文获取途径映射为枚举值
    let acquire_channel = match form.acquire_channel.as_deref() {
        Some("自制") => Some(AcquireChannel::SelfProduced),
        Some("采购") => Some(AcquireChannel::Purchased),
        Some("委外") => Some(AcquireChannel::Outsourced),
        _ => None, // 不修改，保持原值
    };

    let req = UpdateProductReq {
        name: Some(form.name),
        unit: Some(form.unit),
        acquire_channel,
        external_code: form.external_code.filter(|s| !s.is_empty()),
        owner_department_id,
        meta: Some(ProductMeta {
            specification: form.specification,
            old_code: form.old_code.filter(|s| !s.is_empty()),
            remark: form.remark.filter(|s| !s.is_empty()),
            material_consumption_mode: Default::default(),
            over_completion_tolerance: None,
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
        div {
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
                        (match product.acquire_channel {
                            AcquireChannel::SelfProduced => "自制",
                            AcquireChannel::Purchased => "采购",
                            AcquireChannel::Outsourced => "委外",
                            AcquireChannel::NonInventory => "非库存",
                            AcquireChannel::Legacy => "历史遗留",
                        })
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

            // ── Delete button (hx-confirm) ──
            form id="delete-product-form" class="hidden"
                hx-post=(delete_path.to_string())
                hx-confirm=(format!("确定要删除产品「{}」吗？此操作不可撤销。", product.pdt_name))
                hx-target="closest div" {}
        }
    }
}

// ── Edit Page ──

fn product_edit_page(product: &Product) -> Markup {
    let update_path = ProductUpdatePath { id: product.product_id };
    let detail_path = ProductDetailPath { id: product.product_id };

    let acquire_val = match product.acquire_channel {
        AcquireChannel::SelfProduced => "自制",
        AcquireChannel::Purchased => "采购",
        AcquireChannel::Outsourced => "委外",
        AcquireChannel::NonInventory => "非库存",
        AcquireChannel::Legacy => "历史遗留",
    };
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
                            label { "规格型号" }
                            input type="text" name="specification" placeholder="请输入规格型号" value=(product.meta.specification) {}
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


fn status_display(status: ProductStatus) -> (&'static str, &'static str) {
    match status {
        ProductStatus::Active => ("在用", "status-accepted"),
        ProductStatus::Inactive => ("停用", "status-draft"),
        ProductStatus::Obsolete => ("作废", "status-rejected"),
    }
}

