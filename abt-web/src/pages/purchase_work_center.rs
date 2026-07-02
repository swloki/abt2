//! 采购作业中心 — 需求 / 订单 / 对账付款 / 退货 四 card 聚合工作台。
//!
//! 架构（组件化单端点模式，同 mes_work_center）：
//! - 首页内联渲染 4 个 card 外壳，每个 card 占位 div `hx-trigger="load"` 拉各自端点；
//! - 每个 card 一个 GET 端点，card 内 tab/筛选走该端点 + `hx-select="#pc-xxx-card"` 局部刷新；
//! - 写操作（审批/驳回/对账确认/付款审批）POST 广播 `HX-Trigger`（poChanged / reconChanged），
//!   相关 card 声明 `hx-trigger="poChanged from:body"` 自刷新；首页锚点条监听三者重算 summary。
//! - drawer 就地操作：订单审批 / 付款审批。

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::supplier::SupplierService;
use abt_core::master_data::product::ProductService;
use abt_core::shared::identity::UserService;
use abt_core::purchase::demand_handler::{
    CreateOrderFromDemandsReq, DemandPoolQuery, DemandSummary, MaterialAggQuery,
    MaterialAggSummary, PurchaseDemandService,
};
use abt_core::purchase::enums::{
    InvoiceStatus, MiscRequestStatus, PaymentStatus, PurchaseOrderStatus, PurchaseQuotationStatus,
    PurchaseReconStatus, PurchaseReturnStatus,
};
use abt_core::purchase::order::model::{
    CreateOrderItemRequest, PurchaseOrder, PurchaseOrderItem, PurchaseOrderQuery,
    UpdatePurchaseOrderRequest,
};
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::purchase::TaxRateService;
use abt_core::purchase::payment::model::{PaymentRequest, PaymentRequestQuery};
use abt_core::purchase::payment::PaymentRequestService;
use abt_core::purchase::reconciliation::model::{PurchaseReconciliation, PurchaseReconciliationQuery};
use abt_core::purchase::reconciliation::PurchaseReconciliationService;
use abt_core::purchase::return_order::model::{
    PurchaseReturn, PurchaseReturnItem, PurchaseReturnQuery,
};
use abt_core::purchase::return_order::PurchaseReturnService;
use abt_core::purchase::work_center::{
    PoHubSummary, PurchaseWorkCenterService, PurchaseWorkCenterSummary, ReturnHubSummary,
    SettlementHubSummary, SettlementReconType, ThreeWayMatchSummary,
};
use abt_core::shared::types::{DomainError, PageParams};

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use abt_core::purchase::misc_request::model::{MiscRequestQuery, MiscellaneousRequest};
use abt_core::purchase::misc_request::MiscellaneousRequestService;
use abt_core::purchase::quotation::model::{
    PurchaseQuotation, PurchaseQuotationItem, PurchaseQuotationQuery, QuotationComparison,
};
use abt_core::purchase::quotation::PurchaseQuotationService;
use axum::Form;
use crate::components::icon;
use crate::components::overlay::drawer_shell;
use crate::components::pagination::pagination;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_work_center::*;
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// =============================================================================
// 首页
// =============================================================================

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_work_center(_path: PurchaseWorkCenterPath, ctx: RequestContext) -> Result<Html<String>> {
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
        // detail-header：标题 + 待办总数 + 逾期/临期告警 pill
        div class="flex items-center justify-between mb-4 flex-wrap gap-4" {
            div class="flex items-center gap-2.5" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "采购作业中心" }
                span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-accent-bg text-accent text-xs font-semibold" {
                    span class="font-mono tabular-nums font-bold" { (summary.total()) }
                    "待办"
                }
            }
            div class="flex items-center gap-2" {
                @if summary.overdue_count > 0 {
                    span class="inline-flex items-center gap-1 px-2.5 py-1 rounded-full bg-danger-bg text-danger text-[11px] font-semibold" {
                        (icon::alert_triangle_icon("w-3 h-3"))
                        (summary.overdue_count) " 逾期"
                    }
                }
                @if summary.soon_count > 0 {
                    span class="inline-flex items-center gap-1 px-2.5 py-1 rounded-full bg-warn-bg text-warn text-[11px] font-semibold" {
                        (icon::clock_icon("w-3 h-3"))
                        (summary.soon_count) " 临期"
                    }
                }
            }
        }
        // card 外壳（section）与内容（#pc-card）分离，对齐 mes_work_center render_card_shell：
        // 标题栏（图标 + 标题 + meta）持久；#pc-card 由各端点返回替换（tab 栏 + 筛选 + 表格 + 分页）。
        section class="bg-bg border border-border-soft rounded-lg mb-4 shadow-[var(--shadow-card)] overflow-hidden" {
            div class="flex items-center gap-3 px-5 py-3 border-b border-border-soft" {
                div class="w-7 h-7 rounded-md grid place-items-center bg-accent-bg text-accent shrink-0" {
                    (icon::package_icon("w-[18px] h-[18px]"))
                }
                span class="font-semibold text-fg shrink-0" { "采购作业" }
                span class="text-xs text-muted font-mono flex-1 truncate" {
                    (summary.total()) " 件待办 · 需求 / 订单 / 对账 / 退货 一屏处理"
                }
            }
            div id="pc-card"
                hx-get=(PcDemandPath::PATH) hx-trigger="load" hx-target="this" hx-swap="outerHTML" {
                "加载中…"
            }
        }
        (render_drawer_overlay("approve-overlay", "approve-drawer", "approve-drawer-body", "审批采购订单", "w-[480px] max-w-[92vw]"))
        (render_drawer_overlay("pay-overlay", "pay-drawer", "pay-drawer-body", "审批付款", "w-[480px] max-w-[92vw]"))
        (render_drawer_overlay("convert-po-overlay", "convert-po-drawer", "convert-po-drawer-body", "转采购单", "w-[480px] max-w-[92vw]"))
        (render_drawer_overlay("po-detail-overlay", "po-detail-drawer", "po-detail-drawer-body", "采购订单详情", "w-[900px] max-w-[92vw]"))
        (render_drawer_overlay("return-detail-overlay", "return-detail-drawer", "return-detail-drawer-body", "退货详情", "w-[680px] max-w-[92vw]"))
        (render_drawer_overlay("quotation-detail-overlay", "quotation-detail-drawer", "quotation-detail-drawer-body", "报价详情", "w-[680px] max-w-[92vw]"))
    };

    Ok(Html(
        admin_page(
            is_htmx,
            "采购作业中心",
            &claims,
            "purchase",
            PurchaseWorkCenterPath::PATH,
            "采购管理",
            Some("采购作业中心"),
            content,
            &nav_filter,
        )
        .into_string(),
    ))
}

// =============================================================================
// Card 端点
// =============================================================================

// ── ① 采购需求 ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct DemandCardParams {
    /// "material" | "detail"（默认 material）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub view: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub keyword: Option<String>,
    /// 供应商过滤（仅 detail 视图）：列出该供应商可供的待采购需求
    #[serde(default, deserialize_with = "empty_as_none")]
    pub supplier_id: Option<i64>,
    #[serde(default)]
    pub page: Option<u32>,
}

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_demand_card(
    _path: PcDemandPath,
    ctx: RequestContext,
    Query(p): Query<DemandCardParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.purchase_demand_service();
    let view = p.view.as_deref().unwrap_or("material");
    let summary = cached_summary(&state, &service_ctx, &mut conn).await;

    // 当前选中供应商名（detail 视图 supplier_search 回显用；控件搜全部供应商，不限定可转）
    let supplier_name: Option<String> = match (view == "detail", p.supplier_id) {
        (true, Some(sid)) => supplier_names(&state, &service_ctx, &mut conn, &[sid])
            .await
            .get(&sid)
            .cloned(),
        _ => None,
    };

    let page = p.page.unwrap_or(1);
    let (body, pager) = if view == "detail" {
        let result = svc
            .list_pending_demands(
                &service_ctx,
                &mut conn,
                DemandPoolQuery {
                    keyword: p.keyword.clone(),
                    supplier_id: p.supplier_id,
                    ..Default::default()
                },
                PageParams::new(page, 10),
            )
            .await?;
        (
            demand_detail_table(&result, p.supplier_id),
            pagination(PcDemandPath::PATH, "#pc-card", "#pc-filter-form", result.total, result.page, result.total_pages),
        )
    } else {
        let result = svc
            .list_material_aggregated(
                &service_ctx,
                &mut conn,
                MaterialAggQuery {
                    keyword: p.keyword.clone(),
                    ..Default::default()
                },
                PageParams::new(page, 10),
            )
            .await?;
        (
            demand_material_table(&result),
            pagination(PcDemandPath::PATH, "#pc-card", "#pc-filter-form", result.total, result.page, result.total_pages),
        )
    };

    Ok(Html(
        html! {
            div id="pc-card"
                hx-get=(PcDemandPath::PATH)
                hx-trigger="poChanged from:body, reconChanged from:body, returnChanged from:body, demandChanged from:body"
                hx-vals=(serde_json::json!({ "view": view }).to_string())
                hx-include="#pc-filter-form"
                hx-target="this" hx-select="#pc-card" hx-swap="outerHTML" {
                (pc_tab_bar(if view == "detail" { "demand-detail" } else { "demand-material" }, &summary))
                (demand_filter_bar(view, &p, supplier_name.as_deref()))
                (body)
                (pager)
            }
        }
        .into_string(),
    ))
}

// ── ② 采购订单 ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct OrdersCardParams {
    /// 状态筛选（默认全部，对齐 purchase_order_list）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub keyword: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
}

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_orders_card(
    _path: PcOrdersPath,
    ctx: RequestContext,
    Query(p): Query<OrdersCardParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.purchase_order_service();
    let summary = cached_summary(&state, &service_ctx, &mut conn).await;
    let status = p.status.and_then(PurchaseOrderStatus::from_i16);
    let page = p.page.unwrap_or(1);
    let result = svc
        .list(
            &service_ctx,
            &mut conn,
            PurchaseOrderQuery {
                status,
                doc_number: p.keyword.clone(),
                ..Default::default()
            },
            PageParams::new(page, 10),
        )
        .await?;
    let ids: Vec<i64> = result.items.iter().map(|o| o.supplier_id).collect();
    let names = supplier_names(&state, &service_ctx, &mut conn, &ids).await;

    Ok(Html(
        html! {
            div id="pc-card"
                hx-get=(PcOrdersPath::PATH)
                hx-trigger="poChanged from:body, reconChanged from:body, returnChanged from:body, demandChanged from:body"
                hx-include="#pc-filter-form"
                hx-target="this" hx-select="#pc-card" hx-swap="outerHTML" {
                (pc_tab_bar("orders", &summary))
                (orders_filter_bar(p.status, &p))
                (orders_table(&result.items, &names))
                (pagination(PcOrdersPath::PATH, "#pc-card", "#pc-filter-form", result.total, result.page, result.total_pages))
            }
        }
        .into_string(),
    ))
}

// ── ③ 对账付款 ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct SettlementCardParams {
    /// "recon" | "payment"（默认 recon — 对账单 / 付款 实体切换）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub tab: Option<String>,
    /// 状态筛选（默认全部，对齐 recon_list / payment_list）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub keyword: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
}

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_settlement_card(
    _path: PcSettlementPath,
    ctx: RequestContext,
    Query(p): Query<SettlementCardParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let tab = p.tab.as_deref().unwrap_or("recon");
    let summary = cached_summary(&state, &service_ctx, &mut conn).await;

    let page = p.page.unwrap_or(1);
    let (body, pager) = if tab == "payment" {
        let svc = state.payment_request_service();
        let status = p.status.and_then(PaymentStatus::from_i16);
        let result = svc
            .list(
                &service_ctx,
                &mut conn,
                PaymentRequestQuery {
                    status,
                    keyword: p.keyword.clone(),
                    ..Default::default()
                },
                PageParams::new(page, 10),
            )
            .await?;
        let ids: Vec<i64> = result.items.iter().map(|x| x.supplier_id).collect();
        let names = supplier_names(&state, &service_ctx, &mut conn, &ids).await;
        (
            payment_table(&result.items, &names),
            pagination(PcSettlementPath::PATH, "#pc-card", "#pc-filter-form", result.total, result.page, result.total_pages),
        )
    } else {
        let svc = state.purchase_reconciliation_service();
        let status = p.status.and_then(PurchaseReconStatus::from_i16);
        let result = svc
            .list(
                &service_ctx,
                &mut conn,
                PurchaseReconciliationQuery {
                    status,
                    ..Default::default()
                },
                PageParams::new(page, 10),
            )
            .await?;
        let ids: Vec<i64> = result.items.iter().map(|x| x.supplier_id).collect();
        let names = supplier_names(&state, &service_ctx, &mut conn, &ids).await;
        (
            recon_table(&result.items, &names),
            pagination(PcSettlementPath::PATH, "#pc-card", "#pc-filter-form", result.total, result.page, result.total_pages),
        )
    };

    Ok(Html(
        html! {
            div id="pc-card"
                hx-get=(PcSettlementPath::PATH)
                hx-trigger="poChanged from:body, reconChanged from:body, returnChanged from:body, demandChanged from:body"
                hx-include="#pc-filter-form"
                hx-target="this" hx-select="#pc-card" hx-swap="outerHTML" {
                (pc_tab_bar("settlement", &summary))
                (settlement_filter_bar(tab, p.status, &p))
                (body)
                (pager)
            }
        }
        .into_string(),
    ))
}

// ── ④ 采购退货 ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ReturnsCardParams {
    /// 状态筛选（默认全部，对齐 purchase_return_list）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub keyword: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
}

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_returns_card(
    _path: PcReturnsPath,
    ctx: RequestContext,
    Query(p): Query<ReturnsCardParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.purchase_return_service();
    let summary = cached_summary(&state, &service_ctx, &mut conn).await;
    let status = p.status.and_then(PurchaseReturnStatus::from_i16);
    let page = p.page.unwrap_or(1);
    let result = svc
        .list(
            &service_ctx,
            &mut conn,
            PurchaseReturnQuery {
                status,
                ..Default::default()
            },
            PageParams::new(page, 10),
        )
        .await?;
    let ids: Vec<i64> = result.items.iter().map(|x| x.supplier_id).collect();
    let names = supplier_names(&state, &service_ctx, &mut conn, &ids).await;

    Ok(Html(
        html! {
            div id="pc-card"
                hx-get=(PcReturnsPath::PATH)
                hx-trigger="poChanged from:body, reconChanged from:body, returnChanged from:body, demandChanged from:body"
                hx-include="#pc-filter-form"
                hx-target="this" hx-select="#pc-card" hx-swap="outerHTML" {
                (pc_tab_bar("returns", &summary))
                (returns_filter_bar(p.status, &p))
                (returns_table(&result.items, &names))
                (pagination(PcReturnsPath::PATH, "#pc-card", "#pc-filter-form", result.total, result.page, result.total_pages))
            }
        }
        .into_string(),
    ))
}

