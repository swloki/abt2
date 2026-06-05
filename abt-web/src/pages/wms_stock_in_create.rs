use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_stock_in::StockInCreatePath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Handlers ──

#[require_permission("WMS", "write")]
pub async fn get_stock_in_create(
    _path: StockInCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let warehouse_svc = state.warehouse_service();

    let warehouses = warehouse_svc
        .list(&service_ctx, &mut conn, abt_core::wms::warehouse::model::WarehouseFilter::default(), 1, 200)
        .await
        .map(|r| r.items)
        .unwrap_or_default();

    let content = stock_in_create_content(&warehouses, &claims.display_name);
    let page_html = admin_page(
        is_htmx, "新建入库单", &claims, "inventory", StockInCreatePath::PATH, "库存管理", None, content,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("WMS", "write")]
pub async fn create_stock_in(
    _path: StockInCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let claims = ctx.claims;
    let content = html! { "TODO: process stock-in form" };
    let page_html = admin_page(
        is_htmx, "新建入库单", &claims, "inventory", StockInCreatePath::PATH, "库存管理", None, content,
    );
    Ok(Html(page_html.into_string()))
}

// ── Components ──

fn stock_in_create_content(
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
    operator_name: &str,
) -> Markup {
    html! {
        div {
            // ── Back Link ──
            a href="/admin/wms/stock-in" class="back-link" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回入库列表"
            }

            // ── Page Header ──
            div class="page-header" style="margin-bottom:var(--space-6)" {
                h1 class="page-title" { "新建入库单" }
                div class="page-actions" {
                    button class="btn btn-default" type="button" { "保存草稿" }
                    button class="btn btn-primary" type="submit" form="stockInForm" {
                        (icon::check_circle_icon("w-4 h-4"))
                        "确认入库"
                    }
                }
            }

            // ── Type Switch ──
            div class="type-switch" {
                div class="type-btn active" {
                    (icon::download_icon("w-7 h-7"))
                    span class="type-label" { "采购入库" }
                    span class="type-desc" { "PURCHASE_RECEIPT" br; "关联来料通知 / 采购订单" }
                }
                div class="type-btn" {
                    (icon::box_icon("w-7 h-7"))
                    span class="type-label" { "生产入库" }
                    span class="type-desc" { "PRODUCTION_RECEIPT" br; "关联工单完工报工" }
                }
            }

            form id="stockInForm" hx-post=(StockInCreatePath::PATH) hx-swap="none" {
                // ── Source Section ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::link_icon("w-[18px] h-[18px]"))
                        "来源关联"
                    }
                    div class="wms-form-grid" {
                        div class="form-group" {
                            label class="form-label" { "来源类型" }
                            select class="form-select" name="source_type" {
                                option value="arrival" { "来料通知 (AN)" }
                                option value="purchase" { "采购订单 (PO)" }
                                option value="manual" { "手工录入" }
                            }
                        }
                        div class="form-group" {
                            label class="form-label" { "来源单号 " span class="required" { "*" } }
                            input class="form-input" type="text" name="source_ref" placeholder="选择来源单号" readonly;
                        }
                        div class="form-group" {
                            label class="form-label" { "送货单号" }
                            input class="form-input" type="text" name="delivery_no" placeholder="输入送货单号";
                        }
                        div class="form-group" {
                            label class="form-label" { "供应商" }
                            input class="form-input" type="text" placeholder="选择来源后自动填充" readonly style="background:var(--surface)";
                        }
                    }
                }

                // ── Warehouse Section ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::building_icon("w-[18px] h-[18px]"))
                        "入库信息"
                    }
                    div class="wms-form-grid" {
                        div class="form-group" {
                            label class="form-label" { "目标仓库 " span class="required" { "*" } }
                            select class="form-select" name="warehouse_id" {
                                option value="" { "请选择仓库" }
                                @for wh in warehouses {
                                    option value=(wh.id) { (wh.name) }
                                }
                            }
                        }
                        div class="form-group" {
                            label class="form-label" { "目标库区" }
                            select class="form-select" name="zone_id" {
                                option value="" { "请选择库区" }
                            }
                        }
                        div class="form-group" {
                            label class="form-label" { "目标储位" }
                            select class="form-select" name="bin_id" {
                                option value="" { "按上架策略分配" }
                            }
                        }
                        div class="form-group" {
                            label class="form-label" { "操作员" }
                            input class="form-input" type="text" value=(operator_name) readonly style="background:var(--surface)";
                        }
                    }
                }

                // ── Strategy Tip ──
                div style="padding:var(--space-3) var(--space-4);background:rgba(82,196,26,0.05);border:1px solid rgba(82,196,26,0.15);border-radius:var(--radius-md);margin-bottom:var(--space-6);display:flex;align-items:center;gap:var(--space-3)" {
                    (icon::check_circle_icon("w-4 h-4"))
                    span style="font-size:var(--text-sm);color:var(--fg-2)" {
                        "当前仓库上架策略："
                        strong { "同物料合并 (SAME_MERGE)" }
                        " — 系统将自动分配至同物料已有储位，储位满时按就近原则分配。"
                    }
                }

                // ── Line Items ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::box_icon("w-[18px] h-[18px]"))
                        "入库物料明细"
                        span style="margin-left:auto;font-size:var(--text-xs);font-weight:400;color:var(--muted)" { "共 0 项" }
                    }
                    table class="detail-table" {
                        thead {
                            tr {
                                th style="width:40px" { "序号" }
                                th { "产品编码" }
                                th { "产品名称" }
                                th { "规格型号" }
                                th { "批次号" }
                                th style="width:100px" { "入库数量 " span class="required" { "*" } }
                                th style="width:110px" { "单位成本" }
                                th style="width:110px" { "小计" }
                                th { "目标储位" }
                                th style="width:40px" { }
                            }
                        }
                        tbody {
                            // JS-managed dynamic rows
                        }
                    }
                    div style="margin-top:var(--space-4)" {
                        button type="button" class="add-row-btn" {
                            (icon::plus_icon("w-3.5 h-3.5"))
                            "添加物料"
                        }
                    }
                }

                // ── Summary ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::clipboard_list_icon("w-[18px] h-[18px]"))
                        "入库汇总"
                    }
                    div style="display:grid;grid-template-columns:repeat(4,1fr);gap:var(--space-6)" {
                        div style="text-align:center;padding:var(--space-4);background:var(--surface);border-radius:var(--radius-md)" {
                            div style="font-size:11px;color:var(--muted);margin-bottom:var(--space-1)" { "物料种类" }
                            div class="font-mono" style="font-size:var(--text-xl);font-weight:600" { "0" }
                        }
                        div style="text-align:center;padding:var(--space-4);background:var(--surface);border-radius:var(--radius-md)" {
                            div style="font-size:11px;color:var(--muted);margin-bottom:var(--space-1)" { "入库总量" }
                            div class="font-mono" style="font-size:var(--text-xl);font-weight:600" { "0" }
                        }
                        div style="text-align:center;padding:var(--space-4);background:var(--accent-bg);border-radius:var(--radius-md);border:1px solid rgba(22,119,255,0.15)" {
                            div style="font-size:11px;color:var(--accent);margin-bottom:var(--space-1)" { "入库总金额" }
                            div class="font-mono" style="font-size:var(--text-xl);font-weight:600;color:var(--accent)" { "¥0.00" }
                        }
                        div style="text-align:center;padding:var(--space-4);background:var(--surface);border-radius:var(--radius-md)" {
                            div style="font-size:11px;color:var(--muted);margin-bottom:var(--space-1)" { "上架策略" }
                            div style="font-size:var(--text-sm);font-weight:600" { "同物料合并" }
                        }
                    }
                }

                // ── Remark ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::edit_icon("w-[18px] h-[18px]"))
                        "备注"
                    }
                    textarea class="form-input" name="remark" placeholder="输入备注信息…" rows="3" style="width:100%;min-height:80px;padding:var(--space-2) var(--space-3);resize:vertical" { }
                }
            }
        }
    }
}
