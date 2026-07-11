//! 销售作业中心 — 报价单 / 销售订单 / 销售退货 / 对账收款 四 card 聚合工作台。
//!
//! 架构（组件化单端点模式，同 purchase_work_center）：
//! - 首页渲染 card 外壳（section + 标题栏），`#sc-card` 占位 div `hx-trigger="load"` 拉默认 card；
//! - 每个 card 一个 GET 端点，card 内 tab/筛选/分页走该端点 + `hx-select="#sc-card"` 局部刷新；
//! - 行展开 chevron（`row_expand` 组件）按需拉 row-detail，调 `SalesWorkCenterService::*_hub_summary`
//!   聚合发货进度 / 来源链 / AR 台账（客户维度）；
//! - 单号列点击打开 detail drawer（就地查看 + 状态操作），写操作 POST 广播 `HX-Trigger`
//!   （soChanged / salesQuotationChanged / salesReturnChanged / salesReconChanged），相关 card 自刷新。

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Instant;

use abt_core::master_data::customer::model::{Customer, CustomerQuery};
use abt_core::master_data::customer::CustomerService;
use abt_core::sales::quotation::model::{Quotation, QuotationQuery, QuotationStatus};
use abt_core::sales::quotation::QuotationService;
use abt_core::sales::reconciliation::model::{Reconciliation, ReconciliationQuery, ReconciliationStatus};
use abt_core::sales::reconciliation::ReconciliationService;
use abt_core::sales::sales_order::model::{SalesOrder, SalesOrderQuery, SalesOrderStatus};
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::sales::sales_return::model::{ReturnQuery, ReturnStatus, SalesReturn};
use abt_core::sales::sales_return::SalesReturnService;
use abt_core::sales::work_center::{
    QuotationHubSummary, ReconciliationAggregate, SalesOrderHubSummary, SalesOrderSourceChain,
    SalesReturnHubSummary, SalesWorkCenterService, SalesWorkCenterSummary, SettlementHubSummary,
    SettlementReconType,
};
use abt_core::shared::types::{DomainError, PageParams};

use abt_macros::require_permission;
use rust_decimal::Decimal;
use crate::components::icon;
use crate::components::overlay::drawer_shell;
use crate::components::pagination::pagination;
use crate::components::row_expand;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::sales_work_center::*;
use crate::utils::{empty_as_none, RequestContext};

// =============================================================================
// summary 缓存（同采购：TTL 30s + 写操作 invalidate）
// =============================================================================

const SUMMARY_TTL_SECS: u64 = 30;

async fn cached_summary(
    state: &crate::state::AppState,
    ctx: &abt_core::shared::types::context::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
) -> SalesWorkCenterSummary {
    {
        let cache = state.sales_summary_cache.read().unwrap();
        if let Some((at, s)) = cache.as_ref()
            && at.elapsed().as_secs() < SUMMARY_TTL_SECS
        {
            return s.clone();
        }
    }
    let s = state
        .sales_work_center_service()
        .summary(ctx, db)
        .await
        .unwrap_or_default();
    *state.sales_summary_cache.write().unwrap() = Some((Instant::now(), s.clone()));
    s
}

/// 写操作 commit 后调：清缓存，下次请求重算（badge/total 及时）。
fn invalidate_sales_summary(state: &crate::state::AppState) {
    *state.sales_summary_cache.write().unwrap() = None;
}

// =============================================================================
// 首页
// =============================================================================

#[require_permission("SALES_ORDER", "read")]
pub async fn get_work_center(
    _path: SalesWorkCenterPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;
    let summary = cached_summary(&state, &service_ctx, &mut conn).await;

    let content = html! {
        // detail-header：标题 + 待办总数 + AR 逾期/未收告警 pill
        div class="flex items-center justify-between mb-4 flex-wrap gap-4" {
            div class="flex items-center gap-2.5" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "销售作业中心" }
                span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-accent-bg text-accent text-xs font-semibold" {
                    span class="font-mono tabular-nums font-bold" { (summary.total()) }
                    "待办"
                }
            }
            div class="flex items-center gap-2" {
                @if summary.ar_overdue_amount > Decimal::ZERO {
                    span class="inline-flex items-center gap-1 px-2.5 py-1 rounded-full bg-danger-bg text-danger text-[11px] font-semibold" {
                        (icon::alert_triangle_icon("w-3 h-3"))
                        "逾期应收 " (fmt_decimal(summary.ar_overdue_amount))
                    }
                }
                @if summary.ar_outstanding_amount > Decimal::ZERO {
                    span class="inline-flex items-center gap-1 px-2.5 py-1 rounded-full bg-warn-bg text-warn text-[11px] font-semibold" {
                        (icon::clock_icon("w-3 h-3"))
                        "未收余额 " (fmt_decimal(summary.ar_outstanding_amount))
                    }
                }
            }
        }
        // card 外壳（section）与内容（#sc-card）分离（对齐 purchase_work_center）：
        // 标题栏持久；#sc-card 由各端点返回替换（tab 栏 + 筛选 + 表格 + 分页）。
        section class="bg-bg border border-border-soft rounded-lg mb-4 shadow-[var(--shadow-card)] overflow-hidden" {
            div class="flex items-center gap-3 px-5 py-3 border-b border-border-soft" {
                div class="w-7 h-7 rounded-md grid place-items-center bg-accent-bg text-accent shrink-0" {
                    (icon::trending_up_icon("w-[18px] h-[18px]"))
                }
                span class="font-semibold text-fg shrink-0" { "销售作业" }
                span class="text-xs text-muted font-mono flex-1 truncate" {
                    (summary.total()) " 件待办 · 报价 / 订单 / 退货 / 对账 一屏处理"
                }
            }
            div id="sc-card"
                hx-get=(ScOrdersPath::PATH) hx-trigger="load" hx-target="this" hx-swap="outerHTML" {
                "加载中…"
            }
        }
        // detail drawer overlay 壳集合（body 由单号 hx-get 填充）
        (render_sc_drawer_overlay("sc-quo-overlay", "sc-quo-drawer-body", "报价单详情", "w-[680px] max-w-[92vw]"))
        (render_sc_drawer_overlay("sc-quo-create-overlay", "sc-quo-create-drawer-body", "新建报价单", "w-[1000px] max-w-[94vw]"))
        (render_sc_drawer_overlay("sc-order-overlay", "sc-order-drawer-body", "销售订单详情", "w-[760px] max-w-[92vw]"))
        (render_sc_drawer_overlay("sc-return-overlay", "sc-return-drawer-body", "销售退货详情", "w-[680px] max-w-[92vw]"))
        (render_sc_drawer_overlay("sc-return-create-overlay", "sc-return-create-drawer-body", "新建退货", "w-[1000px] max-w-[94vw]"))
        (render_sc_drawer_overlay("sc-recon-overlay", "sc-recon-drawer-body", "对账单详情", "w-[760px] max-w-[92vw]"))
        (render_sc_drawer_overlay("sc-recon-create-overlay", "sc-recon-create-drawer-body", "新建对账单", "w-[480px] max-w-[92vw]"))
    };

    Ok(Html(
        admin_page(
            is_htmx,
            "销售作业中心",
            &claims,
            "sales",
            SalesWorkCenterPath::PATH,
            "销售管理",
            Some("销售作业中心"),
            content,
            &nav_filter,
        )
        .into_string(),
    ))
}

