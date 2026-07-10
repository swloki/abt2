use axum::routing::get;
use axum::Router;
use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use super::overlay::modal_shell;
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
    // fill-input 模式参数
    pub target_id: Option<String>,
    pub display_id: Option<String>,
    // add-row 模式参数
    pub item_row_path: Option<String>,
    pub tbody_id: Option<String>,
    pub modal_id: Option<String>,
    // BOM 物料过滤（逗号分隔的 product_id 列表）
    pub bom_product_ids: Option<String>,
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
    // BOM 物料过滤：有 bom_product_ids 时直接通过 ID 列表 + 本地筛选，避免分页遗漏
    let items: Vec<Product> = if let Some(ref ids_str) = params.bom_product_ids {
        let allowed: std::collections::HashSet<i64> = ids_str
            .split(',')
            .filter_map(|s| s.trim().parse::<i64>().ok())
            .collect();
        if allowed.is_empty() {
            svc.list(&service_ctx, &mut conn, filter, PageParams::new(1, 20)).await?.items
        } else {
            let bom_products = svc.get_by_ids(&service_ctx, &mut conn, allowed.iter().copied().collect()).await?;
            // 本地按 name/code 过滤（不做分页限制，BOM 物料应全部展示）
            bom_products.into_iter().filter(|p| {
                let name_match = filter.name.as_ref().map_or(true, |n| p.pdt_name.contains(n));
                let code_match = filter.code.as_ref().map_or(true, |c| p.product_code.contains(c));
                name_match && code_match
            }).collect()
        }
    } else {
        svc.list(&service_ctx, &mut conn, filter, PageParams::new(1, 20)).await?.items
    };
    let modal = params.modal_id.as_deref().unwrap_or("product-modal");
    // 如果有 item_row_path 参数 → add-row 模式，否则 fill-input 模式
    if let Some(row_path) = &params.item_row_path {
        let tbody = params.tbody_id.as_deref().unwrap_or("item-tbody");
        Ok(Html(product_picker_results_for_table(&items, row_path, tbody, modal).into_string()))
    } else {
        let target = params.target_id.as_deref().unwrap_or("product_id");
        let display = params.display_id.as_deref().unwrap_or("product-display");
        Ok(Html(product_picker_results(&items, target, display, modal).into_string()))
    }
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
    product_picker_modal_with_bom_filter(modal_id, target_id, display_id, None, true)
}

/// 产品选择弹窗（延迟首次加载版）
///
/// 与 [`product_picker_modal`] 的唯一区别：结果区**不带** `intersect once` 自动首次加载，
/// 首次加载完全由调用方负责（如 `openProductPicker` 用 `htmx.ajax` 以**动态 target_id** 拉取）。
///
/// 用于「一个 modal 服务多行 + 动态 target_id」场景：否则结果区 `intersect once` 会用
/// 硬编码 target_id 发出请求，与调用方动态 target_id 的请求并发 swap 同一容器造成竞态覆盖——
/// 后到者覆盖先到者，若 intersect 后到，点击 hyperscript 会指向 DOM 不存在的硬编码 id 而静默失败。
pub fn product_picker_modal_deferred(modal_id: &str, target_id: &str, display_id: &str) -> Markup {
    product_picker_modal_with_bom_filter(modal_id, target_id, display_id, None, false)
}

