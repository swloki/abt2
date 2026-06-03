use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::master_data::supplier::SupplierService;
use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::purchase::reconciliation::PurchaseReconciliationService;

use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_reconciliation::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form request ──

#[derive(Debug, Deserialize)]
pub struct PreconCreateForm {
    pub supplier_id: i64,
    pub period: String,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub remark: Option<String>,
}

// ── Handlers ──

#[require_permission("PURCHASE_RECON", "create")]
pub async fn get_precon_create(
    _path: PreconCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;
    let supplier_svc = state.supplier_service();

    let suppliers = supplier_svc
        .list(
            &service_ctx,
            &mut conn,
            SupplierQuery {
                name: None,
                status: None,
                category: None,
            },
            PageParams::new(1, 200),
        )
        .await?;

    let content = precon_create_page(&suppliers.items);
    let page_html = admin_page(
        is_htmx,
        "新建采购对账单",
        &claims,
        "purchase",
        PreconCreatePath::PATH,
        "采购管理",
        Some("新建采购对账单"),
        content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("PURCHASE_RECON", "create")]
pub async fn create_precon(
    _path: PreconCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<PreconCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.purchase_reconciliation_service();

    let id = svc
        .create(&service_ctx, &mut conn, form.supplier_id, form.period, None)
        .await?;

    let redirect = PreconDetailPath { id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn precon_create_page(suppliers: &[abt_core::master_data::supplier::model::Supplier]) -> Markup {
    let current_month = chrono::Local::now().format("%Y-%m").to_string();

    html! {
        div {
            // ── Page Header ──
            div class="page-header" {
                a class="back-link" href=(PreconListPath::PATH) {
                    (icon::arrow_left_icon("w-4 h-4"))
                    "返回对账单列表"
                }
                h1 class="page-title" { "新建采购对账单" }
            }

            form hx-post=(PreconCreatePath::PATH) hx-swap="none" {

                // ── Supplier Selection ──
                div class="data-card" style="margin-bottom:var(--space-4)" {
                    div class="form-section-title" { "供应商信息" }
                    div class="form-grid" {
                        div class="form-field" {
                            label { "供应商" span style="color:var(--danger)" { "*" } }
                            select name="supplier_id" required {
                                option value="" disabled selected { "请选择供应商" }
                                @for s in suppliers {
                                    option value=(s.id) { (s.name) }
                                }
                            }
                        }
                    }
                }

                // ── Period ──
                div class="data-card" style="margin-bottom:var(--space-4)" {
                    div class="form-section-title" { "对账信息" }
                    div class="form-grid" {
                        div class="form-field" {
                            label { "对账期间" span style="color:var(--danger)" { "*" } }
                            input type="month" name="period" value=(current_month) required {}
                        }
                    }
                }

                // ── Remark ──
                div class="data-card" style="margin-bottom:var(--space-4)" {
                    div class="form-section-title" { "备注" }
                    textarea name="remark" placeholder="输入对账单相关备注信息…" style="width:100%;min-height:80px;padding:8px 12px;border:1px solid var(--border);border-radius:var(--radius-sm);font-size:var(--text-sm);resize:vertical;font-family:inherit" {}
                }

                // ── Action Bar ──
                div class="create-action-bar" {
                    a class="btn btn-default" href=(PreconListPath::PATH) { "取消" }
                    div style="display:flex;gap:var(--space-3)" {
                        button type="submit" class="btn btn-primary" {
                            "提交对账单"
                        }
                    }
                }
            }
        }
    }
}
