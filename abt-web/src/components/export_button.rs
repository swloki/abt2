use maud::{html, Markup};

/// 导出项配置
pub struct ExportItem {
    pub label: &'static str,
    pub export_type: &'static str,
}

/// 单个导出按钮
pub fn export_button(label: &str, export_type: &str) -> Markup {
    let path = format!("/excel/export/{}", export_type);
    html! {
        button type="button" class="btn btn-default"
            hx-post=(path)
            hx-target="#export-result"
            hx-swap="innerHTML"
            hx-indicator="#export-result" {
            (crate::components::icon::download_icon("w-4 h-4"))
            " " (label)
        }
    }
}

/// 导出下拉菜单（多种导出类型）
pub fn export_dropdown(items: &[ExportItem]) -> Markup {
    html! {
        div class="export-dropdown" {
            button type="button" class="btn btn-default" {
                (maud::PreEscaped("<script>me().on('click',function(ev){me(ev).nextElementSibling.classList.toggle('is-open')})</script>"))
                (crate::components::icon::download_icon("w-4 h-4"))
                " 导出"
            }
            div class="export-dropdown-menu" {
                @for item in items {
                    (export_menu_item(item))
                }
            }
        }
    }
}

/// 导出菜单项
fn export_menu_item(item: &ExportItem) -> Markup {
    let path = format!("/excel/export/{}", item.export_type);
    html! {
        button type="button"
            hx-post=(path)
            hx-target="#export-result"
            hx-swap="innerHTML"
            hx-indicator="#export-result"
            onclick="hsRemoveClosest(this,'.export-dropdown-menu','is-open')" {
            (item.label)
        }
    }
}

/// 导出结果区域 HTML 片段（handler 调用）
pub fn render_export_result(task_id: i64, filename: &str) -> Markup {
    let download_path = format!("/excel/export/download/{}", task_id);
    html! {
        div class="export-result" {
            "✓ 导出完成"
            a href=(download_path) class="btn btn-sm btn-primary" download {
                (crate::components::icon::download_icon("w-3.5 h-3.5"))
                " " (filename) ".xlsx"
            }
        }
    }
}