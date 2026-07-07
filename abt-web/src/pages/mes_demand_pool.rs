//! MES 生产需求池 — demand-rows 懒加载端点（按物料展开需求明细）。
//!
//! 独立需求池列表页已下线，需求池收口到作业中心（`/admin/mes/work-center`）的 demand card。
//! 本模块仅保留 `get_demand_rows` 端点（`MesDemandRowsPath`）：作业中心 demand card 物料行
//! 展开时懒加载该物料的需求明细。`mes_demand_pool_create`（创建工单）端点也保留，同样服务作业中心。

use axum::extract::Query;
use axum::response::Html;
use chrono::NaiveDate;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::demand_handler::{DemandPoolQuery, DemandSummary, MesDemandService};
use abt_core::shared::types::PageParams;

use crate::errors::Result;
use crate::routes::mes_demand_pool::*;
use crate::utils::{fmt_qty, RequestContext};
use abt_macros::require_permission;

/// HTMX endpoint: load demand detail rows for a specific product (material expansion)
#[require_permission("WORK_ORDER", "read")]
pub async fn get_demand_rows(
    _path: MesDemandRowsPath,
    ctx: RequestContext,
    Query(params): Query<DemandRowsQueryParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    let svc = state.mes_demand_service();
    let query = DemandPoolQuery {
        status: None,
        product_id: Some(params.product_id),
        order_id: None,
        ..Default::default()
    };
    let result = svc
        .list_pending_demands(&service_ctx, &mut conn, query, PageParams::new(1, 100))
        .await?;

    Ok(Html(demand_expand_rows(&result.items).into_string()))
}

#[derive(Debug, Deserialize)]
pub struct DemandRowsQueryParams {
    pub product_id: i64,
}

// ── Demand Expand Rows (HTMX fragment) ──

fn demand_expand_rows(items: &[DemandSummary]) -> Markup {
    html! {
        @if items.is_empty() {
            tr {
                td colspan="7" class="text-center text-muted" { "暂无需求记录" }
            }
        }
        @for d in items { (demand_expand_row(d)) }
    }
}

fn demand_expand_row(d: &DemandSummary) -> Markup {
    html! {
        tr class="bg-[rgba(37,99,235,0.04)] [&_td]:py-2.5" {
            td class="px-2" {
                div class="flex items-center justify-center" {
                    input type="checkbox" class="demand-cb"
                        value=(d.id) checked
                        data-product-id=(d.product_id)
                        data-product-name=(d.product_name)
                        data-product-code=(d.product_code);
                }
            }
            td class="font-mono tabular-nums text-xs px-2" { (d.id) }
            td class="px-2" {
                a class="text-accent font-medium cursor-pointer text-xs"
                    href=(format!("/admin/orders/{}", d.order_id))
                { (d.order_no.as_deref().unwrap_or("—")) }
            }
            td class="text-right text-[13px] font-mono tabular-nums px-2" { (fmt_qty(d.quantity)) }
            td class="font-mono tabular-nums px-2" { (format_date(d.required_date)) }
            td class="px-2" { (priority_label(d.priority)) }
            td class="px-2" { (demand_status_label(d.demand_status)) }
        }
    }
}

// ── helpers ──

fn format_date(d: Option<NaiveDate>) -> Markup {
    match d {
        Some(date) => html! {
            (date.format("%Y-%m-%d").to_string())
        },
        None => html! {
            span class="text-muted" { "—" }
        },
    }
}

fn demand_status_label(status: i16) -> Markup {
    let (label, cls) = match status {
        1 => ("待处理", "status-pill-muted"),
        2 => ("已确认", "status-pill-info"),
        3 => ("已创建工单", "status-pill-warn"),
        4 => ("已完成", "status-pill-success"),
        5 => ("已拒绝", "status-pill-danger"),
        _ => ("未知", "status-pill-muted"),
    };
    html! {
        span class=(format!("status-pill {}", crate::utils::status_color(cls))) { (label) }
    }
}

fn priority_label(priority: i32) -> Markup {
    let (label, cls) = match priority {
        p if p >= 4 => ("紧急", "bg-danger-bg text-danger"),
        3 => ("高", "bg-warn-bg text-warn"),
        2 => ("中", "bg-accent-bg text-accent"),
        _ => ("低", "bg-slate-50 text-slate-400"),
    };
    html! {
        span class=(format!(
            "inline-flex items-center text-[11px] px-2 py-0.5 rounded-full font-medium {}",
            cls,
        )) { (label) }
    }
}
