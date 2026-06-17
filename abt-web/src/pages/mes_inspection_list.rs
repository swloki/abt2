use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::production_inspection::{InspectionListItem, InspectionListFilter};
use abt_core::mes::production_inspection::ProductionInspectionService;
use abt_core::mes::enums::InspectionType;
use abt_core::shared::types::PaginatedResult;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_inspection::{InspectionCreatePath, InspectionListPath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

fn insp_type_label(t: &abt_core::mes::enums::InspectionType) -> &'static str {
    use abt_core::mes::enums::InspectionType::*;
    match t { FirstArticle => "首检", InProcess => "巡检", Final => "完工检" }
}

fn insp_result_label(r: &abt_core::mes::enums::InspectionResultType) -> (&'static str, &'static str, &'static str) {
    use abt_core::mes::enums::InspectionResultType::*;
    match r {
        Pass => ("合格", "rgba(82,196,26,0.08)", "var(--success)"),
        Fail => ("不合格", "rgba(245,63,63,0.06)", "#f53f3f"),
        Conditional => ("让步接收", "rgba(250,140,22,0.08)", "#fa8c16"),
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct InspectionQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub inspection_type: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

impl InspectionQueryParams {
    fn to_filter(&self) -> InspectionListFilter {
        let inspection_type = self.inspection_type.as_deref().and_then(|s| match s {
            "FirstArticle" => Some(InspectionType::FirstArticle),
            "InProcess" => Some(InspectionType::InProcess),
            "Final" => Some(InspectionType::Final),
            _ => None,
        });
        InspectionListFilter {
            keyword: self.keyword.clone(),
            inspection_type,
        }
    }
}

#[require_permission("INSPECTION", "read")]
pub async fn get_inspection_list(
    _path: InspectionListPath, ctx: RequestContext, Query(params): Query<InspectionQueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_create = ctx.has_permission("INSPECTION", "create").await;
    let RequestContext { mut conn, claims, state, service_ctx, .. } = ctx;
    let filter = params.to_filter();
    let page = params.page.unwrap_or(1);
    let svc = state.production_inspection_service();
    let result = svc.list_inspections(&service_ctx, &mut conn, filter, page, 20).await?;
    let content = inspection_list_page(&result, &params, can_create);
    Ok(Html(admin_page(is_htmx, "生产报检", &claims, "production", InspectionListPath::PATH, "生产管理", None, content, &nav_filter).into_string()))
}

fn inspection_list_page(
    result: &PaginatedResult<InspectionListItem>,
    params: &InspectionQueryParams,
    can_create: bool,
) -> Markup {
    html! { div {
        div class="flex items-center justify-between mb-6" { h1 class="text-xl font-bold text-fg tracking-tight" { "生产报检" } div class="flex gap-3" {
            @if can_create {
                a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-accent text-accent-on border-none hover:bg-accent-hover" href=(InspectionCreatePath::PATH) { (icon::plus_icon("w-4 h-4")) "新建检验" }
            }
        }}
        (inspection_table_fragment(result, params))
    }}
}

fn inspection_table_fragment(
    result: &PaginatedResult<InspectionListItem>,
    params: &InspectionQueryParams,
) -> Markup {
    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(result.total) },
        TabItem { value: "FirstArticle".into(), label: "首检", count: None },
        TabItem { value: "InProcess".into(), label: "巡检", count: None },
        TabItem { value: "Final".into(), label: "完工检", count: None },
    ];
    let sel = params.inspection_type.as_deref().unwrap_or("");

    html! { div {
        (status_tabs_with_param(InspectionListPath::PATH, "#insp-data-card", "#filter-form", tabs, sel, "inspection_type"))
        form id="filter-form" class="flex items-center gap-3 mb-5 flex-wrap filter-form" hx-get=(InspectionListPath::PATH)
            hx-trigger="change, keyup changed delay:300ms from:.search-input"
            hx-target="#insp-data-card" hx-select="#insp-data-card" hx-swap="outerHTML" hx-include="#filter-form"
                hx-push-url="true" {
            div class="relative flex-1 max-w-xs" { (icon::search_icon("w-4 h-4"))
                input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword" style="width:180px" placeholder="搜索报检单号…" value=(params.keyword.as_deref().unwrap_or(""));
            }
        }
        (inspection_data_card(result, params))
    }}
}

fn inspection_data_card(
    result: &PaginatedResult<InspectionListItem>,
    params: &InspectionQueryParams,
) -> Markup {
    let mut qs = vec![];
    if let Some(k) = &params.keyword { qs.push(format!("keyword={k}")); }
    if let Some(t) = &params.inspection_type { qs.push(format!("inspection_type={t}")); }
    let query = qs.join("&");

    html! {
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" id="insp-data-card" {
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] overflow-x-auto" {
                table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" { thead { tr {
                    th { "单号" } th { "工单" } th { "类型" } th { "产品" }
                    th class="text-right text-[13px]" { "样本" } th class="text-right text-[13px]" { "合格" } th { "结果" } th class="text-right" { "操作" }
                }} tbody {
                    @for item in &result.items {
                        @let tl = insp_type_label(&item.inspection_type);
                        @let (rl, rb, rc) = insp_result_label(&item.result);
                        @let dp = format!("/admin/mes/inspections/{}", item.id);
                        tr style="cursor:pointer" onclick=(format!("location.href='{}'", dp)) {
                            td class="text-accent font-medium cursor-pointer font-mono tabular-nums" style="color:var(--accent)" { (item.doc_number) }
                            td class="font-mono tabular-nums" { (item.work_order_doc.as_deref().unwrap_or("\u{2014}")) }
                            td { (tl) }
                            td { (item.product_name.as_deref().unwrap_or("\u{2014}")) }
                            td class="text-right text-[13px] font-mono tabular-nums" { (crate::utils::fmt_qty(item.sample_qty)) }
                            td class="text-right text-[13px] font-mono tabular-nums" { (crate::utils::fmt_qty(item.qualified_qty)) }
                            td { span style=(format!("display:inline-flex;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", rb, rc)) { (rl) } }
                            td { a href=(dp) style="color:var(--accent);font-size:var(--text-xs)" { "查看" } }
                        }
                    }
                    @if result.items.is_empty() {
                        tr { td colspan="8" style="text-align:center;padding:var(--space-8);color:var(--muted)" { "暂无检验记录" } }
                    }
                }}
            }
            (pagination(InspectionListPath::PATH, &query, result.total, result.page, result.total_pages))
        }
    }
}
