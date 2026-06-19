use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::gl::account::model::{GlAccount, GlAccountFilter};
use abt_core::gl::account::{GlAccountService, UpdateGlAccountReq};
use abt_core::gl::enums::AccountType;
use abt_core::shared::types::PaginatedResult;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_param, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::gl::{GlAccountCreatePath, GlAccountListPath, GlAccountTogglePath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct AccountQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub account_type: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub disabled: Option<i16>, // "" 全部 / 0 启用 / 1 停用
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("GL", "read")]
pub async fn get_list(
    _path: GlAccountListPath,
    ctx: RequestContext,
    Query(params): Query<AccountQueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_create = ctx.has_permission("GL", "create").await;
    let can_update = ctx.has_permission("GL", "update").await;
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;

    let svc = state.gl_account_service();
    let filter = build_filter(&params);
    let page_num = params.page.unwrap_or(1);
    let result = svc
        .list(
            &service_ctx,
            &mut conn,
            filter,
            abt_core::shared::types::PageParams::new(page_num, 20),
        )
        .await?;

    let content = account_list_page(&result, &params, can_create, can_update);
    let page_html = admin_page(
        is_htmx,
        "科目表",
        &claims,
        "gl",
        GlAccountListPath::PATH,
        "总账管理",
        None,
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

/// 行内切换启用/停用：先 get 拿 version，再 update.disabled
#[require_permission("GL", "update")]
pub async fn toggle_disabled(
    path: GlAccountTogglePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.gl_account_service();

    let account = svc.get(&service_ctx, &mut conn, path.id).await?;
    let new_disabled = !account.disabled;
    svc.update(
        &service_ctx,
        &mut conn,
        path.id,
        UpdateGlAccountReq {
            name: None,
            disabled: Some(new_disabled),
            version: account.version,
        },
    )
    .await?;

    // 重新查询当前筛选条件下的列表（保持 tab/筛选状态），返回 data-card 片段
    // 由于 toggle 是行操作，无 query 参数，重新拉默认列表即可
    let filter = GlAccountFilter::default();
    let result = svc
        .list(
            &service_ctx,
            &mut conn,
            filter,
            abt_core::shared::types::PageParams::new(1, 20),
        )
        .await?;
    let can_update = true;
    let params = AccountQueryParams::default();
    let fragment = account_data_card(&result, &params, can_update);
    Ok(Html(fragment.into_string()))
}

fn build_filter(params: &AccountQueryParams) -> GlAccountFilter {
    GlAccountFilter {
        keyword: params.keyword.clone(),
        account_type: params.account_type.and_then(AccountType::from_i16),
        disabled: match params.disabled {
            Some(0) => Some(false),
            Some(1) => Some(true),
            _ => None,
        },
    }
}

fn build_query_string(params: &AccountQueryParams) -> String {
    let mut parts = Vec::new();
    if let Some(ref v) = params.keyword {
        parts.push(format!("keyword={v}"));
    }
    if let Some(v) = params.account_type {
        parts.push(format!("account_type={v}"));
    }
    if let Some(v) = params.disabled {
        parts.push(format!("disabled={v}"));
    }
    if let Some(v) = params.page
        && v > 1
    {
        parts.push(format!("page={v}"));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    }
}

// ── Components ──

fn account_type_label(t: &AccountType) -> (&'static str, &'static str, &'static str) {
    // (label, bg, color)
    match t {
        AccountType::Asset => ("资产", "rgba(37,99,235,0.08)", "#2563eb"),
        AccountType::Liability => ("负债", "rgba(217,119,6,0.08)", "#b45309"),
        AccountType::Equity => ("权益", "rgba(124,58,237,0.08)", "#7c3aed"),
        AccountType::Revenue => ("收入", "rgba(22,163,74,0.08)", "#16a34a"),
        AccountType::Cost => ("成本", "rgba(220,38,38,0.08)", "#dc2626"),
        AccountType::Expense => ("费用", "rgba(148,163,184,0.12)", "#475569"),
    }
}

fn account_list_page(
    result: &PaginatedResult<GlAccount>,
    params: &AccountQueryParams,
    can_create: bool,
    can_update: bool,
) -> Markup {
    html! {
        div {
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "科目表" }
                div class="flex gap-3" {
                    @if can_create {
                        a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(GlAccountCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建科目"
                        }
                    }
                }
            }
            (account_table_fragment(result, params, can_update))
        }
    }
}

