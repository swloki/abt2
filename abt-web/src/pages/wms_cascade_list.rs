use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;

use abt_core::wms::inventory_cascade::model::*;

use crate::components::icon;
use crate::layout::page::admin_page;
use crate::routes::wms_cascade::CascadeListPath;
use crate::utils::RequestContext;

use abt_macros::require_permission;

// ── Handlers ──

#[require_permission("INVENTORY", "read")]
pub async fn get_cascade_list(
    _path: CascadeListPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let claims = ctx.claims;

    let content = cascade_page(None);
    let page_html = admin_page(
        is_htmx,
        "级联库存查询",
        &claims,
        "inventory",
        CascadeListPath::PATH,
        "库存管理",
        Some("级联库存查询"),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

// ── Components ──

fn cascade_page(result: Option<&CascadeInventoryResult>) -> Markup {
    html! {
        div {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "级联库存查询" }
            }

            div class="cascade-search" style="background:var(--bg);border:1px solid var(--border-soft);border-radius:var(--radius-md);padding:var(--space-5) var(--space-6);margin-bottom:var(--space-6);display:flex;align-items:center;gap:var(--space-3)" {
                div class="relative flex-1 max-w-xs" style="flex:1" {
                    (icon::search_icon("w-4 h-4"))
                    input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="product_code"
                        placeholder="输入产品编码或产品名称"
                        hx-get=(CascadeListPath::PATH)
                        hx-trigger="keyup changed delay:500ms"
                        hx-sync="this:replace"
                        hx-target=".cascade-results"
                        hx-swap="innerHTML";
                }
                button class="btn bg-accent text-accent-on border-none hover:bg-accent-hover"
                    hx-get=(CascadeListPath::PATH)
                    hx-target=".cascade-results"
                    hx-swap="innerHTML"
                    hx-include="input[name=product_code]" {
                    (icon::search_icon("w-4 h-4"))
                    "查询"
                }
            }

            div class="cascade-results" {
                @if let Some(r) = result {
                    (cascade_results(r))
                } @else {
                    div style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                        "请输入产品编码进行查询"
                    }
                }
            }
        }
    }
}

fn cascade_results(result: &CascadeInventoryResult) -> Markup {
    html! {
        div {
            // Product info card
            div class="cascade-product" style="background:linear-gradient(135deg,#e6f4ff 0%,#f0f7ff 100%);border:1px solid rgba(22,119,255,0.15);border-radius:var(--radius-md);padding:var(--space-5) var(--space-6);margin-bottom:var(--space-6);display:flex;align-items:center;gap:var(--space-5)" {
                div class="cascade-product-icon" style="width:48px;height:48px;border-radius:var(--radius-md);background:linear-gradient(135deg,var(--accent) 0%,#4096ff 100%);display:grid;place-items:center;flex-shrink:0" {
                    (icon::box_icon("w-6 h-6"))
                }
                div class="cascade-product-info" style="flex:1" {
                    div class="cascade-product-name" style="font-size:var(--text-lg);font-weight:700;color:var(--fg);margin-bottom:var(--space-1)" {
                        (result.product_name)
                    }
                    div class="cascade-product-code" style="font-size:var(--text-sm);color:var(--muted);font-family:var(--font-mono)" {
                        (result.product_code)
                    }
                }
                div class="cascade-product-stock" style="text-align:right" {
                    div style="font-size:12px;color:var(--muted)" { "当前库存总量" }
                    div style="font-size:var(--text-2xl);font-weight:700;color:var(--fg);font-family:var(--font-mono)" {
                        (format!("{:.2}", result.total_quantity))
                    }
                }
            }

            // BOM groups
            @for group in &result.bom_groups {
                (bom_group(group))
            }

            @if result.bom_groups.is_empty() {
                div style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                    "该产品无关联BOM"
                }
            }
        }
    }
}

fn bom_group(group: &BomCascadeGroup) -> Markup {
    html! {
        div class="bom-section" style="margin-bottom:var(--space-6)" {
            div class="bom-header" style="display:flex;align-items:center;gap:var(--space-3);margin-bottom:var(--space-3)" {
                span class="bom-badge" style="display:inline-flex;align-items:center;gap:var(--space-1);padding:3px 12px;border-radius:var(--radius-pill);font-size:12px;font-weight:600;background:var(--accent-bg);color:var(--accent)" {
                    (icon::box_icon("w-3.5 h-3.5"))
                    "BOM"
                }
                span class="bom-name" style="font-size:var(--text-base);font-weight:600;color:var(--fg)" {
                    (group.bom_name)
                }
            }
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]-scroll" {
                    table class="data-table" style="min-width:860px" {
                        thead {
                            tr {
                                th { "子件编码" }
                                th { "子件名称" }
                                th { "单位" }
                                th class="num-right" { "BOM用量" }
                                th class="num-right" { "当前库存总量" }
                                th class="num-right" { "损耗率" }
                                th { "是否缺料" }
                            }
                        }
                        tbody {
                            @for child in &group.children {
                                (bom_child_row(child))
                            }
                            @if group.children.is_empty() {
                                tr {
                                    td colspan="7" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "无子件数据"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn bom_child_row(child: &ChildNodeInventory) -> Markup {
    let is_shortage = child.total_stock < child.quantity;
    let loss_pct = child.loss_rate * Decimal::from(100);

    html! {
        tr {
            td class="mono" { (child.product_code) }
            td { (child.product_name) }
            td {
                @if let Some(ref u) = child.unit {
                    (u)
                } @else {
                    span style="color:var(--muted)" { "—" }
                }
            }
            td class="num-right" { (child.quantity) }
            td class="num-right" { (child.total_stock) }
            td class="num-right" {
                (format!("{:.1}%", loss_pct))
            }
            td {
                @if is_shortage {
                    span class="shortage-cell" style="display:inline-flex;align-items:center;gap:var(--space-1);color:var(--danger);font-weight:600;font-family:var(--font-mono)" {
                        (crate::components::icon::circle_alert_icon("w-3.5 h-3.5"))
                        "缺料"
                    }
                } @else {
                    span class="ok-cell" style="color:var(--success);font-weight:500;font-family:var(--font-mono)" {
                        "充足"
                    }
                }
            }
        }
    }
}
