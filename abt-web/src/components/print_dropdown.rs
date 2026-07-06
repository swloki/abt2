use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::print_template::PrintTemplate;

/// 打印入口统一的 query 参数：可选指定模板 id。
/// - `None` → 用该单据 `document_type` 的默认模板（`render_default`）
/// - `Some(id)` → 用指定模板（`render`）
#[derive(Debug, Deserialize, Clone, Default)]
pub struct PrintParam {
    #[serde(default)]
    pub template_id: Option<i64>,
}

/// 打印按钮 dropdown：主按钮用默认模板直打；▾ 下拉列出该 `document_type` 的全部模板
/// （选某个 → 带 `?template_id=N` 打印）+ 「管理打印模板」入口。
///
/// 显隐用 `group` + `.is-open`（与 `layout/header.rs` 用户菜单同款）：父容器
/// `toggle .is-open` / `from elsewhere remove .is-open` 管开关；主按钮与菜单项
/// `halt the event` 阻止冒泡，避免与父容器 toggle 冲突。各选项通过设置页面级隐藏
/// iframe 的 src 触发打印（响应自带 `window.print()`）。
///
/// - `frame_id`：页面隐藏 iframe 的 id（不同页面不同：详情页 `print-frame`、工作中心 `wc-print-frame`）
/// - `default_print_url`：不带 template_id 的打印 URL（走默认模板），如 `/admin/wms/shipping/123/print`
/// - `templates`：该 `document_type` 全部模板（`list_by_document_type`，默认置顶）
/// - `manage_url`：模板管理页 URL，如 `/admin/system/print-templates?document_type=delivery_note`
/// - `compact`：紧凑尺寸（drawer 头部小按钮）；false 为详情页 toolbar 标准尺寸
pub fn print_dropdown(
    frame_id: &str,
    default_print_url: &str,
    templates: &[PrintTemplate],
    manage_url: &str,
    compact: bool,
) -> Markup {
    // 主按钮 / ▾ 按钮共用基础样式（不含圆角，由 split 逻辑控制）。compact 用 drawer 小尺寸。
    let base = if compact {
        "inline-flex items-center gap-1.5 px-3 py-1.5 bg-white text-fg-2 border border-border \
         text-xs font-medium cursor-pointer hover:bg-surface hover:text-accent transition-colors \
         shadow-xs"
    } else {
        "inline-flex items-center gap-2 py-[9px] px-[18px] bg-white text-fg-2 border border-border \
         hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium \
         cursor-pointer transition-all duration-150 shadow-xs"
    };
    let radius = if compact { "rounded-md" } else { "rounded-sm" };
    let (printer_cls, chevron_cls) = if compact {
        ("w-3.5 h-3.5", "w-3 h-3")
    } else {
        ("w-4 h-4", "w-3.5 h-3.5")
    };
    html! {
        div class="print-dropdown group relative inline-flex"
            _="on click toggle .is-open then on click from elsewhere remove .is-open"
        {
            // 主按钮：直接用默认模板打印（halt 阻止冒泡，不触发父容器 toggle）
            button type="button"
                class=(format!("{base} {radius} rounded-r-none border-r-0"))
                _=(format!("on click halt the event then set #{frame_id}'s src to '{default_print_url}'"))
                title="使用默认模板打印"
            {
                (crate::components::icon::printer_icon(printer_cls)) "打印"
            }
            // ▾ 按钮：点开/收起菜单（冒泡到父容器 toggle .is-open）
            button type="button" aria-label="选择打印模板"
                class=(format!("{base} {radius} rounded-l-none px-2 justify-center"))
                title="选择打印模板"
            {
                (crate::components::icon::chevron_down_icon(chevron_cls))
            }
            // 菜单：group-[.is-open] 显隐（header.rs 同款）
            div class="absolute top-full right-0 mt-1 min-w-[208px] bg-surface border \
                       border-border rounded-md shadow-lg p-1.5 z-[60] opacity-0 invisible \
                       -translate-y-1 transition-all duration-150 \
                       group-[.is-open]:opacity-100 group-[.is-open]:visible \
                       group-[.is-open]:translate-y-0"
            {
                @if templates.is_empty() {
                    div class="px-3 py-2 text-xs text-muted" { "暂无可用模板，请先在模板管理中创建" }
                } @else {
                    @for tpl in templates {
                        button type="button"
                            class="w-full flex items-center gap-2 px-3 py-2 text-sm text-fg-2 \
                                   hover:bg-accent-bg hover:text-accent rounded-sm transition-colors \
                                   cursor-pointer border-none bg-transparent text-left"
                            _=(format!(
                                "on click halt the event then set #{frame_id}'s src to \
                                 '{default_print_url}?template_id={}' then remove .is-open \
                                 from closest .print-dropdown",
                                tpl.id
                            ))
                        {
                            span class="flex-1 truncate" { (tpl.name) }
                            @if tpl.is_default {
                                span class="text-[10px] text-accent font-semibold shrink-0" { "默认" }
                            }
                        }
                    }
                }
                div class="border-t border-border-soft mt-1 pt-1" {
                    a class="flex items-center gap-2 px-3 py-2 text-sm text-muted \
                             hover:text-accent rounded-sm transition-colors"
                        href=(manage_url)
                    {
                        (crate::components::icon::sliders_icon("w-3.5 h-3.5")) "管理打印模板"
                    }
                }
            }
        }
    }
}
