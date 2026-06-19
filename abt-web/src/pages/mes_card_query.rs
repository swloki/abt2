use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup, PreEscaped};
use serde::Deserialize;

use abt_core::mes::production_batch::ProductionBatchService;
use abt_core::mes::production_batch::repo::BatchRoutingProgressRepo;
use abt_core::mes::work_order::WorkOrderService;
use abt_core::mes::work_report::WorkReportService;
use abt_core::shared::identity::UserService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_batch::{CardQueryPath, CardQuerySearchPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[require_permission("WORK_ORDER", "read")]
pub async fn get_card_query(_path: CardQueryPath, ctx: RequestContext) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

 // 加载最近批次用于"最近查询"区域
 let batch_svc = state.production_batch_service();
 let recent_result = batch_svc
 .list_batches(&service_ctx, &mut conn, Default::default(), 1, 6)
 .await
 .unwrap_or_else(|_| abt_core::shared::types::PaginatedResult {
 items: vec![],
 total: 0,
 page: 1,
 page_size: 6,
 total_pages: 0,
 });

 let content = card_query_page(&recent_result.items);
 Ok(Html(admin_page(is_htmx, "流转卡查询", &claims, "production", CardQueryPath::PATH, "生产管理", None, content, &nav_filter).into_string()))
}

#[derive(Debug, Deserialize)]
pub struct CardSearchParams {
 pub q: Option<String>,
}

#[require_permission("WORK_ORDER", "read")]
pub async fn search_card(
 _path: CardQuerySearchPath,
 ctx: RequestContext,
 Query(params): Query<CardSearchParams>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;

 let query = match params.q {
 Some(ref q) if !q.trim().is_empty() => q.trim().to_string(),
 _ => {
 return Ok(Html(html! {
 div class="text-center text-muted p-6" {
 "请输入流转卡序列号进行查询"
 }
 }.into_string()));
 }
 };

 let batch_svc = state.production_batch_service();
 let report_svc = state.work_report_service();
 let wo_svc = state.work_order_service();

 let batch = match batch_svc.find_by_card_sn(&service_ctx, &mut conn, query.clone()).await? {
 Some(b) => b,
 None => {
 return Ok(Html(html! {
 div class="text-center text-danger p-6" {
 "未找到流转卡 \"" (query) "\" 对应的批次"
 }
 }.into_string()));
 }
 };

 let product_name = batch_svc.get_product_name(&mut conn, batch.product_id).await?.unwrap_or_default();
 let routings = batch_svc.list_routings(&service_ctx, &mut conn, batch.work_order_id).await?;
 // 查批次工序执行进度（status/completed_qty/defect_qty 已迁移到 batch_routing_progress）
 let progress_list = BatchRoutingProgressRepo::list_by_batch(&mut conn, batch.id).await?;
 let progress_map: std::collections::HashMap<i64, abt_core::mes::production_batch::BatchRoutingProgress> =
 progress_list.into_iter().map(|p| (p.routing_id, p)).collect();
 let reports = report_svc.list_by_batch(&service_ctx, &mut conn, batch.id).await?;

 // 获取工单单号
 let wo_doc_number = wo_svc.find_by_id(&service_ctx, &mut conn, batch.work_order_id)
 .await
 .map(|wo| wo.doc_number)
 .unwrap_or_default();

 // 加载工人名称映射
 let user_svc = state.user_service();
 let users = user_svc.list_users(&service_ctx, &mut conn, 1, 200).await?;
 let user_map: std::collections::HashMap<i64, String> = users.items.iter()
 .map(|u| (u.user_id, u.display_name.clone().unwrap_or_else(|| u.username.clone())))
 .collect();

 let html_content = card_search_result(&batch, &product_name, &wo_doc_number, &routings, &reports, &user_map, &progress_map);
 Ok(Html(html_content.into_string()))
}

fn batch_status_label(s: &abt_core::mes::enums::BatchStatus) -> (&'static str, &'static str) {
 use abt_core::mes::enums::BatchStatus::*;
 match s {
 Pending => ("待生产", "status-draft"),
 InProgress => ("进行中", "status-progress"),
 Suspended => ("已暂停", "status-suspended"),
 PendingReceipt => ("待入库", "status-inspecting"),
 Completed => ("已完成", "status-completed"),
 Cancelled => ("已取消", "status-neutral"),
 }
}

