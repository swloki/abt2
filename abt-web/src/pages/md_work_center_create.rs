use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;
use serde::Deserialize;

use abt_core::master_data::work_center::{model::*, WorkCenterService};
use abt_core::shared::types::DomainError;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::md_work_center::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form ──

#[derive(Debug, Deserialize)]
pub struct WorkCenterForm {
 pub code: String,
 pub name: String,
 pub work_center_type: String,
 pub costs_hour: String,
 pub time_efficiency: String,
 pub setup_time: String,
 pub cleanup_time: String,
 pub default_capacity: String,
 pub location: Option<String>,
}

// ── Create Handler ──

#[require_permission("BOM", "create")]
pub async fn get_work_center_create(
 _path: WorkCenterCreatePath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, .. } = ctx;

 let content = work_center_form_page(None);
 Ok(Html(
 admin_page(
 is_htmx,
 "新建工作中心",
 &claims,
 "md",
 WorkCenterCreatePath::PATH,
 "工程",
 Some("新建工作中心"),
 content,
 &nav_filter,
 )
 .into_string(),
 ))
}

#[require_permission("BOM", "create")]
pub async fn post_work_center_create(
 _path: WorkCenterCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<WorkCenterForm>,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;

 let req = parse_form(&form)?;
 let id = state
 .work_center_service()
 .create(&service_ctx, &mut conn, req)
 .await?;

 let redirect = WorkCenterDetailPath { id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Edit Handler ──

#[require_permission("BOM", "update")]
pub async fn get_work_center_edit(
 path: WorkCenterEditPath,
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

 let wc = state
 .work_center_service()
 .get(&service_ctx, &mut conn, path.id)
 .await?;

 let content = work_center_form_page(Some(&wc));
 Ok(Html(
 admin_page(
 is_htmx,
 "编辑工作中心",
 &claims,
 "md",
 &WorkCenterEditPath { id: path.id }.to_string(),
 "工程",
 Some("编辑工作中心"),
 content,
 &nav_filter,
 )
 .into_string(),
 ))
}

#[require_permission("BOM", "update")]
pub async fn post_work_center_update(
 path: WorkCenterEditPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<WorkCenterForm>,
) -> Result<impl IntoResponse> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;

 let req = parse_update_form(&form)?;
 state
 .work_center_service()
 .update(&service_ctx, &mut conn, path.id, req)
 .await?;

 let redirect = WorkCenterDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn work_center_form_page(wc: Option<&WorkCenter>) -> Markup {
 let is_edit = wc.is_some();
 html! {
 div class="flex items-center justify-between mb-6" {
 div class="flex items-center justify-between mb-6" {
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(WorkCenterListPath::PATH) { "← 返回列表" }
 h1 class="text-xl font-bold text-fg tracking-tight" {
 @if is_edit { "编辑工作中心" } @else { "新建工作中心" }
 }
 }
 }

 form class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)] form-card"
 hx-post={ @if is_edit {
 (WorkCenterEditPath { id: wc.unwrap().id }.to_string())
 } @else {
 (WorkCenterCreatePath::PATH)
 }} {

 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "基本信息" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label { "编码 *" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" type="text" name="code" required
 value=(wc.map(|w| w.code.as_str()).unwrap_or(""))
 disabled[is_edit];
 @if is_edit {
 input type="hidden" name="code"
 value=(wc.map(|w| w.code.as_str()).unwrap_or(""));
 }
 }
 div class="form-field" {
 label { "名称 *" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" type="text" name="name" required
 value=(wc.map(|w| w.name.as_str()).unwrap_or(""));
 }
 div class="form-field" {
 label { "类型" }
 select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" name="work_center_type" {
 @for (val, label) in [("1", "机器"), ("2", "人工"), ("3", "委外")] {
 option value=(val)
 selected=(wc.map(|w| w.work_center_type.to_string()).as_deref() == Some(val)) {
 (label)
 }
 }
 }
 }
 div class="form-field" {
 label { "位置" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" type="text" name="location"
 value=(wc.and_then(|w| w.location.as_deref()).unwrap_or(""));
 }
 }
 }

 div class="form-section" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "产能与成本" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label { "产能/小时" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" type="number" step="0.01" name="default_capacity"
 value=(wc.map(|w| crate::utils::fmt_qty(w.default_capacity)).unwrap_or_else(|| "0".into()));
 }
 div class="form-field" {
 label { "成本费率/小时 (¥)" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" type="number" step="0.01" name="costs_hour"
 value=(wc.map(|w| crate::utils::fmt_qty(w.costs_hour)).unwrap_or_else(|| "0".into()));
 }
 div class="form-field" {
 label { "效率系数" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" type="number" step="0.01" name="time_efficiency"
 value=(wc.map(|w| crate::utils::fmt_qty(w.time_efficiency)).unwrap_or_else(|| "1".into()));
 }
 div class="form-field" {
 label { "准备时间 (分钟)" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" type="number" step="0.01" name="setup_time"
 value=(wc.map(|w| crate::utils::fmt_qty(w.setup_time)).unwrap_or_else(|| "0".into()));
 }
 div class="form-field" {
 label { "清理时间 (分钟)" }
 input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent" type="number" step="0.01" name="cleanup_time"
 value=(wc.map(|w| crate::utils::fmt_qty(w.cleanup_time)).unwrap_or_else(|| "0".into()));
 }
 }
 }

 div class="flex items-center justify-end gap-3 pt-4 border-t border-border-soft" {
 a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(WorkCenterListPath::PATH) { "取消" }
 button class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" type="submit" {
 (icon::check_circle_icon("w-4 h-4"))
 @if is_edit { "保存" } @else { "创建" }
 }
 }
 }
 }
}

