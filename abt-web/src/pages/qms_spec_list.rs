use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::qms::enums::{InspectionType, SpecStatus};
use abt_core::qms::inspection_specification::model::{
    InspectionSpecFilter, InspectionSpecification,
};
use abt_core::qms::inspection_specification::InspectionSpecificationService;

use abt_core::shared::types::PaginatedResult;
use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::qms::{SpecCreatePath, SpecDetailPath, SpecListPath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct SpecQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub inspection_type: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Helpers ──

fn spec_status_label(s: &SpecStatus) -> (&'static str, &'static str, &'static str) {
    match s {
        SpecStatus::Draft => ("草稿", "rgba(0,0,0,0.04)", "var(--muted)"),
        SpecStatus::Active => ("生效", "rgba(82,196,26,0.08)", "var(--success)"),
        SpecStatus::Inactive => ("停用", "rgba(245,63,63,0.06)", "#f53f3f"),
    }
}

fn inspection_type_label(t: &InspectionType) -> (&'static str, &'static str, &'static str) {
    match t {
        InspectionType::Iqc => ("IQC", "rgba(37,99,235,0.08)", "var(--accent)"),
        InspectionType::Ipqc => ("IPQC", "rgba(22,163,74,0.08)", "var(--success)"),
        InspectionType::Fqc => ("FQC", "rgba(124,58,237,0.08)", "#7c3aed"),
        InspectionType::Oqc => ("OQC", "rgba(234,88,12,0.08)", "#ea580c"),
    }
}

fn parse_status(s: &str) -> Option<SpecStatus> {
    match s {
        "Draft" => Some(SpecStatus::Draft),
        "Active" => Some(SpecStatus::Active),
        "Inactive" => Some(SpecStatus::Inactive),
        _ => None,
    }
}

fn parse_inspection_type(s: &str) -> Option<InspectionType> {
    match s {
        "Iqc" => Some(InspectionType::Iqc),
        "Ipqc" => Some(InspectionType::Ipqc),
        "Fqc" => Some(InspectionType::Fqc),
        "Oqc" => Some(InspectionType::Oqc),
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
    let unique_ids: Vec<i64> = product_ids
        .iter()
        .filter(|id| !map.contains_key(*id))
        .copied()
        .collect();
    if unique_ids.is_empty() {
        return map;
    }
    if let Ok(products) = svc.get_by_ids(ctx, db, unique_ids).await {
        for p in products {
            map.insert(p.product_id, p.pdt_name);
        }
    }
    map
}

fn build_query_string(params: &SpecQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref v) = params.keyword {
        q.push(format!("keyword={v}"));
    }
    if let Some(ref v) = params.inspection_type {
        q.push(format!("inspection_type={v}"));
    }
    if let Some(ref v) = params.status {
        q.push(format!("status={v}"));
    }
    q.join("&")
}

// ── Handlers ──

#[require_permission("QMS", "read")]
pub async fn get_list(
    _path: SpecListPath,
    ctx: RequestContext,
    Query(params): Query<SpecQueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_create = ctx.has_permission("INSPECTION", "create").await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.inspection_specification_service();
    let product_svc = state.product_service();

    let filter = InspectionSpecFilter {
        product_id: None,
        inspection_type: params.inspection_type.as_deref().and_then(parse_inspection_type),
        status: params.status.as_deref().and_then(parse_status),
        keyword: params.keyword.clone(),
    };
    let page_num = params.page.unwrap_or(1);
    let result = svc
        .list(&service_ctx, &mut conn, filter, abt_core::shared::types::PageParams::new(page_num, 20))
        .await?;

    let product_ids: Vec<i64> = result.items.iter().map(|s| s.product_id).collect();
    let product_names = resolve_product_names(&product_svc, &service_ctx, &mut conn, &product_ids).await;

    let content = spec_list_page(&result, &product_names, &params, can_create);
    let page_html = admin_page(
        is_htmx,
        "检验规格",
        &claims,
        "quality",
        SpecListPath::PATH,
        "质量管理",
        None,
        content, &nav_filter,    );
    Ok(Html(page_html.into_string()))
}

// ── Components ──

fn spec_list_page(
    result: &PaginatedResult<InspectionSpecification>,
    product_names: &HashMap<i64, String>,
    params: &SpecQueryParams,
    can_create: bool,
) -> Markup {
    html! {
        div {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "检验规格" }
                div class="flex gap-3" {
                    @if can_create {
                        a class="btn btn-primary" href=(SpecCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建规格"
                        }
                    }
                }
            }
            (spec_table_fragment(result, product_names, params))
        }
    }
}

