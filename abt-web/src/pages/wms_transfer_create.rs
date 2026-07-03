use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use chrono::NaiveDate;
use rust_decimal::Decimal;

use abt_core::master_data::product::ProductService;
use abt_core::shared::types::DomainError;
use abt_core::wms::picking::{CreatePickingItemReq, CreatePickingReq, PickingService};
use abt_core::wms::enums::PickingType;
use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_transfer::{TransferCreatePath, TransferItemRowPath};
use crate::routes::wms_work_center::WmsWorkCenterPath;
use crate::utils::{RequestContext, empty_as_none};
use abt_macros::require_permission;

// ── Query Params ──


#[derive(Debug, Deserialize)]
pub struct ItemRowParams {
 pub product_id: i64,
}

// ── Handlers ──

#[require_permission("INVENTORY", "create")]
pub async fn get_transfer_create(
 _path: TransferCreatePath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let warehouse_svc = state.warehouse_service();

 let warehouses = warehouse_svc
 .list(&service_ctx, &mut conn, abt_core::wms::warehouse::model::WarehouseFilter::default(), 1, 200)
 .await
 .map(|r| r.items)
 .unwrap_or_default();

 let content = transfer_create_page(&warehouses, TransferCreatePath::PATH, "", true);
 let page_html = admin_page(
 is_htmx, "新建调拨单", &claims, "inventory", TransferCreatePath::PATH, "库存管理", None, content, &nav_filter,
 );
 Ok(Html(page_html.into_string()))
}

/// HTMX: search products

/// HTMX: return a single item row
#[require_permission("INVENTORY", "create")]
pub async fn get_item_row(
 ctx: RequestContext,
 Query(params): Query<ItemRowParams>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.product_service();
 let product = svc.get(&service_ctx, &mut conn, params.product_id).await?;
 Ok(Html(item_row_fragment(&product).into_string()))
}

// ── Form Data ──

#[derive(Debug, Deserialize)]
struct TransferItemWeb {
 product_id: String,
 quantity: String,
 batch_no: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct TransferCreateForm {
 #[serde(deserialize_with = "empty_as_none")]
 pub from_warehouse_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub from_zone_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub from_bin_id: Option<i64>,
 #[serde(deserialize_with = "empty_as_none")]
 pub to_warehouse_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub to_zone_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub to_bin_id: Option<i64>,
 pub transfer_date: NaiveDate,
 pub remark: Option<String>,
 pub items_json: String,
}

/// 提取的业务逻辑（含 tx），供独立页 POST 与作业中心 drawer POST 共用。
pub async fn do_create_transfer(
    state: &crate::state::AppState,
    service_ctx: &abt_core::shared::types::ServiceContext,
    form: TransferCreateForm,
) -> Result<()> {
    let svc = state.picking_service();
    let from_warehouse_id = form.from_warehouse_id
        .ok_or_else(|| DomainError::validation("请选择调出仓库"))?;
    let to_warehouse_id = form.to_warehouse_id
        .ok_or_else(|| DomainError::validation("请选择调入仓库"))?;

    let web_items: Vec<TransferItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("无效物料数据: {e}")))?;
    if web_items.is_empty() {
        return Err(DomainError::validation("调拨单至少需要一条明细").into());
    }

    let items: Vec<CreatePickingItemReq> = web_items.into_iter().map(|item| {
        CreatePickingItemReq {
            product_id: item.product_id.parse().unwrap_or(0),
            batch_no: item.batch_no,
            qty_requested: item.quantity.parse().unwrap_or(Decimal::ZERO),
            from_bin_id: None,
            to_bin_id: None,
            operation_id: None,
            batch_id: None,
            source_item_id: None,
            remark: None,
        }
    }).collect();

    let req = CreatePickingReq {
        picking_type: PickingType::InternalTransfer,
        source_type: Some("none".into()),
        source_id: None,
        partner_id: None,
        from_warehouse_id: Some(from_warehouse_id),
        from_zone_id: form.from_zone_id,
        from_bin_id: form.from_bin_id,
        to_warehouse_id: Some(to_warehouse_id),
        to_zone_id: form.to_zone_id,
        to_bin_id: form.to_bin_id,
        scheduled_date: Some(form.transfer_date),
        work_order_id: None,
        remark: form.remark.clone(),
        items,
    };

    let mut tx = state.pool.begin().await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    svc.create(service_ctx, &mut tx, req).await?;
    tx.commit().await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    Ok(())
}

