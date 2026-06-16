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
        button type="button" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-white text-fg border border-border hover:bg-surface"
            hx-post=(path)
            hx-confirm=(confirm_msg)
            hx-swap="none" {
            (crate::components::icon::download_icon("w-4 h-4"))
            " " (label)
        }
    }
}

/// 导出下拉菜单（多种导出类型）
pub fn export_dropdown(items: &[ExportItem]) -> Markup {
    html! {
        div class="relative inline-block" {
            button type="button" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-white text-fg border border-border hover:bg-surface"
                _="on click toggle .is-open on next <div/>" {
                (crate::components::icon::download_icon("w-4 h-4"))
                " 导出"
            }
            div class="relative inline-block-menu" {
                @for item in items {
                    (export_menu_item(item))
                }
            }
        }
    }
}

/// 导出菜单项（点击弹确认框，确认后直接下载）
fn export_menu_item(item: &ExportItem) -> Markup {
    let path = format!("{}/{}", crate::routes::excel::EXPORT_START_PATH, item.export_type);
    let confirm_msg = format!("确定要导出「{}」吗？", item.label);
    html! {
        button type="button"
            hx-post=(path)
            hx-confirm=(confirm_msg)
            hx-swap="none"
            _="on click remove .is-open from closest .export-dropdown-menu" {
            (item.label)
        }
    }
}
