use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use crate::errors::Result;
use crate::routes::wms_conversion::ConversionDetailPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;
use crate::layout::page::admin_page;

use abt_core::wms::enums::{ConversionDir, ConversionStatus};
use abt_core::wms::form_conversion::{ConversionItem, FormConversionService};
use crate::components::icon;

#[require_permission("WMS", "read")]
pub async fn get_conversion_detail(
    path: ConversionDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.form_conversion_service();

    let conversion = svc.get(&service_ctx, &mut conn, path.id).await?;
    let items = svc.get_items(&service_ctx, &mut conn, path.id).await?;

    let content = conversion_detail_page(&conversion, &items);
    let page_html = admin_page(
        is_htmx,
        "形态转换详情",
        &claims,
        "inventory",
        "/admin/wms/conversions",
        "库存管理",
        None,
        content,
    );
    Ok(Html(page_html.into_string()))
}

fn conversion_detail_page(
    conversion: &abt_core::wms::form_conversion::FormConversion,
    items: &[ConversionItem],
) -> Markup {
    let (status_label, status_class) = match conversion.status {
        ConversionStatus::Draft => ("草稿", "status-draft"),
        ConversionStatus::Completed => ("已完成", "status-completed"),
        ConversionStatus::Cancelled => ("已取消", "status-cancelled"),
    };

    let show_actions = matches!(conversion.status, ConversionStatus::Draft);

    let consume_items: Vec<_> = items.iter().filter(|i| i.direction == ConversionDir::Consume).collect();
    let produce_items: Vec<_> = items.iter().filter(|i| i.direction == ConversionDir::Produce).collect();

    html! {
        div {
            a href="/admin/wms/conversions" class="back-link" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回形态转换列表"
            }

            div class="detail-header" {
                div {
                    div class="detail-title-row" {
                        h1 class="detail-no font-mono" { (conversion.doc_number) }
                        span class=(format!("status-pill {status_class}")) { (status_label) }
                    }
                }
                div class="page-actions" {
                    @if show_actions {
                        button class="btn btn-default" {
                            (icon::x_icon("w-4 h-4"))
                            "取消"
                        }
                        button class="btn btn-primary" {
                            (icon::check_circle_icon("w-4 h-4"))
                            "确认完成"
                        }
                    }
                }
            }

            // ── Workflow Steps ──
            (conversion_workflow_steps(conversion.status))

            // ── Info Card ──
            div class="info-card" {
                div class="info-card-title" { "转换信息" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "转换单号" }
                        span class="info-value mono" { (conversion.doc_number) }
                    }
                    div class="info-item" {
                        span class="info-label" { "转换仓库" }
                        span class="info-value" { "—" }
                    }
                    div class="info-item" {
                        span class="info-label" { "转换日期" }
                        span class="info-value mono" { (conversion.conversion_date.to_string()) }
                    }
                    div class="info-item" {
                        span class="info-label" { "操作员" }
                        span class="info-value" { "—" }
                    }
                }
            }

            // ── Consume Items ──
            div class="info-card" {
                div style="display:inline-flex;align-items:center;gap:var(--space-2);font-size:var(--text-base);font-weight:600;color:var(--fg);margin-bottom:var(--space-4)" {
                    "消耗物料 "
                    span style="display:inline-flex;align-items:center;padding:3px 10px;border-radius:9999px;font-size:12px;font-weight:600;background:#fff2f0;color:var(--danger)" { "消耗" }
                }
                table class="data-table" {
                    thead {
                        tr {
                            th { "行号" }
                            th { "产品编码" }
                            th { "名称" }
                            th { "规格" }
                            th { "单位" }
                            th class="num-right" { "消耗数量" }
                            th class="num-right" { "单位成本" }
                            th { "批次号" }
                        }
                    }
                    tbody {
                        @for (i, item) in consume_items.iter().enumerate() {
                            tr {
                                td class="mono" { (i + 1) }
                                td class="mono" { "—" }
                                td { "—" }
                                td { "—" }
                                td { "—" }
                                td class="num-right" { (item.quantity.to_string()) }
                                td class="num-right" { (item.unit_cost.to_string()) }
                                td class="mono" {
                                    @if let Some(ref batch) = item.batch_no {
                                        (batch)
                                    } @else {
                                        "—"
                                    }
                                }
                            }
                        }
                        @if consume_items.is_empty() {
                            tr {
                                td colspan="8" style="text-align:center;padding:var(--space-6);color:var(--muted)" {
                                    "暂无消耗物料"
                                }
                            }
                        }
                    }
                }
            }

            // ── Produce Items ──
            div class="info-card" {
                div style="display:inline-flex;align-items:center;gap:var(--space-2);font-size:var(--text-base);font-weight:600;color:var(--fg);margin-bottom:var(--space-4)" {
                    "产出物料 "
                    span style="display:inline-flex;align-items:center;padding:3px 10px;border-radius:9999px;font-size:12px;font-weight:600;background:#f0fff0;color:var(--success)" { "产出" }
                }
                table class="data-table" {
                    thead {
                        tr {
                            th { "行号" }
                            th { "产品编码" }
                            th { "名称" }
                            th { "规格" }
                            th { "单位" }
                            th class="num-right" { "产出数量" }
                            th class="num-right" { "单位成本" }
                            th { "批次号" }
                        }
                    }
                    tbody {
                        @for (i, item) in produce_items.iter().enumerate() {
                            tr {
                                td class="mono" { (i + 1) }
                                td class="mono" { "—" }
                                td { "—" }
                                td { "—" }
                                td { "—" }
                                td class="num-right" { (item.quantity.to_string()) }
                                td class="num-right" { (item.unit_cost.to_string()) }
                                td class="mono" {
                                    @if let Some(ref batch) = item.batch_no {
                                        (batch)
                                    } @else {
                                        "—"
                                    }
                                }
                            }
                        }
                        @if produce_items.is_empty() {
                            tr {
                                td colspan="8" style="text-align:center;padding:var(--space-6);color:var(--muted)" {
                                    "暂无产出物料"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn conversion_workflow_steps(status: ConversionStatus) -> Markup {
    let steps = [
        ("草稿", ConversionStatus::Draft),
        ("已完成", ConversionStatus::Completed),
    ];

    let current_idx = match status {
        ConversionStatus::Draft => 0,
        ConversionStatus::Completed => 1,
        ConversionStatus::Cancelled => 0,
    };

    html! {
        div class="workflow-steps" {
            @for (i, (label, _)) in steps.iter().enumerate() {
                @if i > 0 {
                    div class=(if i <= current_idx { "wf-line completed" } else { "wf-line" }) {}
                }
                div class={
                    @if i < current_idx { "wf-step completed" }
                    @else if i == current_idx { "wf-step current" }
                    @else { "wf-step" }
                } {
                    span class="wf-dot" {}
                    (label)
                }
            }
        }
    }
}
