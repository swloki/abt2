use axum::routing::get;
use axum::Router;
use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use super::overlay::modal_shell;
use abt_core::wms::enums::{PickingStatus, PickingType};
use abt_core::wms::picking::{PickingFilter, PickingService, StockPicking};

use crate::errors::Result;
use crate::utils::RequestContext;

// 路径名保留 /api/material-requisitions/search（前端 hx-get 调用方不变），内部已切 stock_picking
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
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.picking_service();
    let status = params
        .status
        .and_then(|s| if s == -1 { None } else { PickingStatus::from_i16(s) });
    let result = svc
        .list(
            &service_ctx,
            &mut conn,
            PickingFilter {
                doc_number: params.keyword.filter(|s| !s.is_empty()),
                picking_type: Some(PickingType::InternalIssue),
                status,
                source_type: None,
                source_id: None,
                work_order_id: None,
                partner_id: None,
            },
            abt_core::shared::types::pagination::PageParams::new(1, 30),
        )
        .await?;
    let target = params.target_id.as_deref().unwrap_or("mr-id-hidden");
    let display = params.display_id.as_deref().unwrap_or("mr-display");
    Ok(Html(mr_picker_results(&result.items, target, display).into_string()))
}

/// 领料单选择弹窗（fill-input：选领料单→填 hidden target_id + 显示 doc_number + trigger change + 关弹窗）
pub fn material_requisition_picker_modal(modal_id: &str, target_id: &str, display_id: &str) -> Markup {
    let close_hs = format!("on click remove .is-open from #{}", modal_id);
    modal_shell(modal_id, "z-[1100]", html! {
        div class="bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl"
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
                                option value="3" { "已完成" }
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
        })
}

fn mr_picker_results(items: &[StockPicking], target_id: &str, display_id: &str) -> Markup {
    let status_label = |s: &PickingStatus| -> &'static str {
        match s {
            PickingStatus::Draft => "草稿",
            PickingStatus::Confirmed => "已确认",
            PickingStatus::Done => "已完成",
            PickingStatus::Cancelled => "已取消",
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
                p class="text-xs mt-1" { "仅「已确认」状态可发料" }
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
                                (mr.work_order_id.unwrap_or(0))
                                " · "
                                (sl)
                                " · "
                                (mr.scheduled_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into()))
                            }
                        }
                        span class="text-xs text-accent font-medium shrink-0" { "选择" }
                    }
                }
            }
        }
    }
}
