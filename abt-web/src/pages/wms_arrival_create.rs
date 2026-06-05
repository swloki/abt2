use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::master_data::supplier::SupplierService;
use abt_core::master_data::supplier::SupplierStatus;
use abt_core::shared::types::PageParams;
use abt_core::wms::warehouse::model::WarehouseFilter;
use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_arrival::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;


// ── Handlers ──

#[require_permission("WMS", "write")]
pub async fn get_arrival_create(
    _path: ArrivalCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let supplier_svc = state.supplier_service();
    let warehouse_svc = state.warehouse_service();

    let suppliers = supplier_svc
        .list(&service_ctx, &mut conn, SupplierQuery { name: None, status: Some(SupplierStatus::Qualified), category: None }, PageParams::new(1, 200))
        .await?;

    let warehouses = warehouse_svc
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
        .await?;

    let content = arrival_create_page(&suppliers.items, &warehouses.items);
    let page_html = admin_page(
        is_htmx,
        "新建来料通知",
        &claims,
        "inventory",
        ArrivalCreatePath::PATH,
        "库存管理",
        Some("新建来料通知"),
        content,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("WMS", "write")]
pub async fn create_arrival(
    _path: ArrivalCreatePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    // Form processing handled client-side via JS that builds items_json
    // For now, redirect to the list page as a placeholder
    let redirect = ArrivalListPath::PATH.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn arrival_create_page(
    suppliers: &[abt_core::master_data::supplier::model::Supplier],
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
) -> Markup {
    html! {
        div {
            a href=(ArrivalListPath::PATH) class="back-link" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回来料通知列表"
            }

            div class="page-header" style="margin-bottom:var(--space-5)" {
                h1 class="page-title" { "新建来料通知" }
                div class="page-actions" {
                    span style="font-size:var(--text-xs);color:var(--muted);display:flex;align-items:center;gap:var(--space-2)" {
                        (icon::clock_icon("w-3.5 h-3.5"))
                        "自动保存草稿"
                    }
                }
            }

            // ── 供应商信息 ──
            div class="wms-form-section" {
                div class="form-section-title" {
                    (icon::building_icon("w-4 h-4"))
                    "供应商信息"
                }
                div class="wms-form-grid" {
                    div class="form-group" {
                        label class="form-label" { "供应商 " span class="required" { "*" } }
                        select class="form-select" name="supplier_id" required {
                            option value="" { "请选择供应商" }
                            @for s in suppliers {
                                option value=(s.id) { (s.name) }
                            }
                        }
                    }
                    div class="form-group" {
                        label class="form-label" { "联系人" }
                        input class="form-input" type="text" readonly tabindex="-1" placeholder="自动填充";
                    }
                    div class="form-group" {
                        label class="form-label" { "联系电话" }
                        input class="form-input" type="text" readonly tabindex="-1" placeholder="自动填充";
                    }
                    div class="form-group" {
                        label class="form-label" { "来源采购单" }
                        select class="form-select" name="purchase_order_id" {
                            option value="" { "请选择采购单（可选）" }
                        }
                    }
                }
            }

            // ── 到货信息 ──
            div class="wms-form-section" {
                div class="form-section-title" {
                    (icon::truck_icon("w-4 h-4"))
                    "到货信息"
                }
                div class="wms-form-grid" {
                    div class="form-group" {
                        label class="form-label" { "到货仓库 " span class="required" { "*" } }
                        select class="form-select" name="warehouse_id" required {
                            option value="" { "请选择仓库" }
                            @for w in warehouses {
                                option value=(w.id) { (w.name) }
                            }
                        }
                    }
                    div class="form-group" {
                        label class="form-label" { "到货库区" }
                        select class="form-select" name="zone_id" {
                            option value="" { "请选择库区" }
                        }
                    }
                    div class="form-group" {
                        label class="form-label" { "到货日期 " span class="required" { "*" } }
                        input class="form-input" type="date" name="arrival_date" required;
                    }
                    div class="form-group" {
                        label class="form-label" { "送货单号" }
                        input class="form-input" type="text" name="delivery_note" placeholder="请输入送货单号";
                    }
                    div class="form-group" {
                        label class="form-label" { "操作员" }
                        select class="form-select" name="operator_id" {
                            option value="" { "请选择操作员" }
                        }
                    }
                }
            }

            // ── 物料明细 ──
            div class="wms-form-section" style="padding:0;overflow:hidden" {
                div style="padding:var(--space-6) var(--space-6) var(--space-4)" {
                    div class="form-section-title" {
                        (icon::box_icon("w-4 h-4"))
                        "物料明细"
                    }
                    div style="background:rgba(250,173,20,0.06);border:1px solid rgba(250,173,20,0.2);border-radius:var(--radius-md);padding:var(--space-3) var(--space-4);font-size:var(--text-xs);color:#8c6d1f;line-height:1.6;display:flex;align-items:flex-start;gap:var(--space-2);margin-top:var(--space-3)" {
                        (icon::circle_alert_icon("w-4 h-4"))
                        span { "来料确认后将触发 " strong { "IQC 进料检验" } "。检验合格后方计入可用库存。不合格物料将进入 MRB 流程处理，无法上架。" }
                    }
                }
                div style="overflow-x:auto" {
                    table class="line-items-table" {
                        thead {
                            tr {
                                th style="width:40px;text-align:center" { "行号" }
                                th style="min-width:140px" { "产品编码" }
                                th style="min-width:200px" { "产品名称" }
                                th style="min-width:160px" { "规格" }
                                th style="width:64px" { "单位" }
                                th style="width:100px;text-align:right" { "申报数量 " span class="required" { "*" } }
                                th style="width:100px;text-align:right" { "实收数量" }
                                th style="width:100px;text-align:right" { "合格数量" }
                                th style="width:140px" { "批次号" }
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
                                td { input class="form-input" type="number" min="1" style="text-align:right"; }
                                td { input class="form-input" type="number" min="0" style="text-align:right" placeholder="0"; }
                                td { input class="form-input" type="number" min="0" style="text-align:right;color:var(--warn)" placeholder="待检验"; }
                                td { input class="form-input" type="text" placeholder="批次号"; }
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
            }

            // ── Action Bar ──
            div class="action-bar" {
                button type="button" class="btn btn-default" {
                    (icon::clipboard_document_icon("w-4 h-4"))
                    "保存草稿"
                }
                button type="button" class="btn btn-primary" {
                    (icon::check_circle_icon("w-4 h-4"))
                    "确认收货"
                }
            }
        }
    }
}
