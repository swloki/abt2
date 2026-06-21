use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashMap;

use abt_core::master_data::product::ProductService;
use abt_core::mes::work_order::WorkOrderService;
use abt_core::mes::dashboard::MesDashboardService;
use abt_core::wms::backflush::{BackflushFilter, BackflushService};
use abt_core::wms::material_requisition::{MaterialRequisitionService, model::RequisitionFilter};

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_receipt::{MaterialUsageDataPath, MaterialUsagePath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[require_permission("WORK_ORDER", "read")]
pub async fn get_material_usage(_path: MaterialUsagePath, ctx: RequestContext) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

 let wo_svc = state.work_order_service();
 let filter = abt_core::mes::work_order::WorkOrderFilter {
 status: None,
 product_id: None,
 keyword: None,
 date_from: None,
 date_to: None, product_code: None,
 };
 let wo_result = wo_svc.list(&service_ctx, &mut conn, filter, 1, 100).await?;
 let work_orders = wo_result.items;

 // Batch-load product names for dropdown display
 let product_ids: Vec<i64> = work_orders.iter().map(|wo| wo.product_id).collect();
 let products = if product_ids.is_empty() {
 Vec::new()
 } else {
 state.product_service().get_by_ids(&service_ctx, &mut conn, product_ids).await.unwrap_or_default()
 };
 let product_map: HashMap<i64, String> = products
 .into_iter()
 .map(|p| (p.product_id, p.pdt_name))
 .collect();

 let content = material_usage_page(&work_orders, &product_map);
 Ok(Html(admin_page(is_htmx, "物料消耗追踪", &claims, "production", MaterialUsagePath::PATH, "生产管理", None, content, &nav_filter).into_string()))
}

#[derive(Debug, Deserialize)]
pub struct UsageDataParams {
 pub wo_id: Option<i64>,
}

#[require_permission("WORK_ORDER", "read")]
pub async fn load_usage_data(
 _path: MaterialUsageDataPath,
 ctx: RequestContext,
 Query(params): Query<UsageDataParams>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;

 let wo_id = match params.wo_id {
 Some(id) => id,
 None => {
 return Ok(Html(html! {
 div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)] text-center py-8 text-sm text-muted" { "请选择工单" }
 }.into_string()));
 }
 };

 let dash_svc = state.mes_dashboard_service();
 let bf_svc = state.backflush_service();
 let req_svc = state.material_requisition_service();

 let wo_info = dash_svc.get_wo_basic_info(&service_ctx, &mut conn, wo_id).await?;
 let bom_items = dash_svc.get_bom_comparison(&service_ctx, &mut conn, wo_id).await?;
 let bf_records = bf_svc.list(
 &service_ctx, &mut conn,
 BackflushFilter { work_order_id: Some(wo_id), status: None },
 1, 50,
 ).await?;
 let requisitions = req_svc.list(
 &service_ctx, &mut conn,
 RequisitionFilter { work_order_id: Some(wo_id), ..Default::default() },
 1, 50,
 ).await?;

 // Compute summary
 let standard_qty: Decimal = bom_items.iter().map(|i| i.standard_total).sum();
 let backflush_qty: Decimal = bom_items.iter().map(|i| i.backflush_total).sum();
 let picked_total: Decimal = bom_items.iter().map(|i| i.picked_qty).sum();
 let variance = backflush_qty - standard_qty;

 let ctx = MaterialUsageContext {
 bom_items: &bom_items,
 standard_qty, backflush_qty, picked_total, variance,
 bf_records: &bf_records.items,
 requisitions: &requisitions.items,
 };
 let html_content = usage_data_fragment(&wo_info, &ctx);
 Ok(Html(html_content.into_string()))
}

