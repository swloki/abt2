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

use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::demand_handler::{
    CreateOrderFromDemandsReq, DemandPoolQuery, DemandSummary, MaterialAggQuery,
    MaterialAggSummary, PurchaseDemandService,
};
use abt_core::purchase::enums::{
    PaymentStatus, PurchaseOrderStatus, PurchaseReconStatus, PurchaseReturnStatus,
};
use abt_core::purchase::order::model::{PurchaseOrder, PurchaseOrderQuery};
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::purchase::payment::model::{PaymentRequest, PaymentRequestQuery};
use abt_core::purchase::payment::PaymentRequestService;
use abt_core::purchase::reconciliation::model::{PurchaseReconciliation, PurchaseReconciliationQuery};
use abt_core::purchase::reconciliation::PurchaseReconciliationService;
use abt_core::purchase::return_order::model::{PurchaseReturn, PurchaseReturnQuery};
use abt_core::purchase::return_order::PurchaseReturnService;
use abt_core::purchase::work_center::{
    PoHubSummary, PurchaseWorkCenterService, PurchaseWorkCenterSummary, ReturnHubSummary,
    SettlementHubSummary, SettlementReconType, ThreeWayMatchSummary,
};
use abt_core::shared::types::{DomainError, PageParams};

use std::collections::HashMap;

use abt_core::purchase::quotation::model::QuotationComparison;
use abt_core::purchase::quotation::PurchaseQuotationService;
use axum::Form;
use crate::components::icon;
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
    let summary = state
        .purchase_work_center_service()
        .summary(&service_ctx, &mut conn)
        .await
        .unwrap_or_default();

    let demand_meta = format!(
        "{} 条待处理 · 销售订单/请购驱动",
        summary.pending_demand + summary.pending_misc
    );
    let orders_meta = format!(
        "待审批 {} · 待收货 {} · 部分收货 {}",
        summary.po_pending_approval, summary.po_pending_receive, summary.po_partial
    );
    let settle_meta = format!(
        "草稿对账 {} · 待审批付款 {}",
        summary.recon_draft, summary.payment_pending_approval
    );
    let returns_meta = format!(
        "待发货 {} · 已发出 {}",
        summary.return_pending_ship, summary.return_shipped
    );
    let today = chrono::Utc::now().date_naive().format("%Y-%m-%d").to_string();
    let content = html! {
        // detail-header：标题 + meta chip + 内嵌锚点条
        div class="bg-bg border border-border-soft rounded-lg p-6 mb-4 shadow-[var(--shadow-card)]" {
            div class="flex items-center justify-between flex-wrap gap-4" {
                div {
                    h1 class="text-xl font-bold text-fg tracking-tight" { "采购作业中心" }
                    div class="flex items-center gap-2 mt-2 flex-wrap" {
                        span class="inline-flex items-center gap-1.5 px-2.5 py-1 bg-surface border border-border-soft rounded-sm text-xs text-fg-2 font-medium" {
                            (icon::calendar_icon("w-3.5 h-3.5 text-muted"))
                            span class="font-mono" { (today) }
                        }
                        span class="inline-flex items-center gap-1.5 px-2.5 py-1 bg-surface border border-border-soft rounded-sm text-xs text-fg-2 font-medium" {
                            (icon::trending_up_icon("w-3.5 h-3.5 text-muted"))
                            "本周待办 " (summary.total())
                        }
                    }
                }
            }
            (render_anchor_nav(&summary))
        }
        (render_card_shell("pc-demand-card", PcDemandPath::PATH, "采购需求", icon::package_icon("w-[15px] h-[15px]"), &demand_meta))
        (render_card_shell("pc-orders-card", PcOrdersPath::PATH, "采购订单", icon::clipboard_list_icon("w-[15px] h-[15px]"), &orders_meta))
        (render_card_shell("pc-settlement-card", PcSettlementPath::PATH, "对账付款", icon::payment_icon("w-[15px] h-[15px]"), &settle_meta))
        (render_card_shell("pc-returns-card", PcReturnsPath::PATH, "采购退货", icon::return_arrow_icon("w-[15px] h-[15px]"), &returns_meta))
        (render_drawer_overlay("approve-overlay", "approve-drawer", "approve-drawer-body", "审批采购订单"))
        (render_drawer_overlay("pay-overlay", "pay-drawer", "pay-drawer-body", "审批付款"))
        (render_drawer_overlay("convert-po-overlay", "convert-po-drawer", "convert-po-drawer-body", "转采购单"))
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

    let body = if view == "detail" {
        let result = svc
            .list_pending_demands(
                &service_ctx,
                &mut conn,
                DemandPoolQuery {
                    keyword: p.keyword.clone(),
                    ..Default::default()
                },
                PageParams::new(1, 10),
            )
            .await?;
        demand_detail_table(&result)
    } else {
        let result = svc
            .list_material_aggregated(
                &service_ctx,
                &mut conn,
                MaterialAggQuery {
                    keyword: p.keyword.clone(),
                    ..Default::default()
                },
                PageParams::new(1, 10),
            )
            .await?;
        demand_material_table(&result)
    };

    Ok(Html(
        html! {
            div id="pc-demand-card" {
                (demand_filter_bar(view, &p))
                (body)
                (info_box("物料汇总按物料聚合多个来源（销售订单 / 零星请购）需求，一键转采购单；点击物料行展开查看需求明细。"))
                div class="px-5 py-3 border-t border-border-soft text-center" {
                    a class="text-sm text-accent font-semibold no-underline" href="/admin/purchase/demand-pool" { "查看全部需求 →" }
                }
            }
        }
        .into_string(),
    ))
}