// ── ⑤ 供应商报价 ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct QuotationCardParams {
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default)]
    pub page: Option<u32>,
}

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_quotation_card(
    _path: PcQuotationPath,
    ctx: RequestContext,
    Query(p): Query<QuotationCardParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.purchase_quotation_service();
    let summary = cached_summary(&state, &service_ctx, &mut conn).await;
    let status = p.status.and_then(PurchaseQuotationStatus::from_i16);
    let page = p.page.unwrap_or(1);
    let result = svc
        .list(
            &service_ctx,
            &mut conn,
            PurchaseQuotationQuery {
                status,
                ..Default::default()
            },
            PageParams::new(page, 10),
        )
        .await?;
    let ids: Vec<i64> = result.items.iter().map(|q| q.supplier_id).collect();
    let names = supplier_names(&state, &service_ctx, &mut conn, &ids).await;
    Ok(Html(
        html! {
            div id="pc-card"
                hx-get=(PcQuotationPath::PATH)
                hx-trigger="poChanged from:body, reconChanged from:body, returnChanged from:body, demandChanged from:body, quotationChanged from:body"
                hx-include="#pc-filter-form"
                hx-target="this" hx-select="#pc-card" hx-swap="outerHTML" {
                (pc_tab_bar("quotation", &summary))
                (quotation_filter_bar(p.status))
                (quotation_table(&result.items, &names))
                (pagination(PcQuotationPath::PATH, "#pc-card", "#pc-filter-form", result.total, result.page, result.total_pages))
            }
        }
        .into_string(),
    ))
}

// ── ⑥ 零星请购 ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct MiscCardParams {
    #[serde(default, deserialize_with = "empty_as_none")]
    pub status: Option<i16>,
    #[serde(default)]
    pub page: Option<u32>,
}

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_misc_card(
    _path: PcMiscPath,
    ctx: RequestContext,
    Query(p): Query<MiscCardParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.misc_request_service();
    let summary = cached_summary(&state, &service_ctx, &mut conn).await;
    let status = p.status.and_then(MiscRequestStatus::from_i16);
    let page = p.page.unwrap_or(1);
    let result = svc
        .list(
            &service_ctx,
            &mut conn,
            MiscRequestQuery {
                status,
                ..Default::default()
            },
            PageParams::new(page, 10),
        )
        .await?;
    Ok(Html(
        html! {
            div id="pc-card"
                hx-get=(PcMiscPath::PATH)
                hx-trigger="poChanged from:body, reconChanged from:body, returnChanged from:body, demandChanged from:body"
                hx-include="#pc-filter-form"
                hx-target="this" hx-select="#pc-card" hx-swap="outerHTML" {
                (pc_tab_bar("misc", &summary))
                (misc_filter_bar(p.status))
                (misc_table(&result.items))
                (pagination(PcMiscPath::PATH, "#pc-card", "#pc-filter-form", result.total, result.page, result.total_pages))
            }
        }
        .into_string(),
    ))
}

// =============================================================================
// 行展开 row-detail（HTMX hx-swap="afterend" 按需加载单 <tr class="row-detail">）
// =============================================================================

#[derive(Debug, Deserialize, Clone, Default)]
pub struct DemandRowsParams {
    pub product_id: i64,
}

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_demand_rows(
    _path: PcDemandRowsPath,
    ctx: RequestContext,
    Query(p): Query<DemandRowsParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let result = state
        .purchase_demand_service()
        .list_pending_demands(
            &service_ctx,
            &mut conn,
            DemandPoolQuery {
                product_id: Some(p.product_id),
                ..Default::default()
            },
            PageParams::new(1, 50),
        )
        .await?;
    Ok(Html(demand_expand_rows(&result.items).into_string()))
}

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_order_row_detail(
    path: PcOrderRowDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let summary = state
        .purchase_work_center_service()
        .get_po_hub_summary(&service_ctx, &mut conn, path.id)
        .await?;
    Ok(Html(order_row_detail_tr(&summary).into_string()))
}

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_settlement_row_detail(
    path: PcSettlementRowDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let recon_type = SettlementReconType::parse(&path.recon_type)
        .ok_or_else(|| DomainError::validation(format!("未知对账类型: {}", path.recon_type)))?;
    let summary = state
        .purchase_work_center_service()
        .get_settlement_hub_summary(&service_ctx, &mut conn, recon_type, path.ref_id)
        .await?;
    Ok(Html(settlement_row_detail_tr(&summary).into_string()))
}

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_return_row_detail(
    path: PcReturnRowDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let summary = state
        .purchase_work_center_service()
        .get_return_hub_summary(&service_ctx, &mut conn, path.id)
        .await?;
    Ok(Html(return_row_detail_tr(&summary).into_string()))
}

// ── 转采购单 drawer（就地转单，复用 create_order_from_demands）──

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_convert_po_drawer(
    path: PcConvertPoDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let demands = state
        .purchase_demand_service()
        .list_pending_demands(
            &service_ctx,
            &mut conn,
            DemandPoolQuery {
                product_id: Some(path.product_id),
                ..Default::default()
            },
            PageParams::new(1, 100),
        )
        .await?;
    let quotes = state
        .purchase_quotation_service()
        .compare(&service_ctx, &mut conn, path.product_id)
        .await
        .unwrap_or_default();
    let supplier_ids: Vec<i64> = quotes.iter().map(|q| q.supplier_id).collect();
    let names = supplier_names(&state, &service_ctx, &mut conn, &supplier_ids).await;
    Ok(Html(
        render_convert_po_body(path.product_id, &demands.items, &quotes, &names).into_string(),
    ))
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct BatchConvertDrawerParams {
    /// 逗号分隔的 demand id（批量栏 JS 拼接）
    #[serde(default)]
    pub demand_ids: String,
}

/// 批量转采购单 drawer body 加载（采购明细 tab）：按选中 demand_ids 取需求详情 +
/// 供应商名 → 渲染汇总 + 交期/备注表单。提交走 post_batch_convert（同一供应商一张 PO）。
#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_batch_convert_drawer(
    path: PcBatchConvertDrawerPath,
    ctx: RequestContext,
    Query(p): Query<BatchConvertDrawerParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let demand_ids: Vec<i64> = p
        .demand_ids
        .split(',')
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect();
    if demand_ids.is_empty() {
        return Err(DomainError::validation("demand_ids 不能为空").into());
    }
    let demands = state
        .purchase_demand_service()
        .get_demands_by_ids(&service_ctx, &mut conn, &demand_ids)
        .await?;
    if demands.is_empty() {
        return Err(DomainError::validation("所选需求不存在或已处理").into());
    }
    let supplier = supplier_names(&state, &service_ctx, &mut conn, &[path.supplier_id])
        .await
        .get(&path.supplier_id)
        .cloned()
        .unwrap_or_default();
    Ok(Html(
        render_batch_convert_body(path.supplier_id, &supplier, &demands).into_string(),
    ))
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ConvertPoForm {
    /// 逗号分隔的 demand id
    #[serde(default)]
    pub demand_ids: String,
    #[serde(default)]
    pub supplier_id: i64,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub expected_delivery_date: Option<String>,
    #[serde(default)]
    pub remark: String,
}

/// 转采购单核心逻辑（单物料 drawer / 批量 drawer 共用）：解析 demand_ids →
/// `create_order_from_demands`（按物料聚合成同一供应商一张 PO）→ 广播 demandChanged + poChanged。
async fn convert_demands_to_po(
    state: &crate::state::AppState,
    service_ctx: &abt_core::shared::types::context::ServiceContext,
    form: ConvertPoForm,
) -> Result<([(&'static str, &'static str); 1], Html<String>)> {
    let demand_ids: Vec<i64> = form
        .demand_ids
        .split(',')
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect();
    if demand_ids.is_empty() {
        return Err(DomainError::validation("demand_ids 不能为空").into());
    }
    let expected_delivery_date = form
        .expected_delivery_date
        .as_deref()
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    let req = CreateOrderFromDemandsReq {
        demand_ids,
        supplier_id: form.supplier_id,
        expected_delivery_date,
        remark: form.remark,
    };
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .purchase_demand_service()
        .create_order_from_demands(service_ctx, &mut tx, req)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_purchase_summary(state);
    Ok(([("HX-Trigger", "demandChanged, poChanged")], Html(String::new())))
}

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn post_convert_po(
    _path: PcConvertPoPath,
    ctx: RequestContext,
    Form(form): Form<ConvertPoForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    convert_demands_to_po(&state, &service_ctx, form).await
}

/// 批量转采购单（采购明细 tab：同供应商多物料多选 → 一张 PO）。复用 convert_demands_to_po。
#[require_permission("PURCHASE_ORDER", "update")]
pub async fn post_batch_convert(
    _path: PcBatchConvertPath,
    ctx: RequestContext,
    Form(form): Form<ConvertPoForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    convert_demands_to_po(&state, &service_ctx, form).await
}

/// 转采购单 drawer body：物料信息 + 供应商报价选择 + 交期/备注 + 提交表单。
fn render_convert_po_body(
    product_id: i64,
    demands: &[DemandSummary],
    quotes: &[QuotationComparison],
    supplier_names: &HashMap<i64, String>,
) -> Markup {
    let demand_ids: Vec<String> = demands.iter().map(|d| d.id.to_string()).collect();
    let demand_ids_str = demand_ids.join(",");
    let product_name = demands
        .first()
        .map(|d| d.product_name.as_str())
        .unwrap_or("—");
    let product_code = demands
        .first()
        .map(|d| d.product_code.as_str())
        .unwrap_or("");
    let total_qty: rust_decimal::Decimal = demands.iter().map(|d| d.quantity).sum();
    // 默认供应商：优选报价供应商（采购员可用 supplier_search 改选任意供应商）
    let preferred = quotes.iter().find(|q| q.is_preferred);
    let default_sid = preferred.map(|q| q.supplier_id.to_string()).unwrap_or_default();
    let default_name = preferred
        .and_then(|q| supplier_names.get(&q.supplier_id))
        .cloned();
    html! {
        div class="mb-4" {
            div class="font-semibold text-fg text-sm" { (product_name) }
            div class="text-xs text-muted font-mono" { (product_code) }
        }
        div class="grid grid-cols-2 gap-4 mb-4" {
            (field_readonly("待转需求数", &format!("{}", demands.len())));
            (field_readonly("总需求量", &fmt_plain(total_qty)));
        }
        div class="mb-1.5 text-xs text-muted font-medium" { "选择供应商 *" }
        form hx-post=(PcConvertPoPath { product_id }.to_string())
            hx-target="this" hx-swap="none"
            _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #convert-po-overlay then call showToast('PO 草稿已生成，单价待补充')" {
            input type="hidden" name="demand_ids" value=(demand_ids_str) {};
            (crate::components::supplier_search::supplier_search_field(
                "pc-convert-sup", "pc-convert-sup-display", "pc-convert-sup-panel", "pc-convert-sup-results",
                "supplier_id", &default_sid, default_name.as_deref(), "供应商",
                true, "w-full", None,
            ))
            // 报价参考（辅助决策，不限制选择）
            @if !quotes.is_empty() {
                div class="mt-2 flex flex-col gap-1" {
                    @for q in quotes {
                        div class="flex items-center gap-2 px-2 py-1 text-xs bg-surface-raised border border-border-soft rounded-sm" {
                            span class="flex-1 text-fg truncate" {
                                (supplier_names.get(&q.supplier_id).map(|s| s.as_str()).unwrap_or("未知供应商"))
                                @if q.is_preferred {
                                    span class="ml-1 px-1.5 py-0.5 rounded-full bg-success-bg text-success text-[10px] font-semibold" { "优选" }
                                }
                            }
                            span class="font-mono text-fg-2 whitespace-nowrap" { (fmt_plain(q.unit_price)) " " (q.currency) }
                            span class="text-muted whitespace-nowrap" { "至 " (q.valid_until.format("%Y-%m-%d").to_string()) }
                        }
                    }
                }
            } @else {
                div class="mt-2 p-2 bg-warn-bg border border-border-soft rounded-sm text-xs text-warn leading-relaxed" {
                    "该物料暂无有效报价，将生成单价待补的 PO 草稿。"
                }
            }
            div class="grid grid-cols-2 gap-4 mt-4 mb-4" {
                div {
                    label class="block text-xs text-muted font-medium mb-1.5" { "期望交期（可选）" }
                    input type="date" name="expected_delivery_date"
                        class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent" {};
                }
                div {
                    label class="block text-xs text-muted font-medium mb-1.5" { "备注" }
                    input type="text" name="remark"
                        class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent" {};
                }
            }
            div class="p-3 mb-3 bg-surface-raised border border-border-soft rounded-sm text-xs text-muted leading-relaxed" {
                "提交后生成 PO 草稿：按 " (demands.len()) " 个需求聚合 "
                (fmt_plain(total_qty)) " " (product_code)
                "；单价待采购员在 PO 详情补充后 confirm。"
            }
            div class="flex justify-end gap-2 pt-3 border-t border-border-soft" {
                button type="submit" class="inline-flex items-center px-4 py-2 rounded-sm bg-accent text-white text-sm font-semibold border-none cursor-pointer hover:opacity-90" {
                    "生成 PO 草稿"
                }
            }
        }
    }
}

/// 批量转单 drawer body：供应商只读 + 待转需求/物料/总量汇总 + 交期/备注 + 提交表单。
fn render_batch_convert_body(supplier_id: i64, supplier: &str, demands: &[DemandSummary]) -> Markup {
    let demand_ids: Vec<String> = demands.iter().map(|d| d.id.to_string()).collect();
    let demand_ids_str = demand_ids.join(",");
    let product_count = demands
        .iter()
        .map(|d| d.product_id)
        .collect::<HashSet<_>>()
        .len();
    let total_qty: rust_decimal::Decimal = demands.iter().map(|d| d.quantity).sum();
    html! {
        div class="mb-4" {
            (field_readonly("供应商", supplier));
        }
        div class="grid grid-cols-3 gap-4 mb-4" {
            (field_readonly("待转需求", &format!("{} 条", demands.len())));
            (field_readonly("涉及物料", &format!("{} 个", product_count)));
            (field_readonly("总量", &fmt_plain(total_qty)));
        }
        form hx-post=(PcBatchConvertPath::PATH)
            hx-target="this" hx-swap="none"
            _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #convert-po-overlay then call showToast('PO 草稿已生成，单价待补充')" {
            input type="hidden" name="demand_ids" value=(demand_ids_str) {};
            input type="hidden" name="supplier_id" value=(supplier_id) {};
            div class="grid grid-cols-2 gap-4 mb-4" {
                div {
                    label class="block text-xs text-muted font-medium mb-1.5" { "期望交期（可选）" }
                    input type="date" name="expected_delivery_date"
                        class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent" {};
                }
                div {
                    label class="block text-xs text-muted font-medium mb-1.5" { "备注" }
                    input type="text" name="remark"
                        class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent" {};
                }
            }
            div class="p-3 mb-3 bg-surface-raised border border-border-soft rounded-sm text-xs text-muted leading-relaxed" {
                "提交后生成同一供应商的一张 PO 草稿：按 " (product_count) " 个物料聚合 "
                (fmt_plain(total_qty)) "（" (demands.len()) " 条需求）；单价待采购员在 PO 详情补充后 confirm。"
            }
            div class="flex justify-end gap-2 pt-3 border-t border-border-soft" {
                button type="submit" class="inline-flex items-center px-4 py-2 rounded-sm bg-accent text-white text-sm font-semibold border-none cursor-pointer hover:opacity-90" {
                    "生成 PO 草稿"
                }
            }
        }
    }
}

