use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::wms::cycle_count::CycleCountService;
use abt_core::wms::enums::CycleCountStatus;

use crate::layout::page::admin_page;
use crate::routes::wms_cycle_count::{CycleCountDetailPath, CycleCountListPath};
use crate::utils::RequestContext;

use abt_macros::require_permission;

// ── Form Data ──

#[derive(Debug, serde::Deserialize)]
pub struct CycleCountActionForm {
    pub action: String,
}

// ── Handlers ──

#[require_permission("WMS", "read")]
pub async fn get_cycle_count_detail(
    path: crate::routes::wms_cycle_count::CycleCountDetailPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.cycle_count_service();

    let cc = svc.get(&service_ctx, &mut conn, path.id).await?;

    let content = cycle_count_detail_page(&cc);
    let page_html = admin_page(
        is_htmx,
        &format!("{} · 循环盘点详情", cc.doc_number),
        &claims,
        "inventory",
        CycleCountListPath::PATH,
        "库存管理",
        Some("循环盘点详情"),
        content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("WMS", "write")]
pub async fn post_cycle_count_action(
    path: crate::routes::wms_cycle_count::CycleCountDetailPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<CycleCountActionForm>,
) -> crate::errors::Result<axum::response::Response> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.cycle_count_service();

    match form.action.as_str() {
        "start" => svc.start_count(&service_ctx, &mut conn, path.id).await?,
        "complete" => svc.complete(&service_ctx, &mut conn, path.id).await?,
        "adjust" => svc.adjust(&service_ctx, &mut conn, path.id).await?,
        "cancel" => svc.cancel(&service_ctx, &mut conn, path.id).await?,
        _ => {}
    }

    let redirect_url = CycleCountDetailPath { id: path.id }.to_string();
    let mut resp = axum::response::Response::default();
    resp.headers_mut().insert(
        axum::http::HeaderName::from_static("hx-redirect"),
        redirect_url.parse().unwrap(),
    );

    Ok(resp)
}

// ── Helpers ──

fn status_label(s: &CycleCountStatus) -> &'static str {
    match s {
        CycleCountStatus::Draft => "草稿",
        CycleCountStatus::Counting => "盘点中",
        CycleCountStatus::Completed => "已完成",
        CycleCountStatus::Adjusted => "已调整",
        CycleCountStatus::Cancelled => "已取消",
    }
}

fn status_class(s: &CycleCountStatus) -> &'static str {
    match s {
        CycleCountStatus::Draft => "status-draft",
        CycleCountStatus::Counting => "status-progress",
        CycleCountStatus::Completed => "status-completed",
        CycleCountStatus::Adjusted => "status-settled",
        CycleCountStatus::Cancelled => "status-cancelled",
    }
}

// ── Components ──

