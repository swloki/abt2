use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::qms::enums::{
    InspectionResultType, InspectionSourceType, InspectionStatus, InspectionType,
};
use abt_core::qms::inspection_result::model::{InspectionResult, InspectionResultFilter};
use abt_core::qms::inspection_result::InspectionResultService;
use abt_core::qms::inspection_specification::InspectionSpecificationService;
use abt_core::shared::types::PaginatedResult;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::qms::{ResultCreatePath, ResultDetailPath, ResultListPath};
use crate::utils::{empty_as_none, fmt_qty, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ResultQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub inspection_type: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub result: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub source_type: Option<i16>,
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

fn status_label(s: &InspectionStatus) -> (&'static str, &'static str, &'static str) {
    match s {
        InspectionStatus::Pending => ("待检验", "rgba(250,140,22,0.08)", "#fa8c16"),
        InspectionStatus::Completed => ("已完成", "rgba(82,196,26,0.08)", "var(--success)"),
        InspectionStatus::Dispositioned => ("已处置", "rgba(114,46,209,0.08)", "#722ed1"),
    }
}

fn result_type_label(r: &InspectionResultType) -> (&'static str, &'static str, &'static str) {
    match r {
        InspectionResultType::Pass => ("合格", "rgba(82,196,26,0.08)", "var(--success)"),
        InspectionResultType::Fail => ("不合格", "rgba(245,63,63,0.06)", "#f53f3f"),
        InspectionResultType::Conditional => ("让步接收", "rgba(22,119,255,0.08)", "var(--accent)"),
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

fn source_type_label(s: &InspectionSourceType) -> &'static str {
    match s {
        InspectionSourceType::ArrivalNotice => "来料通知",
        InspectionSourceType::WorkOrderRouting => "工单工序",
        InspectionSourceType::ShippingRequest => "发货单",
        InspectionSourceType::OutsourcingOrder => "委外单",
        InspectionSourceType::ProductionReceipt => "完工入库",
    }
}

fn parse_status(s: &str) -> Option<InspectionStatus> {
    match s {
        "Pending" => Some(InspectionStatus::Pending),
        "Completed" => Some(InspectionStatus::Completed),
        "Dispositioned" => Some(InspectionStatus::Dispositioned),
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

fn parse_result_type(s: &str) -> Option<InspectionResultType> {
    match s {
        "Pass" => Some(InspectionResultType::Pass),
        "Fail" => Some(InspectionResultType::Fail),
        "Conditional" => Some(InspectionResultType::Conditional),
        _ => None,
    }
}

async fn resolve_product_names<S: InspectionSpecificationService, P: ProductService>(
    spec_svc: &S,
    product_svc: &P,
    ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    items: &[InspectionResult],
) -> HashMap<i64, String> {
    // spec_id → product_id
    let mut spec_to_product: HashMap<i64, i64> = HashMap::new();
    for item in items {
        if !spec_to_product.contains_key(&item.spec_id)
            && let Ok(spec) = spec_svc.get(ctx, db, item.spec_id).await {
                spec_to_product.insert(item.spec_id, spec.product_id);
            }
    }
    // Batch fetch products
    let product_ids: Vec<i64> = spec_to_product.values().copied().collect();
    let mut product_names: HashMap<i64, String> = HashMap::new();
    if let Ok(products) = product_svc.get_by_ids(ctx, db, product_ids).await {
        for p in products {
            product_names.insert(p.product_id, p.pdt_name);
        }
    }
    // spec_id → product_name
    let mut result = HashMap::new();
    for (spec_id, product_id) in &spec_to_product {
        if let Some(name) = product_names.get(product_id) {
            result.insert(*spec_id, name.clone());
        }
    }
    result
}

fn build_filter(params: &ResultQueryParams) -> InspectionResultFilter {
    InspectionResultFilter {
        source_type: params.source_type.and_then(InspectionSourceType::from_i16),
        source_id: None,
        inspection_type: params.inspection_type.as_deref().and_then(parse_inspection_type),
        result: params.result.as_deref().and_then(parse_result_type),
        status: params.status.as_deref().and_then(parse_status),
        date_from: params.date_from.as_deref().and_then(|d| d.parse().ok()),
        date_to: params.date_to.as_deref().and_then(|d| d.parse().ok()),
    }
}

fn build_query_string(params: &ResultQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref v) = params.keyword
        && !v.is_empty() {
            q.push(format!("keyword={v}"));
        }
    if let Some(ref v) = params.inspection_type {
        q.push(format!("inspection_type={v}"));
    }
    if let Some(ref v) = params.result {
        q.push(format!("result={v}"));
    }
    if let Some(v) = params.source_type {
        q.push(format!("source_type={v}"));
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
    _path: ResultListPath,
    ctx: RequestContext,
    Query(params): Query<ResultQueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_create = ctx.has_permission("INSPECTION", "create").await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.inspection_result_service();
    let spec_svc = state.inspection_specification_service();
    let product_svc = state.product_service();

    let filter = build_filter(&params);
    let page_num = params.page.unwrap_or(1);
    let result = svc
        .list_by_source(
            &service_ctx,
            &mut conn,
            filter,
            abt_core::shared::types::PageParams::new(page_num, 20),
        )
        .await?;

    let product_names =
        resolve_product_names(&spec_svc, &product_svc, &service_ctx, &mut conn, &result.items)
            .await;

    let content = result_list_page(&result, &product_names, &params, can_create);
    let page_html = admin_page(
        is_htmx,
        "检验结果",
        &claims,
        "quality",
        ResultListPath::PATH,
        "质量管理",
        None,
        content, &nav_filter,    );
    Ok(Html(page_html.into_string()))
}

// ── Components ──

fn result_list_page(
    result: &PaginatedResult<InspectionResult>,
    product_names: &HashMap<i64, String>,
    params: &ResultQueryParams,
    can_create: bool,
) -> Markup {
    html! {
        div {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "检验结果" }
                div class="flex gap-3" {
                    @if can_create {
                        a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-accent text-accent-on border-none hover:bg-accent-hover" href=(ResultCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "记录检验"
                        }
                    }
                }
            }
            (result_table_fragment(result, product_names, params))
        }
    }
}

fn result_table_fragment(
    result: &PaginatedResult<InspectionResult>,
    product_names: &HashMap<i64, String>,
    params: &ResultQueryParams,
) -> Markup {
    let total_count = result.total;
    let selected_status = params.status.as_deref().unwrap_or("");

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "Pending".into(), label: "待检验", count: None },
        TabItem { value: "Completed".into(), label: "已完成", count: None },
        TabItem { value: "Dispositioned".into(), label: "已处置", count: None },
    ];

    html! {
        div class="result-list-panel" {
            (status_tabs_with_param(ResultListPath::PATH, "#result-data-card", "#filter-form", tabs, selected_status, "status"))

            // ── Filter Bar ──
            form id="filter-form" class="flex items-center gap-3 mb-5 flex-wrap filter-form"
                hx-get=(ResultListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#result-data-card"
                hx-select="#result-data-card"
                hx-swap="outerHTML"
                hx-include="#filter-form"
                hx-push-url="true" {
                div class="relative flex-1 max-w-xs" {
                    (icon::search_icon("w-4 h-4"))
                    input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword"
                        style="width:160px"
                        placeholder="搜索单号…"
                        value=(params.keyword.as_deref().unwrap_or(""));
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="inspection_type" {
                    option value="" selected[params.inspection_type.is_none()] { "全部类型" }
                    option value="Iqc" selected[params.inspection_type.as_deref() == Some("Iqc")] { "IQC" }
                    option value="Ipqc" selected[params.inspection_type.as_deref() == Some("Ipqc")] { "IPQC" }
                    option value="Fqc" selected[params.inspection_type.as_deref() == Some("Fqc")] { "FQC" }
                    option value="Oqc" selected[params.inspection_type.as_deref() == Some("Oqc")] { "OQC" }
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="result" {
                    option value="" selected[params.result.is_none()] { "全部结果" }
                    option value="Pass" selected[params.result.as_deref() == Some("Pass")] { "合格" }
                    option value="Fail" selected[params.result.as_deref() == Some("Fail")] { "不合格" }
                    option value="Conditional" selected[params.result.as_deref() == Some("Conditional")] { "让步接收" }
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="source_type" {
                    option value="" selected[params.source_type.is_none()] { "全部来源" }
                    option value="1" selected[params.source_type == Some(1)] { "来料通知" }
                    option value="2" selected[params.source_type == Some(2)] { "工单工序" }
                    option value="3" selected[params.source_type == Some(3)] { "发货单" }
                    option value="4" selected[params.source_type == Some(4)] { "委外单" }
                }
                input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="date" name="date_from"
                    style="max-width:140px"
                    value=(params.date_from.as_deref().unwrap_or(""));
                span style="color:var(--muted);font-size:13px" { "至" }
                input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="date" name="date_to"
                    style="max-width:140px"
                    value=(params.date_to.as_deref().unwrap_or(""));
            }

            // ── Data Table ──
            (result_data_card(result, product_names, params))
        }
    }
}

fn result_data_card(
    result: &PaginatedResult<InspectionResult>,
    product_names: &HashMap<i64, String>,
    params: &ResultQueryParams,
) -> Markup {
    let query = build_query_string(params);
    html! {
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" id="result-data-card" {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] overflow-x-auto" {
                table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:text-left [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" {
                    thead {
                        tr {
                            th { "单号" }
                            th { "检验类型" }
                            th { "来源类型" }
                            th { "产品" }
                            th { "批次号" }
                            th { "抽样/合格/不合格" }
                            th { "结果" }
                            th { "状态" }
                            th { "检验日期" }
                        }
                    }
                    tbody {
                        @for item in &result.items {
                            @let (type_label, type_bg, type_color) = inspection_type_label(&item.inspection_type);
                            @let src_label = source_type_label(&item.source_type);
                            @let product_name = product_names.get(&item.spec_id).map(|s| s.as_str()).unwrap_or("—");
                            @let detail_path = ResultDetailPath { id: item.id };
                            @let (r_label, r_bg, r_color) = result_type_label(&item.result);
                            @let (s_label, s_bg, s_color) = status_label(&item.status);
                            @let qty_display = format!(
                                "{} / {} / {}",
                                fmt_qty(item.sample_qty),
                                fmt_qty(item.qualified_qty),
                                fmt_qty(item.unqualified_qty),
                            );
                            @let date_display = item.inspection_date
                                .map(|d| d.format("%Y-%m-%d").to_string())
                                .unwrap_or_else(|| "—".into());
                            tr style="cursor:pointer" onclick=(format!("location.href='{}'", detail_path.to_string())) {
                                td class="font-mono tabular-nums" style="color:var(--accent)" { (item.doc_number) }
                                td {
                                    span style=(format!("display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", type_bg, type_color)) {
                                        (type_label)
                                    }
                                }
                                td { (src_label) }
                                td { (product_name) }
                                td class="font-mono tabular-nums" { (item.batch_no) }
                                td class="font-mono tabular-nums text-right text-[13px]" { (qty_display) }
                                td {
                                    span style=(format!("display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", r_bg, r_color)) {
                                        (r_label)
                                    }
                                }
                                td {
                                    span style=(format!("display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", s_bg, s_color)) {
                                        (s_label)
                                    }
                                }
                                td style="font-size:12px;color:var(--muted)" { (date_display) }
                            }
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="9" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无检验结果"
                                }
                            }
                        }
                    }
                }
            }
            (pagination(ResultListPath::PATH, &query, result.total, result.page, result.total_pages))
        }
    }
}
