use std::collections::HashMap;

use axum::routing::get;
use axum::Router;
use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::customer::model::{Customer, CustomerQuery};
use abt_core::sales::shipping_request::ShippingRequestService;
use abt_core::sales::shipping_request::model::ShippingQuery;
use abt_core::shared::types::PageParams;

use crate::errors::Result;
use crate::utils::RequestContext;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/api/shipping-requests/search")]
pub struct ShippingRequestSearchPath;

#[derive(Debug, Deserialize)]
pub struct SearchShippingParams {
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub customer_id: Option<i64>,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub status: Option<i16>,
}

pub fn router() -> Router<crate::state::AppState> {
    Router::new().route(ShippingRequestSearchPath::PATH, get(search_shipping_requests))
}

pub async fn search_shipping_requests(
    ctx: RequestContext,
    Query(params): Query<SearchShippingParams>,
) -> Result<Html<String>> {
    use abt_core::sales::shipping_request::model::ShippingStatus;
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.shipping_service();
    let customer_svc = state.customer_service();
    let query = ShippingQuery {
        keyword: params.keyword,
        customer_id: params.customer_id,
        status: params.status.and_then(ShippingStatus::from_i16),
        order_id: None,
    };
    let result = svc
        .list(&service_ctx, &mut conn, query, PageParams::new(1, 50))
        .await
        .map(|r| r.items)
        .unwrap_or_default();
    // 仅展示可出库状态（已确认 / 拣货中）
    let filtered: Vec<_> = result
        .into_iter()
        .filter(|s| matches!(s.status, ShippingStatus::Confirmed | ShippingStatus::Picking))
        .collect();
    // 客户名解析（仅取结果中出现的客户）
    let ids: Vec<i64> = filtered.iter().map(|s| s.customer_id).collect();
    let names: HashMap<i64, String> = if ids.is_empty() {
        HashMap::new()
    } else {
        customer_svc
            .list(&service_ctx, &mut conn, CustomerQuery::default(), PageParams::new(1, 500))
            .await
            .map(|r| r.items.into_iter().filter(|c| ids.contains(&c.id)).map(|c| (c.id, c.name)).collect())
            .unwrap_or_default()
    };
    Ok(Html(shipping_picker_results(&filtered, &names).into_string()))
}

/// 发货申请多选弹窗（confirm-shipping：确认按钮 hx-post=confirm_path hx-target=#source-cards，提交选中 shipping_id）
/// confirm_path 由调用方传（出库单传 StockOutConfirmShippingPath）
pub fn shipping_request_picker_modal(modal_id: &str, confirm_path: &str, customers: &[Customer]) -> Markup {
    let close_hs = format!("on click remove .is-open from #{}", modal_id);
    html! {
        div class="fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto"
        id=(modal_id) _=(format!("on click[me is event.target] remove .is-open from #{}", modal_id)) {
            div class="modal bg-bg rounded-xl w-[780px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl" {
                div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                    h2 class="text-lg font-semibold m-0" { "选择发货申请（可多选）" }
                    button type="button" class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg transition-colors" _=(close_hs) { "×" }
                }
                div class="px-6 py-4 border-b border-border-soft shrink-0 grid grid-cols-3 gap-3 shipping-search-bar" {
                    div class="flex flex-col gap-1" {
                        label class="text-xs font-medium text-fg-2" { "发货单号" }
                        input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword" placeholder="单号关键词…"
                        hx-get=(ShippingRequestSearchPath::PATH) hx-trigger="keyup changed delay:300ms" hx-sync="this:replace"
                        hx-target="#shipping-search-results" hx-swap="innerHTML" hx-include=".shipping-search-bar" {}
                    }
                    div class="flex flex-col gap-1" {
                        label class="text-xs font-medium text-fg-2" { "客户" }
                        select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" name="customer_id"
                        hx-get=(ShippingRequestSearchPath::PATH) hx-trigger="change" hx-target="#shipping-search-results" hx-swap="innerHTML" hx-include=".shipping-search-bar" {
                            option value="" { "全部客户" }
                            @for c in customers {
                                option value=(c.id) { (c.name.as_str()) }
                            }
                        }
                    }
                    div class="flex flex-col gap-1" {
                        label class="text-xs font-medium text-fg-2" { "状态" }
                        select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" name="status"
                        hx-get=(ShippingRequestSearchPath::PATH) hx-trigger="change" hx-target="#shipping-search-results" hx-swap="innerHTML" hx-include=".shipping-search-bar" {
                            option value="" { "全部可出库" }
                            option value="2" { "已确认" }
                            option value="3" { "拣货中" }
                        }
                    }
                }
                div id="shipping-search-results" class="overflow-y-auto flex-1 min-h-0"
                hx-get=(ShippingRequestSearchPath::PATH) hx-trigger="intersect once" hx-swap="innerHTML" hx-include=".shipping-search-bar" {
                    div class="text-center text-muted py-10 text-sm" { "加载中…" }
                }
                div class="px-6 py-4 border-t border-border-soft flex items-center justify-between shrink-0" {
                    span class="text-sm text-muted" { "勾选后点击确认" }
                    div class="flex gap-3" {
                        button type="button" class="inline-flex items-center gap-2 py-2 px-4 rounded-sm bg-white text-fg-2 border border-border hover:bg-surface text-sm font-medium cursor-pointer transition-colors"
                        _=(close_hs) { "取消" }
                        button type="button" class="inline-flex items-center gap-2 py-2 px-4 rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-colors"
                        hx-post=(confirm_path) hx-target="#source-cards" hx-swap="innerHTML"
                        hx-include="#shipping-search-results input[name='shipping_id']:checked" { "确认选择" }
                    }
                }
            }
        }
    }
}

fn shipping_picker_results(
    items: &[abt_core::sales::shipping_request::model::ShippingRequest],
    customer_names: &HashMap<i64, String>,
) -> Markup {
    use abt_core::sales::shipping_request::model::ShippingStatus;
    let status_label = |s: &ShippingStatus| -> &'static str {
        match s {
            ShippingStatus::Draft => "草稿",
            ShippingStatus::Confirmed => "已确认",
            ShippingStatus::Picking => "拣货中",
            ShippingStatus::Shipped => "已发货",
            ShippingStatus::Cancelled => "已取消",
        }
    };
    html! {
        @if items.is_empty() {
            div class="text-center text-muted py-10" {
                p class="text-sm" { "未找到可出库的发货申请" }
                p class="text-xs mt-1" { "仅展示「已确认 / 拣货中」状态的发货申请" }
            }
        } @else {
            @for s in items {
                @let sl = status_label(&s.status);
                @let cust = customer_names.get(&s.customer_id).cloned().unwrap_or_else(|| "-".into());
                label class="flex items-center gap-3 px-3 py-2 hover:bg-surface cursor-pointer border-b border-border-soft last:border-b-0 transition-colors duration-100" {
                    input type="checkbox" name="shipping_id" value=(s.id) class="shipping-pick-cb cursor-pointer accent-accent w-4 h-4 shrink-0";
                    div class="flex-1 min-w-0" {
                        div class="text-sm font-medium text-fg truncate" { (s.doc_number) }
                        div class="text-xs text-muted truncate" {
                            (cust.as_str()) " · " (sl) " · " (s.request_date.format("%Y-%m-%d").to_string())
                        }
                    }
                }
            }
        }
    }
}
