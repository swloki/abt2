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

/// 搜索型 select：只读显示框 + 点击弹出搜索面板
pub fn counterparty_search_input(
    input_id: &str,
    dropdown_id: &str,
    panel_id: &str,
    search_path: &str,
    placeholder: &str,
    value: &str,
) -> Markup {
    html! {
        div class="relative"
            _=(format!("on click from elsewhere remove .show from #{}", panel_id))
        {
            // 隐藏 input（提交 form 用）
            input type="hidden" name="keyword" id=(input_id) value=(value);
            // 只读显示框（不能输入，点击弹出面板）
            div
                class="flex items-center border border-border rounded-sm bg-white cursor-pointer text-sm transition-colors duration-150 hover:border-accent"
                _=(format!("on click toggle .show on #{} then if #{} is .show send focus to #{}", panel_id, panel_id, format!("{}-q", panel_id)))
            {
                div id=(dropdown_id)
                    class=(format!("flex-1 pl-2.5 pr-1 py-1.5 text-sm truncate {}",
                        if value.is_empty() { "text-muted" } else { "text-fg" }))
                {
                    @if value.is_empty() { (placeholder) } @else { (value) }
                }
                span class="px-1.5 text-muted text-xs transition-transform"
                    _=(format!("on click halt the event then toggle .show on #{} then toggle .rotate-180 on me", panel_id))
                { "▾" }
            }
            // 弹出搜索面板
            div id=(panel_id) class="absolute left-0 top-full mt-0.5 w-64 bg-white border border-border rounded-sm shadow-[var(--shadow-card)] z-30 hidden"
                _="on click halt"
            {
                // 面板内搜索框
                div class="flex items-center gap-2 p-2 border-b border-border-soft" {
                    (icon::search_icon("w-3.5 h-3.5 text-muted shrink-0"))
                    input
                        class="flex-1 py-1 text-sm bg-transparent text-fg outline-none"
                        type="text"
                        id=(format!("{}-q", panel_id))
                        placeholder=(format!("搜索{}…", placeholder))
                        hx-get=(search_path)
                        hx-trigger="keyup changed delay:200ms, load"
                        hx-include=(format!("#{}", format!("{}-q", panel_id)))
                        hx-target=(format!("#{}", format!("{}-list", panel_id)))
                        hx-swap="innerHTML"
                        autocomplete="off";
                }
                // 面板内结果列表
                div id=(format!("{}-list", panel_id))
                    class="max-h-[200px] overflow-y-auto"
                {}
            }
        }
    }
}

/// 搜索结果列表
pub fn render_counterparty_results(items: &[CounterpartyResult], input_id: &str, display_id: &str, panel_id: &str, empty_msg: &str) -> Markup {
    html! {
        @if items.is_empty() {
            div class="px-3 py-3 text-xs text-muted text-center" { (empty_msg) }
        } @else {
            @for item in items {
                div
                    class="px-3 py-2 text-sm cursor-pointer hover:bg-accent-bg border-b border-border-soft last:border-b-0"
                    _=(format!(
                        "on click put '{}' into #{}'s value
                         then put '{}' into #{}'s innerHTML
                         then remove .text-muted from #{}
                         then remove .show from #{}",
                        item.name, input_id,
                        item.name, display_id,
                        display_id,
                        panel_id
                    ))
                {
                    div class="font-medium" { (item.name) }
                    div class="text-xs text-muted" { (item.code) }
                }
            }
        }
    }
}

