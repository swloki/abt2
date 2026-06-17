use axum::routing::get;
use axum::Router;
use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::{Product, ProductQuery};
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::errors::Result;
use crate::state::AppState;
use crate::utils::RequestContext;

// ── Typed Path ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/api/products/search")]
pub struct ProductSearchPath;

// ── Search Params ──

#[derive(Debug, Deserialize)]
pub struct ProductSearchParams {
    pub name: Option<String>,
    pub code: Option<String>,
    pub target_id: Option<String>,
    pub display_id: Option<String>,
    pub modal_id: Option<String>,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new().route(ProductSearchPath::PATH, get(search_products))
}

// ── Search Handler ──

pub async fn search_products(
    ctx: RequestContext,
    Query(params): Query<ProductSearchParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.product_service();
    let filter = ProductQuery {
        name: params.name.filter(|s| !s.is_empty()),
        code: params.code.filter(|s| !s.is_empty()),
        status: None,
        owner_department_id: None,
        category_id: None,
    };
    let result = svc.list(&service_ctx, &mut conn, filter, PageParams::new(1, 20)).await?;
    let target = params.target_id.as_deref().unwrap_or("product_id");
    let display = params.display_id.as_deref().unwrap_or("product-display");
    let modal = params.modal_id.as_deref().unwrap_or("product-modal");
    Ok(Html(product_picker_results(&result.items, target, display, modal).into_string()))
}

// ── Modal Component ──
//
/// 产品选择弹窗（公共组件）
///
/// 调用方式：`product_picker_modal("product-modal", "product_id", "product-display")`
///
/// 选中后自动：
/// 1. 填充 hidden input（target_id）
/// 2. 显示产品名（display_id）
/// 3. 关闭弹窗
/// 4. 发送 `productSelected` 事件到 body
///
/// 调用方可选监听：`hx-trigger="productSelected from:body"` 做额外处理（加载价格等）
pub fn product_picker_modal(modal_id: &str, target_id: &str, display_id: &str) -> Markup {
    let close_hs = format!("on click remove .is-open from #{}", modal_id);
    html! {
        div class="fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto"
            id=(modal_id)
            _=(close_hs) {
            div class="bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl"
                onclick="event.stopPropagation()" {
                // ── Header ──
                div class="px-6 py-5 [border-bottom:1px_solid_var(--border-soft)] flex justify-between items-center shrink-0" {
                    h2 class="text-lg font-semibold m-0" { "选择产品" }
                    button class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg transition-colors"
                        _=(close_hs) { "×" }
                }
                // ── Body ──
                div class="overflow-y-auto flex-1 min-h-0 p-6" {
                    // ── Search Bar ──
                    div class="product-search-bar flex gap-4 mb-4 pb-4 [border-bottom:1px_solid_var(--border-soft)]" {
                        input type="hidden" name="target_id" value=(target_id);
                        input type="hidden" name="display_id" value=(display_id);
                        input type="hidden" name="modal_id" value=(modal_id);
                        div class="flex-1 flex flex-col gap-1" {
                            label class="text-xs font-medium text-fg-2" { "产品名称" }
                            input class="product-search-input w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                                type="text" name="name" placeholder="输入产品名称…"
                                hx-get=(ProductSearchPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-sync="this:replace"
                                hx-target="#product-search-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                        }
                        div class="flex-1 flex flex-col gap-1" {
                            label class="text-xs font-medium text-fg-2" { "产品编码" }
                            input class="product-search-input w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                                type="text" name="code" placeholder="输入产品编码…"
                                hx-get=(ProductSearchPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-sync="this:replace"
                                hx-target="#product-search-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                        }
                        button type="button" class="self-end py-2 px-4 border border-border rounded-sm bg-bg text-fg-2 text-sm cursor-pointer whitespace-nowrap hover:bg-surface transition-colors"
                            hx-get=(ProductSearchPath::PATH)
                            hx-target="#product-search-results"
                            hx-swap="innerHTML"
                            _="on click set <.product-search-input/>'s value to '' then trigger keyup on the first <.product-search-input/>" {
                            "清除"
                        }
                    }
                    // ── Results ──
                    div id="product-search-results" class="max-h-[400px] overflow-y-auto"
                        hx-get=(ProductSearchPath::PATH)
                        hx-trigger="intersect once"
                        hx-swap="innerHTML"
                        hx-vals=(format!("{{\"target_id\":\"{}\",\"display_id\":\"{}\",\"modal_id\":\"{}\"}}", target_id, display_id, modal_id)) {
                        div class="flex items-center justify-center py-8 text-muted text-sm" { "加载中…" }
                    }
                }
            }
        }
    }
}

// ── Results Fragment ──

pub fn product_picker_results(
    products: &[Product],
    target_id: &str,
    display_id: &str,
    modal_id: &str,
) -> Markup {
    let click_hs = format!(
        "on click set #{}'s value to my @data-pid then put my @data-pname into #{} then remove .is-open from #{} then send productSelected to body",
        target_id, display_id, modal_id
    );
    html! {
        @if products.is_empty() {
            div class="flex flex-col items-center justify-center py-12 text-muted" {
                (icon::package_icon("w-8 h-8 opacity-40"))
                p class="mt-2 text-sm" { "未找到匹配的产品" }
            }
        } @else {
            div class="py-2" {
                @for p in products {
                    div class="flex items-center justify-between p-3 [border-bottom:1px_solid_var(--border-soft)] cursor-pointer hover:bg-accent-bg transition-colors"
                        data-pid=(p.product_id)
                        data-pname=(p.pdt_name.as_str())
                        _=(click_hs) {
                        div class="flex-1 min-w-0" {
                            div class="text-sm font-medium text-fg truncate" { (p.pdt_name.as_str()) }
                            div class="text-xs text-muted flex items-center gap-1.5 flex-wrap mt-0.5" {
                                span class="bg-surface rounded px-1.5 py-0.5 font-mono" { (p.product_code.as_str()) }
                                span class="text-border" { "·" }
                                span { (p.meta.specification.as_str()) }
                                span class="text-border" { "·" }
                                span { (p.unit.as_str()) }
                            }
                        }
                    }
                }
            }
        }
    }
}
