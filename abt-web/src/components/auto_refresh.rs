//! 工作中心定时自动刷新控件（间隔下拉 + 开关 + 倒计时）。
//!
//! 放在 `render_card_shell` 标题栏（card 外部，不受 `hx-select` 刷新影响），
//! 控件状态 + JS 定时器（`static/app.js` 的 `wcAutoRefresh`）天然持久。定时到点
//! JS `dispatch` 事件名，数据区（`body_sel`，如 `#wc-demand-body`）声明
//! `hx-trigger="<event> from:body"` 自刷新——只换数据区，filter 表单节点不动。
//!
//! 输入保护由 `wcAutoRefresh._arShouldSkip` 处理（聚焦输入 / drawer 打开 /
//! checkbox 勾选 / 展开态时跳过本次刷新）。

use maud::{html, Markup};

use crate::components::icon;

/// 自动刷新控件。
///
/// - `event_name`：定时器 dispatch 的事件名（数据区 `hx-trigger` 监听）；
/// - `body_sel`：刷新目标选择器（传给 JS 做 skip-guard 范围判定，如 `"#wc-demand-body"`）；
/// - `default_interval_sec`：默认间隔秒（30/60/300，决定 select 默认选中）。
pub fn auto_refresh_control(event_name: &str, body_sel: &str, default_interval_sec: u32) -> Markup {
    html! {
        div class="flex items-center gap-2 ml-auto shrink-0"
            id="wc-ar-control"
            data-ar-event=(event_name)
            data-ar-body=(body_sel) {
            // 间隔下拉（3 档）
            select id="wc-ar-interval"
                class="text-xs border border-border-soft rounded-sm bg-white px-1.5 py-1 text-fg-2 cursor-pointer outline-none focus:border-accent"
                _="on change call wcAutoRefresh.onIntervalChange(me)" {
                option value="30" selected[default_interval_sec == 30] { "30秒" }
                option value="60" selected[default_interval_sec == 60] { "1分" }
                option value="300" selected[default_interval_sec == 300] { "5分" }
            }
            // 开关（复用 wms_strategy_list 的 act: + .active toggle 范式；默认 OFF）
            label class="flex items-center gap-1.5 cursor-pointer text-xs text-muted select-none whitespace-nowrap"
                _="on click toggle .active on closest .toggle-track then call wcAutoRefresh.onToggleClick(me)" {
                span class="toggle-track w-9 h-5 rounded-full relative shrink-0 transition-colors duration-150 bg-border act:bg-accent after:content-[''] after:absolute after:top-0.5 after:left-0.5 after:w-4 after:h-4 after:bg-white after:rounded-full after:transition-transform after:duration-150 act:after:translate-x-4" {};
                (icon::refresh_icon("w-3 h-3"))
                "自动"
            }
            // 倒计时文本（JS 每秒更新；OFF 时显示 --）
            span id="wc-ar-count" class="text-[11px] text-muted font-mono tabular-nums w-8 text-right" { "--" }
        }
    }
}
