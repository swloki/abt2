//! 往来方搜索控件（搜索型 select：只读框 + 弹出搜索面板）
//!
//! 完全自洽：使用方只需调 counterparty_search_input()，
//! 所有交互（展开/搜索/选中/关闭/提交）都封装在组件内。
//!
//! 关键设计：panel 用 `invisible`（visibility:hidden）而非 `hidden`（display:none），
//! 这样 HTMX 的 `load` trigger 能在页面加载时正常初始化和触发。

use maud::{html, Markup};

use abt_core::fms::cash_journal::model::CounterpartyResult;

use super::icon;

pub fn counterparty_search_input(
    input_id: &str,
    display_id: &str,
    panel_id: &str,
    search_path: &str,
    placeholder: &str,
    value: &str,
) -> Markup {
    let q_id = format!("{}-q", panel_id);
    let list_id = format!("{}-list", panel_id);
    html! {
        div class="relative w-40"
            _=(format!("on click from elsewhere add .invisible to #{}", panel_id))
        {
            input type="hidden" name="keyword" id=(input_id) value=(value);
            // 只读显示框（点击展开 + 加载列表）
            div class="flex items-center w-full border border-border rounded-sm bg-white cursor-pointer text-sm transition-colors duration-150 hover:border-accent"
                _=(format!(
                    "on click toggle .invisible on #{p} then call cpSearch('{s}', '{qi}', '{li}')",
                    p = panel_id, s = search_path, qi = q_id, li = list_id
                ))
            {
                div id=(display_id)
                    class=(format!("flex-1 pl-2.5 pr-1 py-1.5 truncate {}", if value.is_empty() { "text-muted" } else { "text-fg" }))
                {
                    @if value.is_empty() { (placeholder) } @else { (value) }
                }
                span class="px-2 py-1.5 text-muted text-xs pointer-events-none" { "▾" }
            }
            // 弹出面板
            div id=(panel_id) class="absolute left-0 top-full mt-0.5 w-72 bg-white border border-border rounded-sm shadow-[var(--shadow-card)] z-30 invisible transition-all duration-150"
                _="on click halt"
            {
                div class="flex items-center gap-2 p-2 border-b border-border-soft" {
                    (icon::search_icon("w-3.5 h-3.5 text-muted shrink-0"))
                    input
                        class="flex-1 py-1 text-sm bg-transparent text-fg outline-none min-w-0"
                        type="text"
                        id=(q_id)
                        placeholder=(format!("搜索{}…", placeholder))
                        autocomplete="off"
                        _=(format!(
                            "on keyup delay:200ms call cpSearch('{s}', me.id, '{li}')",
                            s = search_path, li = list_id
                        ));
                }
                div id=(list_id) class="max-h-[240px] overflow-y-auto" {}
            }
        }
    }
}

pub fn render_counterparty_results(
    items: &[CounterpartyResult],
    input_id: &str,
    display_id: &str,
    panel_id: &str,
    empty_msg: &str,
) -> Markup {
    html! {
        @if items.is_empty() {
            div class="px-3 py-4 text-xs text-muted text-center" { (empty_msg) }
        } @else {
            @for item in items {
                div
                    class="px-3 py-2 text-sm cursor-pointer hover:bg-accent-bg border-b border-border-soft last:border-b-0"
                    _=(format!(
                        "on click put '{name}' into #{ii}'s value \
                         then put '{name}' into #{di}'s innerHTML \
                         then remove .text-muted from #{di} \
                         then add .invisible to #{pi} \
                         then trigger change on #{ii}",
                        name = item.name, ii = input_id, di = display_id, pi = panel_id
                    ))
                {
                    div class="font-medium text-fg" { (item.name) }
                    div class="text-xs text-muted" { (item.code) }
                }
            }
        }
    }
}