fn account_table_fragment(
    result: &PaginatedResult<GlAccount>,
    params: &AccountQueryParams,
    can_update: bool,
) -> Markup {
    let total_count = result.total;
    // status_tabs 用 "disabled" 作为 param_name
    let selected_disabled = params.disabled.map(|v| v.to_string()).unwrap_or_default();

    let tabs = &[
        TabItem { value: String::new(), label: "全部", count: Some(total_count) },
        TabItem { value: "0".into(), label: "启用", count: None },
        TabItem { value: "1".into(), label: "停用", count: None },
    ];

    html! {
        div {
            (status_tabs_with_param(GlAccountListPath::PATH, "#gl-account-data-card", "#gl-account-filter-form", tabs, &selected_disabled, "disabled"))

            form class="flex items-center gap-3 mb-5 flex-wrap filter-form" id="gl-account-filter-form"
                hx-get=(GlAccountListPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.search-input"
                hx-target="#gl-account-data-card"
                hx-select="#gl-account-data-card"
                hx-swap="outerHTML"
                hx-include="#gl-account-filter-form"
                hx-push-url="true" {
                div class="relative flex-1 max-w-xs icon:absolute icon:left-3 icon:top-1/2 icon:-translate-y-1/2 icon:w-4 icon:h-4 icon:text-muted" {
                    (icon::search_icon(""))
                    input class="w-[200px] pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent search-input" type="text" name="keyword"
                        placeholder="搜索科目编码、名称…"
                        value=(params.keyword.as_deref().unwrap_or(""));
                }
                select class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none cursor-pointer" name="account_type" {
                    option value="" selected[params.account_type.is_none()] { "全部类型" }
                    option value="1" selected[params.account_type == Some(1)] { "资产" }
                    option value="2" selected[params.account_type == Some(2)] { "负债" }
                    option value="3" selected[params.account_type == Some(3)] { "权益" }
                    option value="4" selected[params.account_type == Some(4)] { "收入" }
                    option value="5" selected[params.account_type == Some(5)] { "成本" }
                    option value="6" selected[params.account_type == Some(6)] { "费用" }
                }
            }

            (account_data_card(result, params, can_update))
        }
    }
}

fn account_data_card(
    result: &PaginatedResult<GlAccount>,
    params: &AccountQueryParams,
    can_update: bool,
) -> Markup {
    let query = build_query_string(params);
    html! {
        div class="data-card" id="gl-account-data-card" {
            div class="overflow-x-auto" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "科目编码" }
                            th { "科目名称" }
                            th { "科目类型" }
                            th { "余额方向" }
                            th { "币种" }
                            th { "明细科目" }
                            th { "状态" }
                            th class="w-[100px]" { "操作" }
                        }
                    }
                    tbody {
                        @for item in &result.items {
                            @let (type_label, type_bg, type_color) = account_type_label(&item.account_type);
                            tr {
                                td class="font-mono tabular-nums text-accent" { (&item.code) }
                                td { (&item.name) }
                                td {
                                    span style=(format!("display:inline-flex;align-items:center;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", type_bg, type_color)) {
                                        (type_label)
                                    }
                                }
                                td class="text-fg-2" { (item.balance_direction.as_str()) }
                                td class="font-mono tabular-nums text-muted" { (&item.currency) }
                                td class="text-muted text-xs" { @if item.is_detail { "明细" } @else { "汇总" } }
                                td {
                                    @if item.disabled {
                                        span style="display:inline-flex;align-items:center;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:rgba(220,38,38,0.08);color:#dc2626" { "停用" }
                                    } @else {
                                        span style="display:inline-flex;align-items:center;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:rgba(22,163,74,0.08);color:#16a34a" { "启用" }
                                    }
                                }
                                td {
                                    @if can_update {
                                        @let toggle_path = GlAccountTogglePath { id: item.id };
                                        button class="text-xs px-2 py-1 rounded-sm border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent transition-colors cursor-pointer"
                                            hx-post=(toggle_path.to_string())
                                            hx-target="#gl-account-data-card"
                                            hx-select="#gl-account-data-card"
                                            hx-swap="outerHTML" {
                                            @if item.disabled { "启用" } @else { "停用" }
                                        }
                                    } @else {
                                        span class="text-muted text-xs" { "—" }
                                    }
                                }
                            }
                        }
                        @if result.items.is_empty() {
                            tr {
                                td colspan="8" class="text-center text-muted py-8" { "暂无科目记录" }
                            }
                        }
                    }
                }
            }
            (pagination(GlAccountListPath::PATH, &query, result.total, result.page, result.total_pages))
        }
    }
}
