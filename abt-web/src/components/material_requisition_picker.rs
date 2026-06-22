use axum::routing::get;
use axum::Router;
use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::wms::material_requisition::MaterialRequisitionService;
use abt_core::wms::material_requisition::model::RequisitionFilter;

use crate::errors::Result;
use crate::utils::RequestContext;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/api/material-requisitions/search")]
pub struct MaterialRequisitionSearchPath;

#[derive(Debug, Deserialize)]
pub struct SearchMrParams {
    pub keyword: Option<String>,
    pub status: Option<i16>,
    pub target_id: Option<String>,
    pub display_id: Option<String>,
}

pub fn router() -> Router<crate::state::AppState> {
    Router::new().route(MaterialRequisitionSearchPath::PATH, get(search_material_requisitions))
}

pub async fn search_material_requisitions(
    ctx: RequestContext,
    Query(params): Query<SearchMrParams>,
) -> Result<Html<String>> {
    use abt_core::wms::enums::RequisitionStatus;
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.material_requisition_service();
    let status = params.status.and_then(|s| if s == -1 { None } else { RequisitionStatus::from_i16(s) });
    let result = svc
        .list(
            &service_ctx, &mut conn,
            RequisitionFilter {
                doc_number: params.keyword.filter(|s| !s.is_empty()),
                status,
                work_order_id: None,
                warehouse_id: None,
            },
            1, 30,
        )
        .await?;
    let target = params.target_id.as_deref().unwrap_or("mr-id-hidden");
    let display = params.display_id.as_deref().unwrap_or("mr-display");
    Ok(Html(mr_picker_results(&result.items, target, display).into_string()))
}

/// 领料单选择弹窗（fill-input：选领料单→填 hidden target_id + 显示 doc_number + trigger change + 关弹窗）
/// 调用方的 hidden 自带 hx-trigger="change" → change 后自动 hx-post 加载领料明细（confirm-requisition 渲染明细）
pub fn material_requisition_picker_modal(modal_id: &str, target_id: &str, display_id: &str) -> Markup {
    let close_hs = format!("on click remove .is-open from #{}", modal_id);
    html! {
        div class="fixed inset-0 z-[1100] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto"
            id=(modal_id)
            _=(close_hs)
        {
            div class="bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl"
                _="on click halt the event"
            {
                div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0"
                {
                    h2 class="text-lg font-semibold m-0" { "选择领料单" }
                    button
                        class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg transition-colors"
                        _=(close_hs)
                    { "×" }
                }
                div class="overflow-y-auto flex-1 min-h-0 p-6" {
                    div class="mr-search-bar flex gap-4 mb-4 pb-4 border-b border-border-soft" {
                        input type="hidden" name="target_id" value=(target_id);
                        input type="hidden" name="display_id" value=(display_id);
                        div class="flex-1 flex flex-col gap-1" {
                            label class="text-xs font-medium text-fg-2" { "领料单号" }
                            input
                                class="mr-search-input w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                                type="text"
                                name="keyword"
                                placeholder="领料单号…"
                                hx-get=(MaterialRequisitionSearchPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-sync="this:replace"
                                hx-target="#mr-search-results"
                                hx-swap="innerHTML"
                                hx-include=".mr-search-bar" {}
                        }
                        div class="w-[140px] flex flex-col gap-1" {
                            label class="text-xs font-medium text-fg-2" { "状态" }
                            select
                                class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                                name="status"
                                hx-get=(MaterialRequisitionSearchPath::PATH)
                                hx-trigger="change"
                                hx-target="#mr-search-results"
                                hx-swap="innerHTML"
                                hx-include=".mr-search-bar"
                            {
                                option value="-1" { "全部" }
                                option value="2" { "已确认" }
                                option value="5" { "部分发料" }
                            }
                        }
                    }
                    div id="mr-search-results"
                        class="max-h-[400px] overflow-y-auto"
                        hx-get=(MaterialRequisitionSearchPath::PATH)
                        hx-trigger="intersect once"
                        hx-swap="innerHTML"
                        hx-vals=({
                            format!(
                                "{{\"target_id\":\"{}\",\"display_id\":\"{}\"}}",
                                target_id,
                                display_id,
                            )
                        })
                    {
                        div class="flex items-center justify-center py-8 text-muted text-sm" {
                            "加载中…"
                        }
                    }
                }
            }
        }
    }
}

fn mr_picker_results(
    items: &[abt_core::wms::material_requisition::MaterialRequisition],
    target_id: &str,
    display_id: &str,
) -> Markup {
    use abt_core::wms::enums::RequisitionStatus;
    let status_label = |s: &RequisitionStatus| -> &'static str {
        match s {
            RequisitionStatus::Draft => "草稿",
            RequisitionStatus::Confirmed => "已确认",
            RequisitionStatus::Issued => "已发料",
            RequisitionStatus::Cancelled => "已取消",
            RequisitionStatus::PartiallyIssued => "部分发料",
        }
    };
    let click_hs = format!(
        "on click set #{}'s value to my @data-mid then set #{}'s value to my @data-mnum then trigger change on #{} then remove .is-open from closest .is-open",
        target_id, display_id, target_id
    );
    html! {
        @if items.is_empty() {
            div class="flex flex-col items-center justify-center py-12 text-muted" {
                p class="mt-2 text-sm" { "未找到匹配的领料单" }
                p class="text-xs mt-1" { "仅「已确认 / 部分发料」状态可出库" }
            }
        } @else {
            div class="py-2" {
                @for mr in items {
                    @let sl = status_label(&mr.status);
                    div class="flex items-center justify-between p-3 border-b border-border-soft cursor-pointer hover:bg-accent-bg transition-colors"
                        data-mid=(mr.id)
                        data-mnum=(mr.doc_number.as_str())
                        _=(click_hs.clone())
                    {
                        div class="min-w-0" {
                            div class="text-sm font-medium text-fg truncate" { (mr.doc_number) }
                            div class="text-xs text-muted" {
                                "工单 #"
                                (mr.work_order_id)
                                " · "
                                (sl)
                                " · "
                                (mr.requisition_date.format("%Y-%m-%d").to_string())
                            }
                        }
                        span class="text-xs text-accent font-medium shrink-0" { "选择" }
                    }
                }
            }
        }
    }
}
