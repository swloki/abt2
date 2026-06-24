//! Disclosure（折叠区块）组件 — 工单工作台用。
//!
//! 借鉴 Odoo 渐进式披露：head 可点击展开/收起 body，head 内可放 title + summary 文案 +
//! 可选的操作按钮（di-action）。alert=true 时 icon 容器右上角挂红点（异常提示）。
//!
//! 交互由 Hyperscript 闭环（不向后端发请求）：head 点击时在最近的 `.disclosure` 根上
//! toggle `.open`。根元素挂 `[&.open_.di-body]:block` 与 `[&.open_.di-chev_svg]:rotate-180`
//! 两个 UnoCSS 后代选择器变体，分别控制 body 展开、chevron 旋转。
//!
//! 当 `refresh_trigger` 提供时，外层 disclosure 自带局部刷新属性
//! （hx-trigger / hx-get / hx-select / hx-swap=outerHTML），用于在 drawer 提交后
//! （HX-Trigger 广播事件）局部刷新本区块。

use maud::{Markup, html};

/// 渲染一个 disclosure 折叠区块。
///
/// - `id` — disclosure 根 div 的 HTML id（也是 hx-select 的目标）
/// - `title` — head 标题
/// - `icon_html` — head 左侧 icon（`Markup`，调用方传 `icon::xxx_icon(...)`）
/// - `summary` — head 右侧摘要文案（None 时不渲染）；可含内联 HTML（如异常色 span）
/// - `alert` — true 时 icon 容器右上角挂红点（缺料/报废等异常）
/// - `action_html` — head 内操作按钮（None 时不渲染）；调用方负责 `_="on click halt the event then ..."` 阻止冒泡
/// - `body` — 展开后的内容
/// - `detail_path` — 整页端点路径（OrderDetailPath{id}.to_string()），用于局部刷新
/// - `refresh_trigger` — 局部刷新事件名（如 `"batchChanged"`）；None 时不挂 hx-trigger
#[allow(clippy::too_many_arguments)]
pub fn disclosure(
    id: &str,
    title: &str,
    icon_html: Markup,
    summary: Option<&str>,
    alert: bool,
    action_html: Option<Markup>,
    body: Markup,
    detail_path: &str,
    refresh_trigger: Option<&str>,
) -> Markup {
    // 触发器值：有 refresh_trigger 时形如 "batchChanged from:body"，否则空串（该 disclosure 不自刷新）。
    // 用普通属性赋值（而非 Maud 条件属性语法）以支持带连字符的 hx-trigger。
    let trigger_val = match refresh_trigger {
        Some(t) => format!("{} from:body", t),
        None => String::new(),
    };
    html! {
        div id=(id)
            class="disclosure bg-bg border border-border-soft rounded-md mb-3 shadow-[var(--shadow-xs)] overflow-hidden [&.open_.di-body]:block [&.open_.di-chev_svg]:rotate-180"
            hx-get=(detail_path)
            hx-select=(format!("#{}", id))
            hx-swap="outerHTML"
            hx-disinherit="hx-select"
            hx-trigger=(trigger_val)
        {
            // head：点击在最近的 .disclosure 根上 toggle .open
            div class="di-head flex items-center gap-3 px-5 py-4 cursor-pointer select-none hover:bg-surface-raised transition-colors duration-150"
                _="on click toggle .open on closest .disclosure"
            {
                div class="di-icon relative w-[30px] h-[30px] rounded-md flex items-center justify-center shrink-0 bg-surface text-fg-2" {
                    (icon_html)
                    @if alert {
                        span class="dot-alert absolute -top-[3px] -right-[3px] w-[9px] h-[9px] rounded-full bg-danger ring-2 ring-bg" {};
                    }
                }
                span class="di-title text-sm font-semibold text-fg shrink-0" { (title) }
                @if let Some(s) = summary {
                    span class="di-summary text-xs text-muted font-mono flex-1 min-w-0 overflow-hidden text-ellipsis whitespace-nowrap" {
                        (maud::PreEscaped(s))
                    }
                }
                @if let Some(act) = action_html {
                    (act)
                }
                // chevron：根 .open 时由根类的 [&.open_.di-chev_svg]:rotate-180 旋转
                span class="di-chev text-muted shrink-0 transition-transform duration-200" {
                    (crate::components::icon::chevron_down_icon("w-[18px] h-[18px]"))
                }
            }
            // body：默认 hidden；根 .disclosure.open 时由根类 [&.open_.di-body]:block 显示
            div class="di-body hidden px-5 pb-5 border-t border-border-soft" {
                (body)
            }
        }
    }
}

/// di-action 按钮（head 内的操作入口，如「申请领料」「报工」）。
///
/// 点击时 `halt the event` 阻止冒泡到 head（避免误触发展开），然后执行 `then_open`
/// （通常是 `add .open to #xxx-drawer`）。
pub fn di_action(label: &str, then_open: &str) -> Markup {
    html! {
        button type="button"
            class="di-action inline-flex items-center gap-[3px] px-[11px] py-[5px] text-xs font-semibold text-accent bg-accent-bg border border-transparent rounded-sm cursor-pointer shrink-0 hover:bg-[rgba(37,99,235,0.14)] transition-all duration-150"
            _=(format!("on click halt the event then {}", then_open))
        {
            (crate::components::icon::plus_icon("w-[13px] h-[13px]"))
            (label)
        }
    }
}
