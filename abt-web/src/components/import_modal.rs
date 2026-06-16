use maud::{html, Markup};

/// 导入 Modal 配置
pub struct ImportModalConfig {
    pub import_type: &'static str,
    pub title: &'static str,
    pub template_columns: &'static str,
}

/// 渲染导入 Modal（页面底部声明，Surreal.js 控制 is-open）
/// modal ID 由 import_type 派生，确保唯一性
pub fn import_modal(config: &ImportModalConfig) -> Markup {
    let modal_id = format!("import-modal-{}", config.import_type);
    html! {
        div id=(modal_id) class="modal-overlay" _="on click[me is event.target] remove .is-open" {
            div class="modal modal-import" {
                div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                    h2 { (config.title) }
                    button type="button" class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg"
                        _="on click remove .is-open from closest .modal-overlay" { "×" }
                }
                div class="overflow-y-auto flex-1 min-h-0 p-6" {
                    div id=(format!("import-content-{}", config.import_type)) {
                        (render_import_form(config))
                    }
                }
            }
        }
    }
}

/// 生成导入按钮的 Hyperscript `_` 属性值（供页面文件使用）
pub fn import_modal_onclick(config: &ImportModalConfig) -> String {
    format!("on click add .is-open to #import-modal-{}", config.import_type)
}

/// 初始状态：文件选择区 + 模板下载
fn render_import_form(config: &ImportModalConfig) -> Markup {
    let template_path = format!("/excel/template/{}", config.import_type);
    let upload_path = format!("/excel/import/{}", config.import_type);
    let content_id = format!("import-content-{}", config.import_type);

    html! {
        div class="import-file-zone" {
            p class="import-cols" { "列格式：" (config.template_columns) }
            a href=(template_path) class="btn bg-white text-fg border border-border hover:bg-surface" download {
                (crate::components::icon::download_icon("w-4 h-4"))
                " 下载模板"
            }
            form
                hx-post=(upload_path)
                hx-target=(format!("#{}", content_id))
                hx-swap="innerHTML"
                hx-encoding="multipart/form-data"
                hx-indicator=(format!("#{} .htmx-indicator", content_id)) {
                input type="file" name="file" accept=".xlsx" required;
                div class="import-actions" {
                    button type="submit" class="btn bg-accent text-accent-on border-none hover:bg-accent-hover" {
                        "开始导入"
                    }
                    div class="htmx-indicator" {
                        "上传中..."
                    }
                }
            }
        }
    }
}

/// 进行中状态：进度条 + 轮询触发器（公开，handler 调用）
pub fn render_import_progress(import_type: &str, task_id: i64, current: usize, total: usize) -> Markup {
    let pct = if total > 0 { (current * 100) / total } else { 0 };
    let progress_path = format!("/excel/import/{}/progress/{}", import_type, task_id);
    let content_id = format!("import-content-{}", import_type);

    html! {
        div class="import-progress" {
            p { "正在导入... " (current) "/" (total) }
            div class="import-progress-bar" {
                div class="import-progress-fill" style=(format!("width:{}%", pct)) {}
            }
        }
        div hx-get=(progress_path)
             hx-trigger="every 1s"
             hx-target=(format!("#{}", content_id))
             hx-swap="innerHTML" {}
    }
}

/// 完成状态：结果统计 + 错误详情（公开，handler 调用）
pub fn render_import_result(result: &abt_core::shared::excel::ImportResult) -> Markup {
    html! {
        div class="import-result" {
            div class="import-result-stats" {
                div class="import-stat" {
                    span class="import-text-2xl font-bold font-mono tabular-nums text-fg success" { (result.success_count) }
                    span class="import-text-sm text-muted mt-1" { "成功" }
                }
                div class="import-stat" {
                    span class="import-text-2xl font-bold font-mono tabular-nums text-fg failed" { (result.failed_count) }
                    span class="import-text-sm text-muted mt-1" { "失败" }
                }
            }
            @if !result.row_errors.is_empty() {
                div class="import-errors" {
                    p class="import-error-title" { "错误详情：" }
                    ul {
                        @for err in &result.row_errors {
                            li {
                                "第 " (err.row_index) " 行，列 \"" (err.column_name) "\"："
                                (err.reason)
                                @if let Some(ref v) = err.raw_value {
                                    " (" (v) ")"
                                }
                            }
                        }
                    }
                }
            }
            @if !result.errors.is_empty() {
                div class="import-errors import-error-extra" {
                    p class="import-error-title" { "其他错误：" }
                    ul {
                        @for err in &result.errors {
                            li { (err) }
                        }
                    }
                }
            }
            div class="import-footer-actions" {
                button type="button" class="btn bg-white text-fg border border-border hover:bg-surface"
                    _="on click remove .is-open from closest .modal-overlay" { "关闭" }
            }
        }
    }
}
