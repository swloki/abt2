use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::enums::WorkOrderStatus;
use abt_core::mes::production_receipt::ProductionReceiptService;
use abt_core::mes::work_order::model::{WorkOrder, WorkOrderFilter};
use abt_core::mes::work_order::WorkOrderService;
use abt_core::wms::warehouse::model::WarehouseFilter;
use abt_core::wms::warehouse::WarehouseService;

use crate::components::entity_picker::{self, EntityPickerConfig, EntityPickerItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_receipt::{
    ReceiptCreatePath, ReceiptListPath, ReceiptSearchWoPath, ReceiptSearchWhPath,
    ReceiptWhZonesPath, ReceiptWoSelectedPath, ReceiptZnBinsPath,
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

#[derive(Debug, Deserialize)]
pub struct WhZonesQuery {
    pub warehouse_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct ZnBinsQuery {
    pub zone_id: i64,
}

// ── Form ──

#[derive(Debug, Deserialize)]
pub struct ReceiptCreateForm {
    pub work_order_id: i64,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub product_id: Option<i64>,
    pub received_qty: rust_decimal::Decimal,
    pub warehouse_id: i64,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub zone_id: Option<i64>,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub bin_id: Option<i64>,
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
        date_to: None,
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

// ── HTMX: 搜索仓库 ──

#[require_permission("WORK_ORDER", "read")]
pub async fn search_wh(
    _path: ReceiptSearchWhPath,
    ctx: RequestContext,
    Query(params): Query<SearchParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
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

// ── HTMX: 仓库选中后级联 — 返回库区下拉 ──

#[require_permission("WORK_ORDER", "read")]
pub async fn get_wh_zones(
    _path: ReceiptWhZonesPath,
    ctx: RequestContext,
    Query(params): Query<WhZonesQuery>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let wh_svc = state.warehouse_service();
    let zones = wh_svc
        .list_zones(&service_ctx, &mut conn, params.warehouse_id)
        .await
        .unwrap_or_default();
    Ok(Html(zone_select_fragment(&zones).into_string()))
}

// ── HTMX: 库区选中后级联 — 返回储位下拉 ──

#[require_permission("WORK_ORDER", "read")]
pub async fn get_zn_bins(
    _path: ReceiptZnBinsPath,
    ctx: RequestContext,
    Query(params): Query<ZnBinsQuery>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let wh_svc = state.warehouse_service();
    let bins = wh_svc
        .list_bins(&service_ctx, &mut conn, params.zone_id, None, 1, 200)
        .await
        .map(|r| r.items)
        .unwrap_or_default();
    Ok(Html(bin_select_fragment(&bins).into_string()))
}

// ── POST /receipts/create ──

#[require_permission("WORK_ORDER", "create")]
pub async fn create_receipt(
    _path: ReceiptCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ReceiptCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.production_receipt_service();
    let req = abt_core::mes::production_receipt::CreateReceiptReq {
        work_order_id: form.work_order_id,
        batch_id: None,
        product_id: form.product_id.unwrap_or(0),
        received_qty: form.received_qty,
        warehouse_id: form.warehouse_id,
        zone_id: form.zone_id,
        bin_id: form.bin_id,
        receipt_date: form.receipt_date,
        remark: form.remark,
    };
    let _id = svc.create(&service_ctx, &mut conn, req).await?;
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
    let wh_picker = EntityPickerConfig {
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

    html! {
        div {
            div class="flex items-center justify-between mb-6" {
                div class="flex items-center justify-between mb-6-left" {
                    a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", ReceiptListPath::PATH)) { "\u{2190} 返回列表" }
                    h1 class="text-xl font-bold text-fg tracking-tight" { "新建完工入库" }
                }
            }

            form hx-post=(ReceiptCreatePath::PATH) hx-swap="none" id="receipt-form" {
                // ── 入库来源 ──
                div class="form-section" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "入库来源" }

                    (entity_picker::entity_picker_field(
                        "work_order_id", "work_order_id", "wo-display", "wo-picker",
                        "工单号", true, "点击选择工单…",
                    ))

                    // 工单选中后级联加载：产品名
                    div id="wo-cascade"
                        hx-get=(ReceiptWoSelectedPath::PATH)
                        hx-trigger="woSelected from:body"
                        hx-target="this"
                        hx-swap="outerHTML"
                        hx-include="#work_order_id" {
                        div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                            div class="form-field" {
                                label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "产品" }
                                div class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" style="color:var(--text-muted);background:var(--surface)" { "选择工单后自动填充" }
                            }
                        }
                    }
                }

                // ── 入库明细 ──
                div class="form-section" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "入库明细" }
                    div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "入库数量 " span class="required" { "*" } }
                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" step="0.01" name="received_qty" required placeholder="0";
                        }
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "入库日期 " span class="required" { "*" } }
                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="date" name="receipt_date" value=(today) required;
                        }
                    }
                }

                // ── 目标库位 ──
                div class="form-section" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "目标库位" }

                    (entity_picker::entity_picker_field(
                        "warehouse_id", "warehouse_id", "wh-display", "wh-picker",
                        "目标仓库", true, "点击选择仓库…",
                    ))

                    // 仓库选中后级联加载：库区 + 储位
                    div id="zone-bin-area"
                        hx-get=(ReceiptWhZonesPath::PATH)
                        hx-trigger="whSelected from:body"
                        hx-target="this"
                        hx-swap="outerHTML"
                        hx-include="#warehouse_id" {
                        div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                            div class="form-field" {
                                label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "库区" }
                                select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="zone_id" disabled {
                                    option value="" { "选择仓库后加载" }
                                }
                            }
                            div class="form-field" {
                                label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "储位" }
                                select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="bin_id" disabled {
                                    option value="" { "选择库区后加载" }
                                }
                            }
                        }
                    }
                }

                // ── 备注 ──
                div class="form-section" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "备注" }
                    div class="form-field" {
                        textarea class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="remark" rows="2" placeholder="可选" {}
                    }
                }

                div class="create-action-bar" {
                    a class="btn btn-default" href=(format!("{}?restore=true", ReceiptListPath::PATH)) { "取消" }
                    button type="submit" class="btn btn-primary" { "提交入库" }
                }
            }

            // ── 弹窗 ──
            (entity_picker::entity_picker_modal(&wo_picker))
            (entity_picker::entity_picker_modal(&wh_picker))
        }
    }
}

