use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::qms::enums::{MRBDisposition, MRBStatus, ResponsibleParty};
use abt_core::qms::inspection_result::InspectionResultService;
use abt_core::qms::mrb::model::{Mrb, MrbFilter};
use abt_core::qms::mrb::MrbService;
use abt_core::shared::types::PaginatedResult;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::qms::{MrbCreatePath, MrbDetailPath, MrbListPath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct MrbQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub disposition: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub responsible_party: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub date_from: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub date_to: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Helpers ──

fn disposition_label(d: &MRBDisposition) -> (&'static str, &'static str, &'static str) {
    match d {
        MRBDisposition::Scrap => ("报废", "rgba(220,38,38,0.08)", "#dc2626"),
        MRBDisposition::Return => ("退货", "rgba(234,88,12,0.08)", "#ea580c"),
        MRBDisposition::Degrade => ("降级", "rgba(124,58,237,0.08)", "#7c3aed"),
        MRBDisposition::Rework => ("返工", "rgba(37,99,235,0.08)", "var(--accent)"),
    }
}

fn responsible_party_label(r: &ResponsibleParty) -> (&'static str, &'static str, &'static str) {
    match r {
        ResponsibleParty::Internal => ("内部", "rgba(22,163,74,0.08)", "var(--success)"),
        ResponsibleParty::Supplier => ("供应商", "rgba(37,99,235,0.08)", "var(--accent)"),
        ResponsibleParty::Customer => ("客户", "rgba(124,58,237,0.08)", "#7c3aed"),
    }
}

fn mrb_status_label(s: &MRBStatus) -> (&'static str, &'static str, &'static str) {
    match s {
        MRBStatus::Draft => ("草稿", "rgba(0,0,0,0.04)", "var(--muted)"),
        MRBStatus::UnderReview => ("审批中", "rgba(217,119,6,0.08)", "#b45309"),
        MRBStatus::Approved => ("已批准", "rgba(22,119,255,0.08)", "var(--accent)"),
        MRBStatus::Completed => ("已完成", "rgba(82,196,26,0.08)", "var(--success)"),
    }
}

fn parse_status(s: &str) -> Option<MRBStatus> {
    match s {
        "Draft" => Some(MRBStatus::Draft),
        "UnderReview" => Some(MRBStatus::UnderReview),
        "Approved" => Some(MRBStatus::Approved),
        "Completed" => Some(MRBStatus::Completed),
        _ => None,
    }
}

async fn resolve_product_names<S: ProductService>(
    svc: &S,
    ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    product_ids: &[i64],
) -> HashMap<i64, String> {
    let mut map = HashMap::new();
    if product_ids.is_empty() {
        return map;
    }
    let unique_ids: Vec<i64> = {
        let mut v = product_ids.to_vec();
        v.sort_unstable();
        v.dedup();
        v
    };
    if let Ok(products) = svc.get_by_ids(ctx, db, unique_ids).await {
        for p in products {
            map.insert(p.product_id, p.pdt_name);
        }
    }
    map
}

async fn resolve_result_doc_numbers<S: InspectionResultService>(
    svc: &S,
    ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    result_ids: &[i64],
) -> HashMap<i64, String> {
    let mut map = HashMap::new();
    let mut unique_ids: Vec<i64> = result_ids.to_vec();
    unique_ids.sort_unstable();
    unique_ids.dedup();
    for id in unique_ids {
        if let Ok(r) = svc.get(ctx, db, id).await {
            map.insert(id, r.doc_number);
        }
    }
    map
}

fn build_query_string(params: &MrbQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref v) = params.keyword {
        q.push(format!("keyword={v}"));
    }
    if let Some(v) = params.disposition {
        q.push(format!("disposition={v}"));
    }
    if let Some(v) = params.responsible_party {
        q.push(format!("responsible_party={v}"));
    }
    if let Some(ref v) = params.status {
        q.push(format!("status={v}"));
    }
    if let Some(ref v) = params.date_from {
        q.push(format!("date_from={v}"));
    }
    if let Some(ref v) = params.date_to {
        q.push(format!("date_to={v}"));
    }
    q.join("&")
}

// ── Handlers ──

#[require_permission("QMS", "read")]
pub async fn get_list(
    _path: MrbListPath,
    ctx: RequestContext,
    Query(params): Query<MrbQueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_create = ctx.has_permission("INSPECTION", "create").await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.mrb_service();
    let product_svc = state.product_service();
    let result_svc = state.inspection_result_service();

    let filter = MrbFilter {
        status: params.status.as_deref().and_then(parse_status),
        disposition: params.disposition.and_then(MRBDisposition::from_i16),
        responsible_party: params.responsible_party.and_then(ResponsibleParty::from_i16),
        ..Default::default()
    };
    let page_num = params.page.unwrap_or(1);
    let result = svc
        .list(&service_ctx, &mut conn, filter, abt_core::shared::types::PageParams::new(page_num, 20))
        .await?;

    let product_ids: Vec<i64> = result.items.iter().map(|m| m.product_id).collect();
    let product_names = resolve_product_names(&product_svc, &service_ctx, &mut conn, &product_ids).await;

    let result_ids: Vec<i64> = result.items.iter().map(|m| m.inspection_result_id).collect();
    let result_doc_numbers = resolve_result_doc_numbers(&result_svc, &service_ctx, &mut conn, &result_ids).await;

    let content = mrb_list_page(&result, &product_names, &result_doc_numbers, &params, can_create);
    let page_html = admin_page(
        is_htmx, "MRB 不良评审", &claims, "quality", MrbListPath::PATH, "质量管理", None, content, &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

// ── Components ──

fn mrb_list_page(
    result: &PaginatedResult<Mrb>,
    product_names: &HashMap<i64, String>,
    result_doc_numbers: &HashMap<i64, String>,
    params: &MrbQueryParams,
    can_create: bool,
) -> Markup {
    html! {
        div {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "MRB 不良评审" }
                div class="flex gap-3" {
                    @if can_create {
                        a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-accent text-accent-on border-none hover:bg-accent-hover" href=(MrbCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建MRB"
                        }
                    }
                }
            }
            (mrb_table_fragment(result, product_names, result_doc_numbers, params))
        }
    }
}

