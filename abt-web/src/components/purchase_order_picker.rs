use std::collections::HashMap;

use axum::routing::get;
use axum::Router;
use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use super::overlay::modal_shell;
use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::order::model::PurchaseOrderQuery;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::shared::types::PageParams;

use crate::components::supplier_search::{supplier_search_field, CascadeParams};
use crate::errors::Result;
use crate::utils::RequestContext;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/api/purchase-orders/search")]
pub struct PurchaseOrderSearchPath;

#[derive(Debug, Deserialize)]
pub struct SearchPoParams {
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub doc_number: Option<String>,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub product_code: Option<String>,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub supplier_id: Option<i64>,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub status: Option<i16>,
}

pub fn router() -> Router<crate::state::AppState> {
 Router::new().route(PurchaseOrderSearchPath::PATH, get(search_purchase_orders))
}

pub async fn search_purchase_orders(
 ctx: RequestContext,
 Query(params): Query<SearchPoParams>,
) -> Result<Html<String>> {
 use abt_core::purchase::enums::PurchaseOrderStatus;
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let po_svc = state.purchase_order_service();
 let supplier_svc = state.supplier_service();
 let query = PurchaseOrderQuery {
 doc_number: params.doc_number,
 product_code: params.product_code,
 supplier_id: params.supplier_id,
 status: params.status.and_then(PurchaseOrderStatus::from_i16),
 ..Default::default()
 };
 let result = po_svc
 .list(&service_ctx, &mut conn, query, PageParams::new(1, 50))
 .await
 .map(|r| r.items)
 .unwrap_or_default();
 // 供应商名解析（仅取结果中出现的供应商）
 let ids: Vec<i64> = result.iter().map(|o| o.supplier_id).collect();
 let names: HashMap<i64, String> = if ids.is_empty() {
 HashMap::new()
 } else {
 supplier_svc
 .list(&service_ctx, &mut conn, SupplierQuery::default(), PageParams::new(1, 500))
 .await
 .map(|r| r.items.into_iter().filter(|s| ids.contains(&s.id)).map(|s| (s.id, s.name)).collect())
 .unwrap_or_default()
 };
 Ok(Html(po_picker_results(&result, &names).into_string()))
}