// =============================================================================
// Card 参数 + 端点
// =============================================================================

/// 通用 card 筛选参数（status 默认全部，对齐各列表页）。
#[derive(Debug, Deserialize, Default, Clone)]
pub struct CardParams {
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub keyword: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub per_page: Option<u32>,
}

fn page_of(p: &CardParams) -> (u32, u32) {
    (
        p.page.unwrap_or(1),
        p.per_page.unwrap_or(10).clamp(1, 200),
    )
}

// ── ① 报价单 ──

#[require_permission("SALES_ORDER", "read")]
pub async fn get_quotations_card(
    _path: ScQuotationsPath,
    ctx: RequestContext,
    Query(p): Query<CardParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.quotation_service();
    let summary = cached_summary(&state, &service_ctx, &mut conn).await;
    let status = p.status.and_then(QuotationStatus::from_i16);
    let (page, per_page) = page_of(&p);
    let result = svc
        .list(
            &service_ctx,
            &mut conn,
            QuotationQuery {
                status,
                keyword: p.keyword.clone(),
                ..Default::default()
            },
            PageParams::new(page, per_page),
        )
        .await?;
    let cust_ids: Vec<i64> = result.items.iter().map(|q| q.customer_id).collect();
    let names = customer_names(&state, &service_ctx, &mut conn, &cust_ids).await;

    Ok(Html(
        html! {
            div id="sc-card"
                hx-get=(ScQuotationsPath::PATH)
                hx-trigger="salesQuotationChanged from:body, soChanged from:body"
                hx-include="#sc-filter-form"
                hx-target="this" hx-select="#sc-card" hx-swap="outerHTML" {
                (sc_tab_bar("quotations", &summary))
                div class="flex items-center px-5 py-2 border-b border-border-soft" {
                    button type="button" class=(BTN_PRIMARY)
                        hx-get=(crate::routes::quotation::QuotationCreatePath::PATH)
                        hx-target="#sc-quo-create-drawer-body" hx-swap="innerHTML"
                        hx-select="#quotation-app"
                        _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #sc-quo-create-overlay" {
                        (icon::plus_icon("w-3.5 h-3.5")) "新建报价"
                    }
                }
                (filter_bar(ScQuotationsPath::PATH, "搜索报价号", p.status, per_page, &p, quotation_status_options()))
                (quotation_table(&result.items, &names))
                (pagination(ScQuotationsPath::PATH, "#sc-card", "#sc-filter-form", result.total, result.page, result.total_pages))
            }
        }
        .into_string(),
    ))
}

// ── ② 销售订单 ──

#[require_permission("SALES_ORDER", "read")]
pub async fn get_orders_card(
    _path: ScOrdersPath,
    ctx: RequestContext,
    Query(p): Query<CardParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.sales_order_service();
    let summary = cached_summary(&state, &service_ctx, &mut conn).await;
    let status = p.status.and_then(SalesOrderStatus::from_i16);
    let (page, per_page) = page_of(&p);
    let result = svc
        .list(
            &service_ctx,
            &mut conn,
            SalesOrderQuery {
                status,
                keyword: p.keyword.clone(),
                ..Default::default()
            },
            PageParams::new(page, per_page),
        )
        .await?;
    let cust_ids: Vec<i64> = result.items.iter().map(|o| o.customer_id).collect();
    let names = customer_names(&state, &service_ctx, &mut conn, &cust_ids).await;

    Ok(Html(
        html! {
            div id="sc-card"
                hx-get=(ScOrdersPath::PATH)
                hx-trigger="soChanged from:body"
                hx-include="#sc-filter-form"
                hx-target="this" hx-select="#sc-card" hx-swap="outerHTML" {
                (sc_tab_bar("orders", &summary))
                (filter_bar(ScOrdersPath::PATH, "搜索订单号", p.status, per_page, &p, order_status_options()))
                (orders_table(&result.items, &names))
                (pagination(ScOrdersPath::PATH, "#sc-card", "#sc-filter-form", result.total, result.page, result.total_pages))
            }
        }
        .into_string(),
    ))
}

// ── ③ 销售退货 ──

#[require_permission("SHIPPING", "read")]
pub async fn get_returns_card(
    _path: ScReturnsPath,
    ctx: RequestContext,
    Query(p): Query<CardParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.sales_return_service();
    let summary = cached_summary(&state, &service_ctx, &mut conn).await;
    let status = p.status.and_then(ReturnStatus::from_i16);
    let (page, per_page) = page_of(&p);
    let result = svc
        .list(
            &service_ctx,
            &mut conn,
            ReturnQuery {
                status,
                keyword: p.keyword.clone(),
                ..Default::default()
            },
            PageParams::new(page, per_page),
        )
        .await?;
    let cust_ids: Vec<i64> = result.items.iter().map(|r| r.customer_id).collect();
    let names = customer_names(&state, &service_ctx, &mut conn, &cust_ids).await;

    Ok(Html(
        html! {
            div id="sc-card"
                hx-get=(ScReturnsPath::PATH)
                hx-trigger="salesReturnChanged from:body"
                hx-include="#sc-filter-form"
                hx-target="this" hx-select="#sc-card" hx-swap="outerHTML" {
                (sc_tab_bar("returns", &summary))
                div class="flex items-center px-5 py-2 border-b border-border-soft" {
                    button type="button" class=(BTN_PRIMARY)
                        hx-get=(crate::routes::sales_return::ReturnCreatePath::PATH)
                        hx-target="#sc-return-create-drawer-body" hx-swap="innerHTML"
                        hx-select="#return-app"
                        _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #sc-return-create-overlay" {
                        (icon::plus_icon("w-3.5 h-3.5")) "新建退货"
                    }
                }
                (filter_bar(ScReturnsPath::PATH, "搜索退货号", p.status, per_page, &p, return_status_options()))
                (returns_table(&result.items, &names))
                (pagination(ScReturnsPath::PATH, "#sc-card", "#sc-filter-form", result.total, result.page, result.total_pages))
            }
        }
        .into_string(),
    ))
}

// ── ④ 对账收款 ──

