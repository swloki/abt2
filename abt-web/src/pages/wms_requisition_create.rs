use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use chrono::Local;
use maud::{html, Markup};
use serde::Deserialize;
use rust_decimal::Decimal;

use abt_core::master_data::product::ProductService;
use abt_core::shared::types::DomainError;
use abt_core::wms::inventory_transaction::InventoryTransactionService;
use abt_core::wms::picking::{CreateManualReq, CreateManualItemReq, PickingService};
use abt_core::wms::warehouse::model::{Warehouse, WarehouseFilter};
use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::components::work_order_picker::work_order_picker_modal;
use crate::components::bin_search::{bin_picker_modal, warehouse_bin_cell};
use crate::components::product_picker::product_picker_modal_with_search;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_requisition::*;
use crate::routes::wms_work_center::WmsWorkCenterPath;
use crate::utils::{RequestContext, empty_as_none, fmt_qty};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize)]
pub struct ItemRowParams {
    pub product_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct WoItemsQuery {
    pub work_order_id: i64,
    #[serde(default)]
    pub warehouse_id: i64,
}

// ── Handlers ──

#[require_permission("INVENTORY", "create")]
pub async fn get_requisition_create(
    _path: RequisitionCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let warehouses = state
        .warehouse_service()
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
        .await?;
    let content = requisition_create_page(
        &warehouses.items,
        &claims.display_name,
        RequisitionCreatePath::PATH,
        "",
        true,
        true,
    );
    let page_html = admin_page(
        is_htmx,
        "新建领料单",
        &claims,
        "inventory",
        RequisitionCreatePath::PATH,
        "库存管理",
        Some("新建领料单"),
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

/// HTMX: 手动加一行物料（product_picker 选中后）
#[require_permission("INVENTORY", "create")]
pub async fn get_item_row(
    ctx: RequestContext,
    Query(params): Query<ItemRowParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let product = state.product_service().get(&service_ctx, &mut conn, params.product_id).await?;
    let warehouses = state
        .warehouse_service()
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
        .await?;
    Ok(Html(item_row_fragment(&product, &warehouses.items).into_string()))
}

/// HTMX: 选工单后加载 BOM 明细行（BOM 需求量 + 已领 + 可用量 + 仓库/库位 cell + 批次）。
/// 响应头 HX-Trigger-After-Settle: woItemsLoaded（前端监听 → 重编号 + 汇总）。
#[require_permission("INVENTORY", "create")]
pub async fn get_requisition_wo_items(
    ctx: RequestContext,
    Query(params): Query<WoItemsQuery>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let pick_svc = state.picking_service();
    let preview = pick_svc
        .list_wo_requisition_preview(&service_ctx, &mut conn, params.work_order_id)
        .await?;
    let product_ids: Vec<i64> = preview.iter().map(|p| p.product_id).collect();
    let products = state
        .product_service()
        .get_by_ids(&service_ctx, &mut conn, product_ids.clone())
        .await
        .unwrap_or_default();
    let product_map: std::collections::HashMap<i64, abt_core::master_data::product::model::Product> =
        products.into_iter().map(|p| (p.product_id, p)).collect();
    let warehouses = state
        .warehouse_service()
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
        .await?;
    // 可用量（基于所选仓库；未选仓库则空 —— 前端选仓库后可重新触发）
    let avail_map = if params.warehouse_id > 0 {
        state
            .inventory_transaction_service()
            .query_available_batch(&service_ctx, &mut conn, &product_ids, Some(params.warehouse_id))
            .await
            .unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };
    let html = requisition_wo_item_rows(
        &preview,
        &product_map,
        &avail_map,
        &warehouses.items,
        params.warehouse_id,
    )
    .into_string();
    Ok(([("HX-Trigger-After-Settle", r#"{"woItemsLoaded":""}"#)], Html(html)))
}

// ── Form Data ──

#[derive(Debug, Deserialize)]
struct RequisitionItemWeb {
    product_id: String,
    requested_qty: String,
    #[serde(default, deserialize_with = "empty_as_none")]
    warehouse_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    bin_id: Option<i64>,
    #[serde(default)]
    batch_no: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RequisitionCreateForm {
    #[serde(default, deserialize_with = "empty_as_none")]
    pub work_order_id: Option<i64>,
    pub requisition_date: String,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub remark: Option<String>,
    pub items_json: String,
}

/// 业务逻辑（独立页 POST 与作业中心 drawer POST 共用）。
/// 工单模式（work_order_id=Some）与手动模式统一走 create_manual（带工单关联 + 行级 bin/batch）。
pub async fn do_create_requisition(
    state: &crate::state::AppState,
    service_ctx: &abt_core::shared::types::ServiceContext,
    form: RequisitionCreateForm,
) -> Result<()> {
    let svc = state.picking_service();
    let requisition_date = chrono::NaiveDate::parse_from_str(&form.requisition_date, "%Y-%m-%d")
        .map_err(|e| DomainError::validation(format!("Invalid date: {e}")))?;
    let web_items: Vec<RequisitionItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("无效明细数据: {e}")))?;
    if web_items.is_empty() {
        return Err(DomainError::validation("请至少添加一条领料明细").into());
    }
    // 统一领料仓：取首行 warehouse_id（行内 bin cell 的 hidden；对齐 receive_purchase 头仓范式）
    let warehouse_id = web_items
        .iter()
        .find_map(|it| it.warehouse_id)
        .ok_or_else(|| DomainError::validation("请选择领料仓库（行内仓库/库位）"))?;
    let items: Vec<CreateManualItemReq> = web_items
        .into_iter()
        .map(|it| CreateManualItemReq {
            product_id: it.product_id.parse().unwrap_or(0),
            requested_qty: it.requested_qty.parse().unwrap_or(Decimal::ZERO),
            bin_id: it.bin_id,
            batch_no: it.batch_no.filter(|s| !s.is_empty()),
        })
        .collect();
    let req = CreateManualReq {
        warehouse_id,
        requisition_date,
        work_order_id: form.work_order_id,
        remark: form.remark.filter(|s| !s.is_empty()),
        items,
    };
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    svc.create_manual(service_ctx, &mut tx, req).await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    Ok(())
}

#[require_permission("INVENTORY", "create")]
pub async fn create_requisition(
    _path: RequisitionCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<RequisitionCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    do_create_requisition(&state, &service_ctx, form).await?;
    let redirect = format!("{}?domain=requisition&view=all", WmsWorkCenterPath::PATH);
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

pub fn requisition_create_page(
    warehouses: &[Warehouse],
    operator_name: &str,
    post_path: &str,
    after_request_hs: &str,
    show_header: bool,
    with_picker: bool,
) -> Markup {
    html! {
        div {
            @if show_header {
                a href=(format!("{}?domain=requisition&view=all", WmsWorkCenterPath::PATH))
                    class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150 mb-4"
                { (icon::chevron_left_icon("w-4 h-4")) "返回作业中心" }
                div class="flex items-center justify-between mb-5" {
                    h1 class="text-xl font-bold text-fg tracking-tight" { "新建领料单" }
                }
            }
            form
                hx-post=(post_path)
                hx-swap="none"
                id="requisitionForm"
                onsubmit="return reqCollectItems()"
                _=(after_request_hs)
            {
                // ── 工单信息 ──
                div class="form-section" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft"
                    { (icon::clipboard_document_icon("w-[18px] h-[18px]")) "工单信息" }
                    div class="grid grid-cols-2 gap-4 gap-x-6" {
                        // 关联工单（picker：选工单 → 填 hidden + trigger change → hx-get 加载 BOM 行）
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "关联工单" }
                            div class="flex gap-2" {
                                input type="hidden" id="req-wo-id" name="work_order_id"
                                    hx-get=(RequisitionWoItemsPath::PATH)
                                    hx-trigger="change"
                                    hx-target="#req-item-tbody"
                                    hx-swap="innerHTML"
                                    hx-vals=(r#"js:{work_order_id: me.value, warehouse_id: document.getElementById('req-warehouse').value}"#);
                                input type="text" id="req-wo-display"
                                    class="flex-1 px-3 py-2 border border-border rounded-sm text-sm bg-surface text-fg-2 outline-none"
                                    readonly placeholder="留空为手动创建";
                                button type="button"
                                    class="px-3 py-2 border border-border rounded-sm text-sm text-fg-2 hover:border-accent hover:text-accent cursor-pointer"
                                    _="on click add .is-open to #wo-modal" { "选择" }
                                button type="button" title="清除"
                                    class="px-2 py-2 border border-border rounded-sm text-muted hover:text-danger cursor-pointer"
                                    _="on click set #req-wo-id's value to '' then set #req-wo-display's value to '' then trigger change on #req-wo-id"
                                { (icon::x_icon("w-3.5 h-3.5")) }
                            }
                        }
                        // 领料仓库（统一仓：change 批量应用各行 bin cell，复用 app.js wcApplyWarehouseAll）
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                                "领料仓库 " span class="required" { "*" }
                            }
                            select id="req-warehouse"
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                                _="on change call wcApplyWarehouseAll(me)"
                            {
                                option value="" { "请选择仓库" }
                                @for w in warehouses {
                                    option value=(w.id) { (w.name) }
                                }
                            }
                        }
                        // 领料日期
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                                "领料日期 " span class="required" { "*" }
                            }
                            input type="date" name="requisition_date" required
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                                value=(Local::now().format("%Y-%m-%d")) {}
                        }
                        // 操作员（真实登录用户，只读展示，operator_id 由 ctx.operator_id 落地）
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "操作员" }
                            input type="text" readonly value=(operator_name)
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm text-fg-2 bg-surface outline-none" {}
                        }
                    }
                }
                // ── 领料明细 ──
                div class="form-section p-0 overflow-hidden" {
                    div class="px-6 pt-6 pb-4" {
                        div class="flex items-center gap-2 text-sm font-semibold text-fg mb-3" {
                            (icon::box_icon("w-[18px] h-[18px]")) "领料明细"
                            span id="req-item-count" class="ml-auto text-xs font-normal text-muted" { "共 0 项" }
                        }
                    }
                    div class="overflow-x-auto" {
                        table class="data-table min-w-[1100px]" {
                            thead {
                                tr {
                                    th class="w-10 text-center" { "行号" }
                                    th class="min-w-[180px]" { "产品" }
                                    th class="w-14" { "单位" }
                                    th class="w-[90px] text-right" { "需求量" }
                                    th class="w-[90px] text-right" { "已领" }
                                    th class="w-[90px] text-right" { "可用量" }
                                    th class="w-[180px]" { "仓库 / 库位" }
                                    th class="w-[110px]" { "批次" }
                                    th class="w-[110px] text-right" {
                                        "本次领料 " span class="required" { "*" }
                                    }
                                    th class="w-10" {}
                                }
                            }
                            tbody id="req-item-tbody" {}
                        }
                    }
                    div class="p-4" {
                        button type="button"
                            class="flex items-center justify-center gap-2 w-full text-accent text-sm font-medium cursor-pointer"
                            _="on click add .is-open to #product-modal"
                        { (icon::plus_icon("w-3.5 h-3.5")) "添加物料" }
                    }
                }
                // ── 备注 ──
                div class="form-section" {
                    label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "备注" }
                    textarea name="remark" rows="2"
                        class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent resize-y min-h-[44px]"
                        placeholder="领料用途 / 备注…" {}
                }
                input type="hidden" name="items_json" id="req-items-json" value="[]" {}
                // ── Action Bar ──
                div class="sticky bottom-0 flex items-center justify-between gap-3 px-6 py-4 bg-bg border-t border-border-soft"
                {
                    div {}
                    div class="flex gap-3" {
                        @if show_header {
                            a href=(format!("{}?domain=requisition&view=all", WmsWorkCenterPath::PATH))
                                class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                            { "取消" }
                        } @else {
                            button type="button"
                                class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                                _="on click remove .open from closest .drawer-overlay"
                            { "取消" }
                        }
                        button type="submit"
                            class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                        { (icon::check_circle_icon("w-4 h-4")) "提交领料单" }
                    }
                }
            }
            // ── Modals ──
            (work_order_picker_modal("wo-modal", "req-wo-id", "req-wo-display"))
            (product_picker_modal_with_search("product-modal", RequisitionItemRowPath::PATH, "req-item-tbody"))
            @if with_picker {
                (bin_picker_modal("bin-picker-modal", warehouses))
            }
            // ── JS（宿主页预载：drawer body innerHTML swap 不执行 script[src]）──
            script src="/requisition-create.js" {}
            // 提交成功关 drawer：form 的 hyperscript `on 'htmx:afterRequest'` 对 swap 进来的元素
            // 监听不可靠 + 该事件 bubbles=false（document 级监听收不到），用内联 script 直接在 form
            // 上绑定（PreEscaped，drawer body swap 后执行，form 已在 DOM，target=form 必收到）。
            ({
                maud::PreEscaped(
                    r#"<script>
 (function(){
   var f = document.getElementById('requisitionForm');
   if (!f) return;
   f.addEventListener('htmx:afterRequest', function(e){
     var xhr = e.detail && e.detail.xhr;
     if (xhr && xhr.status < 400 && (xhr.responseText || '').length === 0) {
       var o = document.getElementById('wc-requisition-create-overlay');
       if (o) o.classList.remove('open');
     }
   });
   // form 提交的 afterSettle 冒泡到 overlay 会命中 overlay_hs 的 `add .open`（其守卫
   // `me is event.target` 在 hyperscript 里恒真）误重开 drawer，阻止冒泡即可。
   f.addEventListener('htmx:afterSettle', function(e){ e.stopPropagation(); });
 })();
 </script>"#,
                )
            })
        }
    }
}

