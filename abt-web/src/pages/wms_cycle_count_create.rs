use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use rust_decimal::Decimal;

use abt_core::master_data::product::ProductService;
use abt_core::shared::types::DomainError;
use abt_core::wms::cycle_count::model::{CreateCycleCountReq, CreateCycleCountItemReq};
use abt_core::wms::cycle_count::CycleCountService;
use abt_core::wms::warehouse::model::WarehouseFilter;
use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_cycle_count::*;
use crate::routes::wms_work_center::WmsWorkCenterPath;
use crate::utils::{RequestContext, empty_as_none};
use abt_macros::require_permission;

// ── Query Params ──


#[derive(Debug, Deserialize)]
pub struct ItemRowParams {
 pub product_id: i64,
}

// ── Form Data ──

#[derive(Debug, Deserialize)]
struct CycleCountItemWeb {
 product_id: String,
 bin_id: Option<String>,
 batch_no: Option<String>,
 system_qty: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateCycleCountForm {
 #[serde(deserialize_with = "empty_as_none")]
 pub warehouse_id: Option<i64>,
 #[serde(default, deserialize_with = "empty_as_none")]
 pub zone_id: Option<i64>,
 pub count_date: String,
 pub is_blind: Option<String>,
 pub remark: Option<String>,
 pub action: Option<String>,
 pub items_json: String,
}

// ── Handlers ──

#[require_permission("INVENTORY", "read")]
pub async fn get_cycle_count_create(
 _path: CycleCountCreatePath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let warehouse_svc = state.warehouse_service();

 let warehouses = warehouse_svc
 .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
 .await
 .map(|r| r.items)
 .unwrap_or_default();

 let content = cycle_count_create_page(&warehouses, CycleCountCreatePath::PATH, "", true, true);
 let page_html = admin_page(
 is_htmx,
 "新建盘点",
 &claims,
 "inventory",
 CycleCountCreatePath::PATH,
 "库存管理",
 Some("新建盘点"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

/// HTMX: search products for the modal

/// HTMX: return a single item row fragment
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

/// 提取的业务逻辑（事务编排），供独立页 POST 与作业中心 drawer POST 共用。
/// action=start 时建单后立即 start_count。
pub async fn do_create_cycle_count(
    state: &crate::state::AppState,
    service_ctx: &abt_core::shared::types::ServiceContext,
    form: CreateCycleCountForm,
) -> Result<i64> {
    let svc = state.cycle_count_service();

    let count_date = chrono::NaiveDate::parse_from_str(&form.count_date, "%Y-%m-%d")
        .map_err(|e| DomainError::validation(format!("无效日期格式: {e}")))?;

    let is_blind = form.is_blind.as_deref() == Some("on");
    let warehouse_id = form.warehouse_id
        .ok_or_else(|| DomainError::validation("请选择盘点仓库"))?;

    let web_items: Vec<CycleCountItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("物料数据无效: {e}")))?;

    let items: Vec<CreateCycleCountItemReq> = web_items.into_iter().map(|it| {
        let product_id: i64 = it.product_id.parse().unwrap_or(0);
        let bin_id: i64 = it.bin_id.as_ref().and_then(|s| s.parse().ok()).unwrap_or(0);
        let system_qty: Decimal = it.system_qty.parse().unwrap_or(Decimal::ZERO);
        CreateCycleCountItemReq {
            bin_id,
            product_id,
            batch_no: it.batch_no.filter(|s| !s.is_empty()),
            system_qty,
        }
    }).collect();
    if items.iter().any(|i| i.bin_id <= 0) {
        return Err(DomainError::validation("请为每行物料选择库位").into());
    }

    let req = CreateCycleCountReq {
        warehouse_id,
        zone_id: form.zone_id,
        count_date,
        is_blind,
        remark: form.remark,
        items,
    };

    let mut tx = state.pool.begin().await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    let id = svc.create(service_ctx, &mut tx, req).await?;

    if form.action.as_deref() == Some("start") {
        svc.start_count(service_ctx, &mut tx, id).await?;
    }
    tx.commit().await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    Ok(id)
}

#[require_permission("INVENTORY", "create")]
pub async fn create_cycle_count(
 _path: CycleCountCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<CreateCycleCountForm>,
) -> Result<axum::response::Response> {
 let RequestContext { state, service_ctx, .. } = ctx;
 do_create_cycle_count(&state, &service_ctx, form).await?;

 let redirect = format!("{}?domain=cycle-count&view=all", WmsWorkCenterPath::PATH);
 Ok(([("HX-Redirect", redirect)], Html(String::new())).into_response())
}

// ── Components ──

pub fn cycle_count_create_page(
 warehouses: &[abt_core::wms::warehouse::model::Warehouse],
 post_path: &str,
 after_request_hs: &str,
 show_header: bool,
 with_picker: bool,
) -> Markup {
 let today = chrono::Local::now().format("%Y-%m-%d").to_string();
 html! {
    div class="space-y-5 p-6" {
        @if show_header {
            // ── Back Link ──
            a   href=(format!("{}?domain=cycle-count&view=all", WmsWorkCenterPath::PATH))
                class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
            { (icon::chevron_left_icon("w-4 h-4")) "返回作业中心" }
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "新建盘点单" }
            }
        }
        form
            id="cycleCountForm"
            class="space-y-5"
            hx-post=(post_path)
            hx-swap="none"
            hx-disabled-elt="#cc-submit-btn"
            onsubmit="return cycleCountCollectItems()"
            _=(after_request_hs)
        {
            input type="hidden" name="idempotency_key" _="on load call wcGenIdempotencyKey(me)" {};
            // ── 盘点信息 ──
            div class="grid grid-cols-3 gap-4" {
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "仓库 "
                            span class="text-danger" { "*" }
                        }
                        select
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            name="warehouse_id"
                            required
                        {
                            option value="" { "请选择仓库" }
                            @for w in warehouses {
                                option value=(w.id) { (w.name) }
                            }
                        }
                    }
                    div class="form-field" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "盘点日期 "
                            span class="text-danger" { "*" }
                        }
                        input
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                            type="date"
                            name="count_date"
                            value=(today)
                            required {}
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "盲盘模式" }
                        label
                            class="inline-flex items-center gap-2 cursor-pointer pt-2 text-sm text-fg-2"
                        {
                            input class="w-auto" type="checkbox" name="is_blind";
                            "开启盲盘（录入阶段隐藏系统数量）"
                        }
                    }
            }
            // ── 盘点明细 ──
            div {
                div class="flex items-center gap-2 mb-3" {
                    (icon::clipboard_list_icon("w-4 h-4 text-accent"))
                    span class="text-[13px] font-semibold text-fg" { "盘点明细" }
                    span id="cc-item-count" class="ml-auto text-xs text-muted" { "共 0 项" }
                }
                div class="overflow-x-auto" {
                    table class="data-table" {
                        thead {
                            tr {
                                th class="w-10" { "序号" }
                                th { "产品" }
                                th class="w-[200px]" {
                                    "库位 " span class="text-danger" { "*" }
                                }
                                th class="w-[120px]" { "批次号" }
                                th class="w-[110px] text-right" { "系统数量" }
                                th class="w-10" {}
                            }
                        }
                        tbody id="cc-item-tbody" {}
                    }
                }
                button type="button"
                    class="flex items-center justify-center gap-2 w-full py-2.5 mt-3 border border-dashed border-border rounded-md text-accent text-sm font-medium cursor-pointer transition-all duration-150 hover:border-accent hover:bg-accent-bg"
                    _="on click add .is-open to #product-modal"
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
                    name="remark"
                    placeholder="输入备注信息…"
                    rows="3" {}
            }
            input type="hidden" name="items_json" id="cc-items-json" value="[]" {};
            // ── Action Bar ──
            div class="sticky bottom-0 flex items-center justify-end gap-3 -mx-6 px-6 py-4 bg-bg border-t border-border-soft"
            {
                @if show_header {
                    a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                        href=(format!("{}?domain=cycle-count&view=all", WmsWorkCenterPath::PATH))
                    { "取消" }
                } @else {
                    button type="button"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                        _="on click remove .open from closest .drawer-overlay"
                    { "取消" }
                }
                button type="submit"
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    name="action" value="draft"
                { "保存草稿" }
                button type="submit" id="cc-submit-btn"
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                    name="action" value="start"
                { (icon::check_circle_icon("w-4 h-4")) "开始盘点" }
            }
        }
        ({
            crate::components::product_picker::product_picker_modal_with_search(
                "product-modal",
                CycleCountItemRowPath::PATH,
                "cc-item-tbody",
            )
        })
        @if with_picker {
            (crate::components::bin_search::bin_picker_modal("bin-picker-modal", warehouses))
        }
        script src=(crate::layout::page::cache_url("/wms-cycle-count-create.js")) {}
    }
}
}

