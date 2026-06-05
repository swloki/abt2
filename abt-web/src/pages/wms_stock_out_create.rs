use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;

use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_stock_out::StockOutCreatePath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Handlers ──

#[require_permission("WMS", "write")]
pub async fn get_stock_out_create(
    _path: StockOutCreatePath,
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

    let content = stock_out_create_content(&warehouses, &claims.display_name);
    let page_html = admin_page(
        is_htmx, "新建出库单", &claims, "inventory", StockOutCreatePath::PATH, "库存管理", None, content,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("WMS", "write")]
pub async fn create_stock_out(
    _path: StockOutCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let claims = ctx.claims;
    let content = html! { "TODO: process stock-out form" };
    let page_html = admin_page(
        is_htmx, "新建出库单", &claims, "inventory", StockOutCreatePath::PATH, "库存管理", None, content,
    );
    Ok(Html(page_html.into_string()))
}

// ── Components ──

fn stock_out_create_content(
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
    operator_name: &str,
) -> Markup {
    html! {
        div {
            // ── Back Link ──
            a href="/admin/wms/stock-out" class="back-link" style="display:inline-flex;align-items:center;gap:var(--space-2);color:var(--fg-2);font-size:var(--text-sm);margin-bottom:var(--space-4);text-decoration:none" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回出库列表"
            }

            // ── Page Header ──
            div class="page-header" style="margin-bottom:var(--space-6)" {
                h1 class="page-title" { "新建出库单" }
                div class="page-actions" {
                    button class="btn btn-default" type="button" { "保存草稿" }
                    button class="btn btn-primary" type="submit" form="stockOutForm" style="background:var(--danger);border-color:var(--danger)" {
                        (icon::upload_icon("w-4 h-4"))
                        "确认出库"
                    }
                }
            }

            // ── Type Switch ──
            div style="display:flex;gap:var(--space-3);margin-bottom:var(--space-6)" {
                div style="flex:1;display:flex;flex-direction:column;align-items:center;gap:var(--space-2);padding:var(--space-5) var(--space-4);border:2px solid var(--danger);border-radius:var(--radius-lg);background:var(--danger-bg);cursor:pointer" {
                    (icon::upload_icon("w-7 h-7"))
                    span style="font-weight:600;font-size:var(--text-base);color:var(--fg)" { "销售出库" }
                    span style="font-size:var(--text-xs);color:var(--muted);text-align:center" { "SALES_SHIPMENT\n关联发货申请 / 销售订单\n消耗 SOFT 预留" }
                }
                div style="flex:1;display:flex;flex-direction:column;align-items:center;gap:var(--space-2);padding:var(--space-5) var(--space-4);border:2px solid var(--border);border-radius:var(--radius-lg);background:var(--bg);cursor:pointer" {
                    (icon::clipboard_document_icon("w-7 h-7"))
                    span style="font-weight:600;font-size:var(--text-base);color:var(--fg)" { "生产领料" }
                    span style="font-size:var(--text-xs);color:var(--muted);text-align:center" { "MATERIAL_ISSUE\n关联工单 / 领料单\n消耗 HARD 预留" }
                }
            }

            form id="stockOutForm" hx-post=(StockOutCreatePath::PATH) hx-swap="none" {
                // ── Source Section ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::link_icon("w-4 h-4"))
                        "来源关联"
                    }
                    div class="wms-form-grid" {
                        div class="form-group" {
                            label class="form-label" { "来源类型" }
                            select class="form-select" name="source_type" {
                                option value="shipping" { "发货申请 (SH)" }
                                option value="requisition" { "领料单 (MR)" }
                                option value="manual" { "手工录入" }
                            }
                        }
                        div class="form-group" {
                            label class="form-label" { "来源单号 " span style="color:var(--danger)" { "*" } }
                            input class="form-input" type="text" name="source_ref" placeholder="选择来源单号" readonly;
                        }
                        div class="form-group" {
                            label class="form-label" { "客户/工单" }
                            input class="form-input" type="text" placeholder="选择来源后自动填充" readonly style="background:var(--surface)";
                        }
                        div class="form-group" {
                            label class="form-label" { "预留类型" }
                            input class="form-input" type="text" value="SOFT 预留（发货消耗）" readonly style="background:var(--surface);color:var(--danger)";
                        }
                    }
                }

                // ── Warehouse Section ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::building_icon("w-4 h-4"))
                        "出库信息"
                    }
                    div class="wms-form-grid" {
                        div class="form-group" {
                            label class="form-label" { "来源仓库 " span style="color:var(--danger)" { "*" } }
                            select class="form-select" name="warehouse_id" {
                                option value="" { "请选择仓库" }
                                @for wh in warehouses {
                                    option value=(wh.id) { (wh.name) }
                                }
                            }
                        }
                        div class="form-group" {
                            label class="form-label" { "来源库区" }
                            select class="form-select" name="zone_id" {
                                option value="" { "按拣货策略分配" }
                            }
                        }
                        div class="form-group" {
                            label class="form-label" { "拣货策略" }
                            select class="form-select" name="pick_strategy" {
                                option value="fifo" selected { "FIFO 先进先出" }
                                option value="fefo" { "FEFO 先到期先出" }
                                option value="shortest" { "最短路径" }
                                option value="full_pallet" { "整托优先" }
                            }
                        }
                        div class="form-group" {
                            label class="form-label" { "操作员" }
                            input class="form-input" type="text" value=(operator_name) readonly style="background:var(--surface)";
                        }
                    }
                }

                // ── Pick Strategy Tip ──
                div style="padding:var(--space-3) var(--space-4);background:rgba(250,173,20,0.05);border:1px solid rgba(250,173,20,0.15);border-radius:var(--radius-md);margin-bottom:var(--space-6);display:flex;align-items:center;gap:var(--space-3)" {
                    (icon::circle_alert_icon("w-4 h-4"))
                    span style="font-size:var(--text-sm);color:var(--fg-2)" {
                        "拣货策略："
                        strong { "FIFO 先进先出" }
                        " — 系统优先拣选最早入库批次的物料，确保库存周转。对于有效期管理物料建议使用 FEFO。"
                    }
                }

                // ── Line Items ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::box_icon("w-4 h-4"))
                        "出库物料明细"
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
                                th style="width:100px" { "出库数量 " span style="color:var(--danger)" { "*" } }
                                th style="width:90px" { "可用库存" }
                                th style="width:110px" { "单位成本" }
                                th style="width:110px" { "小计" }
                                th { "来源储位" }
                                th style="width:40px" { }
                            }
                        }
                        tbody {
                            // JS-managed dynamic rows
                        }
                    }
                    div style="margin-top:var(--space-4)" {
                        button type="button" class="add-row-btn" style="display:inline-flex;align-items:center;gap:var(--space-2);padding:var(--space-2) var(--space-4);border:1px dashed var(--border);border-radius:var(--radius-sm);background:var(--bg);color:var(--accent);font-size:var(--text-sm);cursor:pointer" {
                            (icon::plus_icon("w-3.5 h-3.5"))
                            "添加物料"
                        }
                    }
                }

                // ── Reservation Info ──
                div style="margin-top:var(--space-4);padding:var(--space-4);background:linear-gradient(135deg,rgba(250,173,20,0.04),rgba(255,77,79,0.04));border:1px solid var(--border-soft);border-radius:var(--radius-md)" {
                    h4 style="font-size:var(--text-sm);font-weight:600;color:var(--fg-2);margin-bottom:var(--space-3);display:flex;align-items:center;gap:var(--space-2)" {
                        (icon::lock_icon("w-4 h-4"))
                        "库存预留 & 可用性检查"
                    }
                    div style="display:grid;grid-template-columns:repeat(3,1fr);gap:var(--space-4)" {
                        div style="text-align:center;padding:var(--space-3);background:var(--bg);border-radius:var(--radius-md)" {
                            div style="font-size:11px;color:var(--muted);margin-bottom:2px" { "预留类型" }
                            div style="font-size:var(--text-base);font-weight:600;font-family:var(--font-mono);color:var(--danger)" { "SOFT" }
                        }
                        div style="text-align:center;padding:var(--space-3);background:var(--bg);border-radius:var(--radius-md)" {
                            div style="font-size:11px;color:var(--muted);margin-bottom:2px" { "已预留量" }
                            div style="font-size:var(--text-lg);font-weight:600;font-family:var(--font-mono);color:var(--warn)" { "—" }
                        }
                        div style="text-align:center;padding:var(--space-3);background:var(--bg);border-radius:var(--radius-md)" {
                            div style="font-size:11px;color:var(--muted);margin-bottom:2px" { "出库后释放" }
                            div style="font-size:var(--text-base);font-weight:600;font-family:var(--font-mono);color:var(--success)" { "→ available_qty" }
                        }
                    }
                }

                // ── Summary ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::clipboard_list_icon("w-4 h-4"))
                        "出库汇总"
                    }
                    div style="display:grid;grid-template-columns:repeat(4,1fr);gap:var(--space-6)" {
                        div style="text-align:center;padding:var(--space-4);background:var(--surface);border-radius:var(--radius-md)" {
                            div style="font-size:11px;color:var(--muted);margin-bottom:var(--space-1)" { "物料种类" }
                            div style="font-size:var(--text-xl);font-weight:600;font-family:var(--font-mono)" { "0" }
                        }
                        div style="text-align:center;padding:var(--space-4);background:var(--surface);border-radius:var(--radius-md)" {
                            div style="font-size:11px;color:var(--muted);margin-bottom:var(--space-1)" { "出库总量" }
                            div style="font-size:var(--text-xl);font-weight:600;font-family:var(--font-mono)" { "0" }
                        }
                        div style="text-align:center;padding:var(--space-4);background:var(--danger-bg);border-radius:var(--radius-md);border:1px solid rgba(255,77,79,0.15)" {
                            div style="font-size:11px;color:var(--danger);margin-bottom:var(--space-1)" { "出库总金额" }
                            div style="font-size:var(--text-xl);font-weight:600;font-family:var(--font-mono);color:var(--danger)" { "¥0.00" }
                        }
                        div style="text-align:center;padding:var(--space-4);background:var(--surface);border-radius:var(--radius-md)" {
                            div style="font-size:11px;color:var(--muted);margin-bottom:var(--space-1)" { "拣货策略" }
                            div style="font-size:var(--text-sm);font-weight:600;color:var(--fg)" { "FIFO" }
                        }
                    }
                }

                // ── Remark ──
                div class="wms-form-section" {
                    div class="form-section-title" {
                        (icon::edit_icon("w-4 h-4"))
                        "备注"
                    }
                    textarea class="form-input" name="remark" placeholder="输入备注信息…" rows="3" style="width:100%;min-height:80px;padding:var(--space-2) var(--space-3);resize:vertical" { }
                }
            }
        }
    }
}