// ── Parsers ──

/// 两套请求共有的表单字段解析结果（成本/产能/效率/类型/位置）
struct ParsedFields {
 work_center_type: i16,
 costs_hour: Decimal,
 time_efficiency: Decimal,
 setup_time: Decimal,
 cleanup_time: Decimal,
 default_capacity: Decimal,
 location: Option<String>,
}

fn parse_common(form: &WorkCenterForm) -> Result<ParsedFields> {
 Ok(ParsedFields {
 work_center_type: form
 .work_center_type
 .parse()
 .map_err(|_| DomainError::validation("工作中心类型错误"))?,
 costs_hour: form
 .costs_hour
 .parse()
 .map_err(|_| DomainError::validation("成本费率格式错误"))?,
 time_efficiency: form
 .time_efficiency
 .parse()
 .map_err(|_| DomainError::validation("效率系数格式错误"))?,
 setup_time: form
 .setup_time
 .parse()
 .map_err(|_| DomainError::validation("准备时间格式错误"))?,
 cleanup_time: form
 .cleanup_time
 .parse()
 .map_err(|_| DomainError::validation("清理时间格式错误"))?,
 default_capacity: form
 .default_capacity
 .parse()
 .map_err(|_| DomainError::validation("产能格式错误"))?,
 location: form
 .location
 .as_deref()
 .filter(|s| !s.trim().is_empty())
 .map(|s| s.to_string()),
 })
}

fn parse_form(form: &WorkCenterForm) -> Result<CreateWorkCenterReq> {
 let f = parse_common(form)?;
 Ok(CreateWorkCenterReq {
 code: form.code.trim().to_string(),
 name: form.name.trim().to_string(),
 work_center_type: f.work_center_type,
 costs_hour: f.costs_hour,
 time_efficiency: f.time_efficiency,
 setup_time: f.setup_time,
 cleanup_time: f.cleanup_time,
 default_capacity: f.default_capacity,
 calendar_id: None,
 location: f.location,
 })
}

fn parse_update_form(form: &WorkCenterForm) -> Result<UpdateWorkCenterReq> {
 let f = parse_common(form)?;
 Ok(UpdateWorkCenterReq {
 name: Some(form.name.trim().to_string()),
 work_center_type: Some(f.work_center_type),
 costs_hour: Some(f.costs_hour),
 time_efficiency: Some(f.time_efficiency),
 setup_time: Some(f.setup_time),
 cleanup_time: Some(f.cleanup_time),
 default_capacity: Some(f.default_capacity),
 calendar_id: None,
 location: f.location,
 is_active: None,
 })
}
