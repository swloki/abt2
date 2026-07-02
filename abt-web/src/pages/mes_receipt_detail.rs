use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::production_receipt::{
    model::{FqcGate, ProductionReceipt},
    ProductionReceiptService,
};
use abt_core::wms::warehouse::model::{Bin, Zone};
use abt_core::wms::warehouse::{WarehouseFilter, WarehouseService};

use crate::components::entity_picker::{self, EntityPickerConfig, EntityPickerItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_receipt::{
    ReceiptConfirmPath, ReceiptDetailPath, ReceiptListPath, ReceiptSearchWhPath, ReceiptWhZonesPath,
    ReceiptZnBinsPath,
};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

fn receipt_status_label(s: &abt_core::mes::enums::ReceiptStatus) -> (&'static str, &'static str) {
 match s {
 abt_core::mes::enums::ReceiptStatus::Draft => ("草稿", "status-draft"),
 abt_core::mes::enums::ReceiptStatus::Confirmed => ("已确认", "status-completed"),
 abt_core::mes::enums::ReceiptStatus::Cancelled => ("已取消", "status-cancelled"),
 }
}

fn fqc_badge(status: &FqcGate) -> Markup {
 let (label, class) = match status {
 FqcGate::NotRequired => ("无需 FQC", "fqc-badge--na"),
 FqcGate::PendingInspection => ("待 FQC", "fqc-badge--pending"),
 FqcGate::AllPassed => ("FQC 通过", "fqc-badge--passed"),
 FqcGate::HasFailed => ("FQC 不合格", "fqc-badge--failed"),
 };
 html! {
    span class=(format!("fqc-badge {}", class)) { (label) }
}
}

#[require_permission("WORK_ORDER", "read")]
pub async fn get_receipt_detail(path: ReceiptDetailPath, ctx: RequestContext) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.production_receipt_service();
 let receipt = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
 let lookups = svc.get_detail_lookups(&mut conn, &receipt).await?;
 let (sl, sc) = receipt_status_label(&receipt.status);

 let wo = lookups.wo_doc_number.as_deref().unwrap_or("—");
 let batch = lookups.batch_no.as_deref().unwrap_or("—");
 let product = lookups.product_name.as_deref().unwrap_or("—");
 let warehouse = lookups.warehouse_name.as_deref().unwrap_or("—");

 // FQC 状态
 let fqc_status = svc.get_fqc_status(&service_ctx, &mut conn, path.id).await.unwrap_or(FqcGate::NotRequired);

 // 单位成本
 let unit_cost = svc.get_unit_cost(&mut conn, receipt.product_id).await.unwrap_or(rust_decimal::Decimal::ZERO);
 let total_cost = receipt.received_qty * unit_cost;

 let wh_picker_config = EntityPickerConfig {
  modal_id: "wh-picker",
  title: "选择仓库",
  search_label: "仓库名称",
  search_placeholder: "输入仓库名搜索…",
  search_path: ReceiptSearchWhPath::PATH,
  search_param: "q",
  target_id: "warehouse_id",
  display_id: "wh-display",
  event_name: "whSelected",
  extra_include: None,
 };

 let content = html! {
    div {
        a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150 mb-4"
            href=(format!("{}?restore=true", ReceiptListPath::PATH))
        { "← 返回列表" }
        // 标题行：单号 + 状态 pill + FQC badge
        div class="flex items-center justify-between flex-wrap gap-3 mb-5" {
            h1 class="text-xl font-bold text-fg tracking-tight" {
                "入库单 " span class="font-mono" { (receipt.doc_number) }
            }
            div class="flex items-center gap-2" {
                span class=(format!("status-pill {}", crate::utils::status_color(sc))) { (sl) }
                (fqc_badge(&fqc_status))
            }
        }
        // 确认入库（仅 Draft：仓库指定目标库位后触发倒冲/成本/FQC）
        @if receipt.status == abt_core::mes::enums::ReceiptStatus::Draft {
            (confirm_card_inner(&receipt, &fqc_status))
        }
        // 基本信息（多列网格）
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
            div class="text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft" {
                "基本信息"
            }
            div class="grid grid-cols-2 lg:grid-cols-4 gap-5" {
                div {
                    div class="text-xs text-muted mb-1.5" { "单号" }
                    div class="text-sm text-fg font-mono tabular-nums" { (receipt.doc_number) }
                }
                div {
                    div class="text-xs text-muted mb-1.5" { "工单" }
                    div class="text-sm text-fg font-mono" { (wo) }
                }
                div {
                    div class="text-xs text-muted mb-1.5" { "批次" }
                    div class="text-sm text-fg font-mono" { (batch) }
                }
                div {
                    div class="text-xs text-muted mb-1.5" { "产品" }
                    div class="text-sm text-fg" { (product) }
                }
                div {
                    div class="text-xs text-muted mb-1.5" { "入库数量" }
                    div class="text-sm text-fg font-mono tabular-nums" {
                        (crate::utils::fmt_qty(receipt.received_qty))
                    }
                }
                div {
                    div class="text-xs text-muted mb-1.5" { "仓库" }
                    div class="text-sm text-fg" { (warehouse) }
                }
                div {
                    div class="text-xs text-muted mb-1.5" { "入库日期" }
                    div class="text-sm text-fg font-mono" { (receipt.receipt_date) }
                }
                div {
                    div class="text-xs text-muted mb-1.5" { "倒冲触发" }
                    div class="text-sm text-fg" {
                        (if receipt.backflush_triggered { "是" } else { "否" })
                    }
                }
                div {
                    div class="text-xs text-muted mb-1.5" { "创建时间" }
                    div class="text-sm text-fg font-mono" {
                        (receipt.created_at.format("%Y-%m-%d %H:%M"))
                    }
                }
                @if !receipt.remark.is_empty() {
                    div class="col-span-2 lg:col-span-4" {
                        div class="text-xs text-muted mb-1.5" { "备注" }
                        div class="text-sm text-fg-2" { (receipt.remark) }
                    }
                }
            }
        }
        // FQC 质检 + 成本明细（并排两列）
        div class="grid grid-cols-1 lg:grid-cols-2 gap-5 mb-5" {
            div class="bg-bg border border-border-soft rounded-md p-5 shadow-[var(--shadow-sm)]" {
                div class="text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "FQC 质检" }
                (fqc_badge(&fqc_status))
                @if matches!(fqc_status, FqcGate::PendingInspection) {
                    p class="text-xs text-warn mt-3" { "⚠ 尚无 FQC 检验记录，需完成 FQC 后才能确认入库" }
                } @else if matches!(fqc_status, FqcGate::HasFailed) {
                    p class="text-xs text-danger mt-3" { "⚠ FQC 有不合格项" }
                }
            }
            div class="bg-bg border border-border-soft rounded-md p-5 shadow-[var(--shadow-sm)]" {
                div class="text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "成本明细" }
                div class="grid grid-cols-2 gap-5" {
                    div {
                        div class="text-xs text-muted mb-1.5" { "单位成本" }
                        @if unit_cost > rust_decimal::Decimal::ZERO {
                            div class="text-sm text-fg font-mono tabular-nums" {
                                (crate::utils::fmt_amount(unit_cost))
                            }
                        } @else {
                            div class="text-sm text-muted" { "— 无历史成本" }
                        }
                    }
                    div {
                        div class="text-xs text-muted mb-1.5" { "总成本" }
                        div class="text-base text-fg font-mono tabular-nums font-semibold" {
                            (crate::utils::fmt_amount(total_cost))
                        }
                    }
                }
            }
        }
        // 仓库选择弹窗（确认入库时指定目标仓库）
        (entity_picker::entity_picker_modal(&wh_picker_config))
    }
};
 Ok(Html(admin_page(
 is_htmx, "入库详情", &claims, "production",
 &format!("/admin/mes/receipts/{}", path.id), "生产管理",
 Some(ReceiptListPath::PATH), content, &nav_filter,
 ).into_string()))
}

