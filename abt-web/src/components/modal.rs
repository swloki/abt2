use maud::{Markup, html};

use super::overlay::modal_shell;

/// Generic modal component —— 基于 `overlay_shell`（显隐 / Esc / 背景 / 打开守卫由 shell 统一）。
///
/// `modal_id` — overlay div 的 id；`.is-open` 由 shell（afterSettle）+ 调用方（按钮）切换。
/// `title` — modal 标题。
/// `submit_label` — 主提交按钮文案。
/// `form_id` — body 内 `<form>` 的 id，footer 提交按钮据此关联。
/// `hx_post` — form 提交地址。
/// `body` — 表单内容插槽。
pub fn modal(modal_id: &str, title: &str, submit_label: &str, form_id: &str, hx_post: &str, body: Markup) -> Markup {
    modal_shell(modal_id, "z-[1000]", html! {
        form
            id=(form_id)
            class="bg-bg rounded-xl w-[680px] flex flex-col overflow-hidden"
            hx-post=(hx_post)
            hx-swap="none"
            _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .is-open from closest .modal-overlay then reset me"
        {
            div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0"
            {
                h2 { (title) }
                button
                    type="button"
                    class="text-2xl text-muted hover:text-fg cursor-pointer bg-transparent border-none p-1 leading-none"
                    _=(format!("on click remove .is-open from closest .modal-overlay then reset #{}", form_id))
                { "×" }
            }
            div class="overflow-y-auto flex-1 min-h-0 p-6" { (body) }
            div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
                button
                    type="button"
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    _=(format!("on click remove .is-open from closest .modal-overlay then reset #{}", form_id))
                { "取消" }
                button
                    type="submit"
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                { (submit_label) }
            }
        }
    })
}
