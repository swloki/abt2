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

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct PreconCreateForm {
    pub supplier_id: i64,
    pub period: String,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub remark: Option<String>,
    pub action: Option<String>,
    #[serde(default)]
    pub items_json: Option<String>,
}

// ── Handlers ──

#[require_permission("PURCHASE_RECON", "create")]
pub async fn get_precon_create(
    _path: PreconCreatePath,
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

    let buyer_name = &claims.display_name;
    let content = precon_create_page(&suppliers.items, buyer_name);
    let page_html = admin_page(
        is_htmx,
        "新建采购对账单",
        &claims,
        "purchase",
        PreconCreatePath::PATH,
        "采购管理",
        Some("新建采购对账单"),
        content, &nav_filter,    );

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

fn precon_create_page(
    suppliers: &[abt_core::master_data::supplier::model::Supplier],
    buyer_name: &str,
) -> Markup {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let current_month = chrono::Local::now().format("%Y-%m").to_string();

    html! {
        div id="precon-app" {
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                a class="inline-flex items-center gap-2 text-sm text-text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", PreconListPath::PATH)) {
                    (icon::arrow_left_icon("w-4 h-4"))
                    "返回对账单列表"
                }
                h1 class="text-xl font-bold text-fg tracking-tight" { "新建采购对账单" }
            }

            form id="precon-form"
                  hx-post=(PreconCreatePath::PATH)
                  hx-swap="none" {
                input type="hidden" id="items-json" name="items_json" value="[]";

            // ── 对账基本信息 ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" style="margin-bottom:var(--space-4)" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "对账基本信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label { "供应商" span style="color:var(--danger)" { "*" } }
                        select name="supplier_id" id="precon-supplier" required {
                            option value="" disabled selected { "请选择供应商" }
                            @for s in suppliers {
                                option value=(s.id) { (s.name) }
                            }
                        }
                    }
                    div class="form-field" {
                        label { "对账期间" span style="color:var(--danger)" { "*" } }
                        input type="month" name="period" value=(current_month) required {}
                    }
                    div class="form-field" {
                        label { "对账日期" }
                        input type="date" name="recon_date" value=(today) {}
                    }
                    div class="form-field" {
                        label { "采购员" }
                        input type="text" value=(buyer_name) readonly {}
                    }
                    div class="form-field field-full" {
                        label { "联系人 / 电话" }
                        div style="display:flex;gap:var(--space-2)" {
                            input type="text" id="precon-contact" placeholder="联系人" readonly style="flex:1" {}
                            input type="text" id="precon-phone" placeholder="电话" readonly style="flex:1" {}
                        }
                    }
                    div class="form-field field-full" {
                        label { "备注" }
                        textarea name="remark" placeholder="输入对账单相关备注信息…" style="width:100%;min-height:60px;padding:8px 12px;border:1px solid var(--border);border-radius:var(--radius-sm);font-size:var(--text-sm);resize:vertical;font-family:inherit" {}
                    }
                }
            }

            // ── 对账明细 ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" style="padding:0;overflow:hidden;margin-bottom:var(--space-4)" {
                div style="padding:var(--space-5) var(--space-5) var(--space-3);display:flex;justify-content:space-between;align-items:center" {
                    span class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" style="margin:0;padding:0;border:none" { "对账明细" }
                    button type="button" class="btn inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative-sm inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)] [&_svg]:w-4 [&_svg]:h-4"
                        id="btn-add-orders"
                        _="on click add .is-open to #order-modal" {
                        (icon::plus_icon("w-3.5 h-3.5"))
                        "从订单添加"
                    }
                }

                // Empty state
                div id="precon-items-empty" style="padding:var(--space-10) var(--space-5);text-align:center" {
                    div style="color:var(--muted);margin-bottom:var(--space-4)" {
                        "暂无对账明细"
                    }
                    button type="button" class="btn inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative-sm inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)] [&_svg]:w-4 [&_svg]:h-4"
                        _="on click add .is-open to #order-modal" {
                        (icon::plus_icon("w-3.5 h-3.5"))
                        "选择订单"
                    }
                }

                // Line items table (hidden when empty)
                div id="precon-items-table-wrap" style="display:none" {
                    div style="overflow-x:auto" {
                        table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" style="min-width:1100px" {
                            thead {
                                tr {
                                    th style="width:36px;text-align:center" { "#" }
                                    th { "关联订单" }
                                    th { "物料编码" }
                                    th { "物料名称" }
                                    th class="text-right text-[13px]" { "收货数量" }
                                    th class="text-right text-[13px]" { "退货数量" }
                                    th class="text-right text-[13px]" { "退货冲减金额" }
                                    th class="text-right text-[13px]" { "单价" }
                                    th class="text-right text-[13px]" { "应付金额" }
                                    th style="width:36px" { }
                                }
                            }
                            tbody id="precon-item-tbody" { }
                        }
                    }

                }
            }

            // ── Action Bar ──
            div class="flex items-center justify-end gap-3 pt-4 border-t border-border-soft" {
                a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{}?restore=true", PreconListPath::PATH)) { "取消" }
                div style="display:flex;gap:var(--space-3)" {
                    button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" name="action" value="draft" {
                        "保存草稿"
                    }
                    button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
                        "提交对账单"
                    }
                }
            }
            }

            // ── Order Picker Modal ──
            div class="fixed z-[1000] grid place-items-center opacity-0" id="order-modal"
                _="on click[me is event.target] remove .is-open" {
                div class="modal bg-bg rounded-xl w-[680px] flex flex-col overflow-hidden opacity-0-lg" onclick="event.stopPropagation()" {
                    div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                        h2 { "选择待对账订单" }
                        button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                            _="on click remove .is-open from #order-modal" {
                            (icon::x_icon("w-5 h-5"))
                        }
                    }
                    div class="overflow-y-auto flex-1 min-h-0 p-6" style="padding:var(--space-4)" {
                        div style="margin-bottom:var(--space-3)" {
                            input type="text" id="order-search-input"
                                placeholder="搜索订单号…"
                                style="width:100%;padding:8px 12px;border:1px solid var(--border);border-radius:var(--radius-sm);font-size:var(--text-sm)" {}
                        }
                        div id="order-list-body" style="max-height:360px;overflow-y:auto" {
                            div style="display:flex;align-items:center;justify-content:center;padding:var(--space-8);color:var(--muted)" {
                                "请先选择供应商"
                            }
                        }
                    }
                    div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" style="display:flex;justify-content:space-between;align-items:center;padding:var(--space-3) var(--space-4);border-top:1px solid var(--border)" {
                        span style="font-size:var(--text-sm);color:var(--muted)" {
                            "已选择 "
                            span id="order-selected-count" { "0" }
                            " 个订单"
                        }
                        div style="display:flex;gap:var(--space-2)" {
                            button type="button" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                                _="on click remove .is-open from #order-modal" { "取消" }
                            button type="button" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" id="btn-confirm-orders"
                                _="on click remove .is-open from #order-modal" {
                                "确认添加"
                            }
                        }
                    }
                }
            }

        }
    }
}
