//! MES 生产需求池 → 创建生产计划页面

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup, PreEscaped};
use serde::Deserialize;

use abt_core::mes::demand_handler::{
 CreatePlanFromDemandsReq, DemandPoolQuery, DemandSummary, MesDemandService, PlanDemandItemReq,
};
use abt_core::mes::enums::PlanStatus;
use abt_core::mes::production_plan::ProductionPlanService;
use abt_core::shared::types::{DomainError, PageParams};

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_demand_pool::*;
use crate::routes::order::OrderDetailPath;
use crate::routes::mes_plan::PlanDetailPath;
use crate::utils::{fmt_qty, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct DemandPoolCreateParams {
 pub product_id: Option<i64>,
 pub product_code: Option<String>,
 pub product_name: Option<String>,
 pub demand_ids: Option<String>,
}

// ── Form Request ──

#[derive(Debug, Deserialize)]
pub struct CreatePlanForm {
 pub plan_type: i16,
 pub plan_date: String,
 pub remark: Option<String>,
 pub default_scheduled_start: Option<String>,
 pub default_scheduled_end: Option<String>,
 pub demand_ids: String, // comma-separated from hidden input
 pub items_json: Option<String>, // JSON array of per-row scheduling params
 #[serde(default)]
 pub action: Option<String>, // "draft" (default) or "release"
}

// ── Handlers ──

#[require_permission("WORK_ORDER", "create")]
pub async fn get_demand_pool_create(
 _path: MesDemandPoolCreatePath,
 ctx: RequestContext,
 Query(params): Query<DemandPoolCreateParams>,
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

 // Load demands for the selected product
 let demand_svc = state.mes_demand_service();
 let demands = if let Some(product_id) = params.product_id {
 demand_svc
 .list_pending_demands(
 &service_ctx,
 &mut conn,
 DemandPoolQuery {
 status: Some(1), // Pending only
 product_id: Some(product_id),
 order_id: None,
 ..Default::default()
 },
 PageParams::new(1, 100),
 )
 .await?
 .items
 } else {
 vec![]
 };

 // Filter demands by pre-selected demand_ids if provided
 let preselected_ids: Vec<i64> = params
 .demand_ids
 .as_deref()
 .map(|s| {
 s.split(',')
 .filter_map(|id| id.trim().parse::<i64>().ok())
 .collect()
 })
 .unwrap_or_default();

 let product_name = params
 .product_name
 .as_deref()
 .or_else(|| demands.first().map(|d| d.product_name.as_str()))
 .unwrap_or("—");
 let product_code = params
 .product_code
 .as_deref()
 .or_else(|| demands.first().map(|d| d.product_code.as_str()))
 .unwrap_or("—");

 let content = create_page_content(
 &demands,
 &preselected_ids,
 params.product_id,
 product_name,
 product_code,
 );

 let page_html = admin_page(
 is_htmx,
 "创建生产计划",
 &claims,
 "production",
 MesDemandPoolCreatePath::PATH,
 "生产管理",
 Some("创建生产计划"),
 content,
 &nav_filter,
 );

 Ok(Html(page_html.into_string()))
}

/// POST: create production plan from selected demands
#[require_permission("WORK_ORDER", "create")]
pub async fn create_plan_from_demands(
 _path: MesDemandPoolCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<CreatePlanForm>,
) -> Result<impl IntoResponse> {
 let RequestContext {
 state,
 service_ctx,
 ..
 } = ctx;

 // Parse demand_ids from comma-separated string
 let demand_ids: Vec<i64> = form
 .demand_ids
 .split(',')
 .filter_map(|s| s.trim().parse::<i64>().ok())
 .collect();

 if demand_ids.is_empty() {
 return Err(DomainError::validation("请至少选择一条生产需求").into());
 }

 let plan_date = chrono::NaiveDate::parse_from_str(&form.plan_date, "%Y-%m-%d")
 .map_err(|e| DomainError::validation(format!("无效计划日期格式: {e}")))?;

 let default_scheduled_start = form
 .default_scheduled_start
 .as_deref()
 .filter(|s| !s.is_empty())
 .map(|s| {
 chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
 .map_err(|e| DomainError::validation(format!("无效默认排程开始日期: {e}")))
 })
 .transpose()?;

 let default_scheduled_end = form
 .default_scheduled_end
 .as_deref()
 .filter(|s| !s.is_empty())
 .map(|s| {
 chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
 .map_err(|e| DomainError::validation(format!("无效默认排程结束日期: {e}")))
 })
 .transpose()?;

 // Parse per-row scheduling items from JSON
 let items: Option<Vec<PlanDemandItemReq>> = form
 .items_json
 .as_deref()
 .filter(|s| !s.is_empty())
 .map(|j| serde_json::from_str(j))
 .transpose()
 .map_err(|e| DomainError::validation(format!("无效排程参数JSON: {e}")))?;

 let create_req = CreatePlanFromDemandsReq {
 demand_ids,
 plan_type: form.plan_type,
 plan_date,
 remark: form.remark,
 items,
 default_scheduled_start,
 default_scheduled_end,
 };

 // 整个流程必须在同一事务中：乐观锁(状态1→2) → 创建计划 → 更新 target_doc → 发布事件。
 // 任一步骤失败需整体回滚，避免需求成为孤儿状态（status=2 但无 target_doc）。
 let mut tx = state.pool.begin().await
 .map_err(|e| DomainError::Internal(e.into()))?;

 let svc = state.mes_demand_service();
 let result = svc
 .create_plan_from_demands(&service_ctx, &mut tx, create_req)
 .await?;

 // 创建并下达：自动确认 + 下达（同一事务）
 if form.action.as_deref() == Some("release") {
 let plan_svc = state.production_plan_service();
 let plan = plan_svc.find_by_id(&service_ctx, &mut tx, result.doc_id).await?;
 if plan.status == PlanStatus::Draft {
 plan_svc.confirm(&service_ctx, &mut tx, result.doc_id).await?;
 }
 plan_svc.release_to_work_orders(&service_ctx, &mut tx, result.doc_id).await?;
 }

 tx.commit().await
 .map_err(|e| DomainError::Internal(e.into()))?;

 let redirect = PlanDetailPath { id: result.doc_id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Page Content ──

fn create_page_content(
 demands: &[DemandSummary],
 preselected_ids: &[i64],
 product_id: Option<i64>,
 product_name: &str,
 product_code: &str,
) -> Markup {
 let today = chrono::Local::now().format("%Y-%m-%d").to_string();
 let default_start = chrono::Local::now()
 .checked_add_days(chrono::Days::new(1))
 .map(|d| d.format("%Y-%m-%d").to_string())
 .unwrap_or_default();
 let default_end = chrono::Local::now()
 .checked_add_days(chrono::Days::new(10))
 .map(|d| d.format("%Y-%m-%d").to_string())
 .unwrap_or_default();

 // Compute total quantity for summary bar
 let total_qty: rust_decimal::Decimal = demands
 .iter()
 .filter(|d| preselected_ids.is_empty() || preselected_ids.contains(&d.id))
 .map(|d| d.quantity)
 .sum();

 let selected_count = if preselected_ids.is_empty() && !demands.is_empty() {
 demands.len()
 } else {
 preselected_ids.len()
 };

 let preselected_str = if preselected_ids.is_empty() {
 demands.iter().map(|d| d.id.to_string()).collect::<Vec<_>>().join(",")
 } else {
 preselected_ids
 .iter()
 .map(|id| id.to_string())
 .collect::<Vec<_>>()
 .join(",")
 };

 html! {
 div {
 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 div {
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", MesDemandPoolListPath::PATH)) {
 (icon::arrow_left_icon("w-4 h-4"))
 "返回需求池"
 }
 h1 class="text-xl font-bold text-fg tracking-tight" { "从需求创建生产计划" }
 div class="text-[13px] text-muted mt-1" {
 span class="inline-flex items-center gap-[5px] rounded-full text-[11px] font-medium whitespace-nowrap bg-surface text-muted px-2 py-0.5 mr-1.5 bg-[#fef3c7] text-[#d97706]" {
 "生产需求池 · 按物料聚合"
 }
 "将生产需求池中的自制需求聚合为生产计划草稿"
 }
 }
 }

 form id="demand-create-form"
 hx-post=(MesDemandPoolCreatePath::PATH)
 hx-sync="this:drop"
 hx-swap="none" {
 input type="hidden" id="demand-ids-input" name="demand_ids" value=(preselected_str);
 input type="hidden" id="items-json-input" name="items_json";

 // ── Section 1: Plan Info ──
 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
 (icon::sliders_icon("w-[18px] h-[18px]"))
 "计划信息"
 }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "物料名称 " span class="text-danger" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-surface text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" readonly
 value=(product_name) {}
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "物料编码" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-surface text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] font-mono tabular-nums" type="text" readonly
 value=(product_code) {}
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "计划类型 " span class="text-danger" { "*" } }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="plan_type" required {
 option value="1" selected { "按单生产 (MTO)" }
 option value="2" { "按库存备货 (MTS)" }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "计划日期 " span class="text-danger" { "*" } }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="date" name="plan_date"
 value=(today) required {}
 }
 }
 }

 // ── Section 2: Default Scheduling Parameters ──
 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
 (icon::clock_icon("w-[18px] h-[18px]"))
 "默认排程参数"
 }
 div class="text-xs text-muted p-2 bg-surface-raised rounded-sm" {
 "以下参数将应用于所有未单独配置的需求行。可在需求明细中逐行修改排程日期。"
 }
 div class="grid grid-cols-4 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "默认排程开始" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="date"
 id="defaultStart"
 name="default_scheduled_start"
 value=(default_start) {}
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "默认排程结束" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="date"
 id="defaultEnd"
 name="default_scheduled_end"
 value=(default_end) {}
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "工作中心" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" disabled title="待 work_centers 主数据建成" {
 option value="" selected { "自动推断" }
 }
 }
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "优先级" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" id="defaultPriority" name="default_priority" {
 option value="2" selected { "普通 (2)" }
 option value="1" { "高 (1)" }
 option value="3" { "低 (3)" }
 }
 }
 }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6 mt-4" {
 div class="form-field" {
 label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "备注" }
 textarea class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="remark"
 placeholder="可选填写生产备注…"
 rows="1" {}
 }
 }
 }

 // ── Section 3: Demand Details ──
 div class="form-section" {
 div class="flex justify-between items-center mb-3" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg m-0 p-0 border-none" {
 (icon::clipboard_list_icon("w-[18px] h-[18px]"))
 "需求明细"
 @if let Some(pid) = product_id {
 span class="font-normal text-muted ml-2" {
 "(物料 ID: " (pid) ")"
 }
 }
 }
 button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs icon:w-4 icon:h-4" id="applyDefaultBtn" { "应用默认排程" }
 (PreEscaped(r#"<script>document.getElementById('applyDefaultBtn').addEventListener('click',function(){
 var start=document.getElementById('defaultStart').value;
 var end=document.getElementById('defaultEnd').value;
 document.querySelectorAll('#demand-tbody tr').forEach(function(row){
 var inputs=row.querySelectorAll('input[type=date]');
 if(inputs[0]&&start) inputs[0].value=start;
 if(inputs[1]&&end) inputs[1].value=end;
 });
 });</script>"#))
 }

 div class="overflow-x-auto" {
 table class="data-table" {
 thead {
 tr {
 th class="w-10" { input type="checkbox" id="checkAll" title="全选"; }
 th { "需求ID" }
 th { "来源订单" }
 th class="text-right text-[13px]" { "需求数量" }
 th { "需求日期" }
 th { "排程开始" }
 th { "排程结束" }
 th class="!text-right" { "操作" }
 }
 }
 tbody id="demand-tbody" {
 @for d in demands {
 (demand_row(d, preselected_ids))
 }
 @if demands.is_empty() {
 tr {
 td colspan="8" class="text-center text-muted py-8" {
 "暂无待处理需求"
 }
 }
 }
 }
 }
 }

 // ── Summary Bar ──
 div class="flex justify-end gap-8 p-5 border-t border-border-soft bg-surface-raised" {
 div class="flex gap-3" {
 span { "已选需求" }
 span class="font-mono tabular-nums font-semibold" {
 span id="selectedCount" { (selected_count) }
 " 条"
 }
 }
 div class="flex gap-3" {
 span { "总数量" }
 span class="font-mono tabular-nums font-semibold" {
 span id="totalQty" { (fmt_qty(total_qty)) }
 }
 }
 div class="flex gap-3" {
 span { "聚合方式" }
 span class="font-mono tabular-nums font-semibold" { "按物料" }
 }
 }
 }

 // ── Action Bar ──
 div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft" {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{}?restore=true", MesDemandPoolListPath::PATH)) { "取消" }
 div class="flex gap-3" {
 button type="submit" name="action" value="draft" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" {
 (icon::save_icon("w-4 h-4"))
 "保存草稿"
 }
 button type="submit" name="action" value="draft" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
 (icon::send_icon("w-4 h-4"))
 "创建草稿"
 }
 button type="submit" name="action" value="release" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)] bg-[linear-gradient(135deg,var(--accent),#6366f1)]"
 hx-confirm="创建后将自动确认并下达，生成工单（含工序、批次）。继续？"
 hx-disabled-elt="this" {
 (icon::rocket_icon("w-4 h-4"))
 "创建并下达"
 }
 }
 }
 }

 // ── Checkbox, Summary & Form Collection Scripts ──
 (PreEscaped(r#"<script>
 // Check-all checkbox in header
 var checkAllEl = document.getElementById('checkAll');
 if(checkAllEl){
 checkAllEl.addEventListener('change', function(){
 var checked = this.checked;
 document.querySelectorAll('#demand-tbody input[type=checkbox]').forEach(function(c){
 c.checked = checked;
 c.closest('tr').classList.toggle('demand-row-selected', checked);
 });
 updateDemandSummary();
 });
 }

 // Individual checkbox change
 document.addEventListener('change', function(e){
 if(e.target.type === 'checkbox' && e.target.closest('#demand-tbody')){
 e.target.closest('tr').classList.toggle('demand-row-selected', e.target.checked);
 updateDemandSummary();
 // Update check-all state
 var all = document.querySelectorAll('#demand-tbody input[type=checkbox]');
 var checkedOnes = document.querySelectorAll('#demand-tbody input[type=checkbox]:checked');
 var ca = document.getElementById('checkAll');
 if(ca){
 ca.checked = all.length > 0 && all.length === checkedOnes.length;
 }
 }
 });

 function updateDemandSummary(){
 var checked = document.querySelectorAll('#demand-tbody input[type=checkbox]:checked');
 var ids = [];
 var totalQty = 0;
 checked.forEach(function(c){
 ids.push(c.value);
 var qtyEl = c.closest('tr').querySelector('.demand-qty');
 if(qtyEl) totalQty += parseFloat(qtyEl.textContent.replace(/,/g,'')) || 0;
 });
 document.getElementById('selectedCount').textContent = checked.length;
 document.getElementById('totalQty').textContent = totalQty % 1 === 0 ? totalQty : totalQty.toFixed(2);
 document.getElementById('demand-ids-input').value = ids.join(',');
 }

 // Collect per-row scheduling items on form submit
 var dcf = document.getElementById('demand-create-form');
 if(dcf){
 dcf.addEventListener('submit', function(){
 var rows = document.querySelectorAll('#demand-tbody tr');
 var items = [];
 rows.forEach(function(row){
 var cb = row.querySelector('input[type=checkbox]');
 if(!cb || !cb.checked) return;
 var inputs = row.querySelectorAll('input[type=date]');
 var startVal = inputs[0] ? inputs[0].value : '';
 var endVal = inputs[1] ? inputs[1].value : '';
 var priEl = row.querySelector('.priority-val');
 if(startVal && endVal){
 items.push({
 demand_id: parseInt(cb.value),
 scheduled_start: startVal,
 scheduled_end: endVal,
 priority: priEl ? parseInt(priEl.textContent) : (parseInt((document.getElementById('defaultPriority')||{}).value) || 2)
 });
 }
 });
 document.getElementById('items-json-input').value = items.length > 0 ? JSON.stringify(items) : '';
 });
 }
 </script>"#))
 }
 }
}