/// Single item row fragment
fn item_row_fragment(product: &abt_core::master_data::product::model::Product) -> Markup {
    let bid = format!("cc-bin-{}", product.product_id);
    html! {
        tr data-row {
            td class="text-muted text-xs text-center line-num" {}
            td {
                div class="text-sm text-fg font-medium leading-tight" { (product.pdt_name) }
                div class="text-xs text-muted font-mono" { (product.product_code) }
                div class="text-xs text-fg-2" { (product.meta.specification) " · " (product.unit) }
            }
            td class="align-middle" {
                button type="button"
                    class="bin-cell-btn w-full px-2 py-1.5 border border-border rounded-sm text-xs bg-white text-fg-2 hover:border-accent hover:text-accent transition-colors text-left truncate"
                    data-bin-key=(bid) data-product-id=(product.product_id) data-mode="inbound"
                    _="on click call binPickerOpen(me)" { "选择库位" }
                input type="hidden" name="bin_id" data-k="bin_id" data-bin-key=(bid) value=""
                    _="on input call ccRefreshSystemQty(closest <tr/>)" {}
            }
            td {
                input class="w-full px-2 py-[5px] text-[13px] border border-border rounded-sm bg-white text-fg outline-none focus:border-accent"
                    type="text" name="batch_no" placeholder="批次号" {}
            }
            td {
                input class="num-input w-full text-right px-2 py-[5px] text-[13px] font-mono tabular-nums border border-border-soft rounded-sm bg-surface text-muted"
                    type="text" readonly name="system_qty" value="0" {}
            }
            td {
                button type="button"
                    class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:text-danger"
                    title="删除行"
                    _="on click remove closest <tr/> then call ccRenumber()"
                { (icon::x_icon("w-3.5 h-3.5")) }
            }
            input type="hidden" name="product_id" value=(product.product_id) {}
        }
    }
}