// ── HTMX fragments ──

/// 工单选中后返回的产品信息片段
fn wo_cascade_fragment(product_id: i64, product_name: &str) -> Markup {
    html! {
        div id="wo-cascade" {
            div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                // 产品（只读 + 隐藏 ID）
                div class="form-field" {
                    label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "产品" }
                    input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" value=(product_name) disabled
                        style="background:var(--surface)";
                    input type="hidden" name="product_id" value=(product_id);
                }
            }
        }
    }
}

/// 仓库选中后返回的库区下拉 + 储位占位
fn zone_select_fragment(zones: &[abt_core::wms::warehouse::model::Zone]) -> Markup {
    html! {
        div id="zone-bin-area" {
            div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                div class="form-field" {
                    label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "库区" }
                    @if zones.is_empty() {
                        select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="zone_id" disabled {
                            option value="" { "该仓库暂无库区" }
                        }
                        input type="hidden" name="zone_id" value="";
                    } @else {
                        select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="zone_id"
                            hx-get=(ReceiptZnBinsPath::PATH)
                            hx-target="#bin-select-wrap"
                            hx-trigger="change"
                            hx-swap="outerHTML"
                            hx-include="this" {
                            option value="" selected { "默认库区" }
                            @for z in zones {
                                option value=(z.id) { (z.name) }
                            }
                        }
                    }
                }
                div class="form-field" id="bin-select-wrap" {
                    label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "储位" }
                    select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="bin_id" disabled {
                        option value="" { "选择库区后加载" }
                    }
                }
            }
        }
    }
}

/// 库区选中后返回的储位下拉
fn bin_select_fragment(bins: &[abt_core::wms::warehouse::model::Bin]) -> Markup {
    html! {
        div class="form-field" id="bin-select-wrap" {
            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "储位" }
            @if bins.is_empty() {
                select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="bin_id" disabled {
                    option value="" { "该库区暂无储位" }
                }
            } @else {
                select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="bin_id" {
                    option value="" selected { "自动分配" }
                    @for b in bins {
                        option value=(b.id) { (b.code) " " (b.name) }
                    }
                }
            }
        }
    }
}