// ── Components ──

fn demand_row(d: &DemandSummary, preselected_ids: &[i64]) -> Markup {
 let req_date = d
 .required_date
 .map(|dt| dt.format("%Y-%m-%d").to_string())
 .unwrap_or_else(|| "—".into());
 let is_checked = preselected_ids.is_empty() || preselected_ids.contains(&d.id);
 let is_pending = d.demand_status == 1;

 // Per-row default schedule dates
 let row_start = d
 .required_date
 .map(|dt| dt.format("%Y-%m-%d").to_string())
 .unwrap_or_default();
 let row_end = d
 .required_date
 .and_then(|dt| dt.checked_add_days(chrono::Days::new(7)))
 .map(|dt| dt.format("%Y-%m-%d").to_string())
 .unwrap_or_default();

 html! {
 tr class=@if is_checked { "demand-row-selected" } {
 td {
 @if is_pending {
 input type="checkbox" value=(d.id)
 checked[is_checked];
 span class="priority-val hidden" { (d.priority) }
 } @else {
 input type="checkbox" disabled;
 }
 }
 td class="font-mono tabular-nums text-xs" { (d.id) }
 td {
 a class="text-accent font-medium cursor-pointer" href=(OrderDetailPath { id: d.order_id }.to_string()) { (d.order_no.as_deref().unwrap_or("—")) }
 }
 td class="text-right text-[13px] font-mono tabular-nums demand-qty" { (fmt_qty(d.quantity)) }
 td class="font-mono tabular-nums" { (req_date) }
 td {
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] text-xs w-[130px] px-1.5 py-1" type="date"
 value=(row_start) {}
 }
 td {
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] text-xs w-[130px] px-1.5 py-1" type="date"
 value=(row_end) {}
 }
 td {
 button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center" title="移除" _="on click remove closest <tr/> then call updateDemandSummary()" {
 (icon::x_icon("w-3.5 h-3.5"))
 }
 }
 }
 }
}