fn mrb_table_fragment(
    result: &PaginatedResult<Mrb>,
    product_names: &HashMap<i64, String>,
    result_doc_numbers: &HashMap<i64, String>,
    params: &MrbQueryParams,
) -> Markup {
    let total_count = result.total;
    let selected_status = params.status.as_deref().unwrap_or("");

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "Draft".into(), label: "草稿", count: None },
        TabItem { value: "UnderReview".into(), label: "审批中", count: None },
        TabItem { value: "Approved".into(), label: "已批准", count: None },
        TabItem { value: "Completed".into(), label: "已完成", count: None },
    ];

    html! {
        div class="mrb-list-panel" {
            (status_tabs_with_param(MrbListPath::PATH, "#mrb-data-card", "#filter-form", tabs, selected_status, "status"))

            // ── Filter Bar ──
            form id="filter-form" class="flex items-center gap-3 mb-5 flex-wrap filter-form"
                hx-get=(MrbListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#mrb-data-card"
                hx-select="#mrb-data-card"
                hx-swap="outerHTML"
                hx-include="#filter-form"
                hx-push-url="true" {
                div class="relative flex-1 max-w-xs" {
                    (icon::search_icon("w-4 h-4"))
                    input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword"
                        style="width:180px"
                        placeholder="搜索单号…"
                        value=(params.keyword.as_deref().unwrap_or(""));
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="disposition" {
                    option value="" selected[params.disposition.is_none()] { "全部处置" }
                    option value="1" selected[params.disposition == Some(1)] { "报废" }
                    option value="2" selected[params.disposition == Some(2)] { "退货" }
                    option value="3" selected[params.disposition == Some(3)] { "降级" }
                    option value="4" selected[params.disposition == Some(4)] { "返工" }
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="responsible_party" {
                    option value="" selected[params.responsible_party.is_none()] { "全部责任方" }
                    option value="1" selected[params.responsible_party == Some(1)] { "内部" }
                    option value="2" selected[params.responsible_party == Some(2)] { "供应商" }
                    option value="3" selected[params.responsible_party == Some(3)] { "客户" }
                }
                input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="date" name="date_from"
                    style="max-width:160px"
                    value=(params.date_from.as_deref().unwrap_or(""));
                span style="color:var(--muted);font-size:13px" { "至" }
                input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="date" name="date_to"
                    style="max-width:160px"
                    value=(params.date_to.as_deref().unwrap_or(""));
            }

            // ── Data Table ──
            (mrb_data_card(result, product_names, result_doc_numbers, params))
        }
    }
}

fn mrb_data_card(
    result: &PaginatedResult<Mrb>,
    product_names: &HashMap<i64, String>,
    result_doc_numbers: &HashMap<i64, String>,
    params: &MrbQueryParams,
) -> Markup {
    let query = build_query_string(params);
    html! {
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" id="mrb-data-card" {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]-scroll" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "单号" }
                            th { "关联检验" }
                            th { "产品" }
                            th { "缺陷描述" }
                            th { "处置方式" }
                            th { "责任方" }
                            th { "成本影响" }
                            th { "状态" }
                        }
                    }
                    tbody {
                        @for item in &result.items {
                            @let (disp_label, disp_bg, disp_color) = disposition_label(&item.disposition);
                            @let (rp_label, rp_bg, rp_color) = responsible_party_label(&item.responsible_party);
                            @let (st_label, st_bg, st_color) = mrb_status_label(&item.status);
                            @let product_name = product_names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
                            @let result_doc = result_doc_numbers.get(&item.inspection_result_id).map(|s| s.as_str()).unwrap_or("—");
                            @let detail_path = MrbDetailPath { id: item.id };
                            tr style="cursor:pointer" onclick=(format!("location.href='{}'", detail_path.to_string())) {
                                td class="text-accent font-medium cursor-pointer mono" style="color:var(--accent)" { (item.doc_number) }
                                td class="mono" style="font-size:12px" { (result_doc) }
                                td { (product_name) }
                                td style="max-width:200px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap" { (item.defect_description) }
                                td {
                                    span style=(format!("display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", disp_bg, disp_color)) {
                                        (disp_label)
                                    }
                                }
                                td {
                                    span style=(format!("display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", rp_bg, rp_color)) {
                                        (rp_label)
                                    }
                                }
                                td class="mono" { (format!("¥{:.2}", item.cost_impact)) }
                                td {
                                    span style=(format!("display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", st_bg, st_color)) {
                                        (st_label)
                                    }
                                }
                            }
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="8" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无MRB记录"
                                }
                            }
                        }
                    }
                }
            }
            (pagination(MrbListPath::PATH, &query, result.total, result.page, result.total_pages))
        }
    }
}
