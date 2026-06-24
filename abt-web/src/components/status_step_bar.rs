//! 状态步骤条组件 — 工单工作台 detail-header 用。
//!
//! 借鉴 Odoo statusbar：N 个步骤（done/active/pending），圆点 + 连线。
//! 输入来自 `WorkOrderHubSummary.status_steps: Vec<StatusStep>`。
//! step.state ∈ {Done, Active, Pending}。

use abt_core::mes::work_order::{StepState, StatusStep};
use maud::{Markup, html};

/// 渲染状态步骤条。
///
/// 步骤之间渲染连接线：前一步 Done → 线染 success 色；否则 border-soft。
pub fn status_step_bar(steps: &[StatusStep]) -> Markup {
    html! {
        div class="wo-steps flex items-start gap-0" {
            @for (i, step) in steps.iter().enumerate() {
                @if i > 0 {
                    // 连线：前一步 done 则染色
                    @let prev_done = matches!(steps[i - 1].state, StepState::Done);
                    span
                        class=({
                            format!(
                                "wo-step-line flex-1 h-0.5 min-w-[20px] mt-3 {}",
                                if prev_done { "bg-success" } else { "bg-border-soft" }
                            )
                        });
                }
                // 圆点 + 标签（样式按 state 区分）
                @let (dot_cls, label_cls) = match step.state {
                    StepState::Done => ("bg-success text-white border-success", "text-fg-2"),
                    StepState::Active => (
                        "bg-accent text-white border-accent ring-4 ring-accent-bg",
                        "text-accent font-semibold",
                    ),
                    StepState::Pending => ("bg-bg text-muted border-border-soft", "text-muted"),
                };
                div class="wo-step flex flex-col items-center gap-[5px] shrink-0" {
                    span class=({
                        format!(
                            "wo-step-dot w-[26px] h-[26px] rounded-full flex items-center justify-center text-xs font-bold border-2 {}",
                            dot_cls
                        )
                    }) {
                        @match step.state {
                            StepState::Done => { (crate::components::icon::check_circle_icon("w-3 h-3")) }
                            StepState::Active => { "●" }
                            StepState::Pending => { "" }
                        }
                    }
                    span class=({
                        format!(
                            "wo-step-label text-[11px] font-medium whitespace-nowrap {}",
                            label_cls
                        )
                    }) { (step.label) }
                }
            }
        }
    }
}
