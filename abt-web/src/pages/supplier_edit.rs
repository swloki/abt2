use axum::response::{Html, IntoResponse};
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::master_data::supplier::model::{SupplierCategory, SupplierStatus, UpdateSupplierReq};
use abt_core::master_data::supplier::SupplierService;
use abt_macros::require_permission;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::supplier::{SupplierDetailPath, SupplierEditPath};
use crate::utils::RequestContext;

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub struct SupplierEditForm {
 pub name: String,
 pub short_name: Option<String>,
 pub category: i16,
 pub status: i16,
 pub tax_number: Option<String>,
 pub lead_time_days: Option<i32>,
 pub payment_terms: Option<String>,
 pub currency: Option<String>,
 pub remark: Option<String>,
}

// ── Handlers ──

#[require_permission("SUPPLIER", "update")]
pub async fn get_supplier_edit(
 path: SupplierEditPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.supplier_service();
 let supplier = svc.get(&service_ctx, &mut conn, path.id).await?;

 let content = supplier_edit_page(&supplier);
 let edit_path_str = SupplierEditPath { id: path.id }.to_string();
 let page_html = admin_page(
 is_htmx,
 &format!("编辑 {}", supplier.name),
 &claims,
 "purchase",
 &edit_path_str,
 "主数据管理",
 Some(&format!("编辑 {}", supplier.name)),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

#[require_permission("SUPPLIER", "update")]
pub async fn post_supplier_edit(
 path: SupplierEditPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<SupplierEditForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.supplier_service();

 let category = SupplierCategory::from_i16(form.category)
 .ok_or_else(|| abt_core::shared::types::DomainError::validation("无效的供应类别"))?;
 let status = SupplierStatus::from_i16(form.status)
 .ok_or_else(|| abt_core::shared::types::DomainError::validation("无效的供应商状态"))?;

 let req = UpdateSupplierReq {
 supplier_name: Some(form.name),
 short_name: form.short_name.filter(|s| !s.is_empty()),
 category: Some(category),
 status: Some(status),
 tax_number: form.tax_number.filter(|s| !s.is_empty()),
 lead_time_days: form.lead_time_days,
 payment_terms: form.payment_terms.filter(|s| !s.is_empty()),
 remark: form.remark.filter(|s| !s.is_empty()),
 currency: form.currency.filter(|s| !s.is_empty()),
 };
 svc.update(&service_ctx, &mut conn, path.id, req).await?;

 let redirect = SupplierDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn supplier_edit_page(supplier: &abt_core::master_data::supplier::model::Supplier) -> Markup {
 let edit_path = SupplierEditPath { id: supplier.id };
 let detail_path = SupplierDetailPath { id: supplier.id };

 let category_val = supplier.category.as_i16();
 let status_val = supplier.status.as_i16();
 let lead_time = if supplier.lead_time_days > 0 {
 supplier.lead_time_days.to_string()
 } else {
 String::new()
 };

 html! {
 div {
 // ── Page Header ──
 div class="flex items-center justify-between mb-6" {
 a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(detail_path) {
 (icon::arrow_left_icon("w-4 h-4"))
 "返回供应商详情"
 }
 h1 class="text-xl font-bold text-fg tracking-tight" { "编辑供应商" }
 }

 form id="supplier-form"
 hx-post=(edit_path)
 hx-swap="none" {

 // ── Section: 基本信息 ──
 div class="data-card" style="margin-bottom:var(--space-4)" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" { "基本信息" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field" {
 label { "供应商编码" }
 input type="text" value=(supplier.code) readonly
 style="background:var(--surface);color:var(--muted)" {}
 }
 div class="form-field" {
 label { "供应商名称 " span style="color:var(--danger)" { "*" } }
 input type="text" name="name" required value=(supplier.name) {}
 }
 div class="form-field" {
 label { "简称" }
 input type="text" name="short_name" placeholder="请输入简称"
 value=(supplier.short_name.as_deref().unwrap_or("")) {}
 }
 div class="form-field" {
 label { "供应类别 " span style="color:var(--danger)" { "*" } }
 select name="category" required {
 option value="1" selected[attr_selected(category_val, 1)] { "原材料" }
 option value="2" selected[attr_selected(category_val, 2)] { "包装材料" }
 option value="3" selected[attr_selected(category_val, 3)] { "外协加工" }
 option value="4" selected[attr_selected(category_val, 4)] { "辅料耗材" }
 option value="5" selected[attr_selected(category_val, 5)] { "服务" }
 }
 }
 div class="form-field" {
 label { "状态 " span style="color:var(--danger)" { "*" } }
 select name="status" required {
 option value="1" selected[attr_selected(status_val, 1)] { "潜在" }
 option value="2" selected[attr_selected(status_val, 2)] { "合格" }
 option value="3" selected[attr_selected(status_val, 3)] { "试用期" }
 option value="4" selected[attr_selected(status_val, 4)] { "不合格" }
 option value="5" selected[attr_selected(status_val, 5)] { "黑名单" }
 }
 }
 div class="form-field" {
 label { "统一社会信用代码" }
 input type="text" name="tax_number" placeholder="请输入统一社会信用代码"
 value=(supplier.tax_number.as_deref().unwrap_or("")) {}
 }
 div class="form-field" {
 label { "交货天数 " span style="color:var(--danger)" { "*" } }
 input type="number" name="lead_time_days" required min="0" placeholder="请输入交货天数"
 value=(lead_time) {}
 }
 div class="form-field" {
 label { "付款条件" }
 select name="payment_terms" {
 option value="" { "-- 请选择 --" }
 option value="30天净额" selected[attr_selected_str(supplier.payment_terms.as_deref(), "30天净额")] { "30天净额" }
 option value="60天净额" selected[attr_selected_str(supplier.payment_terms.as_deref(), "60天净额")] { "60天净额" }
 option value="预付30%" selected[attr_selected_str(supplier.payment_terms.as_deref(), "预付30%")] { "预付30%" }
 option value="货到付款" selected[attr_selected_str(supplier.payment_terms.as_deref(), "货到付款")] { "货到付款" }
 option value="月结30天" selected[attr_selected_str(supplier.payment_terms.as_deref(), "月结30天")] { "月结30天" }
 option value="月结60天" selected[attr_selected_str(supplier.payment_terms.as_deref(), "月结60天")] { "月结60天" }
 option value="月结90天" selected[attr_selected_str(supplier.payment_terms.as_deref(), "月结90天")] { "月结90天" }
 }
 }
 div class="form-field" {
 label { "结算货币" }
 select name="currency" {
 option value="CNY" selected[attr_selected_str(Some(&supplier.currency), "CNY")] { "CNY - 人民币" }
 option value="USD" selected[attr_selected_str(Some(&supplier.currency), "USD")] { "USD - 美元" }
 option value="JPY" selected[attr_selected_str(Some(&supplier.currency), "JPY")] { "JPY - 日元" }
 option value="AUD" selected[attr_selected_str(Some(&supplier.currency), "AUD")] { "AUD - 澳元" }
 option value="EUR" selected[attr_selected_str(Some(&supplier.currency), "EUR")] { "EUR - 欧元" }
 }
 }
 }
 }

 // ── Section: 其他 ──
 div class="data-card" style="margin-bottom:var(--space-4)" {
 div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 [border-bottom:1px_solid_var(--border-soft)] border-border-soft" { "其他" }
 div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
 div class="form-field field-full" {
 label { "备注" }
 textarea name="remark" placeholder="请输入备注信息…"
 style="width:100%;min-height:80px;resize:vertical" {
 (supplier.remark)
 }
 }
 }
 }

 // ── Action Bar ──
 div class="flex items-center justify-end gap-3 pt-4 [border-top:1px_solid_var(--border-soft)]" {
 a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(detail_path) { "取消" }
 button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" {
 "保存修改"
 }
 }
 }
 }
 }
}

fn attr_selected(val: i16, target: i16) -> bool {
 val == target
}

fn attr_selected_str(val: Option<&str>, target: &str) -> bool {
 val == Some(target)
}
