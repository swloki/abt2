use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::fms::ar_ap::model::{ArApSettlement, SettlementFilter};
use abt_core::fms::ar_ap::ArApService;
use abt_core::shared::types::PaginatedResult;
use rust_decimal::Decimal;

use crate::components::pagination::pagination;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms::{SettlementListPath, SettlementUnsettlePath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[derive(Deserialize, Debug, Default)]
pub struct ListQuery {
    pub page: Option<u32>,
}

#[derive(Deserialize, Debug)]
pub struct SettleForm {
    pub payment_source_type: i16,
    pub payment_source_id: i64,
    pub invoice_source_type: i16,
    pub invoice_source_id: i64,
    pub amount: String,
}

fn fmt_amount(amount: Decimal) -> String { format!("{amount:.2}") }

fn settlement_table(items: &[ArApSettlement], total: u64, page: u32, page_size: u32) -> Markup {
    let total_pages = ((total as f64) / (page_size as f64)).ceil() as u32;
    html! {
        div id="data-card" class="data-card" {
            div class="overflow-x-auto" {
                table class="data-table" {
                    thead {
                        tr {
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase" {
                                "ID"
                            }
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase" {
                                "付款单据"
                            }
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase" {
                                "发票单据"
                            }
                            th  class="px-4 py-3 text-right text-xs font-medium text-fg-2 uppercase"
                            { "核销金额" }
                            th  class="px-4 py-3 text-right text-xs font-medium text-fg-2 uppercase"
                            { "汇兑损益" }
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase" {
                                "核销日期"
                            }
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase" {
                                "操作"
                            }
                        }
                    }
                    tbody class="divide-y divide-border-soft" {
                        @for item in items {
                            tr {
                                td class="px-4 py-3 text-sm text-fg-2" { "#" (item.id) }
                                td class="px-4 py-3 text-sm text-fg-2" {
                                    (item.payment_source_type.prefix())
                                    "-"
                                    (item.payment_source_id)
                                }
                                td class="px-4 py-3 text-sm text-fg-2" {
                                    (item.invoice_source_type.prefix())
                                    "-"
                                    (item.invoice_source_id)
                                }
                                td class="px-4 py-3 text-sm font-mono text-right" {
                                    "¥"
                                    (fmt_amount(item.amount))
                                }
                                td class="px-4 py-3 text-sm font-mono text-right" {
                                    @if item.exchange_gain_loss != Decimal::ZERO {
                                        span style="color:var(--warning)" {
                                            "¥"
                                            (fmt_amount(item.exchange_gain_loss))
                                        }
                                    } @else { "—" }
                                }
                                td class="px-4 py-3 text-sm text-fg-2" { (item.settlement_date) }
                                td class="px-4 py-3 text-sm" {
                                    form
                                        hx-post=({
                                            SettlementUnsettlePath {
                                                id: item.id,
                                            }
                                        })
                                        hx-target="#data-card"
                                        hx-swap="outerHTML"
                                    {
                                        button
                                            type="submit"
                                            class="btn btn-sm btn-outline text-danger"
                                        { "撤销" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            @if total > page_size as u64 {
                (pagination(SettlementListPath::PATH, "", total, page, total_pages))
            }
        }
    }
}

#[require_permission("FMS", "read")]
pub async fn get_list(
    _path: SettlementListPath,
    ctx: RequestContext,
    Query(q): Query<ListQuery>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.ar_ap_service();

    let page = q.page.unwrap_or(1).max(1);
    let page_size = 20u32;
    let page_params = abt_core::shared::types::PageParams::new(page, page_size);

    let result = svc.list_settlements(&service_ctx, &mut conn, SettlementFilter::default(), page_params).await
        .unwrap_or_else(|_| PaginatedResult::new(vec![], 0, page, page_size));

    let content = html! {
        div {
            h1 class="text-xl font-bold text-fg tracking-tight mb-5" { "核销记录" }
            (settlement_table(&result.items, result.total, page, page_size))
        }
    };

    let page_html = admin_page(is_htmx, "核销管理", &claims, "finance", SettlementListPath::PATH, "财务管理", None, content, &nav_filter);
    Ok(Html(page_html.into_string()))
}

#[require_permission("FMS", "write")]
pub async fn unsettle(
    _path: SettlementUnsettlePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.ar_ap_service();
    let settlement_id = _path.id;

    let _ = svc.unsettle(&service_ctx, &mut conn, settlement_id).await;

    // Re-render list
    let page_size = 20u32;
    let page_params = abt_core::shared::types::PageParams::new(1, page_size);
    let result = svc.list_settlements(&service_ctx, &mut conn, SettlementFilter::default(), page_params).await
        .unwrap_or_else(|_| PaginatedResult::new(vec![], 0, 1, page_size));

    let content = settlement_table(&result.items, result.total, 1, page_size);
    let page_html = admin_page(true, "核销管理", &claims, "finance", SettlementListPath::PATH, "财务管理", None, content, &nav_filter);
    Ok(Html(page_html.into_string()))
}
