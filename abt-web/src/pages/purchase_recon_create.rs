use std::collections::HashMap;

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::master_data::supplier::SupplierService;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::purchase::reconciliation::model::PurchaseReconPreviewItem;
use abt_core::purchase::reconciliation::PurchaseReconciliationService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_order::PODetailPath;
use crate::routes::purchase_reconciliation::*;
use crate::utils::{RequestContext, empty_as_none};
use abt_macros::require_permission;

// ── Form / Query Structs ──

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct PreconCreateForm {
    pub supplier_id: i64,
    pub period: String,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub remark: Option<String>,
    pub action: Option<String>,
}

/// 创建页「待对账明细」预览查询参数（select 空串经 empty_as_none 转 None）
#[derive(Debug, Deserialize)]
pub struct PreviewQuery {
    #[serde(default, deserialize_with = "empty_as_none")]
    pub supplier_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub period: Option<String>,
}

// ── Helpers ──

struct ProductInfo {
    code: String,
    name: String,
}

// ── Handlers ──

#[require_permission("PURCHASE_RECON", "create")]
pub async fn get_precon_create(
    _path: PreconCreatePath,
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
    let supplier_svc = state.supplier_service();

    let suppliers = supplier_svc
        .list(
            &service_ctx,
            &mut conn,
            SupplierQuery {
                name: None,
                status: None,
                category: None,
            },
            PageParams::new(1, 200),
        )
        .await?;

    let buyer_name = &claims.display_name;
    let content = precon_create_page(&suppliers.items, buyer_name, PreconCreatePath::PATH, "", true);
    let page_html = admin_page(
        is_htmx,
        "新建采购对账单",
        &claims,
        "purchase",
        PreconCreatePath::PATH,
        "采购管理",
        Some("新建采购对账单"),
        content,
        &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

/// 对账单创建核心逻辑（按供应商+期间自动拉取该期间「未对账已收货」明细全量纳入），创建页与 work_center drawer 共用。
pub async fn do_create_precon(
    state: &crate::state::AppState,
    service_ctx: &abt_core::shared::types::context::ServiceContext,
    form: PreconCreateForm,
) -> Result<i64> {
    let svc = state.purchase_reconciliation_service();

    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    let id = svc
        .create(service_ctx, &mut tx, form.supplier_id, form.period, None)
        .await?;
    tx.commit()
        .await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    Ok(id)
}

#[require_permission("PURCHASE_RECON", "create")]
pub async fn create_precon(
    _path: PreconCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<PreconCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let id = do_create_precon(&state, &service_ctx, form).await?;
    let redirect = PreconDetailPath { id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

/// 创建页「待对账明细」预览：选供应商+期间后自动加载该期间已收货、未对账明细。
/// supplier/period 缺失或 period 格式错/无数据 → 统一降级为空态（200），不抛错，避免前端卡死。
#[require_permission("PURCHASE_RECON", "read")]
pub async fn get_precon_preview(
    ctx: RequestContext,
    Query(params): Query<PreviewQuery>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    let supplier_id = match params.supplier_id {
        Some(id) if id > 0 => id,
        _ => return Ok(Html(preview_empty("请先选择供应商").into_string())),
    };
    let period = match params.period {
        Some(ref p) if !p.is_empty() => p.clone(),
        _ => return Ok(Html(preview_empty("请先选择对账期间").into_string())),
    };

    let svc = state.purchase_reconciliation_service();
    // service 内对 period 格式错误宽松降级为空 vec；此处仅 DB 真实错误才 ? → 500
    let items = svc
        .preview(&service_ctx, &mut conn, supplier_id, period)
        .await?;

    if items.is_empty() {
        return Ok(Html(
            preview_empty("该供应商在所选期间内没有可对账的收货明细").into_string(),
        ));
    }

    // 富化物料编码/名称
    let product_svc = state.product_service();
    let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
    let product_map: HashMap<i64, ProductInfo> = product_svc
        .get_by_ids(&service_ctx, &mut conn, product_ids)
        .await
        .map(|products| {
            products
                .into_iter()
                .map(|p| {
                    (
                        p.product_id,
                        ProductInfo {
                            code: p.product_code,
                            name: p.pdt_name,
                        },
                    )
                })
                .collect()
        })
        .unwrap_or_default();

    // 富化订单号
    let order_svc = state.purchase_order_service();
    let order_ids: Vec<i64> = items
        .iter()
        .map(|i| i.order_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let order_numbers: HashMap<i64, String> = {
        let mut map = HashMap::new();
        for &oid in &order_ids {
            if let Ok(order) = order_svc.get(&service_ctx, &mut conn, oid).await {
                map.insert(oid, order.doc_number);
            }
        }
        map
    };

    let content = preview_table(&items, &product_map, &order_numbers);
    Ok(Html(content.into_string()))
}

// ── Components ──

pub fn precon_create_page(
    suppliers: &[abt_core::master_data::supplier::model::Supplier],
    buyer_name: &str,
    post_path: &str,
    after_request_hs: &str,
    show_header: bool,
) -> Markup {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let current_month = chrono::Local::now().format("%Y-%m").to_string();

    html! {
        div id="precon-app" {
            @if show_header {
                div class="flex items-center justify-between mb-6" {
                    a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
                      href=(format!("{}?restore=true", PreconListPath::PATH))
                    { (icon::arrow_left_icon("w-4 h-4")) "返回对账单列表" }
                    h1 class="text-xl font-bold text-fg tracking-tight" { "新建采购对账单" }
                }
            }

            form id="precon-form" hx-post=(post_path) hx-swap="none" _=(after_request_hs) {
                // ── 对账基本信息 ──
                div class="data-card mb-4" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                    { "对账基本信息" }
                    div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                        div class="form-field" {
                            label {
                                "供应商"
                                span class="text-danger" { "*" }
                            }
                            select name="supplier_id" id="precon-supplier" required onchange="triggerPreview()" {
                                option value="" disabled selected { "请选择供应商" }
                                @for s in suppliers {
                                    option value=(s.id) { (s.name) }
                                }
                            }
                        }
                        div class="form-field" {
                            label {
                                "对账期间"
                                span class="text-danger" { "*" }
                            }
                            input type="month" name="period" id="precon-period" value=(current_month) required onchange="triggerPreview()" {}
                        }
                        div class="form-field" {
                            label { "对账日期" }
                            input type="date" name="recon_date" value=(today) {}
                        }
                        div class="form-field" {
                            label { "采购员" }
                            input type="text" value=(buyer_name) readonly {}
                        }
                        div class="form-field field-full" {
                            label { "备注" }
                            textarea name="remark" placeholder="输入对账单相关备注信息…" {}
                        }
                    }
                }
                // ── 对账明细（预览区，选供应商+期间后自动加载）──
                (preview_empty("请先选择供应商与对账期间，系统将自动加载该期间已收货、未对账的明细"))
                // ── Action Bar ──
                div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft" {
                    @if show_header {
                        a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                          href=(format!("{}?restore=true", PreconListPath::PATH))
                        { "取消" }
                    } @else {
                        button type="button" class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                            _="on click remove .open from closest .drawer-overlay"
                        { "取消" }
                    }
                    div class="flex gap-3" {
                        button type="submit"
                            class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                            name="action" value="draft"
                        { (icon::save_icon("w-4 h-4")) "保存草稿" }
                        button type="submit"
                            class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                        { (icon::send_icon("w-4 h-4")) "提交对账单" }
                    }
                }
            }
            // ── Preview trigger helper ──
            ({
                maud::PreEscaped(
                    r#"<script>
function triggerPreview() {
    htmx.trigger(document.getElementById('precon-app'), 'previewChanged');
}
</script>"#,
                )
            })
        }
    }
}

/// 预览区空态：自身即 HTMX 替换边界（hx-target="this" + outerHTML），
/// 监听 #precon-app 上的 previewChanged 事件重新拉取。
fn preview_empty(message: &str) -> Markup {
    html! {
        div class="bg-bg border border-border rounded overflow-hidden mb-4"
            id="precon-preview-area"
            hx-get=(PreconPreviewPath::PATH)
            hx-trigger="previewChanged from:#precon-app"
            hx-include="#precon-supplier,#precon-period"
            hx-target="this"
            hx-swap="outerHTML"
        {
            div class="text-center p-10 text-muted text-sm" {
                (icon::clipboard_list_icon("w-12 h-12"))
                p class="mt-3" { (message) }
            }
        }
    }
}

/// 预览区明细表：行「应付金额」= amount - returned_amount，与 confirm 的 confirmed_amount 口径一致。
fn preview_table(
    items: &[PurchaseReconPreviewItem],
    product_map: &HashMap<i64, ProductInfo>,
    order_numbers: &HashMap<i64, String>,
) -> Markup {
    let total_amount: rust_decimal::Decimal = items.iter().map(|i| i.amount).sum();
    let total_return: rust_decimal::Decimal = items.iter().map(|i| i.returned_amount).sum();
    let net_amount = total_amount - total_return;
    let item_count = items.len();

    html! {
        div class="bg-bg border border-border rounded overflow-hidden mb-4"
            id="precon-preview-area"
            hx-get=(PreconPreviewPath::PATH)
            hx-trigger="previewChanged from:#precon-app"
            hx-include="#precon-supplier,#precon-period"
            hx-target="this"
            hx-swap="outerHTML"
        {
            div class="flex items-center justify-between p-5 border-b border-border-soft" {
                h3 class="flex items-center gap-2 text-sm font-semibold text-fg m-0" {
                    (icon::package_icon("w-[18px] h-[18px]")) "对账明细"
                }
                span class="text-xs text-muted" { (item_count) " 行待对账" }
            }
            div class="overflow-x-auto" {
                table class="data-table" style="min-width:1100px" {
                    thead {
                        tr {
                            th class="w-9 text-center" { "#" }
                            th { "关联订单" }
                            th { "物料编码" }
                            th { "物料名称" }
                            th class="text-right text-[13px]" { "收货数量" }
                            th class="text-right text-[13px]" { "退货数量" }
                            th class="text-right text-[13px]" { "退货冲减金额" }
                            th class="text-right text-[13px]" { "单价" }
                            th class="text-right text-[13px]" { "应付金额" }
                        }
                    }
                    tbody {
                        @for (i, item) in items.iter().enumerate() {
                            @let product = product_map.get(&item.product_id);
                            @let product_code = product.map(|p| p.code.as_str()).unwrap_or("—");
                            @let product_name = product.map(|p| p.name.as_str()).unwrap_or("—");
                            @let order_num = order_numbers.get(&item.order_id).map(|s| s.as_str()).unwrap_or("—");
                            @let order_detail = PODetailPath { id: item.order_id };
                            @let payable = item.amount - item.returned_amount;

                            tr {
                                td class="text-muted text-xs text-center" { (i + 1) }
                                td {
                                    a href=(order_detail.to_string()) class="link-accent" { (order_num) }
                                }
                                td class="font-mono tabular-nums" { (product_code) }
                                td { (product_name) }
                                td class="text-right text-[13px]" { (item.received_qty) }
                                td class="text-right text-[13px]" { (item.returned_qty) }
                                td class="text-right text-[13px] font-mono tabular-nums" {
                                    (format!("{:.2}", item.returned_amount))
                                }
                                td class="text-right text-[13px] font-mono tabular-nums" {
                                    (format!("{:.2}", item.unit_price))
                                }
                                td class="text-right text-[13px] font-mono tabular-nums" {
                                    (format!("{:.2}", payable))
                                }
                            }
                        }
                    }
                }
            }
        }
        // ── 金额汇总 ──
        div class="flex justify-end gap-8 p-5 border-t border-border-soft bg-surface-raised mb-4" {
            div {
                span class="text-xs text-muted" { "收货总额" }
                span class="block text-lg font-bold font-mono tabular-nums text-fg" {
                    (crate::utils::fmt_amount(total_amount))
                }
            }
            div {
                span class="text-xs text-muted" { "退货冲减" }
                span class="block text-lg font-bold font-mono tabular-nums text-danger" {
                    (crate::utils::fmt_amount(total_return))
                }
            }
            div {
                span class="text-xs text-muted" { "应付净额" }
                span class="block text-lg font-bold font-mono tabular-nums text-fg" {
                    (crate::utils::fmt_amount(net_amount))
                }
            }
        }
    }
}
