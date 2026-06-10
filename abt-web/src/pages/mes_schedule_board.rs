use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;

use abt_core::mes::dashboard::MesDashboardService;
use abt_core::mes::enums::BatchStatus;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_batch::ScheduleBoardPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[require_permission("MES", "read")]
pub async fn get_schedule_board(_path: ScheduleBoardPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    let svc = state.mes_dashboard_service();
    let stats = svc.get_schedule_stats(&service_ctx, &mut conn).await?;
    let cards = svc.get_schedule_cards(&service_ctx, &mut conn).await?;

    let content = schedule_board_page(&stats, &cards);
    Ok(Html(admin_page(is_htmx, "排程看板", &claims, "production", ScheduleBoardPath::PATH, "生产管理", None, content, &nav_filter).into_string()))
}

fn schedule_board_page(
    stats: &abt_core::mes::dashboard::model::ScheduleStats,
    cards: &[abt_core::mes::dashboard::model::ScheduleCard],
) -> Markup {
    // Group cards by status
    let pending: Vec<_> = cards.iter().filter(|c| c.status == BatchStatus::Pending).collect();
    let in_progress: Vec<_> = cards.iter().filter(|c| matches!(c.status, BatchStatus::InProgress | BatchStatus::Suspended)).collect();
    let pending_receipt: Vec<_> = cards.iter().filter(|c| c.status == BatchStatus::PendingReceipt).collect();
    let completed: Vec<_> = cards.iter().filter(|c| c.status == BatchStatus::Completed).collect();

    html! { div {
        div class="page-header" {
            h1 class="page-title" { "排程看板" }
        }

        // Stats row
        div class="board-stats" {
            div class="stat-card" {
                div class="stat-card-value" { (stats.active_orders) }
                div class="stat-card-label" { "活跃工单" }
            }
            div class="stat-card" {
                div class="stat-card-value stat-pending" { (stats.pending_batches) }
                div class="stat-card-label" { "待排产" }
            }
            div class="stat-card" {
                div class="stat-card-value stat-progress" { (stats.in_progress_batches) }
                div class="stat-card-label" { "进行中" }
            }
            div class="stat-card" {
                div class="stat-card-value stat-receipt" { (stats.pending_receipt_batches) }
                div class="stat-card-label" { "待入库" }
            }
            div class="stat-card" {
                div class="stat-card-value stat-done" { (stats.completed_batches) }
                div class="stat-card-label" { "已完成" }
            }
        }

        // Kanban board - 4 columns
        div class="kanban-board" {
            // Column: Pending
            (kanban_column("待排产", &pending, "kanban-col-pending"))

            // Column: In Progress
            (kanban_column("进行中", &in_progress, "kanban-col-progress"))

            // Column: Pending Receipt
            (kanban_column("待入库", &pending_receipt, "kanban-col-receipt"))

            // Column: Completed
            (kanban_column("已完成", &completed, "kanban-col-done"))
        }
    }}
}

fn kanban_column(
    title: &str,
    cards: &[&abt_core::mes::dashboard::model::ScheduleCard],
    col_class: &str,
) -> Markup {
    html! {
        div class=(format!("kanban-column {col_class}")) {
            div class="kanban-col-header" {
                span class="kanban-col-title" { (title) }
                span class="kanban-col-count" { (cards.len()) }
            }
            div class="kanban-col-body" {
                @for card in cards {
                    (kanban_card(card))
                }
                @if cards.is_empty() {
                    div class="kanban-empty" { "暂无数据" }
                }
            }
        }
    }
}

fn kanban_card(card: &abt_core::mes::dashboard::model::ScheduleCard) -> Markup {
    let progress_pct = if card.batch_qty > Decimal::ZERO {
        let pct = (card.completed_qty / card.batch_qty * rust_decimal::Decimal::ONE_HUNDRED)
            .min(rust_decimal::Decimal::ONE_HUNDRED);
        pct.to_string()
    } else {
        "0".to_string()
    };

    let (status_label, status_cls) = match card.status {
        BatchStatus::Pending => ("待排产", "pill-pending"),
        BatchStatus::InProgress => ("进行中", "pill-progress"),
        BatchStatus::Suspended => ("已暂停", "pill-suspended"),
        BatchStatus::PendingReceipt => ("待入库", "pill-receipt"),
        BatchStatus::Completed => ("已完成", "pill-done"),
        _ => ("", ""),
    };

    let step_display = if card.current_step == 0 {
        "未开始".to_string()
    } else {
        let total = card.total_steps.unwrap_or(0);
        let name = card.current_step_name.as_deref().unwrap_or("—");
        format!("{}/{} {}", card.current_step, total, name)
    };

    html! {
        a class="kanban-card" href=(format!("/admin/mes/batches/{}", card.id)) {
            div class="kanban-card-top" {
                span class="kanban-card-no mono" { (card.batch_no) }
                span class=(format!("kanban-card-pill {status_cls}")) { (status_label) }
            }
            div class="kanban-card-product" {
                (card.product_name.as_deref().unwrap_or("—"))
            }
            div class="kanban-card-meta" {
                span { (crate::utils::fmt_qty(card.completed_qty)) " / " (crate::utils::fmt_qty(card.batch_qty)) }
            }
            @if card.current_step > 0 {
                div class="kanban-card-progress" {
                    div class="progress-bar" {
                        div class="progress-fill" style=(format!("width:{}%", progress_pct)) {}
                    }
                    span class="progress-text" { (step_display) }
                }
            }
            @if !card.wo_doc_number.as_ref().is_none_or(|s| s.is_empty()) {
                div class="kanban-card-tag" {
                    "工单 " (card.wo_doc_number.as_deref().unwrap_or(""))
                }
            }
        }
    }
}
