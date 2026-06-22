use axum::routing::get;
use axum::Router;
use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::work_order::WorkOrderService;

use crate::errors::Result;
use crate::utils::RequestContext;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/api/work-orders/search")]
pub struct WorkOrderSearchPath;

#[derive(Debug, Deserialize)]
pub struct SearchWoParams {
 pub keyword: Option<String>,
 pub product_code: Option<String>,
 pub status: Option<i16>,
 pub target_id: Option<String>,
 pub display_id: Option<String>,
}

pub fn router() -> Router<crate::state::AppState> {
 Router::new().route(WorkOrderSearchPath::PATH, get(search_work_orders))
}

pub async fn search_work_orders(
 ctx: RequestContext,
 Query(params): Query<SearchWoParams>,
) -> Result<Html<String>> {
 use abt_core::mes::WorkOrderStatus;
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.work_order_service();
 let status = params.status.and_then(|s| if s == -1 { None } else { WorkOrderStatus::from_i16(s) });
 let result = svc.list(
 &service_ctx, &mut conn,
 abt_core::mes::work_order::WorkOrderFilter {
 status,
 keyword: params.keyword.filter(|s| !s.is_empty()),
 product_code: params.product_code.filter(|s| !s.is_empty()),
 ..Default::default()
 },
 1, 30,
 ).await?;
 let target = params.target_id.as_deref().unwrap_or("work_order_id");
 let display = params.display_id.as_deref().unwrap_or("wo-display");
 Ok(Html(wo_picker_results(&result.items, target, display).into_string()))
}

/// 工单选择弹窗（fill-input：选工单→填 hidden target_id + 显示 doc_number + trigger change + 关弹窗）
/// 调用方的 hidden 自带 hx-trigger="change" → change 后自动 hx-get/hx-post 加载后续（om 加载工序 / 入库单 confirm-wo 渲染明细）
pub fn work_order_picker_modal(modal_id: &str, target_id: &str, display_id: &str) -> Markup {
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
                h2 class="text-lg font-semibold m-0" { "选择工单" }
                button
                    class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg transition-colors"
                    _=(close_hs)
                { "×" }
            }
            div class="overflow-y-auto flex-1 min-h-0 p-6" {
                div class="wo-search-bar flex gap-4 mb-4 pb-4 border-b border-border-soft" {
                    input type="hidden" name="target_id" value=(target_id);
                    input type="hidden" name="display_id" value=(display_id);
                    div class="flex-1 flex flex-col gap-1" {
                        label class="text-xs font-medium text-fg-2" { "工单号" }
                        input
                            class="wo-search-input w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            type="text"
                            name="keyword"
                            placeholder="工单号…"
                            hx-get=(WorkOrderSearchPath::PATH)
                            hx-trigger="keyup changed delay:300ms"
                            hx-sync="this:replace"
                            hx-target="#wo-search-results"
                            hx-swap="innerHTML"
                            hx-include=".wo-search-bar" {}
                    }
                    div class="flex-1 flex flex-col gap-1" {
                        label class="text-xs font-medium text-fg-2" { "产品编码" }
                        input
                            class="wo-search-input w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            type="text"
                            name="product_code"
                            placeholder="产品编码…"
                            hx-get=(WorkOrderSearchPath::PATH)
                            hx-trigger="keyup changed delay:300ms"
                            hx-sync="this:replace"
                            hx-target="#wo-search-results"
                            hx-swap="innerHTML"
                            hx-include=".wo-search-bar" {}
                    }
                    div class="w-[140px] flex flex-col gap-1" {
                        label class="text-xs font-medium text-fg-2" { "状态" }
                        select
                            class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            name="status"
                            hx-get=(WorkOrderSearchPath::PATH)
                            hx-trigger="change"
                            hx-target="#wo-search-results"
                            hx-swap="innerHTML"
                            hx-include=".wo-search-bar"
                        {
                            option value="-1" { "全部" }
                            option value="2" { "已计划" }
                            option value="3" { "已下达" }
                            option value="6" { "进行中" }
                            option value="4" { "已关闭" }
                        }
                    }
                }
                div id="wo-search-results"
                    class="max-h-[400px] overflow-y-auto"
                    hx-get=(WorkOrderSearchPath::PATH)
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
                    div class="flex items-center justify-center py-8 text-muted text-sm" { "加载中…" }
                }
            }
        }
    }
}
}

fn wo_picker_results(
 items: &[abt_core::mes::work_order::WorkOrder],
 target_id: &str,
 display_id: &str,
) -> Markup {
 use abt_core::mes::WorkOrderStatus;
 let status_label = |s: &WorkOrderStatus| -> &'static str {
 match s {
 WorkOrderStatus::Draft => "草稿",
 WorkOrderStatus::Planned => "已计划",
 WorkOrderStatus::Released => "已下达",
 WorkOrderStatus::InProduction => "进行中",
 WorkOrderStatus::Closed => "已关闭",
 WorkOrderStatus::Cancelled => "已取消",
 }
 };
 let click_hs = format!(
 "on click set #{}'s value to my @data-wid then set #{}'s value to my @data-wnum then trigger change on #{} then remove .is-open from closest .is-open",
 target_id, display_id, target_id
 );
 html! {
    @if items.is_empty() {
        div class="flex flex-col items-center justify-center py-12 text-muted" {
            p class="mt-2 text-sm" { "未找到匹配工单" }
        }
    } @else {
        div class="py-2" {
            @for wo in items {
                @let sl = status_label(&wo.status);
                div class="flex items-center justify-between p-3 border-b border-border-soft cursor-pointer hover:bg-accent-bg transition-colors"
                    data-wid=(wo.id)
                    data-wnum=(wo.doc_number.as_str())
                    _=(click_hs.clone())
                {
                    div class="min-w-0" {
                        div class="text-sm font-medium text-fg truncate" { (wo.doc_number) }
                        div class="text-xs text-muted" {
                            "计划 "
                            (wo.planned_qty)
                            " · 完工 "
                            (wo.completed_qty)
                            " · "
                            (sl)
                        }
                    }
                    span class="text-xs text-accent font-medium shrink-0" { "选择" }
                }
            }
        }
    }
}
}
