//! 往来方搜索 autocomplete 控件（HTMX 可复用组件）
//!
//! 使用方式：
//! 1. 在页面 filter 中放 `counterparty_search_input(config)` 渲染输入框 + 下拉
//! 2. 在 routes 中注册搜索端点，handler 调 `render_counterparty_results(items)`
//! 3. 搜索 handler 调 `cash_journal_service().search_counterparties(...)` 查数据

use maud::{html, Markup};

use abt_core::fms::cash_journal::model::CounterpartyResult;

use super::icon;

/// 搜索输入框 + 下拉容器（无状态，SSR 渲染）
pub fn counterparty_search_input(
    input_id: &str,
    dropdown_id: &str,
    search_path: &str,
    placeholder: &str,
    value: &str,
    width: &str,
) -> Markup {
    html! {
        div class=(format!("relative {width}")) {
            div class="relative icon:absolute icon:left-2.5 icon:top-1/2 icon:-translate-y-1/2 icon:w-3.5 icon:h-3.5 icon:text-muted z-10" {
                (icon::search_icon(""))
            }
            input
                class="w-full pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-colors duration-150 focus:border-accent"
                type="text"
                name="keyword"
                id=(input_id)
                hx-preserve
                placeholder=(placeholder)
                value=(value)
                hx-get=(search_path)
                hx-trigger="keyup changed delay:200ms, search"
                hx-include=(format!("next #{}", dropdown_id))
                hx-target=(format!("#{}", dropdown_id))
                hx-swap="innerHTML"
                autocomplete="off";
            // hidden input：携当前 keyword 值到搜索请求
            input type="hidden" id=(dropdown_id) name="keyword" value=(value);
            div id=(dropdown_id)
                class="absolute left-0 top-full mt-0.5 w-60 max-h-[200px] overflow-y-auto bg-white border border-border rounded-sm shadow-[var(--shadow-card)] z-20"
            {}
        }
    }
}

/// 搜索结果列表（搜索 handler 调用，渲染匹配行）
pub fn render_counterparty_results(items: &[CounterpartyResult], input_id: &str, dropdown_id: &str, empty_msg: &str) -> Markup {
    html! {
        @if items.is_empty() {
            div class="px-3 py-2 text-xs text-muted" { (empty_msg) }
        } @else {
            @for item in items {
                div
                    class="px-3 py-1.5 text-sm cursor-pointer hover:bg-accent-bg border-b border-border-soft"
                    data-val=(item.name.clone())
                    _=(format!(
                        "on click put me.dataset.val into #{}'s value then put '' into #{}'s innerHTML",
                        input_id, dropdown_id
                    ))
                {
                    (item.name) " · " span class="text-xs text-muted" { (item.code) }
                }
            }
        }
    }
}