fn spec_table_fragment(
    result: &PaginatedResult<InspectionSpecification>,
    product_names: &HashMap<i64, String>,
    params: &SpecQueryParams,
) -> Markup {
    let total_count = result.total;
    let selected_status = params.status.as_deref().unwrap_or("");

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "Draft".into(), label: "草稿", count: None },
        TabItem { value: "Active".into(), label: "生效", count: None },
        TabItem { value: "Inactive".into(), label: "停用", count: None },
    ];

    html! {
        div class="spec-list-panel" {
            (status_tabs_with_param(SpecListPath::PATH, "#spec-data-card", "#filter-form", tabs, selected_status, "status"))

            // ── Filter Bar ──
            form id="filter-form" class="flex items-center gap-3 mb-5 flex-wrap filter-form"
                hx-get=(SpecListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#spec-data-card"
                hx-select="#spec-data-card"
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
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="inspection_type" {
                    option value="" selected[params.inspection_type.is_none()] { "全部类型" }
                    option value="Iqc" selected[params.inspection_type.as_deref() == Some("Iqc")] { "IQC (来料)" }
                    option value="Ipqc" selected[params.inspection_type.as_deref() == Some("Ipqc")] { "IPQC (过程)" }
                    option value="Fqc" selected[params.inspection_type.as_deref() == Some("Fqc")] { "FQC (成品)" }
                    option value="Oqc" selected[params.inspection_type.as_deref() == Some("Oqc")] { "OQC (出货)" }
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="status" {
                    option value="" selected[params.status.is_none()] { "全部状态" }
                    option value="Draft" selected[params.status.as_deref() == Some("Draft")] { "草稿" }
                    option value="Active" selected[params.status.as_deref() == Some("Active")] { "生效" }
                    option value="Inactive" selected[params.status.as_deref() == Some("Inactive")] { "停用" }
                }
            }

            // ── Data Table ──
            (spec_data_card(result, product_names, params))
        }
    }
}

fn spec_data_card(
    result: &PaginatedResult<InspectionSpecification>,
    product_names: &HashMap<i64, String>,
    params: &SpecQueryParams,
) -> Markup {
    let query = build_query_string(params);
    html! {
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" id="spec-data-card" {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]-scroll" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "单号" }
                            th { "产品" }
                            th { "检验类型" }
                            th { "检验项目数" }
                            th { "抽样方案" }
                            th { "版本" }
                            th { "状态" }
                            th { "创建时间" }
                        }
                    }
                    tbody {
                        @for item in &result.items {
                            @let (status_label, status_bg, status_color) = spec_status_label(&item.status);
                            @let (type_label, type_bg, type_color) = inspection_type_label(&item.inspection_type);
                            @let product_name = product_names.get(&item.product_id).map(|s| s.as_str()).unwrap_or("—");
                            @let detail_path = SpecDetailPath { id: item.id };
                            @let check_count = item.check_items.len();
                            @let sample_plan = format!("Level {}, AQL {}", item.sample_plan.level, item.sample_plan.aql);
                            tr style="cursor:pointer" onclick=(format!("location.href='{}'", detail_path.to_string())) {
                                td class="mono" style="color:var(--accent)" { (item.doc_number) }
                                td { (product_name) }
                                td {
                                    span style=(format!("display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", type_bg, type_color)) {
                                        (type_label)
                                    }
                                }
                                td style="text-align:center" { (check_count) }
                                td { (sample_plan) }
                                td style="text-align:center" { "v" (item.version) }
                                td {
                                    span style=(format!("display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", status_bg, status_color)) {
                                        (status_label)
                                    }
                                }
                                td style="font-size:12px;color:var(--muted)" { (item.created_at.format("%Y-%m-%d %H:%M")) }
                            }
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="8" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无检验规格"
                                }
                            }
                        }
                    }
                }
            }
            (pagination(SpecListPath::PATH, &query, result.total, result.page, result.total_pages))
        }
    }
}