// ── ② 采购订单 ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct OrdersCardParams {
    /// "approval" | "confirmed" | "partial"（默认 approval）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub tab: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub keyword: Option<String>,
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
    let tab = p.tab.as_deref().unwrap_or("approval");
    let status = match tab {
        "confirmed" => PurchaseOrderStatus::Confirmed,
        "partial" => PurchaseOrderStatus::PartiallyReceived,
        _ => PurchaseOrderStatus::PendingApproval,
    };
    let result = svc
        .list(
            &service_ctx,
            &mut conn,
            PurchaseOrderQuery {
                status: Some(status),
                doc_number: p.keyword.clone(),
                ..Default::default()
            },
            PageParams::new(1, 10),
        )
        .await?;
    let ids: Vec<i64> = result.items.iter().map(|o| o.supplier_id).collect();
    let names = supplier_names(&state, &service_ctx, &mut conn, &ids).await;

    Ok(Html(
        html! {
            div id="pc-orders-card" {
                (orders_filter_bar(tab, &p))
                (orders_table(&result.items, &names, tab))
                (info_box("按状态（待审批 / 待收货 / 部分收货）筛选，点订单号进详情，行内就地审批 / 登记收货（触发来料通知 → PO 回写 + 立应付台账）。"))
                div class="px-5 py-3 border-t border-border-soft text-center" {
                    a class="text-sm text-accent font-semibold no-underline" href="/admin/purchase/orders" { "查看全部订单 →" }
                }
            }
        }
        .into_string(),
    ))
}

// ── ③ 对账付款 ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct SettlementCardParams {
    /// "recon" | "payment"（默认 recon — 草稿对账单）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub tab: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub keyword: Option<String>,
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

    let body = if tab == "payment" {
        let svc = state.payment_request_service();
        let result = svc
            .list(
                &service_ctx,
                &mut conn,
                PaymentRequestQuery {
                    status: Some(PaymentStatus::Draft),
                    keyword: p.keyword.clone(),
                    ..Default::default()
                },
                PageParams::new(1, 10),
            )
            .await?;
        let ids: Vec<i64> = result.items.iter().map(|x| x.supplier_id).collect();
        let names = supplier_names(&state, &service_ctx, &mut conn, &ids).await;
        payment_table(&result.items, &names)
    } else {
        let svc = state.purchase_reconciliation_service();
        let result = svc
            .list(
                &service_ctx,
                &mut conn,
                PurchaseReconciliationQuery {
                    status: Some(PurchaseReconStatus::Draft),
                    ..Default::default()
                },
                PageParams::new(1, 10),
            )
            .await?;
        let ids: Vec<i64> = result.items.iter().map(|x| x.supplier_id).collect();
        let names = supplier_names(&state, &service_ctx, &mut conn, &ids).await;
        recon_table(&result.items, &names)
    };

    Ok(Html(
        html! {
            div id="pc-settlement-card" {
                (settlement_filter_bar(tab, &p))
                (body)
                (info_box("对账单是开票 / PO 闭环 / 退货结算的唯一载体：确认对账时结算关联退货并创建付款申请；付款审批后转 FMS 付款。"))
                div class="px-5 py-3 border-t border-border-soft text-center" {
                    a class="text-sm text-accent font-semibold no-underline" href="/admin/purchase/reconciliations" { "查看全部对账 →" }
                }
            }
        }
        .into_string(),
    ))
}