fn material_usage_page(
 work_orders: &[abt_core::mes::work_order::WorkOrder],
 product_map: &HashMap<i64, String>,
) -> Markup {
 html! { div {
 div class="flex items-center justify-between mb-6" {
 h1 class="text-xl font-bold text-fg tracking-tight" { "物料消耗追踪" }
 div class="flex gap-3" {
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" {
 (icon::download_icon("w-4 h-4"))
 "导出"
 }
 }
 }

 // Filter bar
 div class="flex items-center gap-3 mb-5 flex-wrap" {
 select class="w-60 px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent cursor-pointer" name="wo_id"
 hx-get=(MaterialUsageDataPath::PATH)
 hx-target="#usage-content"
 hx-trigger="change"
 hx-swap="innerHTML" {
 option value="" { "选择工单..." }
 @for wo in work_orders {
 @let label = match product_map.get(&wo.product_id) {
 Some(name) => format!("{} · {} ({})", wo.doc_number, name, crate::utils::fmt_qty(wo.planned_qty)),
 None => wo.doc_number.clone(),
 };
 option value=(wo.id) { (label) }
 }
 }
 select class="w-40 px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent cursor-pointer" disabled {
 option { "全部批次" }
 }
 }

 // Content area
 div id="usage-content" {
 div class="data-card text-center py-8 text-sm text-muted" { "请选择工单查看物料消耗数据" }
 }
 }}
}

struct MaterialUsageContext<'a> {
 bom_items: &'a [abt_core::mes::dashboard::model::BomCompareItem],
 standard_qty: Decimal,
 backflush_qty: Decimal,
 picked_total: Decimal,
 variance: Decimal,
 bf_records: &'a [abt_core::wms::backflush::model::BackflushRecord],
 requisitions: &'a [abt_core::wms::material_requisition::model::MaterialRequisition],
}

