use maud::{html, Markup};

/// 通用「可搜索 select」的一个选项：`value` 进 hidden input，`label` 是显示文本。
pub struct ComboOption {
    pub value: String,
    pub label: String,
}

/// 「选中即提交」场景（如工序选择刷新整行）挂在 hidden input 上的 HTMX 属性。
/// `swap` 固定 `outerHTML`、`trigger` 固定 `change`，由组件内部写死。
pub struct ComboHx {
    pub post: String,
    pub target: String,
    pub include: String,
}

/// 通用可搜索下拉单选（combobox）。
///
/// 外观像 `<select>` 的触发按钮 + `position: fixed` 的下拉面板（搜索框 + 选项列表）。
/// fixed 定位使其在 `overflow` 容器（如工序表格的 `overflow-x-auto`）内也不被裁剪——
/// `category_select` 用 `absolute` 只能用在非 overflow 的筛选栏，本组件用 `fixed` + JS
/// 按 trigger 按钮的 `getBoundingClientRect()` 定位，兼容表格行内。
///
/// 选中 → 填 hidden input 真实值 + 更新按钮显示 + 对 hidden 派发 `change`(bubbles)。
/// 配套 JS：`comboToggle` / `filterComboOptions` / `comboSelect` / `comboClose`（`static/app.js`）。
pub fn combo_select(
    input_name: &str,
    options: &[ComboOption],
    selected_value: Option<&str>,
    placeholder: &str,
    search_placeholder: &str,
    hx: Option<&ComboHx>,
) -> Markup {
    let has_sel = selected_value.is_some_and(|v| options.iter().any(|o| o.value == v));
    let current_label = if has_sel {
        selected_value
            .and_then(|v| options.iter().find(|o| o.value == v))
            .map(|o| o.label.as_str())
            .unwrap_or(placeholder)
    } else {
        placeholder
    };
    let label_cls = if has_sel { "text-fg" } else { "text-muted" };

    html! {
        div class="combo-select w-full" {
            // hidden input 持真实值；工序场景挂 hx（选中 change → 刷新整行）
            @if let Some(hx) = hx {
                input type="hidden" name=(input_name) value=(selected_value.unwrap_or(""))
                    hx-post=(&hx.post) hx-target=(&hx.target) hx-swap="outerHTML"
                    hx-include=(&hx.include) hx-trigger="change" {};
            } @else {
                input type="hidden" name=(input_name) value=(selected_value.unwrap_or("")) {};
            }
            // 触发按钮（外观像 select）
            button type="button"
                class="combo-trigger w-full flex items-center justify-between gap-1 px-1.5 py-1 border border-border rounded-sm bg-transparent text-[13px] hover:border-accent focus:border-accent cursor-pointer"
                _="on click call comboToggle(me)" {
                span class=(format!("combo-label truncate flex-1 text-left {}", label_cls)) { (current_label) }
                svg class="w-3.5 h-3.5 text-muted shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" {
                    path d="M19 9l-7 7-7-7" {}
                }
            }
            // 下拉面板（fixed，JS 按 trigger rect 定位；默认隐藏）
            div class="combo-dropdown fixed z-[1100] bg-bg border border-border rounded-sm shadow-[var(--shadow-card)]"
                style="display:none" {
                div class="p-1.5 border-b border-border-soft" {
                    input type="text" placeholder=(search_placeholder) autocomplete="off"
                        class="combo-search w-full px-2 py-1 border border-border rounded-sm text-[13px] bg-bg text-fg outline-none focus:border-accent"
                        _="on input call filterComboOptions(me)" {};
                }
                div class="combo-list max-h-[280px] overflow-y-auto py-1" {
                    button type="button"
                        class="combo-option block w-full text-left px-2.5 py-1 text-[13px] text-fg-2 hover:bg-accent-bg hover:text-accent border-none bg-transparent cursor-pointer"
                        data-value="" data-label=(placeholder)
                        _="on click call comboSelect(me)" { (placeholder) }
                    @for opt in options {
                        button type="button"
                            class="combo-option block w-full text-left px-2.5 py-1 text-[13px] text-fg-2 hover:bg-accent-bg hover:text-accent border-none bg-transparent cursor-pointer"
                            data-value=(opt.value) data-label=(opt.label)
                            _="on click call comboSelect(me)" { (opt.label) }
                    }
                }
            }
            // 背景遮罩（点击外部关闭）
            div class="combo-backdrop fixed inset-0 z-[1099]" style="display:none"
                _="on click call comboClose(me)" {}
        }
    }
}