#[derive(Debug, Deserialize)]
pub struct ReceiptConfirmForm {
 pub warehouse_id: i64,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub zone_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub bin_id: Option<i64>,
}

#[require_permission("WORK_ORDER", "update")]
pub async fn confirm_receipt(
 path: ReceiptConfirmPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<ReceiptConfirmForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;
 let mut tx = state.pool.begin().await
 .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 state
     .production_receipt_service()
     .confirm(&service_ctx, &mut tx, path.receipt_id, form.warehouse_id, form.zone_id, form.bin_id)
     .await?;
 tx.commit().await
 .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 Ok(axum::response::Response::builder()
 .header("HX-Redirect", &format!("/admin/mes/receipts/{}", path.receipt_id))
 .body(axum::body::Body::empty()).unwrap())
}

// ── 仓库选择级联（确认入库指定目标仓库/库位；从 mes_receipt_create 迁入）──

#[derive(Debug, Deserialize)]
pub struct SearchParams {
 pub q: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WhZonesQuery {
 pub warehouse_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct ZnBinsQuery {
 pub zone_id: i64,
}

/// HTMX：搜索仓库
#[require_permission("WORK_ORDER", "read")]
pub async fn search_wh(
 _path: ReceiptSearchWhPath,
 ctx: RequestContext,
 Query(params): Query<SearchParams>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let wh_svc = state.warehouse_service();
 let kw = params.q.as_deref().unwrap_or("").trim().to_string();
 let filter = WarehouseFilter {
  keyword: if kw.is_empty() { None } else { Some(kw) },
  ..Default::default()
 };
 let warehouses = wh_svc
  .list(&service_ctx, &mut conn, filter, 1, 50)
  .await
  .map(|r| r.items)
  .unwrap_or_default();
 let items: Vec<EntityPickerItem> = warehouses
  .iter()
  .map(|wh| EntityPickerItem::new(wh.id, wh.name.clone()))
  .collect();
 Ok(Html(entity_picker::entity_picker_results(&items).into_string()))
}

/// HTMX：仓库选中后级联 — 返回库区下拉
#[require_permission("WORK_ORDER", "read")]
pub async fn get_wh_zones(
 _path: ReceiptWhZonesPath,
 ctx: RequestContext,
 Query(params): Query<WhZonesQuery>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let wh_svc = state.warehouse_service();
 let zones = wh_svc
  .list_zones(&service_ctx, &mut conn, params.warehouse_id)
  .await
  .unwrap_or_default();
 Ok(Html(zone_select_fragment(&zones).into_string()))
}

/// HTMX：库区选中后级联 — 返回库位下拉
#[require_permission("WORK_ORDER", "read")]
pub async fn get_zn_bins(
 _path: ReceiptZnBinsPath,
 ctx: RequestContext,
 Query(params): Query<ZnBinsQuery>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let wh_svc = state.warehouse_service();
 let bins = wh_svc
  .list_bins(&service_ctx, &mut conn, params.zone_id, None, 1, 200)
  .await
  .map(|r| r.items)
  .unwrap_or_default();
 Ok(Html(bin_select_fragment(&bins).into_string()))
}

/// 仓库选中后返回的库区下拉 + 库位占位
fn zone_select_fragment(zones: &[Zone]) -> Markup {
 html! {
  div id="zone-bin-area" {
   div class="grid grid-cols-2 gap-4 gap-x-6" {
    div class="form-field" {
     label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "库区" }
     @if zones.is_empty() {
      select
       class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
       name="zone_id"
       disabled
      {
       option value="" { "该仓库暂无库区" }
      }
      input type="hidden" name="zone_id" value="";
     } @else {
      select
       class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
       name="zone_id"
       hx-get=(ReceiptZnBinsPath::PATH)
       hx-target="#bin-select-wrap"
       hx-trigger="change"
       hx-swap="outerHTML"
       hx-include="this"
      {
       option value="" selected { "默认库区" }
       @for z in zones {
        option value=(z.id) { (z.name) }
       }
      }
     }
    }
    div class="form-field" id="bin-select-wrap" {
     label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "库位" }
     select
      class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
      name="bin_id"
      disabled
     {
      option value="" { "选择库区后加载" }
     }
    }
   }
  }
 }
}