fn usage_data_fragment(wo_info: &abt_core::mes::dashboard::model::WoBasicInfo, ctx: &MaterialUsageContext) -> Markup {
    let status_label = match wo_info.status {
        1 => "待计划",
        2 => "已计划",
        3 => "已下达",
        4 => "已关闭",
        5 => "已取消",
        _ => "—",
    };
    let pill_cls = wo_status_pill_class(wo_info.status);
    let status_pill = html! { span class=(format!("text-[11px] px-2 py-0.5 rounded-full font-medium {}", pill_cls)) { (status_label) } };

    html! {
        // ── WO header ──
 div class="data-card flex items-center justify-between flex-wrap gap-3 mb-5" {
            div class="flex items-center gap-3 flex-wrap" {
                span class="text-lg font-bold font-mono tabular-nums text-fg" { (wo_info.doc_number) }
                span class="text-sm text-fg-2" { (wo_info.product_name.as_deref().unwrap_or("—")) }
                (status_pill)
            }
            div class="flex gap-4 text-sm text-muted" {
                span { "计划: " strong class="font-mono tabular-nums" { (crate::utils::fmt_qty(wo_info.planned_qty)) } }
                span { "完成: " strong class="text-success font-mono tabular-nums" { (crate::utils::fmt_qty(wo_info.completed_qty)) } }
                @if let Some(v) = &wo_info.bom_version {
                    span { "BOM: " strong { (v) } }
                }
            }
        }


        // ── Summary stats ──
 div class="grid grid-cols-2 lg:grid-cols-4 gap-4 mb-5" {
 // BOM standard
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md" {
 div class="w-11 h-11 rounded-md grid place-items-center shrink-0 bg-accent-bg text-accent" { (icon::box_icon("w-5 h-5")) }
 div {
 div class="text-lg font-bold font-mono tabular-nums text-fg" { (crate::utils::fmt_qty(ctx.standard_qty)) }
 div class="text-sm text-muted" { "BOM 标准用量" }
 div class="text-xs text-muted mt-1" {
 "按完成 " (crate::utils::fmt_qty(wo_info.completed_qty)) " 件计算"
 }
 }
 }
 // Actual picked
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md" {
 div class="w-11 h-11 rounded-md grid place-items-center shrink-0 bg-success-bg text-success" { (icon::clipboard_list_icon("w-5 h-5")) }
 div {
 div class="text-lg font-bold font-mono tabular-nums text-fg" { (crate::utils::fmt_qty(ctx.picked_total)) }
 div class="text-sm text-muted" { "实际消耗(领料)" }
 div class="text-xs text-muted mt-1" { "含损耗余量" }
 }
 }
 // Backflush
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md" {
 div class="w-11 h-11 rounded-md grid place-items-center shrink-0 bg-warn-bg text-warn" { (icon::refresh_icon("w-5 h-5")) }
 div {
 div class="text-lg font-bold font-mono tabular-nums text-fg" { (crate::utils::fmt_qty(ctx.backflush_qty)) }
 div class="text-sm text-muted" { "倒冲消耗" }
 }
 }
 // Variance
 div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md" {
 div class="w-11 h-11 rounded-md grid place-items-center shrink-0 bg-danger-bg text-danger" { (icon::circle_alert_icon("w-5 h-5")) }
 div {
 @let variance_cls = if ctx.variance > Decimal::ZERO { "text-danger" } else if ctx.variance < Decimal::ZERO { "text-success" } else { "" };
 div class=(format!("text-lg font-bold font-mono tabular-nums {}", variance_cls)) {
 @if ctx.variance > Decimal::ZERO { "+" }
 (crate::utils::fmt_qty(ctx.variance))
 }
 div class="text-sm text-muted" { "用量差异" }
 @if ctx.standard_qty > Decimal::ZERO {
 @let rate = ((ctx.variance / ctx.standard_qty) * Decimal::ONE_HUNDRED).abs();
 div class="text-xs text-muted mt-1" {
 "超出标准 " (crate::utils::fmt_qty(rate)) "%"
 }
 }
 }
 }
 }

 // ── BOM comparison table ──
 @if !ctx.bom_items.is_empty() {
 div class="data-card mb-5" {
 div class="flex items-center gap-2 px-5 py-4 text-sm font-semibold text-fg border-b border-border-soft" {
 (icon::box_icon("w-5 h-5"))
 "BOM 标准用量 vs 实际消耗"
 }
 div class="overflow-x-auto" {
 table class="data-table" {
 thead { tr {
 th { "物料编码" }
 th { "物料名称" }
 th { "单位" }
 th class="text-right text-[13px]" { "单件用量" }
 th class="text-right text-[13px]" { "标准总量" }
 th class="text-right text-[13px]" { "领料数量" }
 th class="text-right text-[13px]" { "倒冲消耗" }
 th class="text-right text-[13px]" { "损耗率" }
 th class="text-right text-[13px]" { "差异" }
 }}
 tbody {
 @for item in ctx.bom_items {
 @let diff = item.backflush_total - item.standard_total;
                        @let diff_cls = if diff > Decimal::ZERO { "text-danger" } else if diff < Decimal::ZERO { "text-success" } else { "text-muted" };
 @let loss_rate = if item.standard_total > Decimal::ZERO {
 let r = ((item.picked_qty - item.standard_total) / item.standard_total) * Decimal::ONE_HUNDRED;
 format!("{}%", crate::utils::fmt_qty(r))
 } else {
 "—".to_string()
 };
 tr {
 td class="font-mono tabular-nums" { (item.component_code.as_deref().unwrap_or("—")) }
 td { (item.component_name.as_deref().unwrap_or("—")) }
 td { (item.unit.as_deref().unwrap_or("—")) }
 td class="text-right text-[13px] font-mono tabular-nums" { (crate::utils::fmt_qty(item.per_unit_qty)) }
 td class="text-right text-[13px] font-mono tabular-nums" { (crate::utils::fmt_qty(item.standard_total)) }
 td class="text-right text-[13px] font-mono tabular-nums" { (crate::utils::fmt_qty(item.picked_qty)) }
 td class="text-right text-[13px] font-mono tabular-nums" { (crate::utils::fmt_qty(item.backflush_total)) }
 td class="text-right text-[13px] font-mono tabular-nums" { (loss_rate) }
 td class="text-right text-[13px]" {
                                span class=(format!("font-medium {}", diff_cls)) {
 @if diff > Decimal::ZERO { "+" }
 (crate::utils::fmt_qty(diff))
 }
 }
 }
 }
 }
 }
 }
 }
 } @else {
 div class="data-card mb-5 text-center py-8 text-sm text-muted" { "该工单未关联 BOM，无法显示物料对比" }
 }

 // ── Backflush detail records ──
 @if !ctx.bf_records.is_empty() {
 div class="data-card mb-5" {
 div class="flex items-center gap-2 px-5 py-4 text-sm font-semibold text-fg border-b border-border-soft" {
 (icon::refresh_icon("w-5 h-5"))
 "倒冲明细记录"
 }
 div class="overflow-x-auto" {
 table class="data-table" {
 thead { tr {
 th { "倒冲单号" }
 th { "完成数量" }
 th { "倒冲日期" }
 th { "状态" }
 }}
 tbody {
 @for rec in ctx.bf_records {
 tr {
 td class="font-mono tabular-nums" {
 a href=(format!("/admin/wms/backflush/{}", rec.id)) class="text-accent font-medium cursor-pointer" { (rec.doc_number) }
 }
 td class="text-right text-[13px] font-mono tabular-nums" { (crate::utils::fmt_qty(rec.completed_qty)) }
 td { (rec.backflush_date) }
 td { (backflush_status_label(&rec.status)) }
 }
 }
 }
 }
 }
 }
 }

 // ── Requisition records ──
 @if !ctx.requisitions.is_empty() {
 div class="data-card mb-5" {
 div class="flex items-center gap-2 px-5 py-4 text-sm font-semibold text-fg border-b border-border-soft" {
 (icon::clipboard_list_icon("w-5 h-5"))
 "领料记录"
 }
 div class="overflow-x-auto" {
 table class="data-table" {
 thead { tr {
 th { "领料单号" }
 th { "领料日期" }
 th { "状态" }
 }}
 tbody {
 @for req in ctx.requisitions {
 tr {
 td class="font-mono tabular-nums" {
 a href=(format!("/admin/wms/requisition/{}", req.id)) class="text-accent font-medium cursor-pointer" { (req.doc_number) }
 }
 td { (req.requisition_date) }
 td { (requisition_status_label(&req.status)) }
 }
 }
 }
 }
 }
 }
 }
 }
}

