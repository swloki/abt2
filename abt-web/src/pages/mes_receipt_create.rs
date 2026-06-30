use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::enums::WorkOrderStatus;
use abt_core::mes::production_receipt::ProductionReceiptService;
use abt_core::mes::work_order::model::{WorkOrder, WorkOrderFilter};
use abt_core::mes::work_order::WorkOrderService;

use crate::components::icon;
use crate::components::entity_picker::{self, EntityPickerConfig, EntityPickerItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_receipt::{
 ReceiptCreatePath, ReceiptListPath, ReceiptSearchWoPath, ReceiptWoSelectedPath,
};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Query params ──

#[derive(Debug, Deserialize)]
pub struct SearchParams {
 pub q: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WoSelectedQuery {
 pub work_order_id: i64,
}


// ── Form ──

#[derive(Debug, Deserialize)]
pub struct ReceiptCreateForm {
 pub work_order_id: i64,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub product_id: Option<i64>,
 pub received_qty: rust_decimal::Decimal,
 pub receipt_date: chrono::NaiveDate,
 pub remark: Option<String>,
}

// ── GET /receipts/create ──

#[require_permission("WORK_ORDER", "create")]
pub async fn get_receipt_create(
 _path: ReceiptCreatePath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { claims, .. } = ctx;

 let content = receipt_create_content();
 Ok(Html(
 admin_page(
 is_htmx,
 "新建入库",
 &claims,
 "production",
 ReceiptCreatePath::PATH,
 "生产管理",
 Some(ReceiptListPath::PATH),
 content,
 &nav_filter,
 )
 .into_string(),
 ))
}

// ── HTMX: 搜索工单 ──

#[require_permission("WORK_ORDER", "read")]
pub async fn search_wo(
 _path: ReceiptSearchWoPath,
 ctx: RequestContext,
 Query(params): Query<SearchParams>,
) -> Result<Html<String>> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let wo_svc = state.work_order_service();
 let kw = params.q.as_deref().unwrap_or("").trim().to_string();

 let no_filter = WorkOrderFilter {
 status: None,
 product_id: None,
 keyword: None,
 date_from: None,
 date_to: None, product_code: None,
 work_center_id: None,
 };
 let mk_filter = |status: WorkOrderStatus, keyword: String| WorkOrderFilter {
 status: Some(status),
 keyword: if keyword.is_empty() { None } else { Some(keyword) },
 ..no_filter.clone()
 };

 let released = wo_svc
 .list(&service_ctx, &mut conn, mk_filter(WorkOrderStatus::Released, kw.clone()), 1, 50)
 .await
 .map(|r| r.items)
 .unwrap_or_default();
 let in_prod = wo_svc
 .list(&service_ctx, &mut conn, mk_filter(WorkOrderStatus::InProduction, kw), 1, 50)
 .await
 .map(|r| r.items)
 .unwrap_or_default();

 let work_orders: Vec<WorkOrder> = released.into_iter().chain(in_prod).collect();

 // 批量解析产品名
 let mut product_names: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
 let unique_pids: std::collections::HashSet<i64> =
 work_orders.iter().map(|wo| wo.product_id).collect();
 for pid in unique_pids {
 if let Ok(Some(name)) = wo_svc.get_product_name(&mut conn, pid).await {
 product_names.insert(pid, name);
 }
 }

 let items: Vec<EntityPickerItem> = work_orders
 .iter()
 .map(|wo| {
 let pname = product_names.get(&wo.product_id).map(|s| s.as_str()).unwrap_or("—");
 EntityPickerItem::new(wo.id, format!("{} · {}", wo.doc_number, pname))
 .sub(format!("计划数量 {} 件", crate::utils::fmt_qty(wo.planned_qty)))
 })
 .collect();

 Ok(Html(entity_picker::entity_picker_results(&items).into_string()))
}

// ── HTMX: 工单选中后级联 — 返回产品名 + 批次下拉 ──

#[require_permission("WORK_ORDER", "read")]
pub async fn wo_selected(
 _path: ReceiptWoSelectedPath,
 ctx: RequestContext,
 Query(params): Query<WoSelectedQuery>,
) -> Result<Html<String>> {
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 ..
 } = ctx;
 let wo_svc = state.work_order_service();

 let wo = wo_svc
 .find_by_id(&service_ctx, &mut conn, params.work_order_id)
 .await?;
 let product_name = wo_svc
 .get_product_name(&mut conn, wo.product_id)
 .await
 .unwrap_or(None)
 .unwrap_or_else(|| "—".into());

 Ok(Html(
 wo_cascade_fragment(wo.product_id, &product_name).into_string(),
 ))
}

// ── POST /receipts/create ──

