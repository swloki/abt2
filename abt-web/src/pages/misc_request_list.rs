use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::purchase::enums::MiscRequestStatus;
use abt_core::purchase::misc_request::model::*;
use abt_core::purchase::misc_request::MiscellaneousRequestService;
use abt_core::shared::identity::{DepartmentService, UserService};
use abt_core::shared::types::PageParams;
use abt_core::shared::types::ServiceContext;


use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::misc_request::*;
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct MiscQueryParams {
 pub keyword: Option<String>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub status: Option<i16>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub page: Option<u32>,
 pub department: Option<String>,
 pub date_range: Option<String>,
}

// ── Helpers ──

fn parse_date_range(range: &str) -> (Option<chrono::NaiveDate>, Option<chrono::NaiveDate>) {
 let today = chrono::Local::now().date_naive();
 match range {
 "7d" => (Some(today - chrono::Days::new(7)), None),
 "30d" => (Some(today - chrono::Days::new(30)), None),
 "3m" => (Some(today - chrono::Months::new(3)), None),
 _ => (None, None),
 }
}

fn build_filter(params: &MiscQueryParams, dept_id_map: &HashMap<String, i64>) -> MiscRequestQuery {
 let (request_date_start, request_date_end) = params
 .date_range
 .as_deref()
 .map(parse_date_range)
 .unwrap_or((None, None));

 let department_id = params
 .department
 .as_deref()
 .and_then(|name| dept_id_map.get(name).copied());

 MiscRequestQuery {
 department_id,
 status: params.status.and_then(MiscRequestStatus::from_i16),
 request_date_start,
 request_date_end,
 ..Default::default()
 }
}

async fn resolve_operator_names<S: UserService>(
 svc: &S,
 ctx: &ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
 items: &[MiscellaneousRequest],
) -> HashMap<i64, String> {
 let ids: Vec<i64> = items.iter().map(|r| r.operator_id).collect();
 if ids.is_empty() {
 return HashMap::new();
 }
 svc.get_users_by_ids(ctx, db, ids)
 .await
 .map(|users| {
 users
 .into_iter()
 .map(|u| (u.user.user_id, u.user.display_name.unwrap_or(u.user.username)))
 .collect()
 })
 .unwrap_or_default()
}

async fn resolve_department_names<S: DepartmentService>(
 svc: &S,
 ctx: &ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
) -> HashMap<i64, String> {
 svc.list_departments(ctx, db)
 .await
 .map(|depts| {
 depts
 .into_iter()
 .map(|d| (d.department_id, d.department_name))
 .collect()
 })
 .unwrap_or_default()
}

async fn load_dept_id_map<S: DepartmentService>(
 svc: &S,
 ctx: &ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
) -> HashMap<String, i64> {
 svc.list_departments(ctx, db)
 .await
 .map(|depts| {
 depts
 .into_iter()
 .map(|d| (d.department_name, d.department_id))
 .collect()
 })
 .unwrap_or_default()
}

// ── Status Labels ──

fn status_label(s: MiscRequestStatus) -> (&'static str, &'static str) {
 match s {
 MiscRequestStatus::Draft => ("草稿", "status-draft"),
 MiscRequestStatus::Approved => ("已审批", "status-confirmed"),
 MiscRequestStatus::Purchasing => ("采购中", "status-info"),
 MiscRequestStatus::Received => ("已收货", "status-success"),
 MiscRequestStatus::Closed => ("已关闭", "status-cancelled"),
 MiscRequestStatus::Cancelled => ("已取消", "status-cancelled"),
 }
}

// ── Handlers ──

