use maud::{Markup, html};

use super::overlay::drawer_shell;

/// Generic drawer component —— 基于 `overlay_shell`（显隐 / Esc / panel 由 shell 统一）。
///
/// 镜像 `modal` 组件 API：内置 footer（取消 / 提交）。提交按钮通过 HTML5 `form` 属性关联 body 内的 `<form>`。
///
/// `drawer_id` — overlay div 的 id；`.open` 由 shell（afterSettle）+ 调用方切换。
/// `title` — drawer 标题。
/// `submit_label` — 主提交按钮文案。
/// `form_id` — body 内 `<form>` 的 id，提交按钮据此关联。
/// `body` — 内容插槽（渲染在 drawer-body 内）。
pub fn drawer(drawer_id: &str, title: &str, submit_label: &str, form_id: &str, body: Markup) -> Markup {
    drawer_shell(drawer_id, "w-[420px]", html! {
        div class="flex items-center justify-between px-6 py-4 border-b border-border-soft" {
            h2 { (title) }
            button
                type="button"
                class="text-2xl text-muted hover:text-fg cursor-pointer bg-transparent border-none p-1 leading-none"
                _="on click remove .open from closest .drawer-overlay"
            { "×" }
        }
        div class="flex-1 overflow-y-auto p-6" { (body) }
        div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3" {
            button
                type="button"
                class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                _="on click remove .open from closest .drawer-overlay"
            { "取消" }
            button
                type="button"
                class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                _=(format!("on click trigger submit on #{}", form_id))
            { (submit_label) }
        }
    })
}

/// Drawer variant with custom footer content —— 同样基于 `overlay_shell`。
///
/// `drawer_id` — overlay div 的 id；`.open` 由 shell + 调用方切换。
/// `title` — drawer 标题。
/// `body` — 内容插槽。
/// `footer` — 自定义 footer 内容。
pub fn drawer_with_footer(drawer_id: &str, title: &str, body: Markup, footer: Markup) -> Markup {
    drawer_shell(drawer_id, "w-[420px]", html! {
        div class="flex items-center justify-between px-6 py-4 border-b border-border-soft" {
            h2 { (title) }
            button
                type="button"
                class="text-2xl text-muted hover:text-fg cursor-pointer bg-transparent border-none p-1 leading-none"
                _="on click remove .open from closest .drawer-overlay"
            { "×" }
        }
        div class="flex-1 overflow-y-auto p-6" { (body) }
        div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3" { (footer) }
    })
}