// ── Row Fragments ──

/// 工单模式：BOM 明细整组行（填 #req-item-tbody）
fn requisition_wo_item_rows(
    preview: &[abt_core::wms::picking::model::WoReqPreviewItem],
    product_map: &std::collections::HashMap<i64, abt_core::master_data::product::model::Product>,
    avail_map: &std::collections::HashMap<i64, Decimal>,
    warehouses: &[Warehouse],
    default_wh: i64,
) -> Markup {
    html! {
        @for p in preview {
            (req_row_inner(
                p.product_id,
                Some(p.bom_qty),
                Some(p.issued_qty),
                avail_map.get(&p.product_id).copied(),
                product_map.get(&p.product_id),
                warehouses,
                default_wh,
            ))
        }
    }
}

/// 手动模式：单行（product_picker 加，无 BOM/已领/可用量）
fn item_row_fragment(
    product: &abt_core::master_data::product::model::Product,
    warehouses: &[Warehouse],
) -> Markup {
    req_row_inner(product.product_id, None, None, None, Some(product), warehouses, 0)
}

/// 共用行结构：行号(JS重排) / 产品 / 单位 / 需求量 / 已领 / 可用量 / 仓库库位 / 批次 / 本次领料 / 删除
/// hidden 同时带 name（binPickerSelect 写回）+ data-k（reqCollectItems 读）+ data-bin-key，对齐 bin_search::warehouse_bin_cell。
fn req_row_inner(
    product_id: i64,
    bom_qty: Option<Decimal>,
    issued_qty: Option<Decimal>,
    avail_qty: Option<Decimal>,
    product: Option<&abt_core::master_data::product::model::Product>,
    warehouses: &[Warehouse],
    default_wh: i64,
) -> Markup {
    let bid = format!("req-bin-{}", product_id);
    let auto_wh = if default_wh > 0 { default_wh.to_string() } else { String::new() };
    // 工单模式默认本次领料量 = max(需求 - 已领, 0)；手动模式为空
    let default_qty = match (bom_qty, issued_qty) {
        (Some(b), i) => {
            let d = b - i.unwrap_or(Decimal::ZERO);
            if d > Decimal::ZERO { fmt_qty(d) } else { String::new() }
        }
        _ => String::new(),
    };
    html! {
        tr data-row {
            td class="text-muted text-xs text-center line-num" {}
            td class="min-w-0" {
                @if let Some(p) = product {
                    div class="text-sm text-fg font-medium leading-tight truncate" { (p.pdt_name) }
                    div class="text-xs text-muted font-mono truncate" { (p.product_code) }
                    div class="text-xs text-fg-2 truncate" { (p.meta.specification) }
                } @else {
                    span class="text-xs text-muted" { "产品 #" (product_id) }
                }
            }
            td class="text-xs text-fg-2 text-center whitespace-nowrap" {
                @if let Some(p) = product { (p.unit) }
            }
            td class="text-xs text-fg-2 text-right font-mono tabular-nums whitespace-nowrap" {
                @if let Some(q) = bom_qty { (fmt_qty(q)) } @else { "—" }
            }
            td class="text-xs text-fg-2 text-right font-mono tabular-nums whitespace-nowrap" {
                @if let Some(q) = issued_qty { (fmt_qty(q)) } @else { "—" }
            }
            td class="text-xs text-right font-mono tabular-nums whitespace-nowrap" {
                @if let Some(q) = avail_qty {
                    (fmt_qty(q))
                } @else {
                    span class="text-muted" { "—" }
                }
            }
            td { (warehouse_bin_cell(&bid, product_id, warehouses, &auto_wh, "outbound")) }
            td {
                input type="text" name="batch_no" data-k="batch_no"
                    class="w-full px-2 py-[5px] text-[13px] font-mono border border-border rounded-sm bg-white text-fg outline-none focus:border-accent" {}
            }
            td {
                input type="number" step="any" name="requested_qty" data-k="requested_qty"
                    value=(default_qty)
                    class="num-input w-full text-right px-2 py-[5px] text-[13px] font-mono tabular-nums border border-border rounded-sm bg-white text-fg outline-none focus:border-accent"
                    placeholder="0" {}
            }
            td {
                button type="button" title="删除行"
                    class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:text-danger"
                    _="on click remove closest <tr/> then call reqRenumber()"
                { (icon::x_icon("w-3.5 h-3.5")) }
            }
            input type="hidden" name="product_id" value=(product_id) {}
        }
    }
}
