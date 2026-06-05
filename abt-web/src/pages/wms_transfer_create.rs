use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use crate::errors::Result;
use crate::routes::wms_transfer::TransferCreatePath;
use crate::utils::RequestContext;
use abt_macros::require_permission;
use crate::layout::page::admin_page;

use crate::components::icon;

#[require_permission("WMS", "write")]
pub async fn get_transfer_create(
    _path: TransferCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let claims = ctx.claims;
    let content = transfer_create_page();
    let page_html = admin_page(
        is_htmx,
        "新建调拨单",
        &claims,
        "inventory",
        "/admin/wms/transfers/create",
        "库存管理",
        None,
        content,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("WMS", "write")]
pub async fn create_transfer(
    _path: TransferCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let claims = ctx.claims;
    let content = transfer_create_page();
    let page_html = admin_page(
        is_htmx,
        "新建调拨单",
        &claims,
        "inventory",
        "/admin/wms/transfers/create",
        "库存管理",
        None,
        content,
    );
    Ok(Html(page_html.into_string()))
}

fn transfer_create_page() -> Markup {
    html! {
        div {
            a href="/admin/wms/transfers" class="back-link" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回库存调拨列表"
            }

            div class="page-header" {
                h1 class="page-title" { "新建调拨单" }
            }

            // ── Workflow Preview ──
            div class="workflow-steps" {
                div class="wf-step current" { span class="wf-dot" {} "草稿" }
                div class="wf-line" {}
                div class="wf-step" { span class="wf-dot" {} "在途" }
                div class="wf-line" {}
                div class="wf-step" { span class="wf-dot" {} "已完成" }
            }

            form hx-post=(TransferCreatePath::PATH) hx-swap="none" {
                // ── From / To Warehouse ──
                div class="wms-form-section" {
                    h3 class="form-section-title" { "调拨信息" }
                    div class="wms-form-grid" {
                        div class="form-field" {
                            label class="form-label" { "调出仓库" }
                            select class="form-select" name="from_warehouse_id" required {}
                        }
                        div class="form-field" {
                            label class="form-label" { "调出库区" }
                            select class="form-select" name="from_zone_id" {}
                        }
                        div class="form-field" {
                            label class="form-label" { "调出储位" }
                            select class="form-select" name="from_bin_id" {}
                        }
                        div class="form-field" {
                            label class="form-label" { "调入仓库" }
                            select class="form-select" name="to_warehouse_id" required {}
                        }
                        div class="form-field" {
                            label class="form-label" { "调入库区" }
                            select class="form-select" name="to_zone_id" {}
                        }
                        div class="form-field" {
                            label class="form-label" { "调入储位" }
                            select class="form-select" name="to_bin_id" {}
                        }
                        div class="form-field" {
                            label class="form-label" { "调拨日期" }
                            input class="form-input" type="date" name="transfer_date" required {}
                        }
                    }
                }

                // ── Line Items ──
                div class="wms-form-section" {
                    h3 class="form-section-title" { "调拨明细" }
                    div class="data-card" {
                        table class="data-table" {
                            thead {
                                tr {
                                    th { "行号" }
                                    th { "产品" }
                                    th class="num-right" { "数量" }
                                    th { "批次号" }
                                    th { "操作" }
                                }
                            }
                            tbody id="line-items" {
                                // Dynamic rows added via JS
                            }
                        }
                    }
                    button type="button" class="btn btn-default" style="margin-top:var(--space-3)"
                        onclick="addTransferLine()" {
                        (icon::plus_icon("w-4 h-4"))
                        "添加行"
                    }
                }

                // ── Actions ──
                div class="create-action-bar" {
                    a href="/admin/wms/transfers" class="btn btn-default" { "取消" }
                    button type="submit" class="btn btn-default" name="action" value="draft" { "保存草稿" }
                    button type="submit" class="btn btn-primary" name="action" value="submit" { "提交" }
                }
            }
        }
    }
}
