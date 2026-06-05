use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::wms::warehouse::model::WarehouseFilter;
use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_requisition::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Handlers ──

#[require_permission("WMS", "write")]
pub async fn get_requisition_create(
    _path: RequisitionCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let warehouse_svc = state.warehouse_service();

    let warehouses = warehouse_svc
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
        .await?;

    let content = requisition_create_page(&warehouses.items);
    let page_html = admin_page(
        is_htmx,
        "新建领料单",
        &claims,
        "inventory",
        RequisitionCreatePath::PATH,
        "库存管理",
        Some("新建领料单"),
        content,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("WMS", "write")]
pub async fn create_requisition(
    _path: RequisitionCreatePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    // Form processing handled client-side via JS
    // The actual creation is done via the service's create_for_work_order method
    let redirect = RequisitionListPath::PATH.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn requisition_create_page(
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
    html! {
        div {
            a href=(RequisitionListPath::PATH) class="back-link" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回领料单列表"
            }

            div class="page-header" style="margin-bottom:var(--space-5)" {
                h1 class="page-title" { "新建领料单" }
                div class="page-actions" {
                    span style="font-size:var(--text-xs);color:var(--muted);display:flex;align-items:center;gap:var(--space-2)" {
                        (icon::clock_icon("w-3.5 h-3.5"))
                        "自动保存草稿"
                    }
                }
            }

            // ── 工单信息 ──
            div class="wms-form-section" {
                div class="form-section-title" {
                    (icon::clipboard_document_icon("w-4 h-4"))
                    "工单信息"
                }
                div class="wms-form-grid" {
                    div class="form-group" {
                        label class="form-label" { "关联工单 " span class="required" { "*" } }
                        input class="form-input" type="text" name="work_order_id" placeholder="输入或选择工单号";
                    }
                    div class="form-group" {
                        label class="form-label" { "领料仓库 " span class="required" { "*" } }
                        select class="form-select" name="warehouse_id" required {
                            option value="" { "请选择仓库" }
                            @for w in warehouses {
                                option value=(w.id) { (w.name) }
                            }
                        }
                    }
                    div class="form-group" {
                        label class="form-label" { "领料日期 " span class="required" { "*" } }
                        input class="form-input" type="date" name="requisition_date" required;
                    }
                    div class="form-group" {
                        label class="form-label" { "操作员" }
                        select class="form-select" name="operator_id" {
                            option value="" { "请选择操作员" }
                        }
                    }
                }
            }

            // ── 领料明细 ──
            div class="wms-form-section" {
                div style="padding:var(--space-6) var(--space-6) var(--space-4)" {
                    div class="form-section-title" {
                        (icon::box_icon("w-4 h-4"))
                        "领料明细"
                    }
                }
                div style="overflow-x:auto" {
                    table class="line-items-table" {
                        thead {
                            tr {
                                th style="width:40px;text-align:center" { "行号" }
                                th style="min-width:130px" { "产品编码" }
                                th style="min-width:180px" { "产品名称" }
                                th style="min-width:160px" { "规格" }
                                th style="width:64px" { "单位" }
                                th style="width:100px;text-align:right" { "BOM定额" }
                                th style="width:110px;text-align:right" { "实领数量 " span class="required" { "*" } }
                                th style="width:90px;text-align:right" { "差异量" }
                                th style="min-width:120px" { "储位" }
                                th style="width:40px" { "操作" }
                            }
                        }
                        tbody {
                            tr {
                                td class="line-num" { "1" }
                                td {
                                    select class="form-select" style="padding:5px 24px 5px 8px" {
                                        option value="" { "选择产品" }
                                    }
                                }
                                td { input class="form-input" type="text" readonly tabindex="-1" style="background:var(--surface)"; }
                                td { input class="form-input" type="text" readonly tabindex="-1" style="background:var(--surface)"; }
                                td { input class="form-input" type="text" readonly tabindex="-1" style="background:var(--surface);text-align:center"; }
                                td { input class="form-input" type="text" readonly tabindex="-1" style="background:var(--surface);text-align:right"; }
                                td { input class="form-input" type="number" min="0" style="text-align:right"; }
                                td style="font-family:var(--font-mono);font-variant-numeric:tabular-nums;color:var(--success);font-weight:600;text-align:right" { "0" }
                                td { input class="form-input" type="text" placeholder="储位编码"; }
                                td {
                                    button type="button" class="btn-remove-row" title="删除行" {
                                        (icon::x_icon("w-3.5 h-3.5"))
                                    }
                                }
                            }
                        }
                    }
                }
                div class="add-row-bar" {
                    button type="button" class="btn-add-row" {
                        (icon::plus_icon("w-4 h-4"))
                        "添加物料"
                    }
                }
                div class="action-bar" {
                    div {}
                    div style="display:flex;gap:var(--space-3)" {
                        button type="button" class="btn btn-default" {
                            (icon::clipboard_document_icon("w-4 h-4"))
                            "保存草稿"
                        }
                        button type="button" class="btn btn-primary" {
                            (icon::bolt_icon("w-4 h-4"))
                            "确认领料"
                        }
                    }
                }
            }
        }
    }
}
