use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::wms::strategy::model::*;
use abt_core::wms::strategy::StrategyService;
use abt_core::wms::enums::{PickType, PutawayType};

use crate::layout::page::admin_page;
use crate::routes::wms_strategy::{StrategyListPath, StrategyTablePath};
use crate::utils::RequestContext;

use abt_macros::require_permission;

// ── Handlers ──

#[require_permission("WMS", "read")]
pub async fn get_strategy_list(
    _path: StrategyListPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.strategy_service();

    let putaway_strategies = svc.list_putaway(&service_ctx, &mut conn, None).await?;
    let pick_strategies = svc.list_pick(&service_ctx, &mut conn, None).await?;

    let content = strategy_list_page(&putaway_strategies, &pick_strategies);
    let page_html = admin_page(
        is_htmx,
        "策略管理",
        &claims,
        "inventory",
        StrategyListPath::PATH,
        "库存管理",
        Some("策略管理"),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("WMS", "read")]
pub async fn get_strategy_table(
    _path: StrategyTablePath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.strategy_service();

    let putaway_strategies = svc.list_putaway(&service_ctx, &mut conn, None).await?;
    let pick_strategies = svc.list_pick(&service_ctx, &mut conn, None).await?;

    Ok(Html(strategy_list_page(&putaway_strategies, &pick_strategies).into_string()))
}

// ── Helpers ──

fn putaway_type_label(t: &PutawayType) -> &'static str {
    match t {
        PutawayType::SameMerge => "同物料合并",
        PutawayType::Nearest => "就近入库",
        PutawayType::FixedBin => "指定储位",
        PutawayType::EmptyFirst => "空储位优先",
    }
}

fn putaway_type_tag(t: &PutawayType) -> &'static str {
    match t {
        PutawayType::SameMerge => "SAME_MERGE",
        PutawayType::Nearest => "NEAREST",
        PutawayType::FixedBin => "FIXED_BIN",
        PutawayType::EmptyFirst => "EMPTY_FIRST",
    }
}

fn pick_type_label(t: &PickType) -> &'static str {
    match t {
        PickType::Fifo => "先进先出",
        PickType::Fefo => "先到期先出",
        PickType::ShortestPath => "最短路径",
        PickType::FullPallet => "整托优先",
    }
}

fn pick_type_tag(t: &PickType) -> &'static str {
    match t {
        PickType::Fifo => "FIFO",
        PickType::Fefo => "FEFO",
        PickType::ShortestPath => "SHORTEST_PATH",
        PickType::FullPallet => "FULL_PALLET",
    }
}

// ── Components ──

fn strategy_list_page(
    putaway_strategies: &[PutawayStrategy],
    pick_strategies: &[PickStrategy],
) -> Markup {
    html! {
        div {
            div class="page-header" {
                h1 class="page-title" { "策略管理" }
            }

            // ── 上架策略 ──
            div class="section-block" {
                div class="section-block-header" {
                    div class="section-block-title" { "上架策略" }
                }
                div class="data-card" {
                    div class="data-card-scroll" {
                        table class="data-table" style="min-width:760px" {
                            thead {
                                tr {
                                    th { "策略名称" }
                                    th { "策略类型" }
                                    th { "适用仓库" }
                                    th { "产品分类" }
                                    th { "优先级" }
                                    th { "状态" }
                                    th { "操作" }
                                }
                            }
                            tbody {
                                @for s in putaway_strategies {
                                    (putaway_row(s))
                                }
                                @if putaway_strategies.is_empty() {
                                    tr {
                                        td colspan="7" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                            "暂无上架策略"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── 拣货策略 ──
            div class="section-block" {
                div class="section-block-header" {
                    div class="section-block-title" { "拣货策略" }
                }
                div class="data-card" {
                    div class="data-card-scroll" {
                        table class="data-table" style="min-width:760px" {
                            thead {
                                tr {
                                    th { "策略名称" }
                                    th { "策略类型" }
                                    th { "适用仓库" }
                                    th { "产品分类" }
                                    th { "优先级" }
                                    th { "状态" }
                                    th { "操作" }
                                }
                            }
                            tbody {
                                @for s in pick_strategies {
                                    (pick_row(s))
                                }
                                @if pick_strategies.is_empty() {
                                    tr {
                                        td colspan="7" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                            "暂无拣货策略"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn putaway_row(s: &PutawayStrategy) -> Markup {
    let tag = putaway_type_tag(&s.strategy_type);
    let priority_class = format!("priority-{}", s.priority.min(4));
    let toggle_class = if s.is_active { "toggle-switch active" } else { "toggle-switch" };
    let status_text = if s.is_active { "启用" } else { "停用" };

    html! {
        tr {
            td { strong { (s.name) } }
            td {
                span class="type-tag type-tag-putaway" {
                    (tag)
                }
            }
            td {
                @if let Some(wid) = s.warehouse_id {
                    "仓库#" (wid)
                } @else {
                    "全部仓库"
                }
            }
            td {
                @if let Some(cid) = s.product_category_id {
                    "分类#" (cid)
                } @else {
                    span style="color:var(--muted)" { "全部" }
                }
            }
            td {
                span class=(format!("priority-badge {priority_class}")) {
                    (s.priority)
                }
            }
            td {
                label class="toggle-wrap" {
                    span class=(toggle_class) {}
                    (status_text)
                }
            }
            td {
                div class="row-actions" {
                    button class="row-action-btn" title="编辑" {
                        (crate::components::icon::edit_icon("w-4 h-4"))
                    }
                }
            }
        }
    }
}

fn pick_row(s: &PickStrategy) -> Markup {
    let tag = pick_type_tag(&s.strategy_type);
    let priority_class = format!("priority-{}", s.priority.min(4));
    let toggle_class = if s.is_active { "toggle-switch active" } else { "toggle-switch" };
    let status_text = if s.is_active { "启用" } else { "停用" };

    html! {
        tr {
            td { strong { (s.name) } }
            td {
                span class="type-tag type-tag-pick" {
                    (tag)
                }
            }
            td {
                @if let Some(wid) = s.warehouse_id {
                    "仓库#" (wid)
                } @else {
                    "全部仓库"
                }
            }
            td {
                span style="color:var(--muted)" { "全部" }
            }
            td {
                span class=(format!("priority-badge {priority_class}")) {
                    (s.priority)
                }
            }
            td {
                label class="toggle-wrap" {
                    span class=(toggle_class) {}
                    (status_text)
                }
            }
            td {
                div class="row-actions" {
                    button class="row-action-btn" title="编辑" {
                        (crate::components::icon::edit_icon("w-4 h-4"))
                    }
                }
            }
        }
    }
}