// =============================================================================
// Drawer GET（就地操作表单）
// =============================================================================

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_order_approve_drawer(
    path: PcOrderApproveDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let order = state
        .purchase_order_service()
        .get(&service_ctx, &mut conn, path.id)
        .await?;
    let supplier = supplier_names(&state, &service_ctx, &mut conn, &[order.supplier_id])
        .await
        .get(&order.supplier_id)
        .cloned()
        .unwrap_or_default();
    Ok(Html(render_order_approve_body(&order, &supplier).into_string()))
}

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_payment_approve_drawer(
    path: PcPaymentApproveDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let pay = state
        .payment_request_service()
        .get(&service_ctx, &mut conn, path.id)
        .await?;
    let supplier = supplier_names(&state, &service_ctx, &mut conn, &[pay.supplier_id])
        .await
        .get(&pay.supplier_id)
        .cloned()
        .unwrap_or_default();
    let three_way = state
        .purchase_work_center_service()
        .check_three_way_match(&service_ctx, &mut conn, path.id)
        .await
        .unwrap_or_default();
    Ok(Html(render_payment_approve_body(&pay, &supplier, &three_way).into_string()))
}

// ── PO 详情 drawer（就地查看 + Draft 编辑/状态操作，对标 MES order-overlay）──

#[derive(Debug, Deserialize)]
pub(crate) struct PoDrawerForm {
    #[serde(default)]
    supplier_id: i64,
    #[serde(default, deserialize_with = "empty_as_none")]
    expected_delivery_date: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    payment_terms: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    delivery_address: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    remark: Option<String>,
    #[serde(default)]
    items_json: String,
}

#[derive(Debug, Deserialize)]
struct PoItemWeb {
    product_id: String,
    #[serde(default)]
    description: Option<String>,
    quantity: String,
    unit_price: String,
    #[serde(default)]
    item_delivery_date: Option<String>,
    #[serde(default)]
    discount_pct: Option<String>,
    #[serde(default)]
    tax_rate_id: Option<String>,
}

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_po_detail_drawer(
    path: PcPoDetailDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.purchase_order_service();
    let order = svc.get(&service_ctx, &mut conn, path.id).await?;
    let items = svc
        .list_items(&service_ctx, &mut conn, path.id)
        .await
        .unwrap_or_default();
    let supplier_name = supplier_names(&state, &service_ctx, &mut conn, &[order.supplier_id])
        .await
        .get(&order.supplier_id)
        .cloned()
        .unwrap_or_else(|| "未知供应商".into());
    let operator_name = state
        .user_service()
        .get_user(&service_ctx, &mut conn, order.operator_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());
    let (product_codes, product_names): (HashMap<i64, String>, HashMap<i64, String>) = {
        let ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
        if ids.is_empty() {
            (HashMap::new(), HashMap::new())
        } else {
            let products = state
                .product_service()
                .get_by_ids(&service_ctx, &mut conn, ids)
                .await
                .unwrap_or_default();
            (
                products
                    .iter()
                    .map(|p| (p.product_id, p.product_code.clone()))
                    .collect(),
                products
                    .iter()
                    .map(|p| (p.product_id, p.pdt_name.clone()))
                    .collect(),
            )
        }
    };
    let tax_rates = state
        .tax_rate_service()
        .list_active(&service_ctx, &mut conn)
        .await
        .unwrap_or_default();
    Ok(Html(
        render_po_detail_drawer_body(
            &order,
            &items,
            &supplier_name,
            &operator_name,
            &product_codes,
            &product_names,
            &tax_rates,
        )
        .into_string(),
    ))
}

/// Draft PO 编辑保存（复用 purchase_order_edit::update_po 的解析逻辑，但广播 poChanged 而非 HX-Redirect）。
#[require_permission("PURCHASE_ORDER", "update")]
pub async fn update_po(
    path: PcPoUpdatePath,
    ctx: RequestContext,
    Form(form): Form<PoDrawerForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let svc = state.purchase_order_service();
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let existing = svc.get(&service_ctx, &mut tx, path.id).await?;
    if existing.status != PurchaseOrderStatus::Draft {
        return Err(DomainError::business_rule("仅草稿状态的订单可以编辑").into());
    }
    if form.supplier_id == 0 {
        return Err(DomainError::validation("请选择供应商").into());
    }
    let expected_delivery_date = form
        .expected_delivery_date
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|e| DomainError::validation(format!("无效交货日期: {e}")))
        })
        .transpose()?;
    let web_items: Vec<PoItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("无效明细数据: {e}")))?;
    if web_items.is_empty() {
        return Err(DomainError::validation("请至少保留一个明细行").into());
    }
    let items: Vec<CreateOrderItemRequest> = web_items
        .into_iter()
        .enumerate()
        .map(|(idx, it)| {
            let quantity: rust_decimal::Decimal = it
                .quantity
                .parse()
                .map_err(|_| DomainError::validation(format!("第 {} 行无效数量", idx + 1)))?;
            let unit_price: rust_decimal::Decimal = it
                .unit_price
                .parse()
                .map_err(|_| DomainError::validation(format!("第 {} 行无效单价", idx + 1)))?;
            let item_delivery = it
                .item_delivery_date
                .as_deref()
                .filter(|s| !s.is_empty())
                .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
            Ok(CreateOrderItemRequest {
                product_id: it.product_id.parse().unwrap_or(0),
                line_no: (idx as i32) + 1,
                description: it.description.unwrap_or_default(),
                quantity,
                unit_price,
                quotation_item_id: None,
                expected_delivery_date: item_delivery,
                discount_pct: it
                    .discount_pct
                    .as_deref()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(rust_decimal::Decimal::ZERO),
                tax_rate_id: it
                    .tax_rate_id
                    .as_deref()
                    .and_then(|s| s.parse().ok())
                    .filter(|&v: &i64| v > 0),
            })
        })
        .collect::<Result<Vec<_>, DomainError>>()?;
    let req = UpdatePurchaseOrderRequest {
        supplier_id: form.supplier_id,
        expected_delivery_date,
        payment_terms: form.payment_terms,
        delivery_address: form.delivery_address,
        remark: form.remark.unwrap_or_default(),
        currency_code: existing.currency_code.clone(),
        currency_rate: existing.currency_rate,
        discount_amount: existing.discount_amount,
    };
    svc.update(&service_ctx, &mut tx, path.id, req, items).await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_purchase_summary(&state);
    Ok(([("HX-Trigger", "poChanged")], Html(String::new())))
}

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn submit_po(path: PcPoSubmitPath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .purchase_order_service()
        .submit(&service_ctx, &mut tx, path.id, None)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_purchase_summary(&state);
    Ok(([("HX-Trigger", "poChanged")], Html(String::new())))
}

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn confirm_po(path: PcPoConfirmPath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .purchase_order_service()
        .confirm(&service_ctx, &mut tx, path.id, None)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_purchase_summary(&state);
    Ok(([("HX-Trigger", "poChanged")], Html(String::new())))
}

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn cancel_po(path: PcPoCancelPath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .purchase_order_service()
        .cancel(&service_ctx, &mut tx, path.id, None)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_purchase_summary(&state);
    Ok(([("HX-Trigger", "poChanged")], Html(String::new())))
}

/// PO 详情 drawer body：状态/履约进度/订单信息（Draft 可编辑）/行项目（Draft 补单价）/状态操作。
fn render_po_detail_drawer_body(
    order: &PurchaseOrder,
    items: &[PurchaseOrderItem],
    supplier_name: &str,
    operator_name: &str,
    product_codes: &HashMap<i64, String>,
    product_names: &HashMap<i64, String>,
    tax_rates: &[abt_core::purchase::tax::model::TaxRate],
) -> Markup {
    let is_draft = order.status == PurchaseOrderStatus::Draft;
    html! {
        // header：单号 + 状态 pills + 金额
        div class="flex items-center gap-2 mb-4 flex-wrap" {
            span class="font-mono text-sm text-muted" { (order.doc_number) }
            (po_status_pill(order.status))
            (invoice_status_pill(order.invoice_status))
            span class="ml-auto text-lg font-bold font-mono text-accent" { (fmt_decimal(order.total_amount)) }
        }
        @if !items.is_empty() {
            (po_drawer_progress(items))
        }
        form id="pc-po-drawer-form"
            hx-post=(PcPoUpdatePath { id: order.id }.to_string())
            hx-swap="none"
            _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #po-detail-overlay then call showToast('订单已保存')" {
            input type="hidden" id="pc-po-items-json" name="items_json" value="[]" {};
            input type="hidden" name="supplier_id" value=(order.supplier_id) {};
            (po_drawer_info_grid(order, supplier_name, operator_name, is_draft))
            (po_drawer_items_table(items, product_codes, product_names, tax_rates, is_draft))
            (po_drawer_actions(order))
            // Draft 提交时收集行项目（同 purchase_order_edit 范式）
            @if is_draft {
                (maud::PreEscaped(r#"<script>document.currentScript.parentElement.addEventListener('submit',function(ev){var errs=[];document.querySelectorAll('#pc-po-item-tbody tr').forEach(function(r,i){var q=parseFloat((r.querySelector('[name=quantity]')||{}).value)||0;var p=parseFloat((r.querySelector('[name=unit_price]')||{}).value)||0;if(q<=0)errs.push('第'+(i+1)+'行数量必须大于0');if(p<=0)errs.push('第'+(i+1)+'行单价必须大于0');});if(errs.length){alert(errs.join('\n'));ev.preventDefault();return;}var items=[];document.querySelectorAll('#pc-po-item-tbody tr').forEach(function(r){var o={};r.querySelectorAll('input,select,textarea').forEach(function(el){if(el.name&&!o[el.name])o[el.name]=el.value;});items.push(o);});document.getElementById('pc-po-items-json').value=JSON.stringify(items);});</script>"#))
            }
        }
    }
}

fn invoice_status_pill(s: InvoiceStatus) -> Markup {
    use InvoiceStatus::*;
    match s {
        NoInvoice => pill("draft", "未开票"),
        ToInvoice => pill("info", "待开票"),
        FullyInvoiced => pill("completed", "已开票"),
    }
}

/// 履约进度（已收/已检/已退/待收 统计），简化自 po_detail_page 履约进度。
fn po_drawer_progress(items: &[PurchaseOrderItem]) -> Markup {
    let total: rust_decimal::Decimal = items.iter().map(|i| i.quantity).sum();
    let received: rust_decimal::Decimal = items.iter().map(|i| i.received_qty).sum();
    let inspected: rust_decimal::Decimal = items.iter().map(|i| i.inspected_qty).sum();
    let returned: rust_decimal::Decimal = items.iter().map(|i| i.returned_qty).sum();
    let pending = total - received - returned;
    html! {
        div class="flex items-center gap-5 px-4 py-3 mb-4 bg-surface-raised border border-border-soft rounded-sm" {
            (drawer_progress_stat(received, "已收货", "text-success"))
            (drawer_progress_stat(inspected, "已检验", "text-accent"))
            (drawer_progress_stat(returned, "已退货", "text-danger"))
            (drawer_progress_stat(pending, "待收货", "text-fg"))
        }
    }
}

fn drawer_progress_stat(qty: rust_decimal::Decimal, label: &str, cls: &str) -> Markup {
    html! {
        div class="text-center" {
            div class=(format!("text-base font-bold font-mono tabular-nums {cls}")) { (fmt_plain(qty)) }
            div class="text-[11px] text-muted mt-0.5" { (label) }
        }
    }
}

fn po_drawer_info_grid(
    order: &PurchaseOrder,
    supplier_name: &str,
    operator_name: &str,
    is_draft: bool,
) -> Markup {
    let field_cls =
        "w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent";
    let ro_val = |v: String| {
        html! {
            div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg-2 font-mono" {
                (v)
            }
        }
    };
    let expected = order
        .expected_delivery_date
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    html! {
        div class="grid grid-cols-2 gap-4 mb-4" {
            div {
                label class="block text-xs text-muted font-medium mb-1.5" { "供应商" }
                (ro_val(supplier_name.to_string()))
            }
            div {
                label class="block text-xs text-muted font-medium mb-1.5" { "订单日期" }
                (ro_val(order.order_date.format("%Y-%m-%d").to_string()))
            }
            div {
                label class="block text-xs text-muted font-medium mb-1.5" { "预计交期" }
                @if is_draft {
                    input type="date" name="expected_delivery_date" class=(field_cls) value=(expected.clone()) {};
                } @else {
                    (ro_val(if expected.is_empty() { "—".into() } else { expected }))
                }
            }
            div {
                label class="block text-xs text-muted font-medium mb-1.5" { "付款条款" }
                @if is_draft {
                    input type="text" name="payment_terms" class=(field_cls)
                        value=(order.payment_terms.as_deref().unwrap_or("")) {};
                } @else {
                    (ro_val(order.payment_terms.as_deref().unwrap_or("—").to_string()))
                }
            }
            div class="col-span-2" {
                label class="block text-xs text-muted font-medium mb-1.5" { "交货地址" }
                @if is_draft {
                    input type="text" name="delivery_address" class=(field_cls)
                        value=(order.delivery_address.as_deref().unwrap_or("")) {};
                } @else {
                    (ro_val(order.delivery_address.as_deref().unwrap_or("—").to_string()))
                }
            }
            div class="col-span-2" {
                label class="block text-xs text-muted font-medium mb-1.5" { "备注" }
                @if is_draft {
                    input type="text" name="remark" class=(field_cls) value=(order.remark.as_str()) {};
                } @else {
                    (ro_val(if order.remark.is_empty() { "—".into() } else { order.remark.clone() }))
                }
            }
            div {
                label class="block text-xs text-muted font-medium mb-1.5" { "采购员" }
                (ro_val(operator_name.to_string()))
            }
        }
    }
}

fn po_drawer_items_table(
    items: &[PurchaseOrderItem],
    codes: &HashMap<i64, String>,
    names: &HashMap<i64, String>,
    tax_rates: &[abt_core::purchase::tax::model::TaxRate],
    is_draft: bool,
) -> Markup {
    html! {
        div class="overflow-x-auto mb-4 border border-border-soft rounded-sm" {
            table class="w-full text-xs" {
                thead {
                    tr class="bg-surface-raised text-muted" {
                        th class="text-left font-semibold py-2 px-2 uppercase tracking-wide" { "物料" }
                        th class="text-right font-semibold py-2 px-2 uppercase tracking-wide" { "数量" }
                        th class="text-right font-semibold py-2 px-2 uppercase tracking-wide" { "单价" }
                        th class="text-right font-semibold py-2 px-2 uppercase tracking-wide" { "折扣%" }
                        th class="text-left font-semibold py-2 px-2 uppercase tracking-wide" { "税率" }
                        th class="text-right font-semibold py-2 px-2 uppercase tracking-wide" { "小计" }
                    }
                }
                tbody id="pc-po-item-tbody" {
                    @for it in items {
                        (po_drawer_item_row(it, codes, names, tax_rates, is_draft))
                    }
                    @if items.is_empty() {
                        tr { td colspan="6" class="text-center text-muted py-4" { "暂无明细" } }
                    }
                }
            }
        }
    }
}