#[require_permission("MISC_REQUEST", "read")]
pub async fn get_misc_list(
 _path: MiscListPath,
 ctx: RequestContext,
 Query(params): Query<MiscQueryParams>,
) -> Result<Html<String>> {
 let can_create = ctx.has_permission("PURCHASE_ORDER", "create").await;
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
 let svc = state.misc_request_service();
 let user_svc = state.user_service();
 let dept_svc = state.department_service();

 let dept_id_map = load_dept_id_map(&dept_svc, &service_ctx, &mut conn).await;
 let filter = build_filter(&params, &dept_id_map);
 let page = PageParams::new(params.page.unwrap_or(1), 20);
 let result = svc.list(&service_ctx, &mut conn, filter, page).await?;

 let operator_map = resolve_operator_names(&user_svc, &service_ctx, &mut conn, &result.items).await;
 let dept_name_map = resolve_department_names(&dept_svc, &service_ctx, &mut conn).await;

 let content = misc_list_page(&result, &params, &operator_map, &dept_name_map, can_create);
 let page_html = admin_page(
 is_htmx,
 "零星采购",
 &claims,
 "purchase",
 MiscListPath::PATH,
 "采购管理",
 Some("零星采购"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

// ── Components ──

fn misc_list_page(
 result: &abt_core::shared::types::PaginatedResult<MiscellaneousRequest>,
 params: &MiscQueryParams,
 operator_map: &HashMap<i64, String>,
 dept_name_map: &HashMap<i64, String>,
 can_create: bool,
) -> Markup {
 html! {
    div {
        // ── Page Header ──
        div class="flex items-center justify-between mb-6" {
            h1 class="text-xl font-bold text-fg tracking-tight" { "零星采购" }
            div class="flex gap-3" {
                @if can_create {
                    a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                        href=(MiscCreatePath::PATH)
                    { (icon::plus_icon("w-4 h-4")) "新建零星采购" }
                }
            }
        }
        // ── Tabs + Filter + Data Table (HTMX panel) ──
        (misc_table_fragment(result, params, operator_map, dept_name_map))
    }
}
}

fn misc_table_fragment(
 result: &abt_core::shared::types::PaginatedResult<MiscellaneousRequest>,
 params: &MiscQueryParams,
 operator_map: &HashMap<i64, String>,
 dept_name_map: &HashMap<i64, String>,
) -> Markup {
 let active_value = params.status.map(|s| s.to_string()).unwrap_or_default();
 let total_count = result.total;

 let tabs = &[
 TabItem { value: String::new(), label: "全部", count: Some(total_count) },
 TabItem { value: "1".into(), label: "草稿", count: None },
 TabItem { value: "2".into(), label: "已审批", count: None },
 TabItem { value: "3".into(), label: "采购中", count: None },
 TabItem { value: "4".into(), label: "已收货", count: None },
 TabItem { value: "5".into(), label: "已关闭", count: None },
 TabItem { value: "6".into(), label: "已取消", count: None },
 ];

 let dept_value = params.department.as_deref().unwrap_or("");
 let date_range_value = params.date_range.as_deref().unwrap_or("");

 html! {
    div class="misc-list-panel" {
        ({
            status_tabs_with_param(
                MiscListPath::PATH,
                "#misc-data-card",
                "#misc-filter-form",
                tabs,
                &active_value,
                "status",
            )
        })
        // ── Filter Bar ──
        form
            class="flex items-center gap-3 mb-5 flex-wrap filter-form"
            id="misc-filter-form"
            hx-get=(MiscListPath::PATH)
            hx-trigger="change, keyup changed delay:300ms from:.search-input"
            hx-target="#misc-data-card"
            hx-select="#misc-data-card"
            hx-swap="outerHTML"
            hx-select-oob="#status-tabs"
            hx-include="#misc-filter-form"
           
        {
            div class="relative flex-1 max-w-xs icon:absolute icon:left-3 icon:top-1/2 icon:-translate-y-1/2 icon:w-4 icon:h-4 icon:text-muted"
            {
                (icon::search_icon(""))
                input
                    class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input"
                    type="text"
                    name="keyword"
                    placeholder="搜索单据编号…"
                    value=(params.keyword.as_deref().unwrap_or(""));
            }
            select
                class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer"
                name="department"
            {
                option value="" selected[dept_value.is_empty()] { "全部部门" }
                option value="行政部" selected[dept_value == "行政部"] { "行政部" }
                option value="IT部" selected[dept_value == "IT部"] { "IT部" }
                option value="生产部" selected[dept_value == "生产部"] { "生产部" }
                option value="品质部" selected[dept_value == "品质部"] { "品质部" }
                option value="研发部" selected[dept_value == "研发部"] { "研发部" }
                option value="财务部" selected[dept_value == "财务部"] { "财务部" }
                option value="人事部" selected[dept_value == "人事部"] { "人事部" }
                option value="市场部" selected[dept_value == "市场部"] { "市场部" }
            }
            select
                class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer"
                name="date_range"
            {
                option value="" selected[date_range_value.is_empty()] { "请购日期" }
                option value="7d" selected[date_range_value == "7d"] { "最近7天" }
                option value="30d" selected[date_range_value == "30d"] { "最近30天" }
                option value="3m" selected[date_range_value == "3m"] { "最近3个月" }
            }
        }
        // ── Data Table ──
        div class="data-card" id="misc-data-card" {
            div class="overflow-x-auto" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "请购单号" }
                            th { "申请部门" }
                            th { "请购日期" }
                            th { "用途" }
                            th { "状态" }
                            th class="text-right text-[13px]" { "预估金额" }
                            th { "申请人" }
                            th class="!text-right" { "操作" }
                        }
                    }
                    tbody {
                        @for r in &result.items { (misc_row(r, operator_map, dept_name_map)) }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="8" class="text-center text-muted py-8" { "暂无请购数据" }
                            }
                        }
                    }
                }
            }
            ({
                pagination(
                    MiscListPath::PATH,
                    "#misc-data-card",
                    "#misc-filter-form",
                    result.total,
                    result.page,
                    result.total_pages,
                )
            })
        }
    }
}
}

fn misc_row(
 r: &MiscellaneousRequest,
 operator_map: &HashMap<i64, String>,
 dept_name_map: &HashMap<i64, String>,
) -> Markup {
 let detail_path = MiscDetailPath { id: r.id };
 let (status_text, status_class) = status_label(r.status);
 let operator_name = operator_map.get(&r.operator_id).map(|s| s.as_str()).unwrap_or("—");
 let dept_name = dept_name_map.get(&r.department_id).map(|s| s.as_str()).unwrap_or("—");
 let onclick = format!("location.href='{}'", detail_path);
 let is_draft = r.status == MiscRequestStatus::Draft;

 html! {
    tr class="cursor-pointer" {
        td class="text-accent font-medium cursor-pointer font-mono tabular-nums" onclick=(&onclick) {
            (r.doc_number)
        }
        td onclick=(&onclick) { (dept_name) }
        td class="font-mono tabular-nums" onclick=(&onclick) { (r.request_date.format("%Y-%m-%d")) }
        td onclick=(&onclick) { (r.purpose.as_str()) }
        td onclick=(&onclick) {
            span class=(format!("status-pill {}", crate::utils::status_color(status_class))) {
                (status_text)
            }
        }
        td class="text-right text-[13px] font-mono tabular-nums" onclick=(&onclick) {
            (format!("{:.2}", r.total_amount))
        }
        td onclick=(&onclick) { (operator_name) }
        td _="on click halt the event" {
            @if is_draft {
                div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg icon:w-3.5 icon:h-3.5"
                {
                    a   class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer"
                        href=(detail_path.to_string())
                        title="编辑"
                    { (icon::edit_icon("w-4 h-4")) }
                }
            }
        }
    }
}
}