/// 库区选中后返回的库位下拉
fn bin_select_fragment(bins: &[Bin]) -> Markup {
 html! {
  div class="form-field" id="bin-select-wrap" {
   label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "库位" }
   @if bins.is_empty() {
    select
     class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
     name="bin_id"
     disabled
    {
     option value="" { "该库区暂无库位" }
    }
   } @else {
    select
     class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
     name="bin_id"
    {
     option value="" selected { "自动分配" }
     @for b in bins {
      option value=(b.id) { (b.code) " " (b.name) }
     }
    }
   }
  }
 }
}

/// 确认入库卡片（仅 Draft）：仓库指定目标库位 + 确认按钮；受 FQC 门控
fn confirm_card_inner(receipt: &ProductionReceipt, fqc_status: &FqcGate) -> Markup {
 html! {
  div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
   div class="text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "确认入库" }
   @if matches!(fqc_status, FqcGate::AllPassed | FqcGate::NotRequired) {
    form hx-post=({ ReceiptConfirmPath { receipt_id: receipt.id }.to_string() }) hx-swap="none" {
     (entity_picker::entity_picker_field(
      "warehouse_id", "warehouse_id", "wh-display", "wh-picker",
      "目标仓库", true, "点击选择仓库…",
     ))
     div id="zone-bin-area"
      hx-get=(ReceiptWhZonesPath::PATH)
      hx-trigger="whSelected from:body"
      hx-target="this"
      hx-swap="outerHTML"
      hx-include="#warehouse_id"
     {
      div class="grid grid-cols-2 gap-4 gap-x-6" {
       div class="form-field" {
        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "库区" }
        select
         class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
         name="zone_id"
         disabled
        {
         option value="" { "选择仓库后加载" }
        }
       }
       div class="form-field" {
        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "库位" }
        select
         class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
         name="bin_id"
         disabled
        {
         option value="" { "选择库区后加载" }
        }
       }
      }
     }
     div class="flex justify-end" {
      button type="submit"
       class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
       hx-confirm="确认入库？将触发倒冲和成本结转。"
      { "确认入库" }
     }
    }
   } @else if matches!(fqc_status, FqcGate::PendingInspection) {
    p class="text-muted" { "⚠ 需完成 FQC 质检后才能确认入库" }
   } @else {
    p class="text-muted" { "⚠ FQC 有不合格项，暂无法入库" }
   }
  }
 }
}