/// 产品选择弹窗（带 BOM 物料过滤）
///
/// `bom_product_ids` — 可选，逗号分隔的 product_id 列表，限制搜索结果仅在该集合内
pub fn product_picker_modal_with_bom_filter(
    modal_id: &str,
    target_id: &str,
    display_id: &str,
    bom_product_ids: Option<&str>,
    initial_load: bool,
) -> Markup {
    let close_hs = format!("on click remove .is-open from #{}", modal_id);
    let bom_inputs = if let Some(ids) = bom_product_ids {
        html! {
            input type="hidden" name="bom_product_ids" value=(ids);
        }
    } else {
        Markup::default()
    };
    // 结果区：initial_load=true 时由 intersect once 自动首次加载（用硬编码 target_id 的 hx-vals）；
    // initial_load=false 时仅占位容器，首次加载交给调用方（htmx.ajax 以动态 target_id 拉取），
    // 避免两者并发 swap 同一容器造成竞态覆盖（见 product_picker_modal_deferred 文档）。
    let results_div: Markup = if initial_load {
        let mut vals = format!(
            "{{\"target_id\":\"{}\",\"display_id\":\"{}\",\"modal_id\":\"{}\"",
            target_id, display_id, modal_id,
        );
        if let Some(ids) = bom_product_ids {
            use std::fmt::Write;
            write!(&mut vals, ",\"bom_product_ids\":\"{}\"", ids).unwrap();
        }
        vals.push_str("}");
        html! {
            div id="product-search-results"
                class="max-h-[400px] overflow-y-auto"
                hx-get=(ProductSearchPath::PATH)
                hx-trigger="intersect once"
                hx-swap="innerHTML"
                hx-include=".product-search-bar"
                hx-vals=(vals) {
                div class="flex items-center justify-center py-8 text-muted text-sm" {
                    "加载中…"
                }
            }
        }
    } else {
        html! {
            div id="product-search-results" class="max-h-[400px] overflow-y-auto" {
                div class="flex items-center justify-center py-8 text-muted text-sm" {
                    "加载中…"
                }
            }
        }
    };
    modal_shell(modal_id, "z-[1100]", html! {
        div class="bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl"
            {
                // ── Header ──
                div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0"
                {
                    h2 class="text-lg font-semibold m-0" { "选择产品" }
                    button
                        class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg transition-colors"
                        _=(close_hs)
                    { "×" }
                }
                // ── Body ──
                div class="overflow-y-auto flex-1 min-h-0 p-6" {
                    // ── Search Bar ──
                    div class="product-search-bar flex gap-4 mb-4 pb-4 border-b border-border-soft"
                    {
                        input type="hidden" name="target_id" value=(target_id);
                        input type="hidden" name="display_id" value=(display_id);
                        input type="hidden" name="modal_id" value=(modal_id);
                        (bom_inputs)
                        div class="flex-1 flex flex-col gap-1" {
                            label class="text-xs font-medium text-fg-2" { "产品名称" }
                            input
                                class="product-search-input w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                                type="text"
                                name="name"
                                placeholder="输入产品名称…"
                                hx-get=(ProductSearchPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-sync="this:replace"
                                hx-target="#product-search-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                        }
                        div class="flex-1 flex flex-col gap-1" {
                            label class="text-xs font-medium text-fg-2" { "产品编码" }
                            input
                                class="product-search-input w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                                type="text"
                                name="code"
                                placeholder="输入产品编码…"
                                hx-get=(ProductSearchPath::PATH)
                                hx-trigger="keyup changed delay:300ms"
                                hx-sync="this:replace"
                                hx-target="#product-search-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                        }
                        button
                            type="button"
                            class="self-end py-2 px-4 border border-border rounded-sm bg-bg text-fg-2 text-sm cursor-pointer whitespace-nowrap hover:bg-surface transition-colors"
                            hx-get=(ProductSearchPath::PATH)
                            hx-target="#product-search-results"
                            hx-swap="innerHTML"
                            _="on click set <.product-search-input/>'s value to '' then trigger keyup on the first <.product-search-input/>"
                        { "清除" }
                    }
                    // ── Results ──
                    (results_div)
                }
            }
        })
}

/// 产品选择弹窗（选产品→添加表格行模式）
///
/// 搜索统一走 `/api/products/search`，结果由公共组件渲染。
/// 选中产品行后通过 `item_row_path?product_id=xxx` 添加一行到 `tbody_id`。
pub fn product_picker_modal_with_search(modal_id: &str, item_row_path: &str, tbody_id: &str) -> Markup {
    let close_hs = format!("on click remove .is-open from #{}", modal_id);
    let search_path = ProductSearchPath::PATH;
    modal_shell(modal_id, "z-[1100]", html! {
        div class="bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl"
            {
                div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0"
                {
                    h2 class="text-lg font-semibold m-0" { "选择产品" }
                    button
                        class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg transition-colors"
                        _=(close_hs)
                    { "×" }
                }
                div class="overflow-y-auto flex-1 min-h-0 p-6" {
                    div class="product-search-bar flex gap-4 mb-4 pb-4 border-b border-border-soft"
                    {
                        input type="hidden" name="item_row_path" value=(item_row_path);
                        input type="hidden" name="tbody_id" value=(tbody_id);
                        input type="hidden" name="modal_id" value=(modal_id);
                        div class="flex-1 flex flex-col gap-1" {
                            label class="text-xs font-medium text-fg-2" { "产品名称" }
                            input
                                class="product-search-input w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                                type="text"
                                name="name"
                                placeholder="输入产品名称…"
                                hx-get=(search_path)
                                hx-trigger="keyup changed delay:300ms"
                                hx-sync="this:replace"
                                hx-target="#product-search-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                        }
                        div class="flex-1 flex flex-col gap-1" {
                            label class="text-xs font-medium text-fg-2" { "产品编码" }
                            input
                                class="product-search-input w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                                type="text"
                                name="code"
                                placeholder="输入产品编码…"
                                hx-get=(search_path)
                                hx-trigger="keyup changed delay:300ms"
                                hx-sync="this:replace"
                                hx-target="#product-search-results"
                                hx-swap="innerHTML"
                                hx-include=".product-search-bar" {}
                        }
                        button
                            type="button"
                            class="self-end py-2 px-4 border border-border rounded-sm bg-bg text-fg-2 text-sm cursor-pointer whitespace-nowrap hover:bg-surface transition-colors"
                            hx-get=(search_path)
                            hx-target="#product-search-results"
                            hx-swap="innerHTML"
                            _="on click set <.product-search-input/>'s value to '' then trigger keyup on the first <.product-search-input/>"
                        { "清除" }
                    }
                    div id="product-search-results"
                        class="max-h-[400px] overflow-y-auto"
                        hx-get=(search_path)
                        hx-trigger="intersect once"
                        hx-swap="innerHTML"
                        hx-vals=({
                            format!(
                                "{{\"item_row_path\":\"{}\",\"tbody_id\":\"{}\",\"modal_id\":\"{}\"}}",
                                item_row_path,
                                tbody_id,
                                modal_id,
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

/// 渲染产品搜索结果（fill-input 模式：点击行填充 hidden input + 显示名称）
pub fn product_picker_results(
    products: &[Product],
    target_id: &str,
    display_id: &str,
    modal_id: &str,
) -> Markup {
    // Issue #212：productSelected 事件额外携带 productCode（向后兼容：现有监听器不读此字段不受影响）
    let click_hs = format!(
        "on click set #{}'s value to my @data-pid then put my @data-pname into #{} then remove .is-open from #{} then send productSelected(productId: my @data-pid, productName: my @data-pname, productCode: my @data-code) to body",
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
                    div class="flex items-center justify-between p-3 border-b border-border-soft cursor-pointer hover:bg-accent-bg transition-colors"
                        data-pid=(p.product_id)
                        data-pname=(p.pdt_name.as_str())
                        data-code=(p.product_code.as_str())
                        _=(click_hs)
                    {
                        div class="flex-1 min-w-0" {
                            div class="text-sm font-medium text-fg truncate" { (p.pdt_name.as_str()) }
                            div class="text-xs text-muted flex items-center gap-1.5 flex-wrap mt-0.5"
                            {
                                span class="bg-surface rounded px-1.5 py-0.5 font-mono" {
                                    (p.product_code.as_str())
                                }
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

/// 渲染产品搜索结果（点击整行添加表格行模式）
pub fn product_picker_results_for_table(
    products: &[Product],
    item_row_path: &str,
    tbody_id: &str,
    modal_id: &str,
) -> Markup {
    html! {
        @if products.is_empty() {
            div class="flex flex-col items-center justify-center py-12 text-muted" {
                (icon::package_icon("w-8 h-8 opacity-40"))
                p class="mt-2 text-sm" { "未找到匹配的产品" }
            }
        } @else {
            div class="py-2" {
                @for p in products {
                    div class="flex items-center p-3 border-b border-border-soft cursor-pointer hover:bg-accent-bg transition-colors"
                        hx-get=(item_row_path)
                        // 以目标 tbody 锚定其所属表单，仅取【本表单】的 supplier_id（按供应商带最优价）。
                        // 不可用全局 `hx-include="[name=supplier_id]"`：工作中心等多 drawer 共存页面会
                        // 命中所有 name=supplier_id → query 字段重复 → 后端单值反序列化报 duplicate field。
                        hx-vals=(format!(
                            "js:{{ product_id: {}, supplier_id: document.querySelector('#{}')?.closest('form')?.querySelector('[name=supplier_id]')?.value || '' }}",
                            p.product_id, tbody_id
                        ))
                        hx-target=(format!("#{}", tbody_id))
                        hx-swap="beforeend"
                        _=({
                            format!(
                                "on 'htmx:afterRequest' remove .is-open from #{}",
                                modal_id,
                            )
                        })
                    {
                        div class="flex-1 min-w-0" {
                            div class="text-sm font-medium text-fg truncate" { (p.pdt_name.as_str()) }
                            div class="text-xs text-muted flex items-center gap-1.5 flex-wrap mt-0.5"
                            {
                                span class="bg-surface rounded px-1.5 py-0.5 font-mono" {
                                    (p.product_code.as_str())
                                }
                                span class="text-border" { "·" }
                                span { (p.meta.specification.as_str()) }
                                span class="text-border" { "·" }
                                span { (p.unit.as_str()) }
                            }
                        }
                        span class="text-xs text-accent font-medium shrink-0" { "点击添加" }
                    }
                }
            }
        }
    }
}