// ── ④ 采购退货 ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ReturnsCardParams {
    /// "confirmed" | "shipped"（默认 confirmed — 待发货）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub tab: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub keyword: Option<String>,
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
    let tab = p.tab.as_deref().unwrap_or("confirmed");
    let status = match tab {
        "shipped" => PurchaseReturnStatus::Shipped,
        _ => PurchaseReturnStatus::Confirmed,
    };
    let result = svc
        .list(
            &service_ctx,
            &mut conn,
            PurchaseReturnQuery {
                status: Some(status),
                ..Default::default()
            },
            PageParams::new(1, 10),
        )
        .await?;
    let ids: Vec<i64> = result.items.iter().map(|x| x.supplier_id).collect();
    let names = supplier_names(&state, &service_ctx, &mut conn, &ids).await;

    Ok(Html(
        html! {
            div id="pc-returns-card" {
                (returns_filter_bar(tab, &p))
                (returns_table(&result.items, &names, tab))
                (info_box("退货发货后状态转 Shipped，进入对账单结算时自动 Shipped→Settled 并反向冲减应付台账。"))
                div class="px-5 py-3 border-t border-border-soft text-center" {
                    a class="text-sm text-accent font-semibold no-underline" href="/admin/purchase/returns" { "查看全部退货 →" }
                }
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

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn post_convert_po(
    _path: PcConvertPoPath,
    ctx: RequestContext,
    Form(form): Form<ConvertPoForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        state,
        service_ctx,
        ..
    } = ctx;
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
        .create_order_from_demands(&service_ctx, &mut tx, req)
        .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(([("HX-Trigger", "demandChanged, poChanged")], Html(String::new())))
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
    html! {
        div class="mb-4" {
            div class="font-semibold text-fg text-sm" { (product_name) }
            div class="text-xs text-muted font-mono" { (product_code) }
        }
        div class="grid grid-cols-2 gap-4 mb-4" {
            (field_readonly("待转需求数", &format!("{}", demands.len())));
            (field_readonly("总需求量", &fmt_plain(total_qty)));
        }
        div class="mb-4" {
            label class="block text-xs text-muted font-medium mb-1.5" { "选择供应商" }
            @if quotes.is_empty() {
                div class="p-3 mb-2 bg-warn-bg border border-border-soft rounded-sm text-xs text-warn leading-relaxed" {
                    "该物料暂无有效报价，将生成单价待补的 PO 草稿。请输入供应商 ID："
                }
                input type="number" name="supplier_id" required
                    class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                    placeholder="供应商 ID" {};
            } @else {
                div class="flex flex-col gap-2" {
                    @for q in quotes {
                        label class="flex items-center gap-2 p-2.5 border border-border-soft rounded-sm cursor-pointer hover:bg-accent-bg text-sm" {
                            input type="radio" name="supplier_id" value=(q.supplier_id) required class="cursor-pointer" {};
                            span class="flex-1 text-fg" {
                                (supplier_names.get(&q.supplier_id).map(|s| s.as_str()).unwrap_or("未知供应商"))
                                @if q.is_preferred {
                                    span class="ml-1 px-1.5 py-0.5 rounded-full bg-success-bg text-success text-[10px] font-semibold" { "优选" }
                                }
                            }
                            span class="font-mono text-fg-2" { (fmt_plain(q.unit_price)) " " (q.currency) }
                            span class="text-xs text-muted" { "有效期至 " (q.valid_until.format("%Y-%m-%d").to_string()) }
                        }
                    }
                }
            }
        }
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
            "提交后生成 PO 草稿：按 " (demands.len()) " 个需求聚合 "
            (fmt_plain(total_qty)) " " (product_code)
            "；单价待采购员在 PO 详情补充后 confirm。"
        }
        form hx-post=(PcConvertPoPath { product_id }.to_string())
            hx-target="this" hx-swap="none"
            _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #convert-po-overlay then call showToast('PO 草稿已生成，单价待补充')" {
            input type="hidden" name="demand_ids" value=(demand_ids_str) {};
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
    Ok(([("HX-Trigger", "reconChanged")], Html(String::new())))
}

// =============================================================================
// 渲染辅助
// =============================================================================

/// 锚点条：待办总数 + 4 业务 chip + 逾期/临期告警。
fn render_anchor_nav(s: &PurchaseWorkCenterSummary) -> Markup {
    let total = s.total();
    let demand_cnt = s.pending_demand + s.pending_misc;
    let order_cnt = s.po_pending_approval + s.po_pending_receive + s.po_partial;
    let settle_cnt = s.recon_draft + s.payment_pending_approval;
    let return_cnt = s.return_pending_ship + s.return_shipped;
    html! {
        div class="sticky top-0 z-20 flex items-center gap-4 p-3 mt-4 rounded-lg border border-border-soft bg-bg shadow-xs flex-wrap" {
            div class="flex flex-col items-center pr-4 border-r border-border-soft shrink-0" {
                span class="text-xl font-bold font-mono tabular-nums text-accent leading-tight" { (total) }
                span class="text-xs text-muted font-medium" { "待办" }
            }
            div class="flex items-center gap-2 flex-wrap" {
                (nav_chip("#pc-demand-card", icon::package_icon("w-3.5 h-3.5"), "采购需求", demand_cnt))
                (nav_chip("#pc-orders-card", icon::clipboard_list_icon("w-3.5 h-3.5"), "采购订单", order_cnt))
                (nav_chip("#pc-settlement-card", icon::payment_icon("w-3.5 h-3.5"), "对账付款", settle_cnt))
                (nav_chip("#pc-returns-card", icon::return_arrow_icon("w-3.5 h-3.5"), "采购退货", return_cnt))
            }
            div class="ml-auto flex items-center gap-2" {
                @if s.overdue_count > 0 {
                    span class="inline-flex items-center gap-1 px-2.5 py-1 rounded-full bg-danger-bg text-danger text-[11px] font-semibold" {
                        (icon::alert_triangle_icon("w-3 h-3"))
                        (s.overdue_count) " 逾期"
                    }
                }
                @if s.soon_count > 0 {
                    span class="inline-flex items-center gap-1 px-2.5 py-1 rounded-full bg-warn-bg text-warn text-[11px] font-semibold" {
                        (icon::clock_icon("w-3 h-3"))
                        (s.soon_count) " 临期"
                    }
                }
            }
        }
    }
}

fn nav_chip(href: &str, chip_icon: Markup, label: &str, count: u64) -> Markup {
    if count == 0 {
        return html! {};
    }
    html! {
        a class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-surface border border-border-soft text-sm font-semibold text-fg-2 no-underline cursor-pointer hover:bg-accent-bg hover:border-accent hover:text-accent transition-all"
            href=(href)
            _=(format!("on click halt the event then call document.querySelector('{href}')?.scrollIntoView({{behavior:'smooth',block:'center'}})")) {
            (chip_icon)
            (label)
            span class="font-mono font-bold text-accent" { (count) }
        }
    }
}

/// Card 外壳（grp disclosure 折叠）：grp-head（图标 + 标题 + meta + chevron）+ grp-body（占位 div 懒加载）。
/// 监听 `poChanged`/`reconChanged`/`returnChanged` 自刷新（写操作后）。
fn render_card_shell(card_id: &str, src: &str, title: &str, grp_icon: Markup, meta: &str) -> Markup {
    let trigger = format!("load, poChanged from:body, reconChanged from:body, returnChanged from:body, demandChanged from:body");
    html! {
        section class="grp open bg-bg border border-border-soft rounded-lg mb-4 shadow-[var(--shadow-card)] overflow-hidden" {
            div class="grp-head flex items-center gap-3 px-5 py-3 border-b border-border-soft cursor-pointer hover:bg-surface-raised select-none"
                _="on click toggle .open on closest .grp" {
                div class="w-7 h-7 rounded-md grid place-items-center bg-surface text-fg-2 shrink-0" { (grp_icon) }
                span class="font-semibold text-fg text-sm" { (title) }
                span class="text-xs text-muted font-mono flex-1 truncate" { (meta) }
                (icon::chevron_down_icon("w-[18px] h-[18px] text-muted grp-chev"))
            }
            div class="grp-body" {
                div id=(card_id)
                    class="text-sm text-muted"
                    hx-get=(src)
                    hx-trigger=(trigger)
                    hx-target="this"
                    hx-swap="outerHTML" {
                    "加载中…"
                }
            }
        }
    }
}

/// Drawer overlay 壳（同 mes_work_center）：背景点击/关闭按钮收起，body 由 `hx-get` 填充。
fn render_drawer_overlay(overlay_id: &str, drawer_id: &str, body_id: &str, title: &str) -> Markup {
    html! {
        div id=(overlay_id)
            class="fixed inset-0 bg-slate-900/40 opacity-0 invisible pointer-events-none transition-opacity duration-200 z-[90] open:opacity-100 open:visible open:pointer-events-auto"
            _=(format!("on click[me is event.target] remove .open from #{}", overlay_id)) {
            div id=(drawer_id)
                class="fixed top-0 right-0 h-full w-[480px] max-w-[92vw] bg-bg shadow-lg translate-x-full transition-transform duration-300 flex flex-col z-[91] open:translate-x-0"
                _="on click halt the event" {
                div class="flex items-center justify-between px-6 py-5 border-b border-border-soft" {
                    div class="font-bold text-base text-fg" { (title) }
                    button type="button"
                        class="w-8 h-8 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg flex items-center justify-center"
                        _=(format!("on click remove .open from #{}", overlay_id)) {
                        (icon::x_icon("w-4 h-4"))
                    }
                }
                div id=(body_id) class="flex-1 overflow-y-auto px-6 py-5" {}
            }
        }
    }
}

/// 供应商名称映射：按 id 批量反查（拉首页 500 条过滤）。
async fn supplier_names(
    state: &crate::state::AppState,
    ctx: &abt_core::shared::types::context::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    ids: &[i64],
) -> HashMap<i64, String> {
    if ids.is_empty() {
        return HashMap::new();
    }
    let svc = state.supplier_service();
    match svc
        .list(ctx, db, SupplierQuery::default(), PageParams::new(1, 500))
        .await
    {
        Ok(r) => r
            .items
            .into_iter()
            .filter(|s| ids.contains(&s.id))
            .map(|s| (s.id, s.name))
            .collect(),
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

fn toggle_cls(active: bool) -> &'static str {
    if active {
        "inline-flex items-center gap-1 px-3 py-1 text-sm text-accent font-semibold cursor-pointer bg-bg shadow-xs rounded-sm"
    } else {
        "inline-flex items-center gap-1 px-3 py-1 text-sm text-muted cursor-pointer bg-transparent border-none rounded-sm hover:text-fg transition-colors"
    }
}

// ── 需求 card 渲染 ──

fn demand_filter_bar(view: &str, p: &DemandCardParams) -> Markup {
    let is_mat = view == "material";
    let kw = p.keyword.as_deref().unwrap_or("");
    html! {
        div class="flex items-center justify-between flex-wrap gap-3 px-5 py-3 border-b border-border-soft" {
            div class="inline-flex bg-surface border border-border-soft rounded-md p-[3px] gap-0.5" {
                button class=(toggle_cls(is_mat)) type="button"
                    hx-get=(PcDemandPath::PATH)
                    hx-vals="{\"view\":\"material\"}"
                    hx-target="#pc-demand-card" hx-select="#pc-demand-card" hx-swap="outerHTML"
                    hx-include="#pc-demand-filter-form"
                    { "物料汇总" }
                button class=(toggle_cls(!is_mat)) type="button"
                    hx-get=(PcDemandPath::PATH)
                    hx-vals="{\"view\":\"detail\"}"
                    hx-target="#pc-demand-card" hx-select="#pc-demand-card" hx-swap="outerHTML"
                    hx-include="#pc-demand-filter-form"
                    { "请购明细" }
            }
            form class="flex items-center gap-2"
                hx-get=(PcDemandPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.pc-demand-search"
                hx-target="#pc-demand-card" hx-select="#pc-demand-card" hx-swap="outerHTML"
                {
                input type="hidden" name="view" value=(view);
                div class="relative" {
                    (icon::search_icon("w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"));
                    input class="pc-demand-search w-[180px] pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                        type="text" name="keyword" placeholder="搜索物料/订单"
                        value=(kw);
                }
            }
            form id="pc-demand-filter-form" class="hidden" {
                input type="hidden" name="keyword" value=(kw);
            }
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

fn demand_detail_table(result: &abt_core::shared::types::PaginatedResult<DemandSummary>) -> Markup {
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
                                a class="inline-flex items-center px-3 py-1 rounded-sm bg-accent text-white text-xs font-semibold no-underline hover:opacity-90"
                                    href="/admin/purchase/demand-pool" { "转采购单" }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── 订单 card 渲染 ──

fn orders_filter_bar(tab: &str, p: &OrdersCardParams) -> Markup {
    let kw = p.keyword.as_deref().unwrap_or("");
    let tab_btn = |val: &str, label: &str, active: bool| -> Markup {
        html! {
            button class=(toggle_cls(active)) type="button"
                hx-get=(PcOrdersPath::PATH)
                hx-vals=(format!("{{\"tab\":\"{}\"}}", val))
                hx-target="#pc-orders-card" hx-select="#pc-orders-card" hx-swap="outerHTML"
                hx-include="#pc-orders-filter-form"
                { (label) }
        }
    };
    html! {
        div class="flex items-center justify-between flex-wrap gap-3 px-5 py-3 border-b border-border-soft" {
            div class="inline-flex bg-surface border border-border-soft rounded-md p-[3px] gap-0.5" {
                (tab_btn("approval", "待审批", tab == "approval"));
                (tab_btn("confirmed", "待收货", tab == "confirmed"));
                (tab_btn("partial", "部分收货", tab == "partial"));
            }
            form class="flex items-center gap-2"
                hx-get=(PcOrdersPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.pc-orders-search"
                hx-target="#pc-orders-card" hx-select="#pc-orders-card" hx-swap="outerHTML"
                {
                input type="hidden" name="tab" value=(tab);
                div class="relative" {
                    (icon::search_icon("w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"));
                    input class="pc-orders-search w-[180px] pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                        type="text" name="keyword" placeholder="搜索 PO 号"
                        value=(kw);
                }
            }
            form id="pc-orders-filter-form" class="hidden" {
                input type="hidden" name="keyword" value=(kw);
            }
        }
    }
}

fn orders_table(items: &[PurchaseOrder], names: &HashMap<i64, String>, tab: &str) -> Markup {
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
                                @if tab == "approval" {
                                    button class="inline-flex items-center px-3 py-1 rounded-sm bg-accent text-white text-xs font-semibold border-none cursor-pointer hover:opacity-90 mr-1"
                                        hx-get=(PcOrderApproveDrawerPath { id: o.id }.to_string())
                                        hx-target="#approve-drawer-body" hx-swap="innerHTML"
                                        _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #approve-overlay" { "审批" }
                                } @else {
                                    a class="inline-flex items-center px-3 py-1 rounded-sm bg-white text-fg-2 border border-border text-xs font-medium no-underline cursor-pointer hover:bg-surface mr-1"
                                        href=(format!("/admin/purchase/orders/{}", o.id)) { "登记收货" }
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

fn settlement_filter_bar(tab: &str, p: &SettlementCardParams) -> Markup {
    let kw = p.keyword.as_deref().unwrap_or("");
    let tab_btn = |val: &str, label: &str, active: bool| -> Markup {
        html! {
            button class=(toggle_cls(active)) type="button"
                hx-get=(PcSettlementPath::PATH)
                hx-vals=(format!("{{\"tab\":\"{}\"}}", val))
                hx-target="#pc-settlement-card" hx-select="#pc-settlement-card" hx-swap="outerHTML"
                hx-include="#pc-settlement-filter-form"
                { (label) }
        }
    };
    html! {
        div class="flex items-center justify-between flex-wrap gap-3 px-5 py-3 border-b border-border-soft" {
            div class="inline-flex bg-surface border border-border-soft rounded-md p-[3px] gap-0.5" {
                (tab_btn("recon", "草稿对账单", tab == "recon"));
                (tab_btn("payment", "待审批付款", tab == "payment"));
            }
            form class="flex items-center gap-2"
                hx-get=(PcSettlementPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.pc-settlement-search"
                hx-target="#pc-settlement-card" hx-select="#pc-settlement-card" hx-swap="outerHTML"
                {
                input type="hidden" name="tab" value=(tab);
                div class="relative" {
                    (icon::search_icon("w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"));
                    input class="pc-settlement-search w-[180px] pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                        type="text" name="keyword" placeholder="搜索供应商/单号"
                        value=(kw);
                }
            }
            form id="pc-settlement-filter-form" class="hidden" {
                input type="hidden" name="keyword" value=(kw);
            }
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

fn returns_filter_bar(tab: &str, p: &ReturnsCardParams) -> Markup {
    let kw = p.keyword.as_deref().unwrap_or("");
    let tab_btn = |val: &str, label: &str, active: bool| -> Markup {
        html! {
            button class=(toggle_cls(active)) type="button"
                hx-get=(PcReturnsPath::PATH)
                hx-vals=(format!("{{\"tab\":\"{}\"}}", val))
                hx-target="#pc-returns-card" hx-select="#pc-returns-card" hx-swap="outerHTML"
                hx-include="#pc-returns-filter-form"
                { (label) }
        }
    };
    html! {
        div class="flex items-center justify-between flex-wrap gap-3 px-5 py-3 border-b border-border-soft" {
            div class="inline-flex bg-surface border border-border-soft rounded-md p-[3px] gap-0.5" {
                (tab_btn("confirmed", "待发货", tab == "confirmed"));
                (tab_btn("shipped", "已发出", tab == "shipped"));
            }
            form class="flex items-center gap-2"
                hx-get=(PcReturnsPath::PATH)
                hx-trigger="change, keyup changed delay:300ms from:.pc-returns-search"
                hx-target="#pc-returns-card" hx-select="#pc-returns-card" hx-swap="outerHTML"
                {
                input type="hidden" name="tab" value=(tab);
                div class="relative" {
                    (icon::search_icon("w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"));
                    input class="pc-returns-search w-[180px] pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                        type="text" name="keyword" placeholder="搜索退货单号"
                        value=(kw);
                }
            }
            form id="pc-returns-filter-form" class="hidden" {
                input type="hidden" name="keyword" value=(kw);
            }
        }
    }
}

fn returns_table(items: &[PurchaseReturn], names: &HashMap<i64, String>, tab: &str) -> Markup {
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
                                @if tab == "confirmed" {
                                    a class="inline-flex items-center px-3 py-1 rounded-sm bg-accent text-white text-xs font-semibold no-underline cursor-pointer hover:opacity-90 mr-1"
                                        href=(format!("/admin/purchase/returns/{}", r.id)) { "发货" }
                                } @else {
                                    span class="text-xs text-muted" { "待供应商确认" }
                                }
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
// 照 mes_order_list::row_detail_tr 范式：detail-grid 4 列 + detail-actions
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

/// info-box 说明条（card 底部业务说明）。
fn info_box(text: &str) -> Markup {
    html! {
        div class="flex gap-2 m-5 mt-3 p-3 bg-surface-raised border border-border-soft rounded-sm text-xs text-muted leading-relaxed" {
            (icon::info_icon("w-[15px] h-[15px] shrink-0 mt-[2px]"))
            span { (text) }
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
                        (hub_link(&format!("/admin/purchase/orders/{}", s.order.id), "订单详情"))
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
                        (hub_link(&format!("/admin/purchase/returns/{}", s.return_order.id), "退货详情"))
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