fn card_query_page(recent_batches: &[abt_core::mes::production_batch::BatchListItem]) -> Markup {
 html! {
 div {
 // 搜索区
 div class="bg-bg border border-border-soft rounded-lg p-10 text-center relative overflow-hidden" {
 div class="text-2xl font-bold text-fg" { "流转卡查询" }
 div class="text-sm text-muted mt-2" { "输入流转卡号、批次号或扫描二维码，实时查看工序流转进度" }
 div class="mt-6 flex gap-3 items-center justify-center flex-wrap" {
 form hx-get=(CardQuerySearchPath::PATH) hx-target="#card-result" hx-swap="innerHTML" hx-trigger="submit" class="flex gap-2 items-center w-full max-w-xl" {
 input class="flex-1 min-w-0 pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="q" placeholder="输入流转卡号 / 批次号，如 FC-SN-060301" autofocus;
 button class="shrink-0 inline-flex items-center gap-2 py-2 px-4 rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" type="submit" {
 (PreEscaped(r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><path d="M21 21l-4.35-4.35"/></svg>"#))
 "查询"
 }
 }
 button class="shrink-0 inline-flex items-center gap-2 py-2 px-4 rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" {
 (PreEscaped(r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M3 7V5a2 2 0 012-2h2M17 3h2a2 2 0 012 2v2M21 17v2a2 2 0 01-2 2h-2M7 21H5a2 2 0 01-2-2v-2"/><rect x="7" y="7" width="10" height="10" rx="1"/></svg>"#))
 "扫描二维码"
 }
 }
 }

 // 查询结果区域
 div id="card-result" {}

 // 最近查询的流转卡
 @if !recent_batches.is_empty() {
 div class="bg-bg border border-border-soft rounded-lg p-6" {
 div class="text-sm font-semibold text-fg flex items-center gap-2" {
 (PreEscaped(r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/></svg>"#))
 "最近查询的流转卡"
 }
 div class="grid grid-cols-3 gap-4 mt-4" {
 @for batch in recent_batches {
 @let (status_label, status_cls) = batch_status_label(&batch.status);
 @let progress_pct = if batch.total_steps.unwrap_or(0) > 0 {
 ((batch.current_step as f64 / batch.total_steps.unwrap_or(1) as f64) * 100.0) as i32
 } else { 0 };
 @let progress_color = match &batch.status {
 abt_core::mes::enums::BatchStatus::Completed => "var(--success)",
 abt_core::mes::enums::BatchStatus::PendingReceipt => "var(--warn)",
 abt_core::mes::enums::BatchStatus::Suspended => "var(--danger)",
 abt_core::mes::enums::BatchStatus::Pending => "var(--muted)",
 _ => "var(--accent)",
 };
 @let step_info = match batch.total_steps {
 Some(ts) if ts > 0 => format!("{}/{}", batch.current_step, ts),
 _ => "—".to_string(),
 };

                div class="bg-surface border border-border-soft rounded p-4 cursor-pointer transition-colors hover:bg-accent-bg"
                    hx-get=(format!("{}?q={}", CardQuerySearchPath::PATH, batch.card_sn))
                    hx-target="#card-result"
                    hx-swap="innerHTML"
                {
                    div class="flex items-center justify-between mb-2" {
                        span class="text-sm font-mono tabular-nums font-semibold text-fg" { (batch.card_sn) }
                        span class=(format!("status-pill {}", crate::utils::status_color(status_cls))) { (status_label) }
                    }
                    div class="text-sm text-fg-2 mb-2" {
                        (batch.product_name.as_deref().unwrap_or("—"))
                        " · "
                        (step_info)
                        " "
                        (batch.current_step_name.as_deref().unwrap_or(""))
                    }
                    div class="h-1.5 bg-[rgba(0,0,0,0.06)] rounded-full overflow-hidden" {
                        div class="h-full rounded-full transition-all duration-300" style=(format!("width:{}%;background:{}", progress_pct, progress_color)) {}
                    }
                }
 }
 }
 }
 }
 }
 }
}

fn card_search_result(
 batch: &abt_core::mes::production_batch::ProductionBatch,
 product_name: &str,
 wo_doc_number: &str,
 routings: &[abt_core::mes::production_batch::WorkOrderRouting],
 reports: &[abt_core::mes::work_report::WorkReport],
 user_map: &std::collections::HashMap<i64, String>,
 progress_map: &std::collections::HashMap<i64, abt_core::mes::production_batch::BatchRoutingProgress>,
) -> Markup {
 let (status_label, status_cls) = batch_status_label(&batch.status);
 let total_steps = routings.len() as i32;

 let current_step_display = if batch.current_step == 0 {
 "未开始".to_string()
 } else {
 let step_name = routings.iter()
 .find(|r| r.step_no == batch.current_step)
 .map(|r| r.process_name.as_str())
 .unwrap_or("—");
 format!("{}/{} {}", batch.current_step, total_steps, step_name)
 };

 let actual_start_str = batch.actual_start
 .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
 .unwrap_or_else(|| "—".to_string());

 // 构建routing查找映射
 let routing_map: std::collections::HashMap<i64, &abt_core::mes::production_batch::WorkOrderRouting> =
 routings.iter().map(|r| (r.id, r)).collect();

 // 按工序汇总工时（从报工记录聚合，原代码错误地用 completed_qty 代替工时）
 let routing_work_hours: std::collections::HashMap<i64, rust_decimal::Decimal> = {
 let mut m: std::collections::HashMap<i64, rust_decimal::Decimal> = std::collections::HashMap::new();
 for r in reports {
 *m.entry(r.routing_id).or_insert(rust_decimal::Decimal::ZERO) += r.work_hours;
 }
 m
 };

 html! {
 div class="bg-bg border border-border-soft rounded-lg p-5 mb-5 shadow-[var(--shadow-card)] overflow-hidden" {
 // 结果头部
 div class="flex items-center justify-between mb-3" {
 div class="text-lg font-bold font-mono tabular-nums text-fg" { (batch.card_sn) }
 span class=(format!("status-pill {}", crate::utils::status_color(status_cls))) { (status_label) }
 }
 div class="flex items-center gap-4 text-sm text-muted" {
 span class="inline-flex items-center gap-1" {
 (PreEscaped(r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="7" width="20" height="14" rx="2"/></svg>"#))
 "批次 " span class="font-mono tabular-nums" { (batch.batch_no) }
 }
 span class="inline-flex items-center gap-1" {
 (PreEscaped(r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14.7 6.3a1 1 0 000 1.4l1.6 1.6a1 1 0 001.4 0l3.77-3.77a6 6 0 01-7.94 7.94l-6.91 6.91a2.12 2.12 0 01-3-3l6.91-6.91a6 6 0 017.94-7.94l-3.76 3.76z"/></svg>"#))
 "工单 "
 a href=(format!("/admin/mes/orders/{}", batch.work_order_id)) class="text-accent" { (wo_doc_number) }
 }
 }
 div class="h-px bg-border-soft my-4" {}

 // 基本信息网格
 div class="grid grid-cols-3 gap-4" {
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "产品" }
 span class="text-sm text-fg font-medium" { (product_name) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "批次数量" }
 span class="text-sm text-fg font-medium font-mono tabular-nums" { (crate::utils::fmt_qty(batch.batch_qty)) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "完成/报废" }
 span class="text-sm text-fg font-medium" {
 span class="text-success font-mono tabular-nums" { (crate::utils::fmt_qty(batch.completed_qty)) }
 " / "
 span class="text-danger font-mono tabular-nums" { (crate::utils::fmt_qty(batch.scrap_qty)) }
 }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "实际开始" }
 span class="text-sm text-fg font-medium" { (actual_start_str) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "当前工序" }
 span class="text-sm text-fg font-medium text-warn" { (current_step_display) }
 }
 div class="flex flex-col gap-1" {
 span class="text-xs text-muted font-medium" { "状态" }
 span class=(format!("status-pill {}", crate::utils::status_color(status_cls))) { (status_label) }
 }
 }

 // 工序流转进度
 @if !routings.is_empty() {
 div class="flex items-start relative px-4 py-6 gap-2" {
 @for (i, routing) in routings.iter().enumerate() {
 @let progress = progress_map.get(&routing.id);
 @let p_completed = progress.map_or(rust_decimal::Decimal::ZERO, |p| p.completed_qty);
 @let p_defect = progress.map_or(rust_decimal::Decimal::ZERO, |p| p.defect_qty);
 @let p_work_hours = routing_work_hours.get(&routing.id).copied().unwrap_or(rust_decimal::Decimal::ZERO);
 @let is_completed = progress.map_or(false, |p| p.status == abt_core::mes::enums::RoutingStatus::Completed);
 @let is_current = routing.step_no == batch.current_step;
 @let is_inspection = routing.is_inspection_point;
 @let step_str = routing.step_no.to_string();
 @let (node_bg, node_text) = if is_completed {
 ("bg-success text-white", "✓".to_string())
 } else if is_current {
 ("bg-accent text-white", step_str.clone())
 } else if is_inspection {
 ("bg-warn text-white", step_str.clone())
 } else {
 ("bg-slate-50 text-muted", step_str)
 };

 div class="flex flex-col items-center text-center shrink-0 min-w-[80px]" {
 div class=(format!("w-8 h-8 rounded-full grid place-items-center text-sm font-semibold {}", node_bg)) {
 (node_text)
 }
 div class="text-xs font-medium text-fg mt-2" { (routing.process_name) }
 div class="text-[11px] text-muted mt-1" {
 @if is_completed || is_current {
 "完成 " (crate::utils::fmt_qty(p_completed))
 @if p_defect > rust_decimal::Decimal::ZERO {
 br;
 "不良 " (crate::utils::fmt_qty(p_defect))
 }
 br;
 "工时 " (crate::utils::fmt_qty(p_work_hours)) "h"
 } @else {
 "待生产"
 }
 }
 }
 @if i < routings.len() - 1 {
 div class="flex-1 h-px bg-border-soft mt-4 min-w-[20px]" {}
 }
 }
 }
 }

 // 报工明细
 @if !reports.is_empty() {
 div class="bg-bg border border-border-soft rounded-lg mb-0 shadow-[var(--shadow-card)] overflow-hidden" {
 div class="p-4 border-b border-border-soft text-sm font-semibold text-fg flex items-center gap-2 bg-surface-raised" {
 (PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 20h9M16.5 3.5a2.121 2.121 0 013 3L7 19l-4 1 1-4L16.5 3.5z"/></svg>"#))
 "报工明细"
 }
 div class="p-0" {
 div class="overflow-x-auto" {
 table class="data-table" {
 thead { tr {
 th { "报工单号" }
 th { "工序" }
 th { "班次" }
 th { "工人" }
 th class="text-right text-[13px]" { "完成" }
 th class="text-right text-[13px]" { "不良" }
 th { "不良原因" }
 th class="text-right text-[13px]" { "工时" }
 th class="text-right text-[13px]" { "计件工资" }
 }}
 tbody {
 @for report in reports {
 @let process_name = routing_map.get(&report.routing_id)
 .map(|r| r.process_name.as_str())
 .unwrap_or("—");
 @let worker_name = user_map.get(&report.worker_id)
 .map(|s| s.as_str())
 .unwrap_or("—");
 @let unit_price = routing_map.get(&report.routing_id)
 .and_then(|r| r.unit_price)
 .unwrap_or(rust_decimal::Decimal::ZERO);
 @let wage = report.completed_qty * unit_price;
 @let defect_reason_str = match &report.defect_reason {
 Some(abt_core::mes::enums::DefectReason::MaterialDefect) => "物料不良",
 Some(abt_core::mes::enums::DefectReason::EquipmentFault) => "设备故障",
 Some(abt_core::mes::enums::DefectReason::OperatorError) => "操作失误",
 Some(abt_core::mes::enums::DefectReason::ProcessIssue) => "工艺问题",
 _ => "—",
 };
 tr {
 td {
 a href=(format!("/admin/mes/reports/{}", report.id)) class="text-accent font-mono tabular-nums" { (report.doc_number) }
 }
 td { (process_name) }
 td {
 @if report.shift == abt_core::mes::enums::ShiftType::Day { "白班" }
 @else { "夜班" }
 }
 td { (worker_name) }
 td class="text-right text-[13px] font-mono tabular-nums text-success" { (crate::utils::fmt_qty(report.completed_qty)) }
 td class="text-right text-[13px] font-mono tabular-nums" {
 @if report.defect_qty > rust_decimal::Decimal::ZERO {
 span class="text-danger" { (crate::utils::fmt_qty(report.defect_qty)) }
 } @else {
 "0"
 }
 }
 td { (defect_reason_str) }
 td class="text-right text-[13px] font-mono tabular-nums" { (crate::utils::fmt_qty(report.work_hours)) "h" }
 td class="text-right text-[13px] font-mono tabular-nums text-success" { "¥" (crate::utils::fmt_qty(wage)) }
 }
 }
 }
 }
 }
 }
 }
 } @else {
 div class="text-center text-muted p-4" {
 "暂无报工记录"
 }
 }
 }
 }
}
