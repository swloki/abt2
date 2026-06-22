use maud::{html, Markup};

/// 导出项配置
pub struct ExportItem {
 pub label: &'static str,
 pub export_type: &'static str,
}

/// 单个导出按钮（点击弹确认框，确认后直接下载）
pub fn export_button(label: &str, export_type: &str) -> Markup {
 let path = format!("{}/{}", crate::routes::excel::EXPORT_START_PATH, export_type);
 let confirm_msg = format!("确定要导出「{}」吗？", label);
 html! {
    button
        type="button"
        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
        hx-post=(path)
        hx-confirm=(confirm_msg)
        hx-swap="none"
    { (crate::components::icon::download_icon("w-4 h-4")) " " (label) }
}
}

/// 导出下拉菜单（多种导出类型）
pub fn export_dropdown(items: &[ExportItem]) -> Markup {
 html! {
    div class="relative inline-block" {
        button
            type="button"
            class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
            _="on click if next .export-menu's style's display is 'none'
 then show next .export-menu
 else hide next .export-menu"
        { (crate::components::icon::download_icon("w-4 h-4")) " 导出" }
        div class="export-menu absolute right-0 top-full mt-1 bg-white border border-border rounded-sm shadow-[var(--shadow-card)] z-50 min-w-[160px] py-1"
            style="display:none"
        {
            @for item in items { (export_menu_item(item)) }
        }
    }
}
}

/// 导出菜单项（点击弹确认框，确认后直接下载）
fn export_menu_item(item: &ExportItem) -> Markup {
 let path = format!("{}/{}", crate::routes::excel::EXPORT_START_PATH, item.export_type);
 let confirm_msg = format!("确定要导出「{}」吗？", item.label);
 html! {
    button
        type="button"
        class="w-full text-left px-4 py-2 text-sm text-fg-2 hover:bg-accent-bg hover:text-accent transition-colors cursor-pointer border-none bg-transparent"
        hx-post=(path)
        hx-confirm=(confirm_msg)
        hx-swap="none"
        _="on click hide closest .export-menu"
    { (item.label) }
}
}