fn po_drawer_item_row(
    it: &PurchaseOrderItem,
    codes: &HashMap<i64, String>,
    names: &HashMap<i64, String>,
    tax_rates: &[abt_core::purchase::tax::model::TaxRate],
    is_draft: bool,
) -> Markup {
    let code = codes.get(&it.product_id).map(|s| s.as_str()).unwrap_or("—");
    let name = names.get(&it.product_id).map(|s| s.as_str()).unwrap_or("—");
    let subtotal = it.quantity * it.unit_price;
    let num_cls =
        "w-[72px] text-right px-1.5 py-1 border border-border rounded-sm font-mono text-xs bg-white text-fg outline-none focus:border-accent";
    let delivery = it
        .expected_delivery_date
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    html! {
        tr class="border-t border-border-soft" {
            td class="py-1.5 px-2 align-top" {
                div class="text-fg" { (name) }
                div class="text-muted font-mono" { (code) }
                @if is_draft {
                    input type="hidden" name="product_id" value=(it.product_id) {};
                    input type="hidden" name="description" value=(it.description.as_str()) {};
                    input type="hidden" name="item_delivery_date" value=(delivery) {};
                }
            }
            td class="py-1.5 px-2 text-right align-top" {
                @if is_draft {
                    input type="number" step="any" name="quantity" class=(num_cls)
                        value=(it.quantity.normalize().to_string()) {};
                } @else {
                    span class="font-mono" { (fmt_plain(it.quantity)) }
                }
            }
            td class="py-1.5 px-2 text-right align-top" {
                @if is_draft {
                    input type="number" step="any" name="unit_price" class=(num_cls)
                        value=(it.unit_price.to_string()) {};
                } @else {
                    span class="font-mono" { (fmt_plain(it.unit_price)) }
                }
            }
            td class="py-1.5 px-2 text-right align-top" {
                @if is_draft {
                    input type="number" step="any" name="discount_pct" class=(num_cls)
                        value=(it.discount_pct.to_string()) {};
                } @else {
                    span class="font-mono" { (fmt_plain(it.discount_pct)) }
                }
            }
            td class="py-1.5 px-2 align-top" {
                @if is_draft {
                    select name="tax_rate_id"
                        class="px-1.5 py-1 border border-border rounded-sm text-xs bg-white text-fg outline-none focus:border-accent" {
                        option value="" { "—" }
                        @for tr in tax_rates {
                            option value=(tr.id) selected[it.tax_rate_id == Some(tr.id)] { (tr.name) }
                        }
                    }
                } @else {
                    span class="text-muted" {
                        @if let Some(tr) = tax_rates.iter().find(|t| Some(t.id) == it.tax_rate_id) {
                            (tr.name)
                        } @else { "—" }
                    }
                }
            }
            td class="py-1.5 px-2 text-right align-top font-mono" { (fmt_plain(subtotal)) }
        }
    }
}

/// PO 详情 drawer 状态操作（Draft 保存/提交/确认/取消；PendingApproval 审批/退回）。
fn po_drawer_actions(order: &PurchaseOrder) -> Markup {
    let primary =
        "inline-flex items-center px-3.5 py-1.5 rounded-sm bg-accent text-white text-xs font-semibold border-none cursor-pointer hover:opacity-90";
    let ghost =
        "inline-flex items-center px-3.5 py-1.5 rounded-sm bg-white text-fg-2 border border-border text-xs font-medium cursor-pointer hover:bg-surface";
    html! {
        div class="flex items-center justify-end gap-2 pt-3 border-t border-border-soft flex-wrap" {
            @if order.status == PurchaseOrderStatus::Draft {
                button type="button" class=(ghost)
                    hx-post=(PcPoCancelPath { id: order.id }.to_string()) hx-swap="none"
                    hx-confirm="确认取消此订单？取消后不可恢复。"
                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #po-detail-overlay then call showToast('订单已取消')" {
                    "取消订单"
                }
                button type="button" class=(ghost)
                    hx-post=(PcPoConfirmPath { id: order.id }.to_string()) hx-swap="none"
                    hx-confirm="直接确认此订单？"
                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #po-detail-overlay then call showToast('订单已确认')" {
                    "直接确认"
                }
                button type="button" class=(ghost)
                    hx-post=(PcPoSubmitPath { id: order.id }.to_string()) hx-swap="none"
                    hx-confirm="提交审批？"
                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #po-detail-overlay then call showToast('已提交审批')" {
                    "提交审批"
                }
                button type="submit" class=(primary) { "保存修改" }
            } @else if order.status == PurchaseOrderStatus::PendingApproval {
                button type="button" class=(ghost)
                    hx-post=(PcOrderRejectPath { id: order.id }.to_string()) hx-swap="none"
                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #po-detail-overlay then call showToast('已退回修改')" {
                    "退回修改"
                }
                button type="button" class=(primary)
                    hx-get=(PcOrderApproveDrawerPath { id: order.id }.to_string())
                    hx-target="#approve-drawer-body" hx-swap="innerHTML"
                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #po-detail-overlay then add .open to #approve-overlay" {
                    "审批"
                }
            }
        }
    }
}

// ── 退货详情 drawer（就地查看 + Draft 确认/取消）──

#[require_permission("PURCHASE_RETURN", "read")]
pub async fn get_return_detail_drawer(
    path: PcReturnDetailDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.purchase_return_service();
    let pr = svc.get(&service_ctx, &mut conn, path.id).await?;
    let items = svc
        .list_items(&service_ctx, &mut conn, path.id)
        .await
        .unwrap_or_default();
    let supplier_name = supplier_names(&state, &service_ctx, &mut conn, &[pr.supplier_id])
        .await
        .get(&pr.supplier_id)
        .cloned()
        .unwrap_or_else(|| "未知供应商".into());
    let order_doc = state
        .purchase_order_service()
        .get(&service_ctx, &mut conn, pr.order_id)
        .await
        .map(|o| o.doc_number)
        .ok();
    let operator_name = state
        .user_service()
        .get_user(&service_ctx, &mut conn, pr.operator_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());
    let (codes, names): (HashMap<i64, String>, HashMap<i64, String>) = {
        let ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
        if ids.is_empty() {
            (HashMap::new(), HashMap::new())
        } else {
            let products = state
                .product_service()
                .get_by_ids(&service_ctx, &mut conn, ids)
                .await
                .unwrap_or_default();
            (
                products
                    .iter()
                    .map(|p| (p.product_id, p.product_code.clone()))
                    .collect(),
                products
                    .iter()
                    .map(|p| (p.product_id, p.pdt_name.clone()))
                    .collect(),
            )
        }
    };
    Ok(Html(
        render_return_detail_drawer_body(
            &pr,
            &items,
            &supplier_name,
            &order_doc,
            &operator_name,
            &codes,
            &names,
        )
        .into_string(),
    ))
}

#[require_permission("PURCHASE_RETURN", "update")]
pub async fn confirm_return(
    path: PcReturnConfirmPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .purchase_return_service()
        .confirm(&service_ctx, &mut tx, path.id, None)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_purchase_summary(&state);
    Ok(([("HX-Trigger", "returnChanged")], Html(String::new())))
}

#[require_permission("PURCHASE_RETURN", "update")]
pub async fn cancel_return(
    path: PcReturnCancelPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .purchase_return_service()
        .cancel(&service_ctx, &mut tx, path.id, None)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_purchase_summary(&state);
    Ok(([("HX-Trigger", "returnChanged")], Html(String::new())))
}

fn return_status_pill(status: PurchaseReturnStatus) -> Markup {
    use PurchaseReturnStatus::*;
    match status {
        Draft => pill("draft", "草稿"),
        Confirmed => pill("info", "已确认"),
        Shipped => pill("progress", "已发货"),
        Settled => pill("completed", "已结算"),
        Cancelled => pill("cancelled", "已取消"),
    }
}

/// 详情 drawer 通用只读字段（label + 值）。
fn drawer_field(label: &str, value: &str) -> Markup {
    html! {
        div {
            label class="block text-xs text-muted font-medium mb-1.5" { (label) }
            div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg-2 font-mono break-all" {
                (value)
            }
        }
    }
}

fn render_return_detail_drawer_body(
    pr: &PurchaseReturn,
    items: &[PurchaseReturnItem],
    supplier_name: &str,
    order_doc: &Option<String>,
    operator_name: &str,
    codes: &HashMap<i64, String>,
    names: &HashMap<i64, String>,
) -> Markup {
    html! {
        div class="flex items-center gap-2 mb-4 flex-wrap" {
            span class="font-mono text-sm text-muted" { (pr.doc_number) }
            (return_status_pill(pr.status))
            span class="ml-auto text-lg font-bold font-mono text-accent" { (fmt_decimal(pr.total_amount)) }
        }
        div class="grid grid-cols-2 gap-4 mb-4" {
            (drawer_field("供应商", supplier_name))
            (drawer_field("关联订单", order_doc.as_deref().unwrap_or("—")))
            (drawer_field("退货日期", &pr.return_date.format("%Y-%m-%d").to_string()))
            (drawer_field("操作员", operator_name))
            div class="col-span-2" { (drawer_field("退货原因", pr.return_reason.as_str())) }
            @if !pr.remark.is_empty() {
                div class="col-span-2" { (drawer_field("备注", pr.remark.as_str())) }
            }
        }
        div class="overflow-x-auto mb-4 border border-border-soft rounded-sm" {
            table class="w-full text-xs" {
                thead {
                    tr class="bg-surface-raised text-muted" {
                        th class="text-left font-semibold py-2 px-2 uppercase tracking-wide" { "物料" }
                        th class="text-right font-semibold py-2 px-2 uppercase tracking-wide" { "数量" }
                        th class="text-right font-semibold py-2 px-2 uppercase tracking-wide" { "单价" }
                        th class="text-right font-semibold py-2 px-2 uppercase tracking-wide" { "金额" }
                    }
                }
                tbody {
                    @for it in items {
                        tr class="border-t border-border-soft" {
                            td class="py-1.5 px-2" {
                                div class="text-fg" { (names.get(&it.product_id).map(|s| s.as_str()).unwrap_or("—")) }
                                div class="text-muted font-mono" { (codes.get(&it.product_id).map(|s| s.as_str()).unwrap_or("—")) }
                            }
                            td class="py-1.5 px-2 text-right font-mono" { (fmt_plain(it.returned_qty)) }
                            td class="py-1.5 px-2 text-right font-mono" { (fmt_plain(it.unit_price)) }
                            td class="py-1.5 px-2 text-right font-mono" { (fmt_plain(it.amount)) }
                        }
                    }
                    @if items.is_empty() {
                        tr { td colspan="4" class="text-center text-muted py-4" { "暂无明细" } }
                    }
                }
            }
        }
        div class="flex items-center justify-end gap-2 pt-3 border-t border-border-soft flex-wrap" {
            @if pr.status == PurchaseReturnStatus::Draft {
                button type="button"
                    class="inline-flex items-center px-3.5 py-1.5 rounded-sm bg-white text-danger border border-border text-xs font-medium cursor-pointer hover:bg-surface"
                    hx-post=(PcReturnCancelPath { id: pr.id }.to_string()) hx-swap="none"
                    hx-confirm="确认取消此退货单？取消后不可恢复。"
                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #return-detail-overlay then call showToast('退货单已取消')" {
                    "取消退货"
                }
                button type="button"
                    class="inline-flex items-center px-3.5 py-1.5 rounded-sm bg-accent text-white text-xs font-semibold border-none cursor-pointer hover:opacity-90"
                    hx-post=(PcReturnConfirmPath { id: pr.id }.to_string()) hx-swap="none"
                    hx-confirm="确认此退货单？确认后将执行退货。"
                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #return-detail-overlay then call showToast('退货单已确认')" {
                    "确认退货"
                }
            } @else if pr.status == PurchaseReturnStatus::Confirmed {
                span class="text-xs text-muted mr-auto" { "已确认，待 WMS 退货出库后自动流转为已发货" }
            }
        }
    }
}

// ── 报价详情 drawer（就地查看 + 生效/取消/转PO）──

#[require_permission("PURCHASE_QUOTATION", "read")]
pub async fn get_quotation_detail_drawer(
    path: PcQuotationDetailDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.purchase_quotation_service();
    let pq = svc.get(&service_ctx, &mut conn, path.id).await?;
    let items = svc
        .list_items(&service_ctx, &mut conn, path.id)
        .await
        .unwrap_or_default();
    let supplier_name = supplier_names(&state, &service_ctx, &mut conn, &[pq.supplier_id])
        .await
        .get(&pq.supplier_id)
        .cloned()
        .unwrap_or_else(|| "未知供应商".into());
    let operator_name = state
        .user_service()
        .get_user(&service_ctx, &mut conn, pq.operator_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());
    let (codes, names): (HashMap<i64, String>, HashMap<i64, String>) = {
        let ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
        if ids.is_empty() {
            (HashMap::new(), HashMap::new())
        } else {
            let products = state
                .product_service()
                .get_by_ids(&service_ctx, &mut conn, ids)
                .await
                .unwrap_or_default();
            (
                products
                    .iter()
                    .map(|p| (p.product_id, p.product_code.clone()))
                    .collect(),
                products
                    .iter()
                    .map(|p| (p.product_id, p.pdt_name.clone()))
                    .collect(),
            )
        }
    };
    Ok(Html(
        render_quotation_detail_drawer_body(&pq, &items, &supplier_name, &operator_name, &codes, &names)
            .into_string(),
    ))
}

#[require_permission("PURCHASE_QUOTATION", "update")]
pub async fn activate_quotation(
    path: PcQuotationActivatePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .purchase_quotation_service()
        .activate(&service_ctx, &mut tx, path.id, None)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_purchase_summary(&state);
    Ok(([("HX-Trigger", "quotationChanged")], Html(String::new())))
}

#[require_permission("PURCHASE_QUOTATION", "update")]
pub async fn cancel_quotation(
    path: PcQuotationCancelPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .purchase_quotation_service()
        .cancel(&service_ctx, &mut tx, path.id, None)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_purchase_summary(&state);
    Ok(([("HX-Trigger", "quotationChanged")], Html(String::new())))
}

