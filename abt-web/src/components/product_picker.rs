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
// 调用方式：product_picker_modal("product-modal", "product_id", "product-display")
// 选中后自动：填充 hidden input、显示产品名、关闭弹窗、发送 productSelected 事件到 body
// 调用方可选监听：hx-trigger="productSelected from:body" 做额外处理（加载价格等）

pub fn product_picker_modal(modal_id: &str, target_id: &str, display_id: &str) -> Markup {
    html! {
        div class="modal-overlay" id=(modal_id)
            _=(format!("on click remove .is-open from #{}", modal_id)) {
            div class="modal modal-lg" _="on click halt" {
                div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                    h2 { "选择产品" }
                    button style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px"
                        _=(format!("on click remove .is-open from #{}", modal_id)) { "×" }
                }
                div class="overflow-y-auto flex-1 min-h-0 p-6" style="padding:0" {
                    div class="product-search-bar" {
                        input type="hidden" name="target_id" value=(target_id);
                        input type="hidden" name="display_id" value=(display_id);
                        input type="hidden" name="modal_id" value=(modal_id);
                        div class="product-search-field" {
                            label class="product-search-label" { "产品名称" }
                            input class="product-w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="name" placeholder="输入产品名称…"
                                hx-get=(ProductSearchPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-sync="this:replace"
                                hx-target="#product-search-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                        }
                        div class="product-search-field" {
                            label class="product-search-label" { "产品编码" }
                            input class="product-w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="code" placeholder="输入产品编码…"
                                hx-get=(ProductSearchPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-sync="this:replace"
                                hx-target="#product-search-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                            button type="button" class="product-search-clear"
                                hx-get=(ProductSearchPath::PATH)
                                hx-target="#product-search-results"
                                hx-swap="innerHTML"
                                _="on click set (.product-search-input)'s value to '' then trigger keyup on .product-search-input" {
                                "清除"
                            }
                        }
                    }
                    div id="product-search-results" style="max-height:320px;overflow-y:auto"
                        hx-get=(ProductSearchPath::PATH)
                        hx-trigger="intersect once"
                        hx-swap="innerHTML"
                        hx-vals=(format!("{{\"target_id\":\"{}\",\"display_id\":\"{}\",\"modal_id\":\"{}\"}}", target_id, display_id, modal_id)) {
                        div style="display:flex;align-items:center;justify-content:center;padding:var(--space-8);color:var(--muted)" {
                            "加载中…"
                        }
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
            div style="text-align:center;padding:var(--space-12);color:var(--muted)" {
                (icon::package_icon("w-8 h-8"))
                p style="margin:var(--space-2) 0 0;font-size:var(--text-sm)" { "未找到匹配的产品" }
            }
        } @else {
            div class="product-select-list" {
                @for p in products {
                    div class="product-select-item"
                        data-pid=(p.product_id)
                        data-pname=(p.pdt_name.as_str())
                        _=(click_hs) {
                        div class="product-select-info" {
                            div class="product-select-name" { (p.pdt_name.as_str()) }
                            div class="product-select-meta" {
                                span class="product-select-code" { (p.product_code.as_str()) }
                                span class="product-select-sep" { "·" }
                                span { (p.meta.specification.as_str()) }
                                span class="product-select-sep" { "·" }
                                span { (p.unit.as_str()) }
                            }
                        }
                    }
                }
            }
        }
    }
}
