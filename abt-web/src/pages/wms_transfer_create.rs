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
        shipping_requirements: None,
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
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    html! {
        div class="space-y-5 p-6" {
            @if show_header {
                a   href="/admin/wms/transfers"
                    class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
                { (icon::chevron_left_icon("w-4 h-4")) "返回库存调拨列表" }
                div class="flex items-center justify-between mb-6" {
                    h1 class="text-xl font-bold text-fg tracking-tight" { "新建调拨单" }
                }
            }
            form id="transferForm" class="space-y-5"
                hx-post=(post_path) hx-swap="none"
                hx-disabled-elt="#transfer-submit-btn"
                onsubmit="return transferCollectItems()"
                _=(after_request_hs)
            {
                input type="hidden" name="idempotency_key" _="on load call wcGenIdempotencyKey(me)" {};
                // ── 调拨信息 ──
                div class="grid grid-cols-3 gap-4" {
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "调出仓库 " span class="text-danger" { "*" }
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            name="from_warehouse_id" required
                            _="on change call transferRefreshAvail(closest <form/>)"
                        {
                            option value="" { "请选择调出仓库" }
                            @for wh in warehouses {
                                option value=(wh.id) { (wh.name) }
                            }
                        }
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "调入仓库 " span class="text-danger" { "*" }
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            name="to_warehouse_id" required
                        {
                            option value="" { "请选择调入仓库" }
                            @for wh in warehouses {
                                option value=(wh.id) { (wh.name) }
                            }
                        }
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "调拨日期 " span class="text-danger" { "*" }
                        }
                        input
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            type="date" name="transfer_date" value=(today) required {}
                    }
                }
                // ── 调拨明细 ──
                div {
                    div class="flex items-center gap-2 mb-3" {
                        (icon::clipboard_list_icon("w-4 h-4 text-accent"))
                        span class="text-[13px] font-semibold text-fg" { "调拨明细" }
                        span id="transfer-item-count" class="ml-auto text-xs text-muted" { "共 0 项" }
                    }
                    div class="overflow-x-auto" {
                        table class="data-table" {
                            thead {
                                tr {
                                    th class="w-10" { "序号" }
                                    th { "产品" }
                                    th class="w-[110px] text-right" { "可用量" }
                                    th class="w-[110px] text-right" {
                                        "调拨数量 " span class="text-danger" { "*" }
                                    }
                                    th class="w-[130px]" { "批次号" }
                                    th class="w-10" {}
                                }
                            }
                            tbody id="transfer-item-tbody" {}
                        }
                    }
                    button type="button"
                        class="flex items-center justify-center gap-2 w-full py-2.5 mt-3 border border-dashed border-border rounded-md text-accent text-sm font-medium cursor-pointer transition-all duration-150 hover:border-accent hover:bg-accent-bg"
                        _="on click add .is-open to #transfer-product-modal"
                    { (icon::plus_icon("w-3.5 h-3.5")) "添加物料" }
                }
                // ── 备注 ──
                div {
                    div class="flex items-center gap-2 mb-2" {
                        (icon::edit_icon("w-4 h-4 text-accent"))
                        span class="text-[13px] font-semibold text-fg" { "备注" }
                    }
                    textarea
                        class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none resize-y min-h-[64px] transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                        name="remark" placeholder="输入备注信息…" rows="3" {}
                }
                input type="hidden" name="items_json" id="transfer-items-json" value="[]" {}
                // ── Action Bar ──
                div class="sticky bottom-0 flex items-center justify-end gap-3 -mx-6 px-6 py-4 bg-bg border-t border-border-soft" {
                    @if show_header {
                        a   href="/admin/wms/transfers"
                            class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                        { "取消" }
                    } @else {
                        button type="button"
                            class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                            _="on click remove .open from closest .drawer-overlay"
                        { "取消" }
                    }
                    button type="submit" id="transfer-submit-btn"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                    { (icon::send_icon("w-4 h-4")) "提交调拨" }
                }
            }
            ({
                crate::components::product_picker::product_picker_modal_with_search(
                    "transfer-product-modal",
                    TransferItemRowPath::PATH,
                    "transfer-item-tbody",
                )
            })
            script src=(crate::layout::page::cache_url("/wms-transfer-create.js")) {}
        }
    }
}

/// Single item row fragment
fn item_row_fragment(product: &abt_core::master_data::product::model::Product) -> Markup {
    html! {
        tr data-row data-pid=(product.product_id) {
            td class="text-muted text-xs text-center line-num" {}
            td {
                div class="text-sm text-fg font-medium leading-tight" { (product.pdt_name) }
                div class="text-xs text-muted font-mono" { (product.product_code) }
                div class="text-xs text-fg-2" { (product.meta.specification) " · " (product.unit) }
            }
            td class="text-right text-[13px] font-mono tabular-nums text-muted" data-avail { "—" }
            td {
                input
                    class="num-input w-full text-right px-2 py-[5px] text-[13px] font-mono tabular-nums border border-border rounded-sm bg-white text-fg outline-none focus:border-accent"
                    type="number" step="any" name="quantity" placeholder="0" {}
            }
            td {
                input
                    class="w-full px-2 py-[5px] text-[13px] border border-border rounded-sm bg-white text-fg outline-none focus:border-accent"
                    type="text" name="batch_no" placeholder="批次号" {}
            }
            td {
                button type="button"
                    class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:text-danger"
                    title="删除行"
                    _="on click remove closest <tr/> then call transferRenumber()"
                { (icon::x_icon("w-3.5 h-3.5")) }
            }
            input type="hidden" name="product_id" value=(product.product_id) {}
        }
    }
}