/// 基于报价生成 PO（复用 PurchaseOrderService::create_from_quotation），广播 quotationChanged + poChanged。
#[require_permission("PURCHASE_QUOTATION", "update")]
pub async fn quotation_to_po(
    path: PcQuotationToPoPath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .purchase_order_service()
        .create_from_quotation(&service_ctx, &mut tx, path.id, None)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_purchase_summary(&state);
    Ok((
        [("HX-Trigger", "quotationChanged, poChanged")],
        Html(String::new()),
    ))
}

fn render_quotation_detail_drawer_body(
    pq: &PurchaseQuotation,
    items: &[PurchaseQuotationItem],
    supplier_name: &str,
    operator_name: &str,
    codes: &HashMap<i64, String>,
    names: &HashMap<i64, String>,
) -> Markup {
    let can_act = matches!(
        pq.status,
        PurchaseQuotationStatus::Draft | PurchaseQuotationStatus::Active
    );
    let currency = items
        .first()
        .map(|i| i.currency.as_str())
        .unwrap_or("CNY");
    html! {
        div class="flex items-center gap-2 mb-4 flex-wrap" {
            span class="font-mono text-sm text-muted" { (pq.doc_number) }
            (quotation_status_pill(pq.status))
            span class="ml-auto text-xs text-muted font-mono" {
                (pq.valid_from.format("%Y-%m-%d").to_string()) " → " (pq.valid_until.format("%Y-%m-%d").to_string())
            }
        }
        div class="grid grid-cols-2 gap-4 mb-4" {
            (drawer_field("供应商", supplier_name))
            (drawer_field("报价日期", &pq.quotation_date.format("%Y-%m-%d").to_string()))
            (drawer_field("操作员", operator_name))
            (drawer_field("币种", currency))
            @if !pq.remark.is_empty() {
                div class="col-span-2" { (drawer_field("备注", pq.remark.as_str())) }
            }
        }
        div class="overflow-x-auto mb-4 border border-border-soft rounded-sm" {
            table class="w-full text-xs" {
                thead {
                    tr class="bg-surface-raised text-muted" {
                        th class="text-left font-semibold py-2 px-2 uppercase tracking-wide" { "物料" }
                        th class="text-right font-semibold py-2 px-2 uppercase tracking-wide" { "单价" }
                        th class="text-right font-semibold py-2 px-2 uppercase tracking-wide" { "起订量" }
                        th class="text-right font-semibold py-2 px-2 uppercase tracking-wide" { "交期(天)" }
                        th class="text-center font-semibold py-2 px-2 uppercase tracking-wide" { "优选" }
                    }
                }
                tbody {
                    @for it in items {
                        tr class="border-t border-border-soft" {
                            td class="py-1.5 px-2" {
                                div class="text-fg" { (names.get(&it.product_id).map(|s| s.as_str()).unwrap_or("—")) }
                                div class="text-muted font-mono" { (codes.get(&it.product_id).map(|s| s.as_str()).unwrap_or("—")) }
                            }
                            td class="py-1.5 px-2 text-right font-mono" { (fmt_plain(it.unit_price)) " " (it.currency.as_str()) }
                            td class="py-1.5 px-2 text-right font-mono" {
                                @if let Some(q) = it.min_order_qty { (fmt_plain(q)) } @else { "—" }
                            }
                            td class="py-1.5 px-2 text-right font-mono" {
                                @if let Some(d) = it.lead_time_days { (d) } @else { "—" }
                            }
                            td class="py-1.5 px-2 text-center" {
                                @if it.is_preferred {
                                    (icon::check_circle_icon("w-3.5 h-3.5 text-success"))
                                } @else {
                                    span class="text-muted" { "—" }
                                }
                            }
                        }
                    }
                    @if items.is_empty() {
                        tr { td colspan="5" class="text-center text-muted py-4" { "暂无明细" } }
                    }
                }
            }
        }
        div class="flex items-center justify-end gap-2 pt-3 border-t border-border-soft flex-wrap" {
            @if can_act {
                button type="button"
                    class="inline-flex items-center px-3.5 py-1.5 rounded-sm bg-white text-danger border border-border text-xs font-medium cursor-pointer hover:bg-surface"
                    hx-post=(PcQuotationCancelPath { id: pq.id }.to_string()) hx-swap="none"
                    hx-confirm="确认取消此报价单？"
                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #quotation-detail-overlay then call showToast('报价已取消')" {
                    "取消报价"
                }
                @if pq.status == PurchaseQuotationStatus::Draft {
                    button type="button"
                        class="inline-flex items-center px-3.5 py-1.5 rounded-sm bg-white text-fg-2 border border-border text-xs font-medium cursor-pointer hover:bg-surface"
                        hx-post=(PcQuotationActivatePath { id: pq.id }.to_string()) hx-swap="none"
                        hx-confirm="生效此报价单？"
                        _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #quotation-detail-overlay then call showToast('报价已生效')" {
                        "生效"
                    }
                }
                button type="button"
                    class="inline-flex items-center px-3.5 py-1.5 rounded-sm bg-accent text-white text-xs font-semibold border-none cursor-pointer hover:opacity-90"
                    hx-post=(PcQuotationToPoPath { id: pq.id }.to_string()) hx-swap="none"
                    hx-confirm="基于此报价生成采购订单？"
                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #quotation-detail-overlay then call showToast('已生成采购订单')" {
                    "转为采购单"
                }
            }
        }
    }
}

// =============================================================================
// 写 handler（事务包裹，HX-Trigger 广播）
// =============================================================================

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn approve_order(path: PcOrderApprovePath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .purchase_order_service()
        .approve_po(&service_ctx, &mut tx, path.id, None)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_purchase_summary(&state);
    Ok(([("HX-Trigger", "poChanged")], Html(String::new())))
}

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn reject_order(path: PcOrderRejectPath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .purchase_order_service()
        .reject(&service_ctx, &mut tx, path.id, String::new(), None)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_purchase_summary(&state);
    Ok(([("HX-Trigger", "poChanged")], Html(String::new())))
}

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn confirm_recon(path: PcReconConfirmPath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .purchase_reconciliation_service()
        .confirm(&service_ctx, &mut tx, path.id, None)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_purchase_summary(&state);
    Ok(([("HX-Trigger", "reconChanged")], Html(String::new())))
}

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn approve_payment(path: PcPaymentApprovePath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .payment_request_service()
        .approve(&service_ctx, &mut tx, path.id, None)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    invalidate_purchase_summary(&state);
    Ok(([("HX-Trigger", "reconChanged")], Html(String::new())))
}

// =============================================================================
// 渲染辅助
// =============================================================================

/// tab 标题后的待办计数 badge（>0 才显示），对齐 mes_work_center::tab_badge。
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