#[require_permission("WORK_ORDER", "create")]
pub async fn create_receipt(
 _path: ReceiptCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<ReceiptCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext {
 state,
 service_ctx,
 ..
 } = ctx;
 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 let svc = state.production_receipt_service();
 let req = abt_core::mes::production_receipt::CreateReceiptReq {
 work_order_id: form.work_order_id,
 batch_id: None,
 product_id: form.product_id.unwrap_or(0),
 received_qty: form.received_qty,
 warehouse_id: None,
 zone_id: None,
 bin_id: None,
 receipt_date: form.receipt_date,
 remark: form.remark,
 };
 let _id = svc.create(&service_ctx, &mut tx, req).await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 Ok(axum::response::Response::builder()
 .header("HX-Redirect", ReceiptListPath::PATH)
 .body(axum::body::Body::empty())
 .unwrap())
}

// ── Page content ──

fn receipt_create_content() -> Markup {
 let today = chrono::Local::now().format("%Y-%m-%d").to_string();

 let wo_picker = EntityPickerConfig {
 modal_id: "wo-picker",
 title: "选择工单",
 search_label: "工单号 / 产品名",
 search_placeholder: "输入关键词搜索…",
 search_path: ReceiptSearchWoPath::PATH,
 search_param: "q",
 target_id: "work_order_id",
 display_id: "wo-display",
 event_name: "woSelected",
 extra_include: None,
 };

 html! {
    div {
        // ── Back Link ──
        a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150 mb-4"
            href=(format!("{}?restore=true", ReceiptListPath::PATH))
        { (icon::chevron_left_icon("w-4 h-4")) "返回完工入库列表" }
        // ── Page Header ──
        div class="flex items-center justify-between mb-5" {
            h1 class="text-xl font-bold text-fg tracking-tight" { "新建完工入库" }
        }

        form hx-post=(ReceiptCreatePath::PATH) hx-swap="none" id="receipt-form" {
            // ── 入库来源 ──
            div class="form-section" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft"
                { (icon::clipboard_document_icon("w-[18px] h-[18px]")) "入库来源" }

                ({
                    entity_picker::entity_picker_field(
                        "work_order_id",
                        "work_order_id",
                        "wo-display",
                        "wo-picker",
                        "工单号",
                        true,
                        "点击选择工单…",
                    )
                })
                // 工单选中后级联加载：产品名
                div id="wo-cascade"
                    hx-get=(ReceiptWoSelectedPath::PATH)
                    hx-trigger="woSelected from:body"
                    hx-target="this"
                    hx-swap="outerHTML"
                    hx-include="#work_order_id"
                {
                    div class="grid grid-cols-2 gap-4 gap-x-6" {
                        div class="form-field" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            { "产品" }
                            div class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-surface text-muted outline-none"
                            { "选择工单后自动填充" }
                        }
                    }
                }
            }
            // ── 入库明细 ──
            div class="form-section" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft"
                { (icon::box_icon("w-[18px] h-[18px]")) "入库明细" }
                div class="grid grid-cols-2 gap-4 gap-x-6" {
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "入库数量 "
                            span class="required" { "*" }
                        }
                        input
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            type="number"
                            step="any"
                            name="received_qty"
                            required
                            placeholder="0";
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "入库日期 "
                            span class="required" { "*" }
                        }
                        input
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            type="date"
                            name="receipt_date"
                            value=(today)
                            required;
                    }
                }
            }
            // ── 备注 ──
            div class="form-section" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft"
                { (icon::comment_icon("w-[18px] h-[18px]")) "备注" }
                div class="form-field col-span-2" {
                    textarea
                        class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent resize-y"
                        name="remark"
                        rows="2"
                        placeholder="可选备注…" {}
                }
            }
            // ── Action Bar ──
            div class="sticky bottom-0 flex items-center justify-between gap-3 px-6 py-4 bg-bg border-t border-border-soft"
            {
                div {}
                div class="flex gap-3" {
                    a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                        href=(format!("{}?restore=true", ReceiptListPath::PATH))
                    { "取消" }
                    button
                        type="submit"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                    { (icon::check_circle_icon("w-4 h-4")) "提交入库" }
                }
            }
        }
        // ── 弹窗 ──
        (entity_picker::entity_picker_modal(&wo_picker))
    }
}
}

// ── HTMX fragments ──

/// 工单选中后返回的产品信息片段
fn wo_cascade_fragment(product_id: i64, product_name: &str) -> Markup {
 html! {
    div id="wo-cascade" {
        div class="grid grid-cols-2 gap-4 gap-x-6" {
            // 产品（只读 + 隐藏 ID）
            div class="form-field" {
                label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "产品" }
                input
                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-surface text-muted outline-none"
                    type="text"
                    value=(product_name)
                    disabled;
                input type="hidden" name="product_id" value=(product_id);
            }
        }
    }
}
}