#[require_permission("INVENTORY", "create")]
pub async fn create_transfer(
 _path: TransferCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<TransferCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;
 do_create_transfer(&state, &service_ctx, form).await?;
 let redirect = format!("{}?domain=transfer&view=all", WmsWorkCenterPath::PATH);
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

pub fn transfer_create_page(
 warehouses: &[abt_core::wms::warehouse::model::Warehouse],
 post_path: &str,
 after_request_hs: &str,
 show_header: bool,
) -> Markup {
 html! {
    div {
        @if show_header {
            // ── Back Link ──
            a   href="/admin/wms/transfers"
                class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150 mb-4"
            { (icon::chevron_left_icon("w-4 h-4")) "返回库存调拨列表" }
            // ── Page Header ──
            div class="flex items-center justify-between mb-5" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "新建调拨" }
                span class="text-xs text-muted flex items-center gap-2" {
                    (icon::clock_icon("w-3.5 h-3.5"))
                    "自动保存草稿"
                }
            }
        }
        // ── Status Flow ──
        div class="flex items-center gap-2 mb-5 px-4 py-3 bg-bg border border-border-soft rounded-md"
        {
            span
                class="text-xs px-2.5 py-0.5 rounded-full font-semibold text-accent bg-[rgba(22,119,255,0.08)] [border:1px_solid_rgba(22,119,255,0.3)]"
            { "草稿" }
            span class="text-[10px] text-border" { "→" }
            span
                class="text-xs px-2.5 py-0.5 rounded-full text-muted border border-border bg-surface"
            { "在途" }
            span class="text-[10px] text-border" { "→" }
            span
                class="text-xs px-2.5 py-0.5 rounded-full text-muted border border-border bg-surface"
            { "完成" }
        }
        form
            hx-post=(post_path)
            hx-swap="none"
            onsubmit="return transferCollectItems()"
            _=(after_request_hs)
        {
            // ── 调拨信息 ──
            div class="form-section" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft"
                { (icon::arrow_right_icon("w-[18px] h-[18px]")) "调拨信息" }
                // ── 调出方 ──
                div class="text-xs font-semibold text-fg-2 uppercase tracking-wide mb-3 mt-2" {
                    "调出方"
                }
                div class="grid grid-cols-3 gap-4 gap-x-6 mb-4" {
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "调出仓库 "
                            span class="required" { "*" }
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            name="from_warehouse_id"
                            required
                        {
                            option value="" { "请选择调出仓库" }
                            @for wh in warehouses {
                                option value=(wh.id) { (wh.name) }
                            }
                        }
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "调出库区"
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            name="from_zone_id"
                        {
                            option value="" { "请选择库区" }
                        }
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "调出库位"
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            name="from_bin_id"
                        {
                            option value="" { "请选择库位" }
                        }
                    }
                }
                // ── 调入方 ──
                div class="text-xs font-semibold text-fg-2 uppercase tracking-wide mb-3 mt-2" {
                    "调入方"
                }
                div class="grid grid-cols-3 gap-4 gap-x-6 mb-4" {
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "调入仓库 "
                            span class="required" { "*" }
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            name="to_warehouse_id"
                            required
                        {
                            option value="" { "请选择调入仓库" }
                            @for wh in warehouses {
                                option value=(wh.id) { (wh.name) }
                            }
                        }
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "调入库区"
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            name="to_zone_id"
                        {
                            option value="" { "请选择库区" }
                        }
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "调入库位"
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            name="to_bin_id"
                        {
                            option value="" { "请选择库位" }
                        }
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "调拨日期 "
                            span class="required" { "*" }
                        }
                        input
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            type="date"
                            name="transfer_date"
                            value="2026-06-17"
                            required {}
                    }
                }
                // ── 备注 ──
                div class="grid grid-cols-3 gap-4 gap-x-6 mb-2" {
                    div class="form-field col-span-2" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "备注"
                        }
                        textarea
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent resize-y"
                            name="remark"
                            placeholder="输入调拨相关备注信息…"
                            rows="2" {}
                    }
                }
            }
            // ── 调拨明细 ──
            div class="form-section p-0 overflow-hidden" {
                div class="px-6 pt-6 pb-4" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-3" {
                        (icon::box_icon("w-[18px] h-[18px]"))
                        "调拨明细"
                        span
                            id="transfer-item-count"
                            class="ml-auto text-xs font-normal text-muted"
                        { "共 0 项" }
                    }
                }
                div class="overflow-x-auto" {
                    table class="data-table" {
                        thead {
                            tr {
                                th class="w-10 text-center" { "行号" }
                                th class="min-w-[140px]" { "产品编码" }
                                th class="min-w-[180px]" { "产品名称" }
                                th class="min-w-[160px]" { "规格" }
                                th class="w-16" { "单位" }
                                th class="w-[90px] text-right" { "调出库存" }
                                th class="w-[100px] text-right" {
                                    "调拨数量 "
                                    span class="required" { "*" }
                                }
                                th class="w-[130px]" { "批次号" }
                                th class="w-10" {}
                            }
                        }
                        tbody id="transfer-item-tbody" {}
                    }
                }
                div class="p-4" {
                    button
                        type="button"
                        class="flex items-center justify-center gap-2 w-full text-accent text-sm font-medium cursor-pointer"
                        _="on click add .is-open to #transfer-product-modal"
                    { (icon::plus_icon("w-3.5 h-3.5")) "添加物料" }
                }
            }
            input type="hidden" name="items_json" id="transfer-items-json" value="[]" {}
            // ── Action Bar ──
            div class="sticky bottom-0 flex items-center justify-between gap-3 px-6 py-4 bg-bg border-t border-border-soft"
            {
                div {}
                div class="flex gap-3" {
                    @if show_header {
                        a   href="/admin/wms/transfers"
                            class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                        { (icon::save_icon("w-4 h-4")) "取消" }
                    } @else {
                        button type="button"
                            class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                            _="on click remove .open from closest .drawer-overlay"
                        { "取消" }
                    }
                    button
                        type="submit"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                    { (icon::send_icon("w-4 h-4")) "提交调拨" }
                }
            }
        }
        ({
            crate::components::product_picker::product_picker_modal_with_search(
                "transfer-product-modal",
                TransferItemRowPath::PATH,
                "transfer-item-tbody",
            )
        })
        // ── Line Item JS ──
        ({
            maud::PreEscaped(
                r#"<script>
 function transferCollectItems() {
 var tbody = document.getElementById('transfer-item-tbody');
 var rows = tbody.querySelectorAll('tr');
 var items = [];
 rows.forEach(function(row) {
 items.push({
 product_id: row.querySelector('input[name="product_id"]').value,
 quantity: row.querySelector('input[name="quantity"]').value || '0',
 batch_no: row.querySelector('input[name="batch_no"]').value || null
 });
 });
 document.getElementById('transfer-items-json').value = JSON.stringify(items);
 if (items.length === 0) { alert('请至少添加一个物料'); return false; }
 return true;
 }
 function transferRenumber() {
 var tbody = document.getElementById('transfer-item-tbody');
 tbody.querySelectorAll('tr').forEach(function(row, i) {
 row.querySelector('.line-num').textContent = i + 1;
 });
 }
 </script>"#,
            )
        })
    }
}
}