fn cycle_count_detail_page(cc: &abt_core::wms::cycle_count::model::CycleCount) -> Markup {
    let sl = status_label(&cc.status);
    let sc = status_class(&cc.status);
    let detail_path = CycleCountDetailPath { id: cc.id }.to_string();

    html! {
        div {
            a class="back-link" href=(CycleCountListPath::PATH) {
                (crate::components::icon::chevron_left_icon("w-4 h-4"))
                "返回循环盘点列表"
            }

            div class="detail-header" {
                div {
                    div class="detail-title-row" {
                        span class="detail-no mono" { (cc.doc_number) }
                        span class=(format!("status-pill {sc}")) { (sl) }
                    }
                }
                div class="page-actions" {
                    (action_buttons(cc, &detail_path))
                }
            }

            (workflow_steps(&cc.status))

            div class="info-card" {
                div class="info-card-title" { "盘点信息" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "盘点单号" }
                        span class="info-value mono" { (cc.doc_number) }
                    }
                    div class="info-item" {
                        span class="info-label" { "仓库" }
                        span class="info-value" { "仓库#" (cc.warehouse_id) }
                    }
                    div class="info-item" {
                        span class="info-label" { "库区" }
                        span class="info-value" {
                            @if let Some(zid) = cc.zone_id {
                                "库区#" (zid)
                            } @else {
                                "—"
                            }
                        }
                    }
                    div class="info-item" {
                        span class="info-label" { "盘点日期" }
                        span class="info-value mono" { (cc.count_date.format("%Y-%m-%d")) }
                    }
                    div class="info-item" {
                        span class="info-label" { "盲盘" }
                        span class="info-value" {
                            @if cc.is_blind { "是" } @else { "否" }
                        }
                    }
                    div class="info-item" {
                        span class="info-label" { "操作员" }
                        span class="info-value" { "操作员#" (cc.operator_id) }
                    }
                }
            }

            div class="summary-stats" style="display:grid;grid-template-columns:repeat(4,1fr);gap:var(--space-4);margin-bottom:var(--space-6)" {
                (summary_card("总项数", "—", "blue"))
                (summary_card("一致项", "—", "green"))
                (summary_card("差异项", "—", "orange"))
                (summary_card("已调整项", "—", "purple"))
            }

            div class="data-card" {
                div style="padding:var(--space-5) var(--space-6) var(--space-3)" {
                    div class="info-card-title" style="border-bottom:none;padding-bottom:0;margin-bottom:0" { "盘点明细" }
                }
                div class="data-card-scroll" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "行号" }
                                th { "储位" }
                                th { "产品编码" }
                                th { "产品名称" }
                                th { "批次号" }
                                th class="num-right" { "系统数量" }
                                th class="num-right" { "实盘数量" }
                                th class="num-right" { "差异数量" }
                                th { "差异原因" }
                                th { "已调整" }
                            }
                        }
                        tbody {
                            tr {
                                td colspan="10" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "盘点明细将通过后续版本加载"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn workflow_steps(status: &CycleCountStatus) -> Markup {
    let steps = &[
        ("草稿", matches!(status, CycleCountStatus::Draft)),
        ("盘点中", matches!(status, CycleCountStatus::Counting)),
        ("已完成", matches!(status, CycleCountStatus::Completed)),
        ("已调整", matches!(status, CycleCountStatus::Adjusted)),
    ];

    let idx = match status {
        CycleCountStatus::Draft => 0,
        CycleCountStatus::Counting => 1,
        CycleCountStatus::Completed => 2,
        CycleCountStatus::Adjusted => 3,
        CycleCountStatus::Cancelled => 0,
    };

    html! {
        div class="workflow-steps" {
            @for (i, (label, _)) in steps.iter().enumerate() {
                @if i > 0 {
                    div class=(if i <= idx { "wf-line completed" } else { "wf-line" }) {}
                }
                @if matches!(status, CycleCountStatus::Cancelled) {
                    div class="wf-step" {
                        span class="wf-dot" {}
                        (label)
                    }
                } @else if i < idx {
                    div class="wf-step completed" {
                        span class="wf-dot" {}
                        (label)
                    }
                } @else if i == idx {
                    div class="wf-step current" {
                        span class="wf-dot" {}
                        (label)
                    }
                } @else {
                    div class="wf-step" {
                        span class="wf-dot" {}
                        (label)
                    }
                }
            }
        }
    }
}

fn action_buttons(cc: &abt_core::wms::cycle_count::model::CycleCount, detail_path: &str) -> Markup {
    match &cc.status {
        CycleCountStatus::Draft => {
            html! {
                button class="btn btn-default"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"start"}"#
                    hx-confirm="确定要开始盘点吗？"
                    hx-redirect=(detail_path) {
                    "开始盘点"
                }
            }
        }
        CycleCountStatus::Counting => {
            html! {
                button class="btn btn-primary"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"complete"}"#
                    hx-confirm="确定要完成盘点吗？"
                    hx-redirect=(detail_path) {
                    "完成盘点"
                }
            }
        }
        CycleCountStatus::Completed => {
            html! {
                button class="btn btn-default"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"cancel"}"#
                    hx-confirm="确定要取消此盘点单吗？"
                    hx-redirect=(detail_path) {
                    (crate::components::icon::x_icon("w-4 h-4"))
                    "取消"
                }
                button class="btn btn-primary"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"adjust"}"#
                    hx-confirm="确定要确认调整库存吗？此操作不可撤销。"
                    hx-redirect=(detail_path) {
                    "确认调整"
                }
            }
        }
        _ => html! {},
    }
}

fn summary_card(label: &str, value: &str, color: &str) -> Markup {
    html! {
        div class="summary-stat-card" style="background:var(--bg);border:1px solid var(--border-soft);border-radius:var(--radius-md);padding:var(--space-4) var(--space-5);display:flex;align-items:center;gap:var(--space-3)" {
            div class=(format!("summary-stat-icon {color}")) style="width:40px;height:40px;border-radius:var(--radius-md);display:grid;place-items:center;flex-shrink:0" {}
            div {
                div style="font-size:var(--text-xl);font-weight:700;line-height:1.1" { (value) }
                div style="font-size:12px;color:var(--muted);margin-top:2px" { (label) }
            }
        }
    }
}
