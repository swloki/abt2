use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::work_order::{WorkOrderFilter, WorkOrderService};
use abt_core::master_data::product::ProductService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_order::{OrderCreatePath, OrderListPath, OrderTablePath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct OrderQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

fn wo_status_label(s: &abt_core::mes::enums::WorkOrderStatus) -> (&'static str, &'static str, &'static str) {
    use abt_core::mes::enums::WorkOrderStatus::*;
    match s {
        Draft => ("待计划", "rgba(0,0,0,0.04)", "var(--muted)"),
        Planned => ("已计划", "rgba(22,119,255,0.08)", "var(--accent)"),
        Released => ("已下达", "rgba(82,196,26,0.08)", "var(--success)"),
        Closed => ("已关闭", "rgba(114,46,209,0.08)", "#722ed1"),
        Cancelled => ("已取消", "rgba(245,63,63,0.06)", "#f53f3f"),
    }
}

async fn resolve_op_names<S: UserService>(
    svc: &S, ctx: &abt_core::shared::types::ServiceContext, db: abt_core::shared::types::PgExecutor<'_>,
    items: &[abt_core::mes::work_order::WorkOrder],
) -> HashMap<i64, String> {
    let mut map = HashMap::new();
    for item in items {
        if !map.contains_key(&item.operator_id) {
            if let Ok(u) = svc.get_user(ctx, db, item.operator_id).await {
                map.insert(item.operator_id, u.display_name.unwrap_or_default());
            }
        }
    }
    map
}

fn parse_wo_status(s: &str) -> Option<abt_core::mes::enums::WorkOrderStatus> {
    use abt_core::mes::enums::WorkOrderStatus::*;
    match s { "Draft" => Some(Draft), "Planned" => Some(Planned), "Released" => Some(Released), "Closed" => Some(Closed), "Cancelled" => Some(Cancelled), _ => None }
}

#[require_permission("MES", "read")]
pub async fn get_order_list(
    _path: OrderListPath, ctx: RequestContext, Query(params): Query<OrderQueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_create = ctx.has_permission("WORK_ORDER", "create").await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.work_order_service();
    let user_svc = state.user_service();
    let product_svc = state.product_service();
    let filter = WorkOrderFilter {
        status: params.status.as_deref().and_then(parse_wo_status),
        product_id: None, keyword: params.keyword.clone(), date_from: None, date_to: None,
    };
    let result = svc.list(&service_ctx, &mut conn, filter, params.page.unwrap_or(1), 20).await?;
    let op_names = resolve_op_names(&user_svc, &service_ctx, &mut conn, &result.items).await;
    let product_names: HashMap<i64, String> = {
        let pids: Vec<i64> = result.items.iter().map(|i| i.product_id).collect();
        product_svc.get_by_ids(&service_ctx, &mut conn, pids).await
            .map(|ps| ps.iter().map(|p| (p.product_id, p.pdt_name.clone())).collect())
            .unwrap_or_default()
    };
    let content = order_list_page(&result, &op_names, &product_names, &params, can_create);
    Ok(Html(admin_page(is_htmx, "工单管理", &claims, "production", OrderListPath::PATH, "生产管理", None, content, &nav_filter).into_string()))
}

#[require_permission("MES", "read")]
pub async fn get_order_table(
    _path: OrderTablePath, ctx: RequestContext, Query(params): Query<OrderQueryParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.work_order_service();
    let user_svc = state.user_service();
    let product_svc = state.product_service();
    let filter = WorkOrderFilter {
        status: params.status.as_deref().and_then(parse_wo_status),
        product_id: None, keyword: params.keyword.clone(), date_from: None, date_to: None,
    };
    let result = svc.list(&service_ctx, &mut conn, filter, params.page.unwrap_or(1), 20).await?;
    let op_names = resolve_op_names(&user_svc, &service_ctx, &mut conn, &result.items).await;
    let product_names: HashMap<i64, String> = {
        let pids: Vec<i64> = result.items.iter().map(|i| i.product_id).collect();
        product_svc.get_by_ids(&service_ctx, &mut conn, pids).await
            .map(|ps| ps.iter().map(|p| (p.product_id, p.pdt_name.clone())).collect())
            .unwrap_or_default()
    };
    Ok(Html(order_data_card(&result, &op_names, &product_names, &params).into_string()))
}

fn order_list_page(
    result: &abt_core::shared::types::PaginatedResult<abt_core::mes::work_order::WorkOrder>,
    op_names: &HashMap<i64, String>, product_names: &HashMap<i64, String>, params: &OrderQueryParams,
    can_create: bool,
) -> Markup {
    html! { div {
        div class="page-header" { h1 class="page-title" { "工单管理" } div class="page-actions" {
            @if can_create {
                a class="btn btn-primary" href=(OrderCreatePath::PATH) { (icon::plus_icon("w-4 h-4")) "新建工单" }
            }
        }}
        (order_table_fragment(result, op_names, product_names, params))
    }}
}

fn order_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<abt_core::mes::work_order::WorkOrder>,
    op_names: &HashMap<i64, String>, product_names: &HashMap<i64, String>, params: &OrderQueryParams,
) -> Markup {
    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(result.total) },
        TabItem { value: "Draft".into(), label: "待计划", count: None },
        TabItem { value: "Planned".into(), label: "已计划", count: None },
        TabItem { value: "Released".into(), label: "已下达", count: None },
        TabItem { value: "Closed".into(), label: "已关闭", count: None },
    ];
    let sel = params.status.as_deref().unwrap_or("");

    html! { div {
        (status_tabs_with_param(OrderTablePath::PATH, "#order-data-card", "closest form", tabs, sel, "status"))
        form class="filter-bar filter-form" hx-get=(OrderTablePath::PATH)
            hx-trigger="change, keyup changed delay:300ms from:.search-input"
            hx-target="#order-data-card" hx-select="#order-data-card" hx-swap="outerHTML" hx-include="closest form" {
            div class="search-wrap" { (icon::search_icon("w-4 h-4"))
                input class="search-input" type="text" name="keyword" style="width:180px" placeholder="搜索工单编号…" value=(params.keyword.as_deref().unwrap_or(""));
            }
        }
        (order_data_card(result, op_names, product_names, params))
    }}
}
fn order_data_card(
    result: &abt_core::shared::types::PaginatedResult<abt_core::mes::work_order::WorkOrder>,
    op_names: &HashMap<i64, String>, product_names: &HashMap<i64, String>, params: &OrderQueryParams,
) -> Markup {
    let mut qs = vec![];
    if let Some(ref k) = params.keyword { qs.push(format!("keyword={k}")); }
    if let Some(ref s) = params.status { qs.push(format!("status={s}")); }
    let query = qs.join("&");
    html! {
        div class="data-card" id="order-data-card" {
            div class="data-card-scroll" {
                table class="data-table" { thead { tr {
                    th { "工单编号" } th { "产品" } th class="num-right" { "计划数量" }
                    th { "开始日期" } th { "结束日期" } th { "状态" } th { "创建人" }
                    th { "创建时间" } th { "操作" }
                }} tbody {
                    @for item in &result.items {
                        @let (sl, sb, sc) = wo_status_label(&item.status);
                        @let on = op_names.get(&item.operator_id).map(|s| s.as_str()).unwrap_or("\u{2014}");
                        @let pn = product_names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("\u{2014}");
                        @let dp = format!("/admin/mes/orders/{}", item.id);
                        tr style="cursor:pointer" onclick=(format!("location.href='{}'", dp)) {
                            td class="link-cell mono" style="color:var(--accent)" { (item.doc_number) }
                            td { (pn) }
                            td class="num-right mono" { (crate::utils::fmt_qty(item.planned_qty)) }
                            td { (item.scheduled_start) }
                            td { (item.scheduled_end) }
                            td { span style=(format!("display:inline-flex;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", sb, sc)) { (sl) } }
                            td { (on) }
                            td style="font-size:12px;color:var(--muted)" { (item.created_at.format("%Y-%m-%d %H:%M")) }
                            td { a href=(dp) style="color:var(--accent);font-size:var(--text-xs)" { "查看" } }
                        }
                    }
                    @if result.items.is_empty() {
                        tr { td colspan="9" style="text-align:center;padding:var(--space-8);color:var(--muted)" { "暂无工单" } }
                    }
                }}
            }
            (pagination(OrderListPath::PATH, &query, result.total, result.page, result.total_pages))
        }
    }
}