fn backflush_status_label(s: &abt_core::wms::enums::BackflushStatus) -> Markup {
    use abt_core::wms::enums::BackflushStatus;
    let (label, cls) = match s {
        BackflushStatus::Draft => ("待处理", "bg-warn-bg text-warn"),
        BackflushStatus::Executed => ("已完成", "bg-success-bg text-success"),
        BackflushStatus::Adjusted => ("已调整", "bg-accent-bg text-accent"),
    };
    html! { span class=(format!("text-[11px] px-2 py-0.5 rounded-full font-medium {}", cls)) { (label) } }
}

fn requisition_status_label(s: &abt_core::wms::enums::RequisitionStatus) -> Markup {
    use abt_core::wms::enums::RequisitionStatus;
    let (label, cls) = match s {
        RequisitionStatus::Draft => ("待确认", "bg-warn-bg text-warn"),
        RequisitionStatus::Confirmed => ("已确认", "bg-accent-bg text-accent"),
        RequisitionStatus::Issued => ("已发料", "bg-success-bg text-success"),
        RequisitionStatus::Cancelled => ("已取消", "bg-slate-50 text-muted"),
        RequisitionStatus::PartiallyIssued => ("部分发料", "bg-accent-bg text-accent"),
    };
    html! { span class=(format!("text-[11px] px-2 py-0.5 rounded-full font-medium {}", cls)) { (label) } }
}

/// Returns a UnoCSS pill color class for a work-order status code.
fn wo_status_pill_class(status: i16) -> &'static str {
    match status {
        1 => "bg-warn-bg text-warn",       // 待计划
        2 => "bg-accent-bg text-accent",   // 已计划
        3 => "bg-[rgba(124,58,237,0.1)] text-purple", // 已下达
        4 => "bg-success-bg text-success", // 已关闭
        5 => "bg-slate-50 text-muted",   // 已取消
        _ => "bg-slate-50 text-muted",
    }
}