#[require_permission("SALES_ORDER", "read")]
pub async fn get_settlement_card(
    _path: ScSettlementPath,
    ctx: RequestContext,
    Query(p): Query<CardParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.reconciliation_service();
    let summary = cached_summary(&state, &service_ctx, &mut conn).await;
    let status = p.status.and_then(ReconciliationStatus::from_i16);
    let (page, per_page) = page_of(&p);
    let result = svc
        .list(
            &service_ctx,
            &mut conn,
            ReconciliationQuery {
                status,
                keyword: p.keyword.clone(),
                ..Default::default()
            },
            PageParams::new(page, per_page),
        )
        .await?;
    let cust_ids: Vec<i64> = result.items.iter().map(|r| r.customer_id).collect();
    let names = customer_names(&state, &service_ctx, &mut conn, &cust_ids).await;

    Ok(Html(
        html! {
            div id="sc-card"
                hx-get=(ScSettlementPath::PATH)
                hx-trigger="salesReconChanged from:body"
                hx-include="#sc-filter-form"
                hx-target="this" hx-select="#sc-card" hx-swap="outerHTML" {
                (sc_tab_bar("settlement", &summary))
                div class="flex items-center px-5 py-2 border-b border-border-soft" {
                    button type="button" class=(BTN_PRIMARY)
                        hx-get=(ScReconCreateDrawerPath::PATH)
                        hx-target="#sc-recon-create-drawer-body" hx-swap="innerHTML"
                        _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #sc-recon-create-overlay" {
                        (icon::plus_icon("w-3.5 h-3.5")) "新建对账单"
                    }
                }
                (filter_bar(ScSettlementPath::PATH, "搜索对账单号", p.status, per_page, &p, recon_status_options()))
                (settlement_table(&result.items, &names))
                (pagination(ScSettlementPath::PATH, "#sc-card", "#sc-filter-form", result.total, result.page, result.total_pages))
            }
        }
        .into_string(),
    ))
}

// =============================================================================
// 行展开 row-detail（调 SalesWorkCenterService 聚合）
// =============================================================================

#[require_permission("SALES_ORDER", "read")]
pub async fn get_quotation_row_detail(
    ScQuotationRowDetailPath { id }: ScQuotationRowDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let hub = state
        .sales_work_center_service()
        .get_quotation_hub_summary(&service_ctx, &mut conn, id)
        .await?;
    let row_id = format!("sc-quo-{}", id);
    Ok(Html(
        html! {
            (row_expand::row_expand_detail(&row_id, 5, quotation_detail_grid(&hub)))
        }
        .into_string(),
    ))
}

#[require_permission("SALES_ORDER", "read")]
pub async fn get_order_row_detail(
    ScOrderRowDetailPath { id }: ScOrderRowDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let hub = state
        .sales_work_center_service()
        .get_order_hub_summary(&service_ctx, &mut conn, id)
        .await?;
    let row_id = format!("sc-order-{}", id);
    Ok(Html(
        html! {
            (row_expand::row_expand_detail(&row_id, 5, order_detail_grid(&hub)))
        }
        .into_string(),
    ))
}

#[require_permission("SHIPPING", "read")]
pub async fn get_return_row_detail(
    ScReturnRowDetailPath { id }: ScReturnRowDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let hub = state
        .sales_work_center_service()
        .get_return_hub_summary(&service_ctx, &mut conn, id)
        .await?;
    let row_id = format!("sc-return-{}", id);
    Ok(Html(
        html! {
            (row_expand::row_expand_detail(&row_id, 6, return_detail_grid(&hub)))
        }
        .into_string(),
    ))
}

#[require_permission("SALES_ORDER", "read")]
pub async fn get_settlement_row_detail(
    ScSettlementRowDetailPath { recon_type, ref_id }: ScSettlementRowDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let rt = SettlementReconType::parse(&recon_type).unwrap_or(SettlementReconType::DraftRecon);
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let hub = state
        .sales_work_center_service()
        .get_settlement_hub_summary(&service_ctx, &mut conn, rt, ref_id)
        .await?;
    let row_id = format!("sc-recon-{}", ref_id);
    Ok(Html(
        html! {
            (row_expand::row_expand_detail(&row_id, 7, settlement_detail_grid(&hub)))
        }
        .into_string(),
    ))
}

// =============================================================================
// 详情 drawer GET（就地查看 + 状态操作）
// =============================================================================

#[require_permission("SALES_ORDER", "read")]
pub async fn get_quotation_detail_drawer(
    ScQuotationDetailDrawerPath { id }: ScQuotationDetailDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let hub = state
        .sales_work_center_service()
        .get_quotation_hub_summary(&service_ctx, &mut conn, id)
        .await?;
    Ok(Html(
        html! {
            div class="space-y-4" {
                (quotation_detail_grid(&hub))
                (quotation_drawer_actions(&hub.quotation))
            }
        }
        .into_string(),
    ))
}

#[require_permission("SALES_ORDER", "read")]
pub async fn get_order_detail_drawer(
    ScOrderDetailDrawerPath { id }: ScOrderDetailDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let hub = state
        .sales_work_center_service()
        .get_order_hub_summary(&service_ctx, &mut conn, id)
        .await?;
    Ok(Html(
        html! {
            div class="space-y-4" {
                (order_detail_grid(&hub))
                (order_drawer_actions(&hub.order))
            }
        }
        .into_string(),
    ))
}

#[require_permission("SHIPPING", "read")]
pub async fn get_return_detail_drawer(
    ScReturnDetailDrawerPath { id }: ScReturnDetailDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let hub = state
        .sales_work_center_service()
        .get_return_hub_summary(&service_ctx, &mut conn, id)
        .await?;
    Ok(Html(
        html! {
            div class="space-y-4" {
                (return_detail_grid(&hub))
                (return_drawer_actions(&hub.return_order))
            }
        }
        .into_string(),
    ))
}

#[require_permission("SALES_ORDER", "read")]
pub async fn get_settlement_detail_drawer(
    ScSettlementDetailDrawerPath { id }: ScSettlementDetailDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let hub = state
        .sales_work_center_service()
        .get_settlement_hub_summary(
            &service_ctx,
            &mut conn,
            SettlementReconType::DraftRecon,
            id,
        )
        .await?;
    Ok(Html(
        html! {
            div class="space-y-4" {
                (settlement_detail_grid(&hub))
                (recon_drawer_actions(&hub.recon, hub.recon_type))
            }
        }
        .into_string(),
    ))
}

// =============================================================================
// 写操作 POST（事务包裹 + invalidate + HX-Trigger 广播）
// =============================================================================

