//! overlay_shell —— modal / drawer 外壳的统一组件。
//!
//! 把「overlay 容器 + 显隐 + Esc 关闭 + afterSettle 打开守卫」收敛为**单一来源**。
//! modal / drawer / picker 全部基于本组件 → 显隐 / 关闭 / 动画以后只改这里一处。
//! body 由调用方传入（静态插槽）或动态 `hx-get` swap（模式 B 保留）。
//!
//! 详见 `docs/frontend/htmx-patterns.md` §3。

use maud::{Markup, html};

/// 生成 overlay 的显隐 + 关闭 hyperscript —— 全项目 modal/drawer 显隐与关闭的【单一来源】。
///
/// - `htmx:afterSettle[me is event.target] add .<open_class>`：内容 swap 进本容器时打开。
///   守卫 `me is event.target` 防保存 form（`hx-swap=none`）冒泡的 afterSettle 误开（见 htmx-patterns §3.2 陷阱三）。
/// - `keydown Escape from body remove .<open_class>`：Esc 关闭。`from body` 保证焦点不在弹窗内时也能触发
///   （原生 keydown 事件可靠；区别于 `from:body` 监听 htmx 自定义事件不可靠）。
fn overlay_hs(open_class: &str) -> String {
    format!(
        "on 'htmx:afterSettle'[me is event.target] add .{open_class}\non keydown[event.key is 'Escape'] from body remove .{open_class}"
    )
}

/// Modal 外壳：居中 overlay + 背景模糊 + `[&.is-open]` variant 显隐 + Esc。
///
/// - `z_class`：层叠层级，普通 `"z-[1000]"`、嵌套 picker `"z-[1100]"`。
/// - overlay 带 `modal-overlay` 标记 class，兼容现有 `remove .is-open from closest .modal-overlay` 关闭写法（接入时无需改关闭逻辑）。
/// - `inner`：居中的 modal 卡片（header + body + footer），或动态加载时的空 body 容器。
pub fn modal_shell(id: &str, z_class: &str, inner: Markup) -> Markup {
    html! {
        div id=(id)
            class=(format!(
                "{z_class} modal-overlay fixed inset-0 grid place-items-center bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto"
            ))
            _=(overlay_hs("is-open")) {
            (inner)
        }
    }
}

/// Drawer 外壳：右侧 overlay + `drawer-overlay`（触发 preflight 显隐 + translateX 滑动动画）+ `.open` + Esc。
///
/// - 必须保留 `drawer-overlay` class —— preflight（`uno.config.ts`）靠它做 `display:flex` + `translateX` 动画，不能用 variant 替代。
/// - `width_class`：panel 宽度（如 `"w-[640px]"`）；shell 内置 `max-w-[92vw]` 小屏保护。
/// - `inner`：panel 内容（header + body + footer）。
pub fn drawer_shell(id: &str, width_class: &str, inner: Markup) -> Markup {
    html! {
        div id=(id)
            class="drawer-overlay fixed inset-0 z-[90] flex justify-end bg-slate-900/40"
            _=(overlay_hs("open")) {
            div class=(format!("drawer-panel bg-bg h-full {width_class} max-w-[92vw] shadow-lg flex flex-col")) {
                (inner)
            }
        }
    }
}