/// Single item row fragment
fn item_row_fragment(product: &abt_core::master_data::product::model::Product) -> Markup {
 html! {
    tr {
        td class="text-muted text-xs text-center line-num" {}
        td class="font-mono tabular-nums" { (product.product_code) }
        td { (product.pdt_name) }
        td class="text-sm text-fg-2" { (product.meta.specification) }
        td class="text-sm text-fg-2 text-center" { (product.unit) }
        td class="text-right text-[13px] font-mono tabular-nums text-muted" { "—" }
        td {
            input
                class="num-input w-full text-right px-2 py-[5px] text-[13px] font-mono tabular-nums border border-border rounded-sm bg-white text-fg outline-none focus:border-accent"
                type="number"
                step="any"
                name="quantity"
                placeholder="0" {}
        }
        td {
            input
                class="w-full px-2 py-[5px] text-[13px] border border-border rounded-sm bg-white text-fg outline-none focus:border-accent"
                type="text"
                name="batch_no"
                placeholder="批次号" {}
        }
        td {
            button
                type="button"
                class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:text-danger"
                title="删除行"
                _="on click remove closest <tr/> then call transferRenumber()"
            { (icon::x_icon("w-3.5 h-3.5")) }
        }
        input type="hidden" name="product_id" value=(product.product_id) {}
    }
}
}
