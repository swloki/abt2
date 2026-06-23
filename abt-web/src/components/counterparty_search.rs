//! 往来方搜索控件（搜索型 select：输入 → 下拉匹配 → 点击选中）
//!
//! 使用方式：
//! 1. 页面 filter 中放 `counterparty_search_input(...)` — 输入框 + 下拉容器
//! 2. 注册搜索端点 → handler 调 `search_counterparties()` →
//!    调 `render_counterparty_results(...)` 返回结果 HTML
//! 3. 结果自带 hyperscript：点击填入 input + 清空下拉 + 提交 form

use maud::{html, Markup};

use abt_core::fms::cash_journal::model::CounterpartyResult;

use super::icon;

/// 搜索输入框 + 下拉容器
pub fn counterparty_search_input(
    input_id: &str,
    dropdown_id: &str,
    search_path: &str,
    placeholder: &str,
    value: &str,
    _form_id: &str,
) -> Markup {
    html! {
        div class=(format!("relative"))
            _=(format!("on click from elsewhere remove .show from #{}", dropdown_id))
        {
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
                hx-trigger="keyup changed delay:200ms"
                hx-include=(format!("#{}", input_id))
                hx-target=(format!("#{}", dropdown_id))
                hx-swap="innerHTML"
                autocomplete="off"
                _=(format!(
                    "on focus if #{}'s innerHTML is not '' then add .show to #{} end",
                    dropdown_id, dropdown_id
                ));
            div id=(dropdown_id)
                class="absolute left-0 top-full mt-0.5 w-56 max-h-[200px] overflow-y-auto bg-white border border-border rounded-sm shadow-[var(--shadow-card)] z-20 hidden [&.show]:block"
            {}
        }
    }
}

/// 搜索结果列表（搜索 handler 调用）
pub fn render_counterparty_results(items: &[CounterpartyResult], input_id: &str, dropdown_id: &str, form_id: &str, empty_msg: &str) -> Markup {
    html! {
        @if items.is_empty() {
            div class="px-3 py-2 text-xs text-muted" { (empty_msg) }
        } @else {
            @for item in items {
                div
                    class="px-3 py-2 text-sm cursor-pointer hover:bg-accent-bg border-b border-border-soft last:border-b-0"
                    _=(format!(
                        "on click put '{}' into #{}'s value then remove .show from #{} then send change to #{}",
                        item.name, input_id, dropdown_id, form_id
                    ))
                {
                    div class="text-sm font-medium" { (item.name) }
                    div class="text-xs text-muted" { (item.code) }
                }
            }
        }
    }
}