/// 采购订单多选弹窗（confirm-post：确认按钮 hx-post=confirm_path hx-target=#po-cards，提交选中 po_id）
/// confirm_path 由调用方传（入库单传 StockInConfirmPosPath）
pub fn purchase_order_picker_modal(modal_id: &str, confirm_path: &str) -> Markup {
 let close_hs = format!("on click remove .is-open from #{}", modal_id);
 modal_shell(modal_id, "z-[1000]", html! {
    div class="modal bg-bg rounded-xl w-[780px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl"
        {
            div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0"
            {
                h2 class="text-lg font-semibold m-0" { "选择采购订单（可多选）" }
                button
                    type="button"
                    class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg transition-colors"
                    _=(close_hs)
                { "×" }
            }
            div class="px-6 py-4 border-b border-border-soft shrink-0 grid grid-cols-2 gap-3 po-search-bar"
            {
                div class="flex flex-col gap-1" {
                    label class="text-xs font-medium text-fg-2" { "采购单号" }
                    input
                        class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                        type="text"
                        name="doc_number"
                        placeholder="单号关键词…"
                        hx-get=(PurchaseOrderSearchPath::PATH)
                        hx-trigger="keyup changed delay:300ms"
                        hx-sync="this:replace"
                        hx-target="#po-search-results"
                        hx-swap="innerHTML"
                        hx-include=".po-search-bar" {}
                }
                div class="flex flex-col gap-1" {
                    label class="text-xs font-medium text-fg-2" { "产品编码" }
                    input
                        class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                        type="text"
                        name="product_code"
                        placeholder="产品编码关键词…"
                        hx-get=(PurchaseOrderSearchPath::PATH)
                        hx-trigger="keyup changed delay:300ms"
                        hx-sync="this:replace"
                        hx-target="#po-search-results"
                        hx-swap="innerHTML"
                        hx-include=".po-search-bar" {}
                }
                div class="flex flex-col gap-1" {
                    label class="text-xs font-medium text-fg-2" { "供应商" }
                    (supplier_search_field(
                        "po-supplier-input", "po-supplier-display", "po-supplier-panel", "po-supplier-results",
                        "supplier_id", "", "全部供应商",
                        true,
                        "w-full",
                        Some(&CascadeParams {
                            hx_get: PurchaseOrderSearchPath::PATH.to_string(),
                            hx_target: "#po-search-results".to_string(),
                            hx_include: ".po-search-bar".to_string(),
                            hx_swap: "innerHTML".to_string(),
                        }),
                    ))
                }
                div class="flex flex-col gap-1" {
                    label class="text-xs font-medium text-fg-2" { "状态" }
                    select
                        class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                        name="status"
                        hx-get=(PurchaseOrderSearchPath::PATH)
                        hx-trigger="change"
                        hx-target="#po-search-results"
                        hx-swap="innerHTML"
                        hx-include=".po-search-bar"
                    {
                        option value="" { "全部状态" }
                        option value="3" { "部分到货" }
                        option value="4" { "已到货" }
                        option value="2" { "已确认" }
                    }
                }
            }
            div id="po-search-results"
                class="overflow-y-auto flex-1 min-h-0"
                hx-get=(PurchaseOrderSearchPath::PATH)
                hx-trigger="intersect once"
                hx-swap="innerHTML"
                hx-include=".po-search-bar"
            {
                div class="text-center text-muted py-10 text-sm" { "加载中…" }
            }
            div class="px-6 py-4 border-t border-border-soft flex items-center justify-between shrink-0"
            {
                span class="text-sm text-muted" { "勾选后点击确认" }
                div class="flex gap-3" {
                    button
                        type="button"
                        class="inline-flex items-center gap-2 py-2 px-4 rounded-sm bg-white text-fg-2 border border-border hover:bg-surface text-sm font-medium cursor-pointer transition-colors"
                        _=(close_hs)
                    { "取消" }
                    button
                        type="button"
                        class="inline-flex items-center gap-2 py-2 px-4 rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-colors"
                        hx-post=(confirm_path)
                        hx-target="#po-cards"
                        hx-swap="innerHTML"
                        hx-include="#po-search-results input[name='po_id']:checked"
                    { "确认选择" }
                }
            }
        }
    })
}

fn po_picker_results(
 orders: &[abt_core::purchase::order::model::PurchaseOrder],
 supplier_names: &HashMap<i64, String>,
) -> Markup {
 use abt_core::purchase::enums::PurchaseOrderStatus;
 let status_label = |s: &PurchaseOrderStatus| -> &'static str {
 match s {
 PurchaseOrderStatus::Draft => "草稿",
 PurchaseOrderStatus::PendingApproval => "待审批",
 PurchaseOrderStatus::Confirmed => "已确认",
 PurchaseOrderStatus::PartiallyReceived => "部分到货",
 PurchaseOrderStatus::Received => "已到货",
 PurchaseOrderStatus::Closed => "已关闭",
 PurchaseOrderStatus::Cancelled => "已取消",
 }
 };
 html! {
    @if orders.is_empty() {
        div class="text-center text-muted py-10" {
            p class="text-sm" { "未找到匹配的采购订单" }
        }
    } @else {
        @for o in orders {
            @let sl = status_label(&o.status);
            @let sup = supplier_names
                .get(&o.supplier_id)
                .cloned()
                .unwrap_or_else(|| "-".into());
            label
                class="flex items-center gap-3 px-3 py-2 hover:bg-surface cursor-pointer border-b border-border-soft last:border-b-0 transition-colors duration-100"
            {
                input
                    type="checkbox"
                    name="po_id"
                    value=(o.id)
                    class="po-pick-cb cursor-pointer accent-accent w-4 h-4 shrink-0";
                div class="flex-1 min-w-0" {
                    div class="text-sm font-medium text-fg truncate" { (o.doc_number) }
                    div class="text-xs text-muted truncate" {
                        (sup.as_str())
                        " · "
                        (sl)
                        " · "
                        (o.order_date.format("%Y-%m-%d").to_string())
                    }
                }
            }
        }
    }
}
}
