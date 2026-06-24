//! 物料可用性 4 级徽章组件 — 工单工作台 detail-header / 列表行用。
//!
//! 借鉴 Odoo components_availability：success(warn/danger) 4 级胶囊徽章，点击 drill-down
//! 到对应 disclosure（工作台）或仅展示（列表 mini 变体）。
//!
//! level 映射（颜色严格用 UnoCSS 语义 token，禁硬编码 hex）：
//! - Available → success（绿）
//! - Expected  → accent（蓝，齐套需等在途但 ≤ 计划开工）
//! - Late      → warn（黄，在途晚于计划开工）
//! - Unavailable → danger（红，ATP 不足且在途补不齐）

use abt_core::mes::work_order::{MaterialAvailability, MaterialAvailabilityLevel};
use maud::{Markup, html};

/// level → (语义色 token 前缀, 中文标签)
fn level_meta(l: MaterialAvailabilityLevel) -> (&'static str, &'static str) {
    use MaterialAvailabilityLevel::*;
    match l {
        Available => ("success", "齐套"),
        Expected => ("accent", "待齐套"),
        Late => ("warn", "迟料"),
        Unavailable => ("danger", "缺料"),
    }
}

/// 完整徽章（工作台 detail-header 用）：4 级胶囊，点击 drill-down 展开 `target_id` disclosure。
///
/// 文案：`<标签> · <headline>`（headline 是最严重缺料/迟料物料名）。
pub fn material_badge(avail: &MaterialAvailability, target_id: &str) -> Markup {
    let (token, label) = level_meta(avail.level);
    html! {
        span
            class=({
                format!(
                    "mat-badge inline-flex items-center gap-[5px] px-[11px] py-[5px] rounded-full text-xs font-semibold cursor-pointer shrink-0 hover:brightness-96 transition-filter duration-150 bg-{}-bg text-{} border border-current/20",
                    token, token
                )
            })
            title="点击查看物料明细"
            // 纯前端 drill-down：展开目标 disclosure 并滚动定位（不向后端发请求）
            _=(format!(
                "on click add .open to #{} then call #{}.scrollIntoView() with {{behavior:'smooth',block:'center'}}",
                target_id, target_id
            ))
        {
            (crate::components::icon::info_icon(&format!("w-[14px] h-[14px] text-{}", token)))
            (label)
            @if let Some(h) = avail.headline.as_ref() {
                span class="opacity-70" { "· " (h) }
            }
        }
    }
}

/// mini 徽章（列表行用）：降级展示，无 headline、无 drill-down。
/// 仅传入 level + headline（列表降级为 Available/Unavailable 两级时 headline 可空）。
pub fn material_badge_mini(level: MaterialAvailabilityLevel, headline: Option<&str>) -> Markup {
    let (token, label) = level_meta(level);
    html! {
        span
            class=({
                format!(
                    "mat-badge-mini inline-flex items-center gap-[3px] px-2 py-[3px] rounded-full text-[11px] font-semibold bg-{}-bg text-{} border border-current/20",
                    token, token
                )
            })
        {
            span class=({ format!("inline-block w-1.5 h-1.5 rounded-full bg-{}", token) }) {}
            (label)
            @if let Some(h) = headline {
                span class="opacity-70 truncate max-w-[120px]" { "· " (h) }
            }
        }
    }
}