/// 报价：提交（Draft → Sent）
#[require_permission("SALES_ORDER", "update")]
pub async fn submit_quotation(
    path: ScQuotationSubmitPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .quotation_service()
        .submit(&service_ctx, &mut tx, path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_sales_summary(&state);
    Ok(([("HX-Trigger", "salesQuotationChanged")], Html(String::new())))
}

/// 报价：接受（Sent → Accepted）
#[require_permission("SALES_ORDER", "update")]
pub async fn accept_quotation(
    path: ScQuotationAcceptPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .quotation_service()
        .accept(&service_ctx, &mut tx, path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_sales_summary(&state);
    Ok(([("HX-Trigger", "salesQuotationChanged")], Html(String::new())))
}

/// 报价：转销售订单（Accepted → 销售订单）。广播两个事件（报价 + 新订单）。
#[require_permission("SALES_ORDER", "create")]
pub async fn quotation_to_so(
    path: ScQuotationToSoPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let _so_id = state
        .sales_order_service()
        .create_from_quotation(&service_ctx, &mut tx, path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_sales_summary(&state);
    Ok((
        [("HX-Trigger", r#"{"salesQuotationChanged":"","soChanged":""}"#)],
        Html(String::new()),
    ))
}

/// 销售订单：确认（Draft → Confirmed，触发需求生成 / 库存分配）
#[require_permission("SALES_ORDER", "update")]
pub async fn confirm_order(
    path: ScOrderConfirmPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .sales_order_service()
        .confirm(&service_ctx, &mut tx, path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_sales_summary(&state);
    Ok(([("HX-Trigger", "soChanged")], Html(String::new())))
}

/// 销售订单：取消
#[require_permission("SALES_ORDER", "update")]
pub async fn cancel_order(
    path: ScOrderCancelPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .sales_order_service()
        .cancel(&service_ctx, &mut tx, path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_sales_summary(&state);
    Ok(([("HX-Trigger", "soChanged")], Html(String::new())))
}

/// 销售退货：确认 / 审批（Draft → Confirmed）
#[require_permission("SHIPPING", "update")]
pub async fn approve_return(
    path: ScReturnApprovePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .sales_return_service()
        .approve(&service_ctx, &mut tx, path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_sales_summary(&state);
    Ok((
        [("HX-Trigger", "salesReturnChanged")],
        Html(String::new()),
    ))
}

/// 销售退货：收货（Confirmed → Received）
#[require_permission("SHIPPING", "update")]
pub async fn receive_return(
    path: ScReturnReceivePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .sales_return_service()
        .receive(&service_ctx, &mut tx, path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_sales_summary(&state);
    Ok((
        [("HX-Trigger", "salesReturnChanged")],
        Html(String::new()),
    ))
}

/// 销售退货：完成（Inspecting → Completed，触发 SalesReturnReceived 事件冲减 AR）
#[require_permission("SHIPPING", "update")]
pub async fn complete_return(
    path: ScReturnCompletePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .sales_return_service()
        .complete(&service_ctx, &mut tx, path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_sales_summary(&state);
    Ok((
        [("HX-Trigger", "salesReturnChanged")],
        Html(String::new()),
    ))
}

/// 销售退货：取消
#[require_permission("SHIPPING", "update")]
pub async fn cancel_return(
    path: ScReturnCancelPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .sales_return_service()
        .cancel(&service_ctx, &mut tx, path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_sales_summary(&state);
    Ok((
        [("HX-Trigger", "salesReturnChanged")],
        Html(String::new()),
    ))
}

/// 月对账单：发送（Draft → Sent，发给客户）
#[require_permission("SALES_ORDER", "update")]
pub async fn send_recon(
    path: ScReconSendPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .reconciliation_service()
        .send(&service_ctx, &mut tx, path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_sales_summary(&state);
    Ok((
        [("HX-Trigger", "salesReconChanged")],
        Html(String::new()),
    ))
}

/// 月对账单：确认（Sent → Confirmed，客户确认无误）
#[require_permission("SALES_ORDER", "update")]
pub async fn confirm_recon(
    path: ScReconConfirmPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .reconciliation_service()
        .confirm(&service_ctx, &mut tx, path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_sales_summary(&state);
    Ok((
        [("HX-Trigger", "salesReconChanged")],
        Html(String::new()),
    ))
}

/// 月对账单：结算（Confirmed → Settled）
#[require_permission("SALES_ORDER", "update")]
pub async fn settle_recon(
    path: ScReconSettlePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .reconciliation_service()
        .settle(&service_ctx, &mut tx, path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_sales_summary(&state);
    Ok((
        [("HX-Trigger", "salesReconChanged")],
        Html(String::new()),
    ))
}

/// 报价：拒绝（Sent → Rejected）
#[require_permission("SALES_ORDER", "update")]
pub async fn reject_quotation(
    path: ScQuotationRejectPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .quotation_service()
        .reject(&service_ctx, &mut tx, path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_sales_summary(&state);
    Ok(([("HX-Trigger", "salesQuotationChanged")], Html(String::new())))
}

/// 报价：失效（手动置为 Expired）
#[require_permission("SALES_ORDER", "update")]
pub async fn expire_quotation(
    path: ScQuotationExpirePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .quotation_service()
        .expire(&service_ctx, &mut tx, path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_sales_summary(&state);
    Ok(([("HX-Trigger", "salesQuotationChanged")], Html(String::new())))
}

/// 销售退货：检验（Received → Inspecting）
#[require_permission("SHIPPING", "update")]
pub async fn inspect_return(
    path: ScReturnInspectPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .sales_return_service()
        .inspect(&service_ctx, &mut tx, path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_sales_summary(&state);
    Ok((
        [("HX-Trigger", "salesReturnChanged")],
        Html(String::new()),
    ))
}

/// 销售退货：拒绝（不予受理）
#[require_permission("SHIPPING", "update")]
pub async fn reject_return(
    path: ScReturnRejectPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .sales_return_service()
        .reject(&service_ctx, &mut tx, path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_sales_summary(&state);
    Ok((
        [("HX-Trigger", "salesReturnChanged")],
        Html(String::new()),
    ))
}

/// 月对账单：异议（Sent → Disputed，客户有异议）
#[require_permission("SALES_ORDER", "update")]
pub async fn dispute_recon(
    path: ScReconDisputePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .reconciliation_service()
        .dispute(&service_ctx, &mut tx, path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_sales_summary(&state);
    Ok((
        [("HX-Trigger", "salesReconChanged")],
        Html(String::new()),
    ))
}

/// 月对账单：重开（Disputed/Settled → 可编辑）
#[require_permission("SALES_ORDER", "update")]
pub async fn reopen_recon(
    path: ScReconReopenPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .reconciliation_service()
        .reopen(&service_ctx, &mut tx, path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_sales_summary(&state);
    Ok((
        [("HX-Trigger", "salesReconChanged")],
        Html(String::new()),
    ))
}

/// 月对账单：强制结算（→ Settled，跳过客户确认）
#[require_permission("SALES_ORDER", "update")]
pub async fn force_settle_recon(
    path: ScReconForceSettlePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .reconciliation_service()
        .force_settle(&service_ctx, &mut tx, path.id)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_sales_summary(&state);
    Ok((
        [("HX-Trigger", "salesReconChanged")],
        Html(String::new()),
    ))
}

// =============================================================================
// 创建 drawer（就地新建）
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct ReconCreateForm {
    pub customer_id: i64,
    pub period: String,
}

#[require_permission("SALES_ORDER", "read")]
pub async fn get_recon_create_drawer(
    _path: ScReconCreateDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let customers = state
        .customer_service()
        .list(
            &service_ctx,
            &mut conn,
            CustomerQuery {
                name: None,
                status: None,
                category: None,
                owner_id: None,
            },
            PageParams::new(1, 200),
        )
        .await
        .map(|r| r.items)
        .unwrap_or_default();
    Ok(Html(recon_create_form(&customers).into_string()))
}

#[require_permission("SALES_ORDER", "create")]
pub async fn post_recon_create(
    _path: ScReconCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ReconCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .reconciliation_service()
        .create(&service_ctx, &mut tx, form.customer_id, form.period)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_sales_summary(&state);
    Ok(([("HX-Trigger", "salesReconChanged")], Html(String::new())))
}

fn recon_create_form(customers: &[Customer]) -> Markup {
    html! {
        form id="sc-recon-create-form" class="space-y-4"
            hx-post=(ScReconCreatePath::PATH) hx-swap="none"
            _="on 'htmx:afterRequest'[detail.xhr.responseText.length == 0 and detail.elt is me] remove .open from #sc-recon-create-overlay" {
            div {
                label class="block text-xs font-medium text-fg-2 mb-1" { "客户 " span class="text-danger" { "*" } }
                select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                    name="customer_id" required {
                    option value="" disabled selected { "请选择客户" }
                    @for c in customers {
                        option value=(c.id) { (c.name) }
                    }
                }
            }
            div {
                label class="block text-xs font-medium text-fg-2 mb-1" { "对账期间 " span class="text-danger" { "*" } }
                input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                    type="month" name="period" required;
            }
            div class="flex justify-end gap-2 pt-3 border-t border-border-soft" {
                button type="button" class=(BTN_GHOST)
                    _="on click remove .open from #sc-recon-create-overlay" { "取消" }
                button type="submit" class=(BTN_PRIMARY) { "创建对账单" }
            }
        }
    }
}

// =============================================================================
// 渲染辅助：tab 栏 / 筛选 / 表格 / 行展开明细 / drawer 操作
// =============================================================================

/// 顶部业务 tab 栏（4 业务 + badge），放进各 card 端点返回的 HTML，随刷新重渲染。
fn sc_tab_bar(active: &str, s: &SalesWorkCenterSummary) -> Markup {
    let tab = |val: &str,
               path: &'static str,
               tab_icon: Markup,
               label: &str,
               cnt: u64|
     -> Markup {
        html! {
            button class=(toggle_cls(active == val)) type="button"
                hx-get=(path) hx-vals="{}"
                hx-target="#sc-card" hx-select="#sc-card" hx-swap="outerHTML" {
                (tab_icon) (label) (tab_badge(cnt))
            }
        }
    };
    html! {
        div class="flex items-center gap-1 flex-wrap px-5 pt-3 border-b border-border-soft" {
            (tab("quotations", ScQuotationsPath::PATH, icon::file_text_icon("w-4 h-4"), "报价单", s.total_quotations))
            (tab("orders", ScOrdersPath::PATH, icon::package_icon("w-4 h-4"), "销售订单", s.total_orders))
            (tab("returns", ScReturnsPath::PATH, icon::return_arrow_icon("w-4 h-4"), "销售退货", s.total_returns))
            (tab("settlement", ScSettlementPath::PATH, icon::payment_icon("w-4 h-4"), "对账收款", s.total_recon))
        }
    }
}

fn tab_badge(n: u64) -> Markup {
    if n > 0 {
        html! {
            span class="ml-1 inline-flex items-center justify-center min-w-[20px] h-5 px-1.5 rounded-full bg-accent text-accent-on text-[11px] font-bold font-mono tabular-nums leading-none" {
                (n)
            }
        }
    } else {
        html! {}
    }
}

fn toggle_cls(active: bool) -> &'static str {
    if active {
        "inline-flex items-center gap-1 px-3.5 py-1.5 text-sm text-accent font-semibold cursor-pointer bg-accent-bg rounded-sm border-none transition-colors"
    } else {
        "inline-flex items-center gap-1 px-3.5 py-1.5 text-sm text-muted font-medium cursor-pointer bg-transparent border-none rounded-sm hover:text-fg hover:bg-surface transition-colors"
    }
}

/// 状态下拉选项（(value, label)），各业务由 *_status_options 提供。
fn filter_bar(
    path: &'static str,
    search_placeholder: &str,
    status: Option<i16>,
    per_page: u32,
    p: &CardParams,
    status_opts: Vec<(i16, &'static str)>,
) -> Markup {
    let kw = p.keyword.as_deref().unwrap_or("");
    html! {
        form class="flex items-center gap-2 flex-wrap px-5 py-3 border-b border-border-soft"
            hx-get=(path)
            hx-trigger="change, keyup changed delay:300ms from:.sc-search"
            hx-target="#sc-card" hx-select="#sc-card" hx-swap="outerHTML" {
            select class="px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg cursor-pointer"
                name="status" {
                option value="" selected[status.is_none()] { "全部状态" }
                @for (v, label) in &status_opts {
                    option value=(v) selected[status == Some(*v)] { (label) }
                }
            }
            div class="relative" {
                (icon::search_icon("w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"));
                input class="sc-search w-[200px] pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                    type="text" name="keyword" placeholder=(search_placeholder)
                    value=(kw);
            }
            (per_page_select(per_page))
        }
        form id="sc-filter-form" class="hidden" {
            input type="hidden" name="keyword" value=(kw);
            input type="hidden" name="status" value=(status.map(|s| s.to_string()).unwrap_or_default());
            input type="hidden" name="per_page" value=(per_page);
        }
    }
}

fn per_page_select(cur: u32) -> Markup {
    html! {
        select class="px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg cursor-pointer"
            name="per_page" {
            @for &v in &[5u32, 10, 20, 50] {
                option value=(v) selected[cur == v] { (v) " /页" }
            }
        }
    }
}

fn quotation_status_options() -> Vec<(i16, &'static str)> {
    vec![
        (1, "草稿"),
        (2, "已发送"),
        (3, "已接受"),
        (4, "已拒绝"),
        (5, "已失效"),
    ]
}

fn order_status_options() -> Vec<(i16, &'static str)> {
    vec![
        (1, "草稿"),
        (2, "待发货"),
        (3, "可发货"),
        (8, "已申请发货"),
        (4, "部分发货"),
        (5, "已发货"),
        (7, "已取消"),
    ]
}

fn return_status_options() -> Vec<(i16, &'static str)> {
    vec![
        (1, "草稿"),
        (2, "已确认"),
        (3, "已收货"),
        (4, "检验中"),
        (5, "已完成"),
        (6, "已取消"),
        (7, "已拒绝"),
    ]
}

fn recon_status_options() -> Vec<(i16, &'static str)> {
    vec![
        (1, "草稿"),
        (2, "已发送"),
        (3, "已确认"),
        (4, "有异议"),
        (5, "已结算"),
    ]
}

fn th(label: &str, cls_extra: &str) -> Markup {
    html! {
        th class=(format!(
            "font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft {}",
            cls_extra
        )) { (label) }
    }
}

const ROW_ROTATE: &str = "[&.open_.row-chev]:rotate-90";

/// 单号列：点击打开 detail drawer（hx-get 填充 body + afterRequest add .open）。
fn doc_drawer_btn(doc_number: &str, drawer_path: &str, body_sel: &str, overlay_sel: &str) -> Markup {
    html! {
        button class="text-accent hover:underline cursor-pointer border-none bg-transparent p-0 font-mono"
            hx-get=(drawer_path)
            hx-target=(body_sel) hx-swap="innerHTML"
            _=(format!(
                "on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to {}",
                overlay_sel
            )) {
            (doc_number)
        }
    }
}

fn quotation_table(items: &[Quotation], names: &HashMap<i64, String>) -> Markup {
    html! {
        div class="overflow-x-auto mt-2" {
            table class="w-full text-sm border-collapse" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="w-10 py-2 px-2 border border-border-soft" {}
                        (th("报价号", "text-left"))
                        (th("客户", "text-left"))
                        (th("金额", "text-right"))
                        (th("状态", "text-left"))
                    }
                }
                tbody {
                    @if items.is_empty() {
                        tr { td colspan="5" class="text-center text-muted py-8 border border-border-soft" { "暂无报价单" } }
                    } @else {
                        @for q in items {
                            @let row_id = format!("sc-quo-{}", q.id);
                            tr class=(ROW_ROTATE) {
                                td class="text-center py-2 px-2 border border-border-soft" {
                                    (row_expand::row_expand_toggle(&row_id, &ScQuotationRowDetailPath { id: q.id }.to_string()))
                                }
                                td class="py-2.5 px-3 border border-border-soft whitespace-nowrap" {
                                    (doc_drawer_btn(&q.doc_number, &ScQuotationDetailDrawerPath { id: q.id }.to_string(), "#sc-quo-drawer-body", "#sc-quo-overlay"))
                                }
                                td class="py-2.5 px-3 border border-border-soft whitespace-nowrap" {
                                    (name_cell(names.get(&q.customer_id)))
                                }
                                td class="py-2.5 px-3 text-right font-mono border border-border-soft whitespace-nowrap" { (fmt_decimal(q.total_amount)) }
                                td class="py-2.5 px-3 border border-border-soft whitespace-nowrap" { (quotation_status_pill(q.status)) }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn orders_table(items: &[SalesOrder], names: &HashMap<i64, String>) -> Markup {
    html! {
        div class="overflow-x-auto mt-2" {
            table class="w-full text-sm border-collapse" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="w-10 py-2 px-2 border border-border-soft" {}
                        (th("订单号", "text-left"))
                        (th("客户", "text-left"))
                        (th("金额", "text-right"))
                        (th("状态", "text-left"))
                    }
                }
                tbody {
                    @if items.is_empty() {
                        tr { td colspan="5" class="text-center text-muted py-8 border border-border-soft" { "暂无销售订单" } }
                    } @else {
                        @for o in items {
                            @let row_id = format!("sc-order-{}", o.id);
                            tr class=(ROW_ROTATE) {
                                td class="text-center py-2 px-2 border border-border-soft" {
                                    (row_expand::row_expand_toggle(&row_id, &ScOrderRowDetailPath { id: o.id }.to_string()))
                                }
                                td class="py-2.5 px-3 border border-border-soft whitespace-nowrap" {
                                    (doc_drawer_btn(&o.doc_number, &ScOrderDetailDrawerPath { id: o.id }.to_string(), "#sc-order-drawer-body", "#sc-order-overlay"))
                                }
                                td class="py-2.5 px-3 border border-border-soft whitespace-nowrap" {
                                    (name_cell(names.get(&o.customer_id)))
                                }
                                td class="py-2.5 px-3 text-right font-mono border border-border-soft whitespace-nowrap" { (fmt_decimal(o.total_amount)) }
                                td class="py-2.5 px-3 border border-border-soft whitespace-nowrap" { (order_status_pill(o.status)) }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn returns_table(items: &[SalesReturn], names: &HashMap<i64, String>) -> Markup {
    html! {
        div class="overflow-x-auto mt-2" {
            table class="w-full text-sm border-collapse" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="w-10 py-2 px-2 border border-border-soft" {}
                        (th("退货号", "text-left"))
                        (th("客户", "text-left"))
                        (th("来源单", "text-left"))
                        (th("金额", "text-right"))
                        (th("状态", "text-left"))
                    }
                }
                tbody {
                    @if items.is_empty() {
                        tr { td colspan="6" class="text-center text-muted py-8 border border-border-soft" { "暂无销售退货" } }
                    } @else {
                        @for r in items {
                            @let row_id = format!("sc-return-{}", r.id);
                            tr class=(ROW_ROTATE) {
                                td class="text-center py-2 px-2 border border-border-soft" {
                                    (row_expand::row_expand_toggle(&row_id, &ScReturnRowDetailPath { id: r.id }.to_string()))
                                }
                                td class="py-2.5 px-3 border border-border-soft whitespace-nowrap" {
                                    (doc_drawer_btn(&r.doc_number, &ScReturnDetailDrawerPath { id: r.id }.to_string(), "#sc-return-drawer-body", "#sc-return-overlay"))
                                }
                                td class="py-2.5 px-3 border border-border-soft whitespace-nowrap" {
                                    (name_cell(names.get(&r.customer_id)))
                                }
                                td class="py-2.5 px-3 font-mono text-muted border border-border-soft whitespace-nowrap" {
                                    "#" (r.order_id)
                                }
                                td class="py-2.5 px-3 text-right font-mono border border-border-soft whitespace-nowrap" { (fmt_decimal(r.total_amount)) }
                                td class="py-2.5 px-3 border border-border-soft whitespace-nowrap" { (return_status_pill(r.status)) }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn settlement_table(items: &[Reconciliation], names: &HashMap<i64, String>) -> Markup {
    html! {
        div class="overflow-x-auto mt-2" {
            table class="w-full text-sm border-collapse" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="w-10 py-2 px-2 border border-border-soft" {}
                        (th("对账单号", "text-left"))
                        (th("客户", "text-left"))
                        (th("期间", "text-left"))
                        (th("金额", "text-right"))
                        (th("已确认", "text-right"))
                        (th("状态", "text-left"))
                    }
                }
                tbody {
                    @if items.is_empty() {
                        tr { td colspan="7" class="text-center text-muted py-8 border border-border-soft" { "暂无对账单" } }
                    } @else {
                        @for r in items {
                            @let row_id = format!("sc-recon-{}", r.id);
                            tr class=(ROW_ROTATE) {
                                td class="text-center py-2 px-2 border border-border-soft" {
                                    (row_expand::row_expand_toggle(&row_id, &ScSettlementRowDetailPath { recon_type: "draft".into(), ref_id: r.id }.to_string()))
                                }
                                td class="py-2.5 px-3 border border-border-soft whitespace-nowrap" {
                                    (doc_drawer_btn(&r.doc_number, &ScSettlementDetailDrawerPath { id: r.id }.to_string(), "#sc-recon-drawer-body", "#sc-recon-overlay"))
                                }
                                td class="py-2.5 px-3 border border-border-soft whitespace-nowrap" {
                                    (name_cell(names.get(&r.customer_id)))
                                }
                                td class="py-2.5 px-3 font-mono text-muted border border-border-soft whitespace-nowrap" { (r.period) }
                                td class="py-2.5 px-3 text-right font-mono border border-border-soft whitespace-nowrap" { (fmt_decimal(r.total_amount)) }
                                td class="py-2.5 px-3 text-right font-mono border border-border-soft whitespace-nowrap" { (fmt_decimal(r.confirmed_amount)) }
                                td class="py-2.5 px-3 border border-border-soft whitespace-nowrap" { (recon_status_pill(r.status)) }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn name_cell(name: Option<&String>) -> Markup {
    html! {
        span class="inline-block max-w-[180px] truncate align-middle"
            title=(name.map(|s| s.as_str()).unwrap_or("—")) {
            (name.map(|s| s.as_str()).unwrap_or("—"))
        }
    }
}

// ── 行展开 / drawer 共用的明细 grid ──

fn kv(label: &str, value: Markup, value_cls: &str) -> Markup {
    html! {
        div {
            div class="text-xs text-muted mb-0.5 font-semibold uppercase tracking-wide" { (label) }
            div class=(format!("font-mono {}", value_cls)) { (value) }
        }
    }
}

fn quotation_detail_grid(hub: &QuotationHubSummary) -> Markup {
    html! {
        div class="grid grid-cols-3 gap-6 text-sm" {
            (kv("客户", html! { (hub.customer_name) }, "text-fg"))
            (kv("报价金额", html! { (fmt_decimal(hub.total_amount)) }, "text-fg"))
            (kv("明细行数", html! { (hub.item_count) " 行" }, "text-fg"))
            div class="col-span-3" {
                @if hub.can_convert_to_so {
                    span class="inline-flex items-center gap-1 px-2.5 py-1 rounded-full bg-success-bg text-success text-[11px] font-semibold" {
                        (icon::check_circle_icon("w-3 h-3")) "已接受，可转销售订单"
                    }
                } @else {
                    span class="inline-flex items-center gap-1 px-2.5 py-1 rounded-full bg-surface text-muted text-[11px] font-semibold" {
                        (icon::info_icon("w-3 h-3")) "当前状态不可转单"
                    }
                }
            }
        }
    }
}

fn order_detail_grid(hub: &SalesOrderHubSummary) -> Markup {
    html! {
        div class="text-sm" {
            div class="grid grid-cols-4 gap-6 mb-3" {
                (kv("客户", html! { (hub.customer_name) }, "text-fg"))
                (kv("已立应收", html! { (fmt_decimal(hub.ar_summary.ar_amount)) }, "text-fg"))
                (kv("未清应收", html! { (fmt_decimal(hub.ar_summary.outstanding)) }, "text-warn"))
                (kv("来源报价", source_chain_cell(&hub.source_chain), "text-fg"))
            }
            div class="text-xs text-muted mb-1 font-semibold uppercase tracking-wide" { "发货进度" }
            div class="flex items-center gap-4 font-mono" {
                span { "订购 " (fmt_decimal(hub.progress.ordered_qty)) }
                span class="text-success" { "已发 " (fmt_decimal(hub.progress.shipped_qty)) }
                span class="text-warn" { "未交 " (fmt_decimal(hub.progress.open_qty)) }
                span class="text-danger" { "已退 " (fmt_decimal(hub.progress.returned_qty)) }
                span class="text-muted" { "(" (hub.progress.item_count) " 行 · " (fmt_decimal(hub.progress.shipped_pct)) "%)" }
            }
        }
    }
}

fn return_detail_grid(hub: &SalesReturnHubSummary) -> Markup {
    html! {
        div class="grid grid-cols-4 gap-6 text-sm" {
            (kv("客户", html! { (hub.customer_name) }, "text-fg"))
            (kv("来源销售订单", html! { (hub.source_so_doc) }, "text-accent"))
            (kv("退货数量", html! { (fmt_decimal(hub.total_qty)) " (" (hub.item_count) " 行)" }, "text-fg"))
            (kv("退货金额", html! { (fmt_decimal(hub.return_order.total_amount)) }, "text-danger"))
            div class="col-span-4 text-xs text-muted" { (hub.status_hint) }
        }
    }
}

fn settlement_detail_grid(hub: &SettlementHubSummary) -> Markup {
    html! {
        div class="grid grid-cols-4 gap-6 text-sm" {
            (kv("客户", html! { (hub.customer_name) }, "text-fg"))
            (kv("对账期间", html! { (hub.recon.period) }, "text-fg"))
            (kv("对账金额", html! { (fmt_decimal(hub.recon.total_amount)) }, "text-fg"))
            (kv("已确认", html! { (fmt_decimal(hub.recon.confirmed_amount)) }, "text-success"))
            (kv("差额", html! { (fmt_decimal(hub.recon.difference)) }, "text-warn"))
            (kv("明细行数", html! { (hub.recon.item_count) " 行" }, "text-fg"))
            (kv("客户未收余额", html! { (fmt_decimal(hub.ar_outstanding)) }, "text-danger"))
        }
    }
}

fn source_chain_cell(chain: &SalesOrderSourceChain) -> Markup {
    if chain.quotation_docs.is_empty() {
        html! { span class="text-muted" { "—" } }
    } else {
        html! {
            span class="flex flex-col gap-0.5" {
                @for d in &chain.quotation_docs {
                    span { (d) }
                }
            }
        }
    }
}

// ── drawer 状态操作按钮（按状态条件渲染）──

const BTN_PRIMARY: &str =
    "inline-flex items-center px-3.5 py-1.5 rounded-sm bg-accent text-white text-xs font-semibold border-none cursor-pointer hover:opacity-90";
const BTN_GHOST: &str =
    "inline-flex items-center px-3.5 py-1.5 rounded-sm bg-white text-fg-2 border border-border text-xs font-medium cursor-pointer hover:bg-surface";

/// 写操作按钮：hx-post + hx-swap=none + 成功后关 drawer + toast。
fn action_btn(label: &str, post_path: &str, overlay_sel: &str, toast_msg: &str, primary: bool) -> Markup {
    let cls = if primary { BTN_PRIMARY } else { BTN_GHOST };
    html! {
        button type="button" class=(cls)
            hx-post=(post_path) hx-swap="none"
            hx-confirm=(format!("确认{}？", label))
            _=(format!(
                "on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from {} then call showToast('{}')",
                overlay_sel, toast_msg
            )) {
            (label)
        }
    }
}

fn quotation_drawer_actions(q: &Quotation) -> Markup {
    let overlay = "#sc-quo-overlay";
    html! {
        div class="flex items-center justify-end gap-2 pt-3 border-t border-border-soft flex-wrap" {
            @match q.status {
                QuotationStatus::Draft => {
                    (action_btn("提交报价", &ScQuotationSubmitPath { id: q.id }.to_string(), overlay, "报价已提交", true))
                }
                QuotationStatus::Sent => {
                    (action_btn("拒绝", &ScQuotationRejectPath { id: q.id }.to_string(), overlay, "报价已拒绝", false))
                    (action_btn("客户接受", &ScQuotationAcceptPath { id: q.id }.to_string(), overlay, "报价已接受", true))
                }
                QuotationStatus::Accepted => {
                    (action_btn("失效", &ScQuotationExpirePath { id: q.id }.to_string(), overlay, "报价已失效", false))
                    (action_btn("转销售订单", &ScQuotationToSoPath { id: q.id }.to_string(), overlay, "已转销售订单", true))
                }
                _ => {}
            }
        }
    }
}

fn order_drawer_actions(o: &SalesOrder) -> Markup {
    let overlay = "#sc-order-overlay";
    html! {
        div class="flex items-center justify-end gap-2 pt-3 border-t border-border-soft flex-wrap" {
            @match o.status {
                SalesOrderStatus::Draft => {
                    (action_btn("取消订单", &ScOrderCancelPath { id: o.id }.to_string(), overlay, "订单已取消", false))
                    (action_btn("确认订单", &ScOrderConfirmPath { id: o.id }.to_string(), overlay, "订单已确认", true))
                }
                SalesOrderStatus::Confirmed | SalesOrderStatus::ReadyToShip => {
                    // 申请发货：复用销售订单详情页的 RequestShipPath modal（建发货单 + 订单 ShippingRequested）
                    button type="button" class=(BTN_PRIMARY)
                        hx-get=(crate::routes::order::RequestShipPath { id: o.id }.to_string())
                        hx-target="body" hx-swap="beforeend" {
                        (icon::truck_icon("w-3.5 h-3.5")) "申请发货"
                    }
                    (action_btn("取消订单", &ScOrderCancelPath { id: o.id }.to_string(), overlay, "订单已取消", false))
                }
                SalesOrderStatus::ShippingRequested | SalesOrderStatus::PartiallyShipped => {
                    button type="button" class=(BTN_GHOST)
                        hx-get=(crate::routes::order::RequestShipPath { id: o.id }.to_string())
                        hx-target="body" hx-swap="beforeend" {
                        (icon::truck_icon("w-3.5 h-3.5")) "继续发货"
                    }
                }
                _ => {}
            }
        }
    }
}

fn return_drawer_actions(r: &SalesReturn) -> Markup {
    let overlay = "#sc-return-overlay";
    html! {
        div class="flex items-center justify-end gap-2 pt-3 border-t border-border-soft flex-wrap" {
            @match r.status {
                ReturnStatus::Draft => {
                    (action_btn("拒绝", &ScReturnRejectPath { id: r.id }.to_string(), overlay, "退货已拒绝", false))
                    (action_btn("取消", &ScReturnCancelPath { id: r.id }.to_string(), overlay, "退货已取消", false))
                    (action_btn("确认退货", &ScReturnApprovePath { id: r.id }.to_string(), overlay, "退货已确认", true))
                }
                ReturnStatus::Confirmed => {
                    (action_btn("登记收货", &ScReturnReceivePath { id: r.id }.to_string(), overlay, "已登记收货", true))
                }
                ReturnStatus::Received => {
                    (action_btn("完成退货", &ScReturnCompletePath { id: r.id }.to_string(), overlay, "退货已完成", true))
                    (action_btn("开始检验", &ScReturnInspectPath { id: r.id }.to_string(), overlay, "已进入检验", false))
                }
                ReturnStatus::Inspecting => {
                    (action_btn("完成退货", &ScReturnCompletePath { id: r.id }.to_string(), overlay, "退货已完成", true))
                }
                _ => {}
            }
        }
    }
}

fn recon_drawer_actions(r: &ReconciliationAggregate, _rt: SettlementReconType) -> Markup {
    let overlay = "#sc-recon-overlay";
    html! {
        div class="flex items-center justify-end gap-2 pt-3 border-t border-border-soft flex-wrap" {
            @if r.difference != Decimal::ZERO {
                span class="text-xs text-warn mr-auto" { "差额非零，确认前请核对" }
            }
            @match r.status {
                ReconciliationStatus::Draft => {
                    (action_btn("发送客户", &ScReconSendPath { id: r.id }.to_string(), overlay, "对账单已发送", true))
                }
                ReconciliationStatus::Sent => {
                    (action_btn("有异议", &ScReconDisputePath { id: r.id }.to_string(), overlay, "已标记异议", false))
                    (action_btn("客户已确认", &ScReconConfirmPath { id: r.id }.to_string(), overlay, "对账单已确认", true))
                }
                ReconciliationStatus::Confirmed => {
                    (action_btn("结算", &ScReconSettlePath { id: r.id }.to_string(), overlay, "对账单已结算", true))
                }
                ReconciliationStatus::Disputed => {
                    (action_btn("重开", &ScReconReopenPath { id: r.id }.to_string(), overlay, "对账单已重开", false))
                    (action_btn("强制结算", &ScReconForceSettlePath { id: r.id }.to_string(), overlay, "对账单已强制结算", true))
                }
                _ => {}
            }
        }
    }
}

/// Drawer overlay 壳（同采购 render_drawer_overlay）：背景点击/关闭按钮收起，body 由 hx-get 填充。
fn render_sc_drawer_overlay(overlay_id: &str, body_id: &str, title: &str, width_class: &str) -> Markup {
    drawer_shell(overlay_id, width_class, html! {
        div class="flex items-center justify-between px-6 py-5 border-b border-border-soft" {
            div class="font-bold text-base text-fg" { (title) }
            button type="button"
                class="w-8 h-8 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg flex items-center justify-center"
                _=(format!("on click remove .open from #{}", overlay_id)) {
                (icon::x_icon("w-4 h-4"))
            }
        }
        div id=(body_id) class="flex-1 overflow-y-auto px-6 py-5" {}
    })
}

// =============================================================================
// 通用辅助
// =============================================================================

async fn customer_names(
    state: &crate::state::AppState,
    ctx: &abt_core::shared::types::context::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    ids: &[i64],
) -> HashMap<i64, String> {
    if ids.is_empty() {
        return HashMap::new();
    }
    match state.customer_service().get_by_ids(ctx, db, ids).await {
        Ok(r) => r.into_iter().map(|c| (c.id, c.name)).collect(),
        Err(_) => HashMap::new(),
    }
}

fn pill(key: &str, label: &str) -> Markup {
    html! {
        span class=(format!("status-pill {} text-[11px]", crate::utils::status_color(key))) {
            (label)
        }
    }
}

fn quotation_status_pill(s: QuotationStatus) -> Markup {
    use QuotationStatus::*;
    match s {
        Draft => pill("draft", "草稿"),
        Sent => pill("progress", "已发送"),
        Accepted => pill("info", "已接受"),
        Rejected => pill("cancelled", "已拒绝"),
        Expired => pill("cancelled", "已失效"),
    }
}

fn order_status_pill(s: SalesOrderStatus) -> Markup {
    use SalesOrderStatus::*;
    match s {
        Draft => pill("draft", "草稿"),
        Confirmed => pill("progress", "待发货"),
        ReadyToShip => pill("info", "可发货"),
        ShippingRequested => pill("progress", "已申请发货"),
        PartiallyShipped => pill("partial", "部分发货"),
        Shipped => pill("completed", "已发货"),
        Cancelled => pill("cancelled", "已取消"),
    }
}

fn return_status_pill(s: ReturnStatus) -> Markup {
    use ReturnStatus::*;
    match s {
        Draft => pill("draft", "草稿"),
        Confirmed => pill("progress", "已确认"),
        Received => pill("info", "已收货"),
        Inspecting => pill("partial", "检验中"),
        Completed => pill("completed", "已完成"),
        Cancelled => pill("cancelled", "已取消"),
        Rejected => pill("cancelled", "已拒绝"),
    }
}

fn recon_status_pill(s: ReconciliationStatus) -> Markup {
    use ReconciliationStatus::*;
    match s {
        Draft => pill("draft", "草稿"),
        Sent => pill("progress", "已发送"),
        Confirmed => pill("info", "已确认"),
        Disputed => pill("warn", "有异议"),
        Settled => pill("completed", "已结算"),
    }
}

fn fmt_decimal(d: Decimal) -> String {
    d.round_dp(2).to_string()
}
