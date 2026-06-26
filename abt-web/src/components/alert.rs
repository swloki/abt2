//! Alert 行内提示组件 — 保存失败 / 操作反馈等场景。
//!
//! 4 个语义变体（颜色严格用 UnoCSS 语义 token，禁硬编码 hex）：
//! - error   → danger（红）+ triangle-alert
//! - warning → warn（黄）+ triangle-alert
//! - success → success（绿）+ circle-check
//! - info    → accent（蓝）+ info
//!
//! 用法：`(alert::alert_error("保存失败：xxx"))` / `alert::alert_info(...)`。

use maud::{Markup, html};

use crate::components::icon;

/// severity → (语义色 token, 图标渲染函数)。
fn meta(severity: &str) -> (&'static str, fn(&str) -> Markup) {
    match severity {
        "error" => ("danger", icon::alert_triangle_icon),
        "warning" => ("warn", icon::alert_triangle_icon),
        "success" => ("success", icon::check_circle_icon),
        _ => ("accent", icon::info_icon), // info / 未知 → accent
    }
}

/// 通用 alert。`severity` ∈ {error, warning, success, info}（其他值按 info 处理）。
pub fn alert(severity: &str, msg: &str) -> Markup {
    let (token, icon_fn) = meta(severity);
    html! {
        div class=(format!(
            "flex items-start gap-2 rounded-sm px-3 py-2 text-xs bg-{token}-bg text-{token} border border-current/20"
        )) {
            (icon_fn("w-4 h-4 shrink-0 mt-0.5"))
            span class="leading-relaxed" { (msg) }
        }
    }
}

pub fn alert_error(msg: &str) -> Markup {
    alert("error", msg)
}

pub fn alert_warning(msg: &str) -> Markup {
    alert("warning", msg)
}

pub fn alert_success(msg: &str) -> Markup {
    alert("success", msg)
}

pub fn alert_info(msg: &str) -> Markup {
    alert("info", msg)
}
