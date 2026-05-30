use axum_extra::routing::TypedPath;
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse};
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::master_data::product::model::{CreateProductReq, Product, ProductMeta, ProductStatus};
use abt_core::master_data::product::ProductService;
use abt_macros::require_permission;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::product::{ProductCopyPath, ProductCreatePath, ProductDetailPath, ProductListPath};
use crate::utils::RequestContext;

// ── Query Params ──

#[derive(Debug, Deserialize)]
pub struct CreateQueryParams {
    #[serde(default)]
    pub copy_from: Option<i64>,
}

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub struct ProductCreateForm {
    pub name: String,
    pub unit: String,
    pub specification: String,
    pub acquire_channel: Option<String>,
    pub external_code: Option<String>,
    pub owner_department_id: Option<String>,
    pub old_code: Option<String>,
    pub remark: Option<String>,
}

// ── Handlers ──

#[require_permission("PRODUCT", "create")]
pub async fn get_product_create(
    _path: ProductCreatePath,
    axum::extract::Query(params): axum::extract::Query<CreateQueryParams>,
    ctx: RequestContext,
    headers: HeaderMap,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, claims } = ctx;

    let copy_source = if let Some(id) = params.copy_from {
        let svc = state.product_service();
        Some(svc.get(&service_ctx, &mut *conn, id).await?)
    } else {
        None
    };

    let title = if copy_source.is_some() { "复制产品" } else { "新建产品" };
    let content = product_create_page(copy_source.as_ref());
    let page_html = admin_page(
        &headers,
        title,
        &claims,
        "md",
        ProductCreatePath::PATH,
        "主数据管理",
        Some(title),
        content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("PRODUCT", "create")]
pub async fn post_product_create(
    _path: ProductCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ProductCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.product_service();

    let owner_department_id = form
        .owner_department_id
        .as_deref()
        .and_then(|s| if s.is_empty() { None } else { s.parse::<i64>().ok() });

    let create_req = CreateProductReq {
        name: form.name,
        unit: form.unit,
        status: ProductStatus::Active,
        external_code: form.external_code.filter(|s| !s.is_empty()),
        owner_department_id,
        meta: ProductMeta {
            specification: form.specification,
            acquire_channel: form
                .acquire_channel
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "采购".to_string()),
            old_code: form.old_code.filter(|s| !s.is_empty()),
            remark: form.remark.filter(|s| !s.is_empty()),
        },
    };

    let id = svc.create(&service_ctx, &mut conn, create_req).await?;

    let redirect = ProductDetailPath { id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn product_create_page(source: Option<&Product>) -> Markup {
    let title = if source.is_some() { "复制产品" } else { "新建产品" };
    let btn_label = if source.is_some() { "保存副本" } else { "保存产品" };

    let name_val = source.map(|p| format!("{}-1", p.pdt_name)).unwrap_or_default();
    let spec_val = source.map(|p| p.meta.specification.as_str()).unwrap_or("");
    let unit_val = source.map(|p| p.unit.as_str()).unwrap_or("");
    let acquire_val = source.map(|p| p.meta.acquire_channel.as_str()).unwrap_or("采购");
    let external_code_val = source.as_ref().and_then(|p| p.external_code.as_deref()).unwrap_or("");
    let old_code_val = source.as_ref().and_then(|p| p.meta.old_code.as_deref()).unwrap_or("");

    html! {
        div {
            // ── Page Header ──
            div class="page-header" {
                a class="back-link" href=(ProductListPath::PATH) {
                    (icon::arrow_left_icon("w-4 h-4"))
                    "返回产品列表"
                }
                h1 class="page-title" { (title) }
            }

            form id="product-form"
                  hx-post=(ProductCreatePath::PATH)
                  hx-swap="none" {

                // ── Section: 基本信息 ──
                div class="data-card" style="margin-bottom:var(--space-4)" {
                    div class="form-section-title" { "基本信息" }
                    div class="form-grid" {
                        div class="form-field" {
                            label { "产品名称 " span style="color:var(--danger)" { "*" } }
                            input type="text" name="name" required placeholder="请输入产品名称" value=(name_val) {}
                        }
                        div class="form-field" {
                            label { "产品编码" }
                            input type="text" value="自动生成" readonly
                                style="background:var(--surface);color:var(--muted)" {}
                        }
                        div class="form-field" {
                            label { "规格型号 " span style="color:var(--danger)" { "*" } }
                            input type="text" name="specification" required placeholder="请输入规格型号" value=(spec_val) {}
                        }
                        div class="form-field" {
                            label { "计量单位 " span style="color:var(--danger)" { "*" } }
                            input type="text" name="unit" required placeholder="请输入计量单位" value=(unit_val) {}
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
                                style="width:100%;min-height:80px;resize:vertical" {}
                        }
                    }
                }

                // ── Action Bar ──
                div class="create-action-bar" {
                    a class="btn btn-default" href=(ProductListPath::PATH) { "取消" }
                    button type="submit" class="btn btn-primary" {
                        (btn_label)
                    }
                }
            }
        }
    }
}

// ── Copy Handler ──

#[require_permission("PRODUCT", "create")]
pub async fn copy_product(path: ProductCopyPath, _ctx: RequestContext) -> crate::errors::Result<impl IntoResponse> {
    Ok(axum::response::Redirect::to(&format!("/admin/md/products/new?copy_from={}", path.id)))
}