/// 顶部业务 tab 栏（4 业务 + badge）。放进各 card 端点返回的 HTML，随刷新重渲染，
/// 高亮态 + badge 自然更新（无需额外同步）。对齐 mes_work_center::demand_filter_bar 第一行。
///
/// 切业务 tab 不携带旧 tab 的筛选参数（各业务 keyword/子 tab 语义不互通），干净切换。
fn pc_tab_bar(active: &str, s: &PurchaseWorkCenterSummary) -> Markup {
    // badge = 各业务「全部」计数（与 card 默认全部查询的数据一致；pending_* 待办计数另留给 header pill）
    let tab = |val: &str, path: &str, vals: &'static str, tab_icon: Markup, label: &str, cnt: u64| -> Markup {
        html! {
            button class=(toggle_cls(active == val)) type="button"
                hx-get=(path) hx-vals=(vals)
                hx-target="#pc-card" hx-select="#pc-card" hx-swap="outerHTML"
                { (tab_icon) (label) (tab_badge(cnt)) }
        }
    };
    html! {
        div class="flex items-center gap-1 flex-wrap px-5 pt-3 border-b border-border-soft" {
            (tab("demand-detail", PcDemandPath::PATH, r#"{"view":"detail"}"#, icon::rows_icon("w-4 h-4"), "采购明细", s.demand_detail_total))
            (tab("demand-material", PcDemandPath::PATH, r#"{"view":"material"}"#, icon::grid_4_icon("w-4 h-4"), "物料汇总", s.pending_demand))
            (tab("orders", PcOrdersPath::PATH, "{}", icon::clipboard_list_icon("w-4 h-4"), "采购订单", s.total_orders))
            (tab("settlement", PcSettlementPath::PATH, "{}", icon::payment_icon("w-4 h-4"), "对账付款", s.total_recon))
            (tab("returns", PcReturnsPath::PATH, "{}", icon::return_arrow_icon("w-4 h-4"), "采购退货", s.total_returns))
            (tab("quotation", PcQuotationPath::PATH, "{}", icon::clipboard_document_icon("w-4 h-4"), "供应商报价", s.total_quotations))
            (tab("misc", PcMiscPath::PATH, "{}", icon::clipboard_module_icon("w-4 h-4"), "零星请购", s.total_misc))
        }
    }
}

/// Drawer overlay 壳（同 mes_work_center）：背景点击/关闭按钮收起，body 由 `hx-get` 填充。
///
/// 开关：overlay 用 `.drawer-overlay` class，hyperscript toggle `.open`（preflight CSS 驱动显隐+平移，同 components/drawer.rs）。
fn render_drawer_overlay(overlay_id: &str, _drawer_id: &str, body_id: &str, title: &str, width_class: &str) -> Markup {
    drawer_shell(overlay_id, width_class, html! {
        div class="flex items-center justify-between px-6 py-5 border-b border-border-soft" {
            div class="font-bold text-base text-fg" { (title) }
            button type="button"
                class="w-8 h-8 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg flex items-center justify-center"
                _=(format!("on click remove .open from #{}", overlay_id)) {
                (icon::x_icon("w-4 h-4"))
            }
        }
        div id=(body_id) class="flex-1 overflow-y-auto px-6 py-5"
            _=(format!("on 'htmx:afterSettle' add .open to #{}", overlay_id)) {}
    })
}

/// 供应商名称映射：按 id 精确批量反查（`WHERE id = ANY($1)`），避免「拉首页 500 条内存 filter」。
async fn supplier_names(
    state: &crate::state::AppState,
    ctx: &abt_core::shared::types::context::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    ids: &[i64],
) -> HashMap<i64, String> {
    if ids.is_empty() {
        return HashMap::new();
    }
    match state.supplier_service().get_by_ids(ctx, db, ids).await {
        Ok(r) => r.into_iter().map(|s| (s.id, s.name)).collect(),
        Err(_) => HashMap::new(),
    }
}

/// summary 缓存有效期（秒）：写操作 invalidate 之外的兜底，防遗漏导致脏数据。
const SUMMARY_TTL_SECS: u64 = 30;

/// 读 summary（带缓存）：未过期直接返回（0 查询）；过期/无则并行算一次并回填。
/// 翻页/搜索/切 tab 都走这里，summary 只在首次/写操作后算。
async fn cached_summary(
    state: &crate::state::AppState,
    ctx: &abt_core::shared::types::context::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
) -> PurchaseWorkCenterSummary {
    {
        let cache = state.purchase_summary_cache.read().unwrap();
        if let Some((at, s)) = cache.as_ref() {
            if at.elapsed().as_secs() < SUMMARY_TTL_SECS {
                return s.clone();
            }
        }
    }
    // 缓存 miss：算一次（summary 内部 15 查询并行 ~30ms），回填
    let s = state
        .purchase_work_center_service()
        .summary(ctx, db)
        .await
        .unwrap_or_default();
    *state.purchase_summary_cache.write().unwrap() = Some((Instant::now(), s.clone()));
    s
}

/// 写操作 commit 后调：清缓存，下次请求重算（badge/total 及时）。
fn invalidate_purchase_summary(state: &crate::state::AppState) {
    *state.purchase_summary_cache.write().unwrap() = None;
}

fn pill(key: &str, label: &str) -> Markup {
    html! {
        span class=(format!("status-pill {} text-[11px]", crate::utils::status_color(key))) {
            (label)
        }
    }
}

fn toggle_cls(active: bool) -> &'static str {
    if active {
        "inline-flex items-center gap-1 px-3.5 py-1.5 text-sm text-accent font-semibold cursor-pointer bg-accent-bg rounded-sm border-none transition-colors"
    } else {
        "inline-flex items-center gap-1 px-3.5 py-1.5 text-sm text-muted font-medium cursor-pointer bg-transparent border-none rounded-sm hover:text-fg hover:bg-surface transition-colors"
    }
}

// ── 需求 card 渲染 ──

fn demand_filter_bar(view: &str, p: &DemandCardParams, supplier_name: Option<&str>) -> Markup {
    let kw = p.keyword.as_deref().unwrap_or("");
    let sid = p.supplier_id;
    let sid_str = sid.map(|s| s.to_string()).unwrap_or_default();
    html! {
        form class="flex items-center gap-2 flex-wrap px-5 py-3 border-b border-border-soft"
            hx-get=(PcDemandPath::PATH)
            hx-trigger="change, keyup changed delay:300ms from:.pc-demand-search"
            hx-target="#pc-card" hx-select="#pc-card" hx-swap="outerHTML"
            {
            input type="hidden" name="view" value=(view);
            @if view == "detail" {
                // 供应商搜索控件（搜全部供应商；store_id=true 存 id，选中 trigger change → 本表单 hx-get 刷新）
                (crate::components::supplier_search::supplier_search_field(
                    "pc-supplier-input", "pc-supplier-display", "pc-supplier-panel", "pc-supplier-results",
                    "supplier_id", &sid_str, supplier_name, "供应商",
                    true, "w-52 min-w-[208px]", None,
                ))
            }
            div class="relative" {
                (icon::search_icon("w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"));
                input class="pc-demand-search w-[180px] pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                    type="text" name="keyword" placeholder="搜索物料/订单"
                    value=(kw);
            }
        }
        form id="pc-filter-form" class="hidden" {
            input type="hidden" name="keyword" value=(kw);
            input type="hidden" name="view" value=(view);
            input type="hidden" name="supplier_id" value=(sid_str);
        }
    }
}

fn demand_material_table(result: &abt_core::shared::types::PaginatedResult<MaterialAggSummary>) -> Markup {
    html! {
        div class="p-5" {
            @if result.items.is_empty() {
                div class="text-center text-muted py-8" { "暂无待处理需求" }
            }
            @for item in &result.items {
                (demand_mat_row(item))
            }
        }
    }
}

/// 需求物料卡片行：物料图标 + 名称 + 总需求量（紧急度配色）+ 来源/交期/紧急度提示 + 转采购单 + 点击懒加载明细。
fn demand_mat_row(item: &MaterialAggSummary) -> Markup {
    let pid = item.product_id;
    let (icon_cls, mat_icon) = material_icon(pid);
    let qty_cls = demand_qty_class(item.total_demand_qty, item.earliest_required_date);
    let hint = urgency_hint(item.earliest_required_date);
    let earliest_str = item
        .earliest_required_date
        .map(|d| d.format("%m-%d").to_string())
        .unwrap_or_else(|| "—".into());
    let latest_str = item
        .latest_required_date
        .map(|d| d.format("%m-%d").to_string())
        .unwrap_or_else(|| "—".into());
    let rows_url = format!("{}?product_id={}", PcDemandRowsPath::PATH, pid);
    html! {
        div class="grid grid-cols-[1fr_auto_auto_auto] items-center gap-6 px-4 py-3 border-b border-border-soft" {
            // 物料信息（点击懒加载该物料需求明细）
            div class="flex items-center gap-4 cursor-pointer"
                hx-get=(rows_url)
                hx-target=(format!("#pc-demand-expand-{pid}"))
                hx-swap="innerHTML"
                hx-trigger="click once"
                _=(format!("on click toggle .expanded on #pc-demand-toggle-{pid}")) {
                div class=(format!("w-[40px] h-[40px] rounded-md grid place-items-center shrink-0 {icon_cls}")) { (mat_icon) }
                div {
                    div class="font-semibold text-fg text-sm" { (item.product_name) }
                    div class="text-xs text-muted font-mono" { (item.product_code) }
                }
            }
            // 总需求量（紧急度配色）
            div class="flex flex-col" {
                div class=(format!("text-lg font-bold font-mono tabular-nums {qty_cls}")) { (fmt_plain(item.total_demand_qty)) }
                div class="text-xs text-muted mt-0.5" { "总需求量" }
            }
            // 来源数 + 交期 + 紧急度提示
            div class="flex flex-col" {
                div class="text-sm font-semibold text-fg" {
                    (item.demand_count) " 个来源 · " (earliest_str) " → " (latest_str)
                }
                @if let Some((hint_text, hint_cls)) = &hint {
                    div class=(format!("text-xs font-medium {hint_cls}")) { (hint_text) }
                }
            }
            // 转采购单（就地 drawer）
            div {
                button class="inline-flex items-center px-3 py-1.5 rounded-sm bg-accent text-white text-xs font-semibold border-none cursor-pointer hover:opacity-90"
                    hx-get=(PcConvertPoDrawerPath { product_id: pid }.to_string())
                    hx-target="#convert-po-drawer-body" hx-swap="innerHTML"
                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #convert-po-overlay" { "转采购单" }
            }
        }
        // 展开区（懒加载 demand-rows，.expanded 切换显隐）
        div class="mat-expand bg-surface-raised border-b border-border-soft" id=(format!("pc-demand-toggle-{pid}")) {
            div class="p-4" {
                table class="w-full text-sm" {
                    tbody id=(format!("pc-demand-expand-{pid}")) {
                        tr { td colspan="3" class="text-center text-muted py-4" { "点击物料加载需求明细…" } }
                    }
                }
            }
        }
    }
}

/// 总需求量着色：交期紧急度优先（≤3天 danger / ≤7天 warn），其次量大（>100 warn），默认 accent。
fn demand_qty_class(total: rust_decimal::Decimal, earliest: Option<chrono::NaiveDate>) -> &'static str {
    if let Some(d) = earliest {
        let today = chrono::Local::now().date_naive();
        let diff = (d - today).num_days();
        if diff <= 3 {
            return "text-danger";
        }
        if diff <= 7 {
            return "text-warn";
        }
    }
    if total > rust_decimal::Decimal::from(100) {
        return "text-warn";
    }
    "text-accent"
}

fn demand_detail_table(
    result: &abt_core::shared::types::PaginatedResult<DemandSummary>,
    supplier_id: Option<i64>,
) -> Markup {
    if let Some(sid) = supplier_id {
        // 供应商已选：checkbox 多选模式（.pc-batch-scope 限定批量栏计数作用域）。
        // 同一供应商的多个物料需求可勾选 → 批量栏「转采购单」生成同一供应商一张 PO。
        html! {
            div class="pc-batch-scope overflow-x-auto" {
                table class="w-full text-sm" {
                    thead {
                        tr class="bg-surface-raised text-xs text-muted" {
                            th class="w-10 py-2 px-2" {
                                input type="checkbox" title="全选"
                                    _="on change call pcToggleAllDemands(me, closest <table/>)";
                            }
                            th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "物料" }
                            th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "来源订单" }
                            th class="text-right font-semibold py-2 px-3 uppercase tracking-wide" { "数量" }
                            th class="text-left font-semibold py-2 px-5 uppercase tracking-wide" { "需求日期" }
                        }
                    }
                    tbody {
                        @if result.items.is_empty() {
                            tr { td colspan="5" class="text-center text-muted py-8" { "该供应商暂无可供的待采购需求" } }
                        }
                        @for item in &result.items {
                            tr class="border-b border-border-soft hover:bg-accent-bg" {
                                td class="w-10 py-2.5 px-2 text-center" {
                                    input type="checkbox" class="pc-demand-cb" value=(item.id);
                                }
                                td class="py-2.5 px-3" {
                                    div class="font-medium text-fg" { (item.product_name) }
                                    div class="text-xs text-muted font-mono" { (item.product_code) }
                                }
                                td class="py-2.5 px-3 font-mono text-accent" { (item.order_no.as_deref().unwrap_or("—")) }
                                td class="text-right font-mono py-2.5 px-3" { (fmt_plain(item.quantity)) }
                                td class="py-2.5 px-5 text-muted" { (fmt_date(item.required_date)) }
                            }
                        }
                    }
                }
            }
            (pc_batch_bar(sid))
        }
    } else {
        // 未选供应商：只读需求列表（保持现状，「转采购单」走 demand-pool）
        html! {
            div class="overflow-x-auto" {
                table class="w-full text-sm" {
                    thead {
                        tr class="bg-surface-raised text-xs text-muted" {
                            th class="text-left font-semibold py-2 px-5 uppercase tracking-wide" { "物料" }
                            th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "来源订单" }
                            th class="text-right font-semibold py-2 px-3 uppercase tracking-wide" { "数量" }
                            th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "需求日期" }
                            th class="text-right font-semibold py-2 px-5 uppercase tracking-wide" { "操作" }
                        }
                    }
                    tbody {
                        @if result.items.is_empty() {
                            tr { td colspan="5" class="text-center text-muted py-8" { "暂无待处理需求" } }
                        }
                        @for item in &result.items {
                            tr class="border-b border-border-soft hover:bg-accent-bg" {
                                td class="py-2.5 px-5" {
                                    div class="font-medium text-fg" { (item.product_name) }
                                    div class="text-xs text-muted font-mono" { (item.product_code) }
                                }
                                td class="py-2.5 px-3 font-mono text-accent" { (item.order_no.as_deref().unwrap_or("—")) }
                                td class="text-right font-mono py-2.5 px-3" { (fmt_decimal(item.quantity)) }
                                td class="py-2.5 px-3 text-muted" { (fmt_date(item.required_date)) }
                                td class="text-right py-2.5 px-5" {
                                    button class="inline-flex items-center px-3 py-1 rounded-sm bg-accent text-white text-xs font-semibold border-none cursor-pointer hover:opacity-90"
                                        hx-get=(PcConvertPoDrawerPath { product_id: item.product_id }.to_string())
                                        hx-target="#convert-po-drawer-body" hx-swap="innerHTML"
                                        _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #convert-po-overlay" { "转采购单" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 采购明细批量栏（.pc-batch-bar，对齐 MES detail_batch_bar 范式）。勾选 .pc-demand-cb
/// 后由 app.js（pcUpdateBatchBar）显示，并拼接批量转单 drawer URL（同供应商多物料 → 一张 PO）。
/// 与 MES .demand-cb / .batch-bar 隔离（MES 强制单物料，采购允许同供应商多物料）。
fn pc_batch_bar(supplier_id: i64) -> Markup {
    html! {
        div class="pc-batch-bar hidden show:flex items-center gap-4 fixed bottom-4 left-1/2 -translate-x-1/2 z-50 px-5 py-3 rounded-md bg-fg text-white text-sm shadow-lg"
            data-supplier-id=(supplier_id) {
            span {
                "已选 "
                span class="pc-batch-count inline-block px-2 rounded-full bg-white/15 font-mono font-bold" { "0" }
                " 条需求 · 可转采购单"
            }
            a class="pc-batch-create-btn ml-auto inline-flex items-center gap-2 py-[5px] px-3 text-[13px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover font-medium cursor-pointer transition-all duration-150 no-underline"
                hx-target="#convert-po-drawer-body" hx-swap="innerHTML"
                _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #convert-po-overlay" { "转采购单" }
            button class="pc-batch-clear-btn inline-flex items-center gap-2 py-[5px] px-3 text-[13px] rounded-sm border border-[rgba(255,255,255,0.15)] text-[rgba(255,255,255,0.7)] hover:text-white hover:bg-[rgba(255,255,255,0.1)] bg-transparent font-medium cursor-pointer transition-all duration-150"
                type="button" { "清除选择" }
        }
    }
}

// ── 订单 card 渲染 ──

fn orders_filter_bar(status: Option<i16>, p: &OrdersCardParams) -> Markup {
    let kw = p.keyword.as_deref().unwrap_or("");
    html! {
        form class="flex items-center gap-2 flex-wrap px-5 py-3 border-b border-border-soft"
            hx-get=(PcOrdersPath::PATH)
            hx-trigger="change, keyup changed delay:300ms from:.pc-orders-search"
            hx-target="#pc-card" hx-select="#pc-card" hx-swap="outerHTML"
            {
            select class="px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg cursor-pointer"
                name="status" {
                option value="" selected[status.is_none()] { "全部状态" }
                option value="1" selected[status == Some(1)] { "草稿" }
                option value="2" selected[status == Some(2)] { "已确认" }
                option value="3" selected[status == Some(3)] { "部分收货" }
                option value="4" selected[status == Some(4)] { "已收货" }
                option value="5" selected[status == Some(5)] { "已关闭" }
                option value="6" selected[status == Some(6)] { "已取消" }
            }
            div class="relative" {
                (icon::search_icon("w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"));
                input class="pc-orders-search w-[180px] pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                    type="text" name="keyword" placeholder="搜索 PO 号"
                    value=(kw);
            }
        }
        form id="pc-filter-form" class="hidden" {
            input type="hidden" name="keyword" value=(kw);
            input type="hidden" name="status" value=(status.map(|s| s.to_string()).unwrap_or_default());
        }
    }
}

fn orders_table(items: &[PurchaseOrder], names: &HashMap<i64, String>) -> Markup {
    html! {
        div class="overflow-x-auto" {
            table class="w-full text-sm" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="text-left font-semibold py-2 px-5 uppercase tracking-wide" { "订单号" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "供应商" }
                        th class="text-right font-semibold py-2 px-3 uppercase tracking-wide" { "金额" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "交期" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "状态" }
                        th class="text-right font-semibold py-2 px-5 uppercase tracking-wide" { "操作" }
                    }
                }
                tbody {
                    @if items.is_empty() {
                        tr { td colspan="6" class="text-center text-muted py-8" { "暂无待处理订单" } }
                    }
                    @for o in items {
                        tr class="border-b border-border-soft hover:bg-accent-bg" {
                            td class="py-2.5 px-5 font-mono text-accent font-medium" { (o.doc_number) }
                            td class="py-2.5 px-3 text-fg" { (names.get(&o.supplier_id).map(|s| s.as_str()).unwrap_or("—")) }
                            td class="text-right font-mono py-2.5 px-3" { (fmt_decimal(o.total_amount)) }
                            td class="py-2.5 px-3 text-muted" { (fmt_date(o.expected_delivery_date)) }
                            td class="py-2.5 px-3" { (po_status_pill(o.status)) }
                            td class="text-right py-2.5 px-5 whitespace-nowrap" {
                                button class="inline-flex items-center px-3 py-1 rounded-sm bg-white text-fg-2 border border-border text-xs font-medium cursor-pointer hover:bg-surface mr-1"
                                    hx-get=(PcPoDetailDrawerPath { id: o.id }.to_string())
                                    hx-target="#po-detail-drawer-body" hx-swap="innerHTML"
                                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #po-detail-overlay" { "详情" }
                                @if o.status == PurchaseOrderStatus::PendingApproval {
                                    button class="inline-flex items-center px-3 py-1 rounded-sm bg-accent text-white text-xs font-semibold border-none cursor-pointer hover:opacity-90 mr-1"
                                        hx-get=(PcOrderApproveDrawerPath { id: o.id }.to_string())
                                        hx-target="#approve-drawer-body" hx-swap="innerHTML"
                                        _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #approve-overlay" { "审批" }
                                }
                                button class="expand-btn inline-flex items-center justify-center w-[26px] h-[26px] ml-1 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg align-middle transition-all"
                                    title="展开详情"
                                    hx-get=(PcOrderRowDetailPath { id: o.id }.to_string())
                                    hx-target="this" hx-swap="afterend"
                                    _="on click toggle .open on closest <tr/>" {
                                    (icon::chevron_right_icon("w-[15px] h-[15px]"))
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn po_status_pill(status: PurchaseOrderStatus) -> Markup {
    use PurchaseOrderStatus::*;
    match status {
        PendingApproval => pill("progress", "待审批"),
        Confirmed => pill("info", "待收货"),
        PartiallyReceived => pill("partial", "部分收货"),
        Received => pill("completed", "已收货"),
        Closed => pill("completed", "已关闭"),
        Cancelled => pill("cancelled", "已取消"),
        Draft => pill("draft", "草稿"),
    }
}

// ── 对账付款 card 渲染 ──

fn settlement_filter_bar(tab: &str, status: Option<i16>, p: &SettlementCardParams) -> Markup {
    let kw = p.keyword.as_deref().unwrap_or("");
    html! {
        form class="flex items-center gap-2 flex-wrap px-5 py-3 border-b border-border-soft"
            hx-get=(PcSettlementPath::PATH)
            hx-trigger="change, keyup changed delay:300ms from:.pc-settlement-search"
            hx-target="#pc-card" hx-select="#pc-card" hx-swap="outerHTML"
            {
            select class="px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg cursor-pointer"
                name="tab" {
                option value="recon" selected[tab == "recon"] { "对账单" }
                option value="payment" selected[tab == "payment"] { "付款" }
            }
            select class="px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg cursor-pointer"
                name="status" {
                option value="" selected[status.is_none()] { "全部状态" }
                @if tab == "payment" {
                    option value="1" selected[status == Some(1)] { "草稿" }
                    option value="2" selected[status == Some(2)] { "已核准" }
                    option value="3" selected[status == Some(3)] { "已付款" }
                    option value="4" selected[status == Some(4)] { "已取消" }
                } @else {
                    option value="1" selected[status == Some(1)] { "草稿" }
                    option value="2" selected[status == Some(2)] { "已确认" }
                    option value="3" selected[status == Some(3)] { "已结算" }
                }
            }
            div class="relative" {
                (icon::search_icon("w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"));
                input class="pc-settlement-search w-[180px] pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                    type="text" name="keyword" placeholder="搜索供应商/单号"
                    value=(kw);
            }
        }
        form id="pc-filter-form" class="hidden" {
            input type="hidden" name="keyword" value=(kw);
            input type="hidden" name="tab" value=(tab);
            input type="hidden" name="status" value=(status.map(|s| s.to_string()).unwrap_or_default());
        }
    }
}

fn recon_table(items: &[PurchaseReconciliation], names: &HashMap<i64, String>) -> Markup {
    html! {
        div class="overflow-x-auto" {
            table class="w-full text-sm" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="text-left font-semibold py-2 px-5 uppercase tracking-wide" { "对账单号" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "供应商" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "期间" }
                        th class="text-right font-semibold py-2 px-3 uppercase tracking-wide" { "金额" }
                        th class="text-right font-semibold py-2 px-5 uppercase tracking-wide" { "操作" }
                    }
                }
                tbody {
                    @if items.is_empty() {
                        tr { td colspan="5" class="text-center text-muted py-8" { "暂无草稿对账单" } }
                    }
                    @for r in items {
                        tr class="border-b border-border-soft hover:bg-accent-bg" {
                            td class="py-2.5 px-5 font-mono text-accent font-medium" { (r.doc_number) }
                            td class="py-2.5 px-3 text-fg" { (names.get(&r.supplier_id).map(|s| s.as_str()).unwrap_or("—")) }
                            td class="py-2.5 px-3 text-muted font-mono" { (r.period) }
                            td class="text-right font-mono py-2.5 px-3" { (fmt_decimal(r.total_amount)) }
                            td class="text-right py-2.5 px-5 whitespace-nowrap" {
                                button class="inline-flex items-center px-3 py-1 rounded-sm bg-accent text-white text-xs font-semibold border-none cursor-pointer hover:opacity-90 mr-1"
                                    hx-post=(PcReconConfirmPath { id: r.id }.to_string())
                                    hx-target="this" hx-swap="none"
                                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] call showToast('对账单已确认')"
                                    { "确认对账" }
                                button class="expand-btn inline-flex items-center justify-center w-[26px] h-[26px] ml-1 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg align-middle transition-all"
                                    title="展开详情"
                                    hx-get=(PcSettlementRowDetailPath { recon_type: "draft".into(), ref_id: r.id }.to_string())
                                    hx-target="this" hx-swap="afterend"
                                    _="on click toggle .open on closest <tr/>" {
                                    (icon::chevron_right_icon("w-[15px] h-[15px]"))
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn payment_table(items: &[PaymentRequest], names: &HashMap<i64, String>) -> Markup {
    html! {
        div class="overflow-x-auto" {
            table class="w-full text-sm" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="text-left font-semibold py-2 px-5 uppercase tracking-wide" { "付款单号" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "供应商" }
                        th class="text-right font-semibold py-2 px-3 uppercase tracking-wide" { "金额" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "付款日" }
                        th class="text-right font-semibold py-2 px-5 uppercase tracking-wide" { "操作" }
                    }
                }
                tbody {
                    @if items.is_empty() {
                        tr { td colspan="5" class="text-center text-muted py-8" { "暂无待审批付款" } }
                    }
                    @for pay in items {
                        tr class="border-b border-border-soft hover:bg-accent-bg" {
                            td class="py-2.5 px-5 font-mono text-accent font-medium" { (pay.doc_number) }
                            td class="py-2.5 px-3 text-fg" { (names.get(&pay.supplier_id).map(|s| s.as_str()).unwrap_or("—")) }
                            td class="text-right font-mono py-2.5 px-3" { (fmt_decimal(pay.amount)) }
                            td class="py-2.5 px-3 text-muted font-mono" { (fmt_date(Some(pay.payment_date))) }
                            td class="text-right py-2.5 px-5 whitespace-nowrap" {
                                button class="inline-flex items-center px-3 py-1 rounded-sm bg-accent text-white text-xs font-semibold border-none cursor-pointer hover:opacity-90 mr-1"
                                    hx-get=(PcPaymentApproveDrawerPath { id: pay.id }.to_string())
                                    hx-target="#pay-drawer-body" hx-swap="innerHTML"
                                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #pay-overlay" { "审批付款" }
                                button class="expand-btn inline-flex items-center justify-center w-[26px] h-[26px] ml-1 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg align-middle transition-all"
                                    title="展开详情"
                                    hx-get=(PcSettlementRowDetailPath { recon_type: "payment".into(), ref_id: pay.id }.to_string())
                                    hx-target="this" hx-swap="afterend"
                                    _="on click toggle .open on closest <tr/>" {
                                    (icon::chevron_right_icon("w-[15px] h-[15px]"))
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── 退货 card 渲染 ──

fn returns_filter_bar(status: Option<i16>, p: &ReturnsCardParams) -> Markup {
    let kw = p.keyword.as_deref().unwrap_or("");
    html! {
        form class="flex items-center gap-2 flex-wrap px-5 py-3 border-b border-border-soft"
            hx-get=(PcReturnsPath::PATH)
            hx-trigger="change, keyup changed delay:300ms from:.pc-returns-search"
            hx-target="#pc-card" hx-select="#pc-card" hx-swap="outerHTML"
            {
            select class="px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg cursor-pointer"
                name="status" {
                option value="" selected[status.is_none()] { "全部状态" }
                option value="1" selected[status == Some(1)] { "草稿" }
                option value="2" selected[status == Some(2)] { "已确认" }
                option value="3" selected[status == Some(3)] { "已发货" }
                option value="4" selected[status == Some(4)] { "已结算" }
                option value="5" selected[status == Some(5)] { "已取消" }
            }
            div class="relative" {
                (icon::search_icon("w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"));
                input class="pc-returns-search w-[180px] pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                    type="text" name="keyword" placeholder="搜索退货单号"
                    value=(kw);
            }
        }
        form id="pc-filter-form" class="hidden" {
            input type="hidden" name="keyword" value=(kw);
            input type="hidden" name="status" value=(status.map(|s| s.to_string()).unwrap_or_default());
        }
    }
}

fn returns_table(items: &[PurchaseReturn], names: &HashMap<i64, String>) -> Markup {
    html! {
        div class="overflow-x-auto" {
            table class="w-full text-sm" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="text-left font-semibold py-2 px-5 uppercase tracking-wide" { "退货单号" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "供应商" }
                        th class="text-right font-semibold py-2 px-3 uppercase tracking-wide" { "金额" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "原因" }
                        th class="text-right font-semibold py-2 px-5 uppercase tracking-wide" { "操作" }
                    }
                }
                tbody {
                    @if items.is_empty() {
                        tr { td colspan="5" class="text-center text-muted py-8" { "暂无退货" } }
                    }
                    @for r in items {
                        tr class="border-b border-border-soft hover:bg-accent-bg" {
                            td class="py-2.5 px-5 font-mono text-accent font-medium" { (r.doc_number) }
                            td class="py-2.5 px-3 text-fg" { (names.get(&r.supplier_id).map(|s| s.as_str()).unwrap_or("—")) }
                            td class="text-right font-mono py-2.5 px-3" { (fmt_decimal(r.total_amount)) }
                            td class="py-2.5 px-3 text-muted" { @if r.return_reason.is_empty() { "—" } @else { (r.return_reason.as_str()) } }
                            td class="text-right py-2.5 px-5 whitespace-nowrap" {
                                button class="inline-flex items-center px-3 py-1 rounded-sm bg-white text-fg-2 border border-border text-xs font-medium cursor-pointer hover:bg-surface mr-1"
                                    hx-get=(PcReturnDetailDrawerPath { id: r.id }.to_string())
                                    hx-target="#return-detail-drawer-body" hx-swap="innerHTML"
                                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #return-detail-overlay" { "详情" }
                                button class="expand-btn inline-flex items-center justify-center w-[26px] h-[26px] ml-1 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg align-middle transition-all"
                                    title="展开详情"
                                    hx-get=(PcReturnRowDetailPath { id: r.id }.to_string())
                                    hx-target="this" hx-swap="afterend"
                                    _="on click toggle .open on closest <tr/>" {
                                    (icon::chevron_right_icon("w-[15px] h-[15px]"))
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── 供应商报价 card 渲染 ──

fn quotation_filter_bar(status: Option<i16>) -> Markup {
    html! {
        form class="flex items-center gap-2 flex-wrap px-5 py-3 border-b border-border-soft"
            hx-get=(PcQuotationPath::PATH)
            hx-trigger="change"
            hx-target="#pc-card" hx-select="#pc-card" hx-swap="outerHTML"
            {
            select class="px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg cursor-pointer"
                name="status" {
                option value="" selected[status.is_none()] { "全部状态" }
                option value="1" selected[status == Some(1)] { "草稿" }
                option value="2" selected[status == Some(2)] { "已生效" }
                option value="3" selected[status == Some(3)] { "已过期" }
                option value="4" selected[status == Some(4)] { "已取消" }
            }
        }
        form id="pc-filter-form" class="hidden" {
            input type="hidden" name="status" value=(status.map(|s| s.to_string()).unwrap_or_default());
        }
    }
}

fn quotation_table(items: &[PurchaseQuotation], names: &HashMap<i64, String>) -> Markup {
    html! {
        div class="overflow-x-auto" {
            table class="w-full text-sm" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="text-left font-semibold py-2 px-5 uppercase tracking-wide" { "报价单号" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "供应商" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "有效期" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "状态" }
                        th class="text-right font-semibold py-2 px-5 uppercase tracking-wide" { "操作" }
                    }
                }
                tbody {
                    @if items.is_empty() {
                        tr { td colspan="5" class="text-center text-muted py-8" { "暂无报价" } }
                    }
                    @for q in items {
                        tr class="border-b border-border-soft hover:bg-accent-bg" {
                            td class="py-2.5 px-5 font-mono text-accent font-medium" { (q.doc_number) }
                            td class="py-2.5 px-3 text-fg" { (names.get(&q.supplier_id).map(|s| s.as_str()).unwrap_or("—")) }
                            td class="py-2.5 px-3 text-muted font-mono" { (fmt_date(Some(q.valid_from))) " → " (fmt_date(Some(q.valid_until))) }
                            td class="py-2.5 px-3" { (quotation_status_pill(q.status)) }
                            td class="text-right py-2.5 px-5" {
                                button class="inline-flex items-center px-3 py-1 rounded-sm bg-white text-fg-2 border border-border text-xs font-medium cursor-pointer hover:bg-surface"
                                    hx-get=(PcQuotationDetailDrawerPath { id: q.id }.to_string())
                                    hx-target="#quotation-detail-drawer-body" hx-swap="innerHTML"
                                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #quotation-detail-overlay" { "详情" }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn quotation_status_pill(status: PurchaseQuotationStatus) -> Markup {
    use PurchaseQuotationStatus::*;
    match status {
        Draft => pill("draft", "草稿"),
        Active => pill("completed", "已生效"),
        Expired => pill("cancelled", "已过期"),
        Cancelled => pill("cancelled", "已取消"),
    }
}

// ── 零星请购 card 渲染 ──

fn misc_filter_bar(status: Option<i16>) -> Markup {
    html! {
        form class="flex items-center gap-2 flex-wrap px-5 py-3 border-b border-border-soft"
            hx-get=(PcMiscPath::PATH)
            hx-trigger="change"
            hx-target="#pc-card" hx-select="#pc-card" hx-swap="outerHTML"
            {
            select class="px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg cursor-pointer"
                name="status" {
                option value="" selected[status.is_none()] { "全部状态" }
                option value="1" selected[status == Some(1)] { "草稿" }
                option value="2" selected[status == Some(2)] { "已核准" }
                option value="3" selected[status == Some(3)] { "采购中" }
                option value="4" selected[status == Some(4)] { "已到货" }
                option value="5" selected[status == Some(5)] { "已关闭" }
                option value="6" selected[status == Some(6)] { "已取消" }
            }
        }
        form id="pc-filter-form" class="hidden" {
            input type="hidden" name="status" value=(status.map(|s| s.to_string()).unwrap_or_default());
        }
    }
}

fn misc_table(items: &[MiscellaneousRequest]) -> Markup {
    html! {
        div class="overflow-x-auto" {
            table class="w-full text-sm" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="text-left font-semibold py-2 px-5 uppercase tracking-wide" { "请购单号" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "用途" }
                        th class="text-right font-semibold py-2 px-3 uppercase tracking-wide" { "金额" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide" { "状态" }
                        th class="text-right font-semibold py-2 px-5 uppercase tracking-wide" { "日期" }
                    }
                }
                tbody {
                    @if items.is_empty() {
                        tr { td colspan="5" class="text-center text-muted py-8" { "暂无请购" } }
                    }
                    @for m in items {
                        tr class="border-b border-border-soft hover:bg-accent-bg" {
                            td class="py-2.5 px-5 font-mono text-accent font-medium" { (m.doc_number) }
                            td class="py-2.5 px-3 text-fg" { (m.purpose) }
                            td class="text-right font-mono py-2.5 px-3" { (fmt_decimal(m.total_amount)) }
                            td class="py-2.5 px-3" { (misc_status_pill(m.status)) }
                            td class="py-2.5 px-5 text-muted" { (fmt_date(Some(m.request_date))) }
                        }
                    }
                }
            }
        }
    }
}

fn misc_status_pill(status: MiscRequestStatus) -> Markup {
    use MiscRequestStatus::*;
    match status {
        Draft => pill("draft", "草稿"),
        Approved => pill("info", "已核准"),
        Purchasing => pill("progress", "采购中"),
        Received => pill("partial", "已到货"),
        Closed => pill("completed", "已关闭"),
        Cancelled => pill("cancelled", "已取消"),
    }
}

// ── Drawer body ──

fn field_readonly(label: &str, value: &str) -> Markup {
    html! {
        div class="mb-4" {
            label class="block text-xs text-muted font-medium mb-1.5" { (label) }
            div class="px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm text-fg-2 font-mono" { (value) }
        }
    }
}

fn render_order_approve_body(o: &PurchaseOrder, supplier: &str) -> Markup {
    html! {
        div class="grid grid-cols-2 gap-4" {
            (field_readonly("供应商", supplier));
            (field_readonly("订单日期", &fmt_date(Some(o.order_date))));
        }
        div class="grid grid-cols-2 gap-4 mt-2" {
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "期望到货" }
                div class=(format!("px-2.5 py-1.5 bg-surface border border-border-soft rounded-sm text-sm font-mono {}", if is_overdue_or_soon(o.expected_delivery_date) { "text-warn" } else { "text-fg-2" })) {
                    (fmt_date(o.expected_delivery_date))
                }
            }
            (field_readonly("订单金额", &fmt_decimal(o.total_amount)));
        }
        div class="p-3 mt-3 bg-surface-raised border border-border-soft rounded-sm text-xs text-muted leading-relaxed" {
            "审批通过后订单转 Confirmed，进入「待收货」，可继续登记收货（触发来料通知 → PO 回写 + 立应付台账）；驳回则退回草稿。"
        }
        form class="mt-5"
            hx-post=(PcOrderApprovePath { id: o.id }.to_string())
            hx-target="this" hx-swap="none"
            _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #approve-overlay then call showToast('审批通过')" {
            div class="mb-4" {
                label class="block text-xs text-muted font-medium mb-1.5" { "审批意见（可选）" }
                textarea class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent resize-y"
                    name="remark" rows="3" placeholder="填写审批意见" {}
            }
            div class="flex justify-between gap-2 pt-3 border-t border-border-soft" {
                button type="button" class="inline-flex items-center px-4 py-2 rounded-sm bg-white text-danger border border-border text-sm font-medium cursor-pointer hover:bg-surface"
                    hx-post=(PcOrderRejectPath { id: o.id }.to_string())
                    hx-swap="none"
                    _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #approve-overlay then call showToast('已驳回')" {
                    "驳回"
                }
                button type="submit" class="inline-flex items-center px-4 py-2 rounded-sm bg-accent text-white text-sm font-semibold border-none cursor-pointer hover:opacity-90" {
                    "通过审批"
                }
            }
        }
    }
}

fn render_payment_approve_body(pay: &PaymentRequest, supplier: &str, three_way: &ThreeWayMatchSummary) -> Markup {
    html! {
        div class="grid grid-cols-2 gap-4" {
            (field_readonly("供应商", supplier));
            (field_readonly("付款单号", &pay.doc_number));
        }
        div class="grid grid-cols-2 gap-4 mt-2" {
            (field_readonly("付款金额", &fmt_decimal(pay.amount)));
            (field_readonly("付款日", &fmt_date(Some(pay.payment_date))));
        }
        @if let Some(inv) = &pay.invoice_number {
            div class="mt-2" { (field_readonly("发票号", inv)) }
        }
        // 三单匹配校验 ci-row（PO / 入库 / 发票 + 可付款 badge）
        div class="mt-4" {
            label class="block text-xs text-muted font-medium mb-1.5" { "三单匹配校验" }
            div class="flex items-center gap-3 flex-wrap p-2.5 bg-surface-raised border border-border-soft rounded-sm" {
                (ci_item(three_way.po_matched, "PO 已匹配"))
                (ci_item(three_way.receipt_matched, "入库已匹配"))
                (ci_item(three_way.invoice_matched, "发票已匹配"))
                @if three_way.can_pay {
                    span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-success-bg text-success text-[11px] font-semibold ml-auto" {
                        (icon::check_circle_icon("w-3 h-3")) "可付款"
                    }
                } @else {
                    span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-danger-bg text-danger text-[11px] font-semibold ml-auto" {
                        (icon::alert_triangle_icon("w-3 h-3")) "不可付款"
                    }
                }
            }
            @if !three_way.differences.is_empty() {
                div class="mt-2 text-xs text-danger leading-relaxed" {
                    @for diff in &three_way.differences {
                        (diff) br;
                    }
                }
            }
        }
        div class="p-3 mt-3 bg-surface-raised border border-border-soft rounded-sm text-xs text-muted leading-relaxed" {
            "审批通过后付款申请转 Approved，等待 FMS 实际付款；FMS 付款成功后自动回写 Paid 并结算应付台账。"
        }
        div class="flex justify-end gap-2 mt-5 pt-3 border-t border-border-soft" {
            form hx-post=(PcPaymentApprovePath { id: pay.id }.to_string()) hx-swap="none"
                _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #pay-overlay then call showToast('付款已审批')" {
                button type="submit" class="inline-flex items-center px-4 py-2 rounded-sm bg-accent text-white text-sm font-semibold border-none cursor-pointer hover:opacity-90" {
                    "通过付款"
                }
            }
        }
    }
}

// ── 格式化辅助 ──

fn fmt_decimal(d: rust_decimal::Decimal) -> String {
    let f: f64 = d.to_string().parse().unwrap_or(0.0);
    if f.abs() >= 1_000_000.0 {
        format!("¥{:.1}M", f / 1_000_000.0)
    } else if f.abs() >= 10_000.0 {
        format!("¥{:.1}K", f / 1_000.0)
    } else {
        format!("¥{:.0}", f)
    }
}

fn fmt_date(d: Option<chrono::NaiveDate>) -> String {
    match d {
        Some(d) => d.format("%Y-%m-%d").to_string(),
        None => "—".to_string(),
    }
}

fn is_overdue_or_soon(d: Option<chrono::NaiveDate>) -> bool {
    // 交期在 7 天内或已过期 → drawer 交期标 warn 色（逾期/临期提示）。
    let Some(d) = d else {
        return false;
    };
    let today = chrono::Utc::now().date_naive();
    let soon = today.checked_add_days(chrono::Days::new(7)).unwrap_or(today);
    d <= soon
}

// =============================================================================
// 行展开 row-detail 渲染（HTMX hx-swap="afterend" 注入单 <tr class="row-detail">）
// 照 row_detail_tr 范式：detail-grid 4 列 + detail-actions
// =============================================================================

/// 纯数字格式化（无 ¥，进度条/数量用）。
fn fmt_plain(d: rust_decimal::Decimal) -> String {
    let f: f64 = d.to_string().parse().unwrap_or(0.0);
    if f.abs() >= 1000.0 {
        format!("{:.0}", f)
    } else {
        format!("{:.1}", f)
    }
}

/// 物料图标（按 product_id hash 选语义色 + 图标），对齐 mes_work_center::material_icon。
fn material_icon(product_id: i64) -> (&'static str, Markup) {
    match (product_id % 4) as u8 {
        0 => ("bg-warn-bg text-warn", icon::tool_icon("w-[20px] h-[20px]")),
        1 => ("bg-purple-bg text-purple", icon::cube_icon("w-[20px] h-[20px]")),
        2 => ("bg-accent-bg text-accent", icon::briefcase_icon("w-[20px] h-[20px]")),
        _ => (
            "bg-success-bg text-success",
            icon::check_circle_icon("w-[20px] h-[20px]"),
        ),
    }
}

/// 最早需求日期的紧急度提示文案 + 颜色（逾期/临期/正常），对齐 mes_work_center::urgency_hint。
fn urgency_hint(earliest: Option<chrono::NaiveDate>) -> Option<(String, &'static str)> {
    let d = earliest?;
    let today = chrono::Local::now().date_naive();
    let diff = (d - today).num_days();
    if diff < 0 {
        Some((format!("⚠ 已逾期 {} 天", diff.abs()), "text-danger"))
    } else if diff == 0 {
        Some(("⚠ 今天到期".into(), "text-danger"))
    } else if diff <= 3 {
        Some((format!("⚠ {} 天后到期", diff), "text-danger"))
    } else if diff <= 7 {
        Some((format!("{} 天后到期", diff), "text-warn"))
    } else {
        None
    }
}

/// 收货进度条（动态宽度用 style，同 mes wo_progress 既定做法）。
fn po_progress_bar(p: &abt_core::purchase::work_center::PoProgress) -> Markup {
    let pct_f: f64 = p.received_pct.to_string().parse().unwrap_or(0.0);
    let bar_color = if pct_f < 30.0 {
        "bg-muted"
    } else if pct_f <= 70.0 {
        "bg-accent"
    } else {
        "bg-success"
    };
    html! {
        div class="flex flex-col gap-[3px]" {
            div class="w-[96px] h-[6px] bg-border-soft rounded-[3px] overflow-hidden" {
                div class=(format!("h-full rounded-[3px] {bar_color} transition-all duration-150"))
                    style=(format!("width:{}%", pct_f as u32)) {}
            }
            div class="text-[11px] text-muted font-mono tabular-nums" {
                (format!("{:.0}%", pct_f)) " · " (fmt_plain(p.received_qty)) "/"
                (fmt_plain(p.ordered_qty))
            }
        }
    }
}

/// ci-row 就绪态项（✓ ok / ✗ missing）。
fn ci_item(ok: bool, label: &str) -> Markup {
    let (cls, ic) = if ok {
        ("text-success", icon::check_circle_icon("w-3 h-3"))
    } else {
        ("text-danger", icon::x_icon("w-3 h-3"))
    };
    html! {
        span class=(format!("inline-flex items-center gap-1 text-[11px] font-medium {cls}")) {
            (ic) (label)
        }
    }
}

/// detail-grid 标签块（label + 值 Markup）。
fn detail_block(label: &str, value: Markup) -> Markup {
    html! {
        div {
            div class="text-[11px] text-muted font-medium mb-1.5 uppercase tracking-wide" { (label) }
            div class="text-sm text-fg leading-relaxed" { (value) }
        }
    }
}

/// hub-link（详情跳转 + 箭头）。
fn hub_link(href: &str, text: &str) -> Markup {
    html! {
        a class="inline-flex items-center gap-1 text-sm text-accent font-semibold no-underline"
            href=(href) {
            (text) (icon::arrow_right_icon("w-3.5 h-3.5"))
        }
    }
}

/// 订单行展开详情。
fn order_row_detail_tr(s: &PoHubSummary) -> Markup {
    html! {
        tr class="row-detail" {
            td colspan="6" class="p-0 border-none bg-surface-raised" {
                div class="p-5 border-t border-dashed border-border-soft border-b border-border-soft" {
                    div class="grid grid-cols-4 gap-5 mb-4" {
                        (detail_block("来源链", html! {
                            @if s.source_chain.sales_order_docs.is_empty() {
                                span class="text-muted" { "—" }
                            } @else {
                                @for doc in &s.source_chain.sales_order_docs {
                                    span class="font-mono text-accent" { (doc) " " }
                                }
                            }
                        }))
                        (detail_block("收货进度", po_progress_bar(&s.progress)))
                        (detail_block("金额 / 交期", html! {
                            (fmt_decimal(s.order.total_amount)) br;
                            span class="text-xs text-muted" {
                                "交期 " (fmt_date(s.order.expected_delivery_date))
                                " · " (s.progress.item_count) " 行"
                            }
                        }))
                        (detail_block("应付台账", html! {
                            "已立 " (fmt_decimal(s.ap_summary.ap_amount)) br;
                            "已付 " (fmt_decimal(s.ap_summary.paid_amount))
                        }))
                    }
                    div class="flex items-center gap-2 pt-3 border-t border-border-soft flex-wrap" {
                        button class="inline-flex items-center gap-1 text-sm text-accent font-semibold border-none bg-transparent cursor-pointer hover:underline"
                            hx-get=(PcPoDetailDrawerPath { id: s.order.id }.to_string())
                            hx-target="#po-detail-drawer-body" hx-swap="innerHTML"
                            _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #po-detail-overlay" {
                            "订单详情" (icon::arrow_right_icon("w-3.5 h-3.5"))
                        }
                    }
                }
            }
        }
    }
}

/// 对账付款行展开详情（draft / payment 两套 grid）。
fn settlement_row_detail_tr(s: &SettlementHubSummary) -> Markup {
    html! {
        tr class="row-detail" {
            td colspan="5" class="p-0 border-none bg-surface-raised" {
                div class="p-5 border-t border-dashed border-border-soft border-b border-border-soft" {
                    @if let Some(d) = &s.draft_recon {
                        div class="grid grid-cols-4 gap-5 mb-4" {
                            (detail_block("对账明细", html! {
                                (d.item_count) " 行明细" br;
                                "总额 " (fmt_decimal(d.total_amount))
                            }))
                            (detail_block("退货结算", html! {
                                "待结算退货 " (d.pending_returns_count) " 笔" br;
                                "金额 " (fmt_decimal(d.pending_returns_amount))
                            }))
                            (detail_block("差额", html! { (fmt_decimal(d.difference)) }))
                            (detail_block("应付余额", html! { (fmt_decimal(d.ap_outstanding)) }))
                        }
                        div class="flex items-center gap-2 pt-3 border-t border-border-soft flex-wrap" {
                            (hub_link("/admin/purchase/reconciliations", "对账详情"))
                        }
                    } @else if let Some(p) = &s.pending_payment {
                        div class="grid grid-cols-4 gap-5 mb-4" {
                            (detail_block("来源对账单", html! {
                                span class="font-mono" { (p.source_recon_doc.as_deref().unwrap_or("—")) }
                            }))
                            (detail_block("付款金额", html! {
                                (fmt_decimal(p.amount)) br;
                                span class="text-xs text-muted" { (p.payment_method) }
                            }))
                            (detail_block("发票", html! {
                                (p.invoice_number.as_deref().unwrap_or("—")) br;
                                @if let Some(amt) = p.invoice_amount {
                                    span class="text-xs text-muted" { (fmt_decimal(amt)) }
                                }
                            }))
                            (detail_block("三单匹配", html! {
                                div class="flex flex-col gap-1" {
                                    (ci_item(p.three_way_match.po_matched, "PO 已匹配"))
                                    (ci_item(p.three_way_match.receipt_matched, "入库已匹配"))
                                    (ci_item(p.three_way_match.invoice_matched, "发票已匹配"))
                                }
                            }))
                        }
                        @if !p.three_way_match.differences.is_empty() {
                            div class="mt-3 text-xs text-danger" {
                                @for diff in &p.three_way_match.differences {
                                    (diff) br;
                                }
                            }
                        }
                        div class="flex items-center gap-2 pt-3 border-t border-border-soft flex-wrap" {
                            (hub_link("/admin/purchase/payments", "付款详情"))
                        }
                    }
                }
            }
        }
    }
}

/// 退货行展开详情。
fn return_row_detail_tr(s: &ReturnHubSummary) -> Markup {
    html! {
        tr class="row-detail" {
            td colspan="5" class="p-0 border-none bg-surface-raised" {
                div class="p-5 border-t border-dashed border-border-soft border-b border-border-soft" {
                    div class="grid grid-cols-4 gap-5 mb-4" {
                        (detail_block("来源订单", html! {
                            span class="font-mono text-accent" { (s.source_po_doc) } br;
                            span class="text-xs text-muted" { "状态 " (s.source_po_status) }
                        }))
                        (detail_block("退货明细", html! {
                            (s.item_count) " 行 · " (fmt_plain(s.total_qty))
                        }))
                        (detail_block("金额", html! { (fmt_decimal(s.return_order.total_amount)) }))
                        (detail_block("结算", html! {
                            span class="text-muted" { (s.settlement_hint) }
                        }))
                    }
                    div class="flex items-center gap-2 pt-3 border-t border-border-soft flex-wrap" {
                        button class="inline-flex items-center gap-1 text-sm text-accent font-semibold border-none bg-transparent cursor-pointer hover:underline"
                            hx-get=(PcReturnDetailDrawerPath { id: s.return_order.id }.to_string())
                            hx-target="#return-detail-drawer-body" hx-swap="innerHTML"
                            _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #return-detail-overlay" {
                            "退货详情" (icon::arrow_right_icon("w-3.5 h-3.5"))
                        }
                    }
                }
            }
        }
    }
}

/// 需求物料行懒加载的需求明细 rows（注入展开 tbody，照 mes demand_expand_rows）。
fn demand_expand_rows(items: &[DemandSummary]) -> Markup {
    html! {
        @for d in items {
            tr class="bg-accent-bg/30" {
                td class="py-1.5 px-4 font-mono text-xs text-accent" {
                    (d.order_no.as_deref().unwrap_or("—"))
                }
                td class="py-1.5 px-3 text-right font-mono text-xs" { (fmt_plain(d.quantity)) }
                td class="py-1.5 px-3 font-mono text-xs text-muted" { (fmt_date(d.required_date)) }
            }
        }
        @if items.is_empty() {
            tr { td colspan="3" class="text-center text-muted py-4" { "暂无需求明细" } }
        }
    }
}
