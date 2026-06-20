use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::gl::enums::PeriodStatus;
use abt_core::gl::period::{AccountingPeriod, GlPeriodService, PeriodFilter};

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::gl::{GlPeriodClosePath, GlPeriodListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Handlers ──

#[require_permission("GL", "read")]
pub async fn get_list(
    _path: GlPeriodListPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_update = ctx.has_permission("GL", "update").await;
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;

    let periods = state
        .gl_period_service()
        .list(&service_ctx, &mut conn, PeriodFilter::default())
        .await?;

    let content = period_list_page(&periods, can_update);
    let page_html = admin_page(
        is_htmx,
        "会计期间",
        &claims,
        "gl",
        GlPeriodListPath::PATH,
        "总账管理",
        None,
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

/// 关闭期间：open → closed（单向，不可再开）。成功后 HX-Redirect 回列表
#[require_permission("GL", "update")]
pub async fn close(path: GlPeriodClosePath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    state
        .gl_period_service()
        .close(&service_ctx, &mut conn, path.id)
        .await?;
    let redirect = GlPeriodListPath::PATH.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn period_status_label(s: PeriodStatus) -> (&'static str, &'static str, &'static str) {
    // (label, bg, color)
    match s {
        PeriodStatus::Open => ("Open", "rgba(22,163,74,0.08)", "#16a34a"),
        PeriodStatus::Closed => ("Closed", "rgba(148,163,184,0.12)", "#475569"),
    }
}

fn period_list_page(periods: &[AccountingPeriod], can_update: bool) -> Markup {
    html! {
        div {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "会计期间" }
                span class="text-xs text-muted" { "期间关闭为单向操作（Open → Closed），不可重新开启" }
            }
            (period_data_card(periods, can_update))
        }
    }
}

fn period_data_card(periods: &[AccountingPeriod], can_update: bool) -> Markup {
    html! {
        div class="data-card" id="gl-period-data-card" {
            div class="overflow-x-auto" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "期间名称" }
                            th { "会计年度" }
                            th { "开始日期" }
                            th { "结束日期" }
                            th { "状态" }
                            th class="w-[120px]" { "操作" }
                        }
                    }
                    tbody {
                        @for p in periods {
                            @let (status_label, status_bg, status_color) = period_status_label(p.status);
                            tr {
                                td class="font-mono tabular-nums text-accent" { (&p.name) }
                                td class="text-fg-2" { (&p.fiscal_year) }
                                td class="text-fg-2" { (p.start_date.format("%Y-%m-%d")) }
                                td class="text-fg-2" { (p.end_date.format("%Y-%m-%d")) }
                                td {
                                    span style=(format!("display:inline-flex;align-items:center;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", status_bg, status_color)) {
                                        (status_label)
                                    }
                                }
                                td {
                                    @if p.status == PeriodStatus::Open {
                                        @if can_update {
                                            @let close_path = GlPeriodClosePath { id: p.id };
                                            button class="text-xs px-2 py-1 rounded-sm border border-border hover:bg-danger-bg hover:border-[rgba(220,38,38,0.3)] hover:text-danger transition-colors cursor-pointer"
                                                hx-post=(close_path.to_string())
                                                hx-confirm="关闭期间后将无法再开启，且需该期间无 Draft 凭证。确认关闭？"
                                                hx-target="this" {
                                                "关闭"
                                            }
                                        } @else {
                                            span class="text-muted text-xs" { "—" }
                                        }
                                    } @else {
                                        span class="text-muted text-xs" { "—" }
                                    }
                                }
                            }
                        }
                        @if periods.is_empty() {
                            tr {
                                td colspan="6" class="text-center text-muted py-8" { "暂无会计期间记录" }
                            }
                        }
                    }
                }
            }
        }
    }
}
