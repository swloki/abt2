//! 行展开控件 — 表格行点击 chevron 就地懒加载明细子表的统一范式。
//!
//! 复用面：采购订单 / 对账 / 付款行展开（`pages/purchase_work_center.rs`），
//! 后续 mes/wms/fms 等工作中心的列表行明细展开均可复用。配套单端点 row-detail
//! GET handler 返回 [`row_expand_detail`]。
//!
//! 健壮范式（规避「重复点击插重复明细行」quirk）：
//! - 触发器 `hx-trigger="loadItem"`（自定义事件，默认 click 不发请求）
//!   + `hx-target="closest <tr/>"` + `hx-swap="afterend"`（明细行插到本行之后，结构合法）。
//! - hyperscript：click → toggle `.open` on 最近 tr；展开则 `trigger loadItem`（htmx 拉明细注入），
//!   收起则按 id `remove`（不发请求 → 无重复行）。
//! - chevron 旋转由调用方在 tr class 挂 `[&.open_.row-chev]:rotate-90`（UnoCSS 后代选择器，
//!   同 [`crate::components::disclosure`] 范式）。
//!
//! 调用方职责：① `row_id` 全局唯一前缀；② 提供 row-detail GET handler（返回
//! [`row_expand_detail`]，`row_id` 与触发器一致）；③ tr 加旋转 variant；④ thead 留出
//! chevron 列、空行/展开行 colspan 对齐。

use maud::{html, Markup};

use crate::components::icon;

/// 行展开触发按钮（放 `<td>` 内）。`row_id` 为展开行 tr 的 id（收起按此移除），
/// `detail_url` 为 row-detail GET 端点（返回 [`row_expand_detail`]）。
///
/// tr 需挂 `[&.open_.row-chev]:rotate-90` 控制 chevron 旋转（见模块文档）。
pub fn row_expand_toggle(row_id: &str, detail_url: &str) -> Markup {
    html! {
        button class="row-expand-toggle inline-flex items-center justify-center w-[26px] h-[26px] border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg align-middle transition-all"
                title="展开详情"
                aria-label="展开详情"
                hx-get=(detail_url)
                hx-trigger="loadItem"
                hx-target="closest <tr/>"
                hx-swap="afterend"
                _=(format!(
                    "on click toggle .open on closest <tr/> then if (closest <tr/> matches .open) then trigger loadItem on me else add .closing to first .row-expand-anim in #{} then wait 220ms then remove #{} end",
                    row_id, row_id
                )) {
            (icon::chevron_right_icon("row-chev w-[15px] h-[15px]"))
        }
    }
}

/// 行展开明细行外壳（row-detail handler 返回它）。`row_id` 与触发器一致（收起按此移除），
/// `colspan` 对齐列表列数，`children` 为明细内容（子表 / 卡片）。
pub fn row_expand_detail(row_id: &str, colspan: u32, children: Markup) -> Markup {
    html! {
        tr class="row-detail" id=(row_id) {
            td colspan=(colspan.to_string()) class="p-0 border-none bg-surface-raised" {
                div class="row-expand-anim" { (children) }
            }
        }
    }
}
