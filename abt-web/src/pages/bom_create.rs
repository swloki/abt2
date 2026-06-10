use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::master_data::bom::model::*;
use abt_core::master_data::bom::{BomCategoryService, BomCommandService};
use abt_core::shared::types::PageParams;

use abt_macros::require_permission;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::bom::{BomCreatePath, BomEditPath, BomListPath};
use crate::utils::RequestContext;

// ── Form request ──

#[derive(Debug, Deserialize)]
pub struct BomCreateForm {
    pub bom_name: String,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub bom_category_id: Option<i64>,
}

// ── Handlers ──

#[require_permission("BOM", "create")]
pub async fn get_bom_create(
    _path: BomCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;

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
        is_htmx,
        "新建物料清单",
        &claims,
        "md",
        BomCreatePath::PATH,
        "主数据管理",
        Some("新建物料清单"),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

/// POST: create BOM header only, then redirect to edit page to add nodes
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

    // Redirect to edit page (step 2) to add nodes
    let redirect = BomEditPath { id: bom_id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn bom_create_page(categories: &[BomCategory]) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="page-header" {
                a class="back-link" href=(BomListPath::PATH) {
                    (icon::arrow_left_icon("w-4 h-4"))
                    "返回物料清单列表"
                }
                h1 class="page-title" { "新建物料清单" }
                p style="color:var(--muted);font-size:var(--text-sm);margin:var(--space-1) 0 0" { "第一步：基本信息" }
            }

            // ── Form Card ──
            div class="data-card" style="margin-bottom:var(--space-4)" {
                div class="form-section-title" { "基本信息" }
                form hx-post=(BomCreatePath::PATH)
                      hx-swap="none" {
                    div class="form-grid" {
                        div class="form-field" {
                            label { "BOM名称 " span style="color:var(--danger)" { "*" } }
                            input type="text" name="bom_name" required placeholder="请输入BOM名称" {}
                        }
                        div class="form-field" {
                            label { "BOM分类 " span style="color:var(--danger)" { "*" } }
                            select name="bom_category_id" required {
                                option value="" disabled selected { "-- 请选择 --" }
                                @for cat in categories {
                                    option value=(cat.bom_category_id) { (cat.bom_category_name) }
                                }
                            }
                        }
                    }

                    // ── Action Bar ──
                    div class="create-action-bar" {
                        a class="btn btn-default" href=(BomListPath::PATH) { "取消" }
                        div style="display:flex;gap:var(--space-3)" {
                            button type="submit" class="btn btn-primary" {
                                "下一步"
                            }
                        }
                    }
                }
            }
        }
    }
}
