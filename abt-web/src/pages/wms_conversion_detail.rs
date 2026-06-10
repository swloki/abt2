use std::collections::HashMap;

use axum::response::Html;
use maud::{html, Markup};

use crate::errors::Result;
use crate::routes::wms_conversion::ConversionDetailPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;
use crate::layout::page::admin_page;

use abt_core::wms::enums::{ConversionDir, ConversionStatus};
use abt_core::wms::form_conversion::{ConversionItem, FormConversionService};
use abt_core::master_data::product::ProductService;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::shared::identity::UserService;
use crate::components::icon;

// ── Resolved Product Info ──

struct ProductInfo {
    codes: HashMap<i64, String>,
    names: HashMap<i64, String>,
    specs: HashMap<i64, String>,
    units: HashMap<i64, String>,
}

impl ProductInfo {
    fn code(&self, id: &i64) -> &str { self.codes.get(id).map(|s| s.as_str()).unwrap_or("—") }
    fn name(&self, id: &i64) -> &str { self.names.get(id).map(|s| s.as_str()).unwrap_or("—") }
    fn spec(&self, id: &i64) -> &str { self.specs.get(id).map(|s| s.as_str()).unwrap_or("—") }
    fn unit(&self, id: &i64) -> &str { self.units.get(id).map(|s| s.as_str()).unwrap_or("—") }
}

// ── Form Data ──

#[derive(Debug, serde::Deserialize)]
pub struct ConversionActionForm {
    pub action: String,
}

// ── Handlers ──

#[require_permission("WMS", "read")]
pub async fn get_conversion_detail(
    path: ConversionDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.form_conversion_service();

    let conversion = svc.get(&service_ctx, &mut conn, path.id).await?;
    let items = svc.get_items(&service_ctx, &mut conn, path.id).await?;

    // Resolve warehouse name
    let wh_name = state.warehouse_service()
        .get(&service_ctx, &mut conn, conversion.warehouse_id)
        .await
        .map(|w| w.name)
        .unwrap_or_else(|_| "—".into());

    // Resolve operator name
    let operator_name = state.user_service()
        .get_user(&service_ctx, &mut conn, conversion.operator_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());

    // Resolve product names for all items
    let product_svc = state.product_service();
    let mut product_names: HashMap<i64, String> = HashMap::new();
    let mut product_specs: HashMap<i64, String> = HashMap::new();
    let mut product_units: HashMap<i64, String> = HashMap::new();
    let mut product_codes: HashMap<i64, String> = HashMap::new();
    for item in &items {
        if product_names.contains_key(&item.product_id) {
            continue;
        }
        if let Ok(p) = product_svc.get(&service_ctx, &mut conn, item.product_id).await {
            product_codes.insert(item.product_id, p.product_code.clone());
            product_names.insert(item.product_id, p.pdt_name.clone());
            product_specs.insert(item.product_id, p.meta.specification.clone());
            product_units.insert(item.product_id, p.unit.clone());
        }
    }

    let detail_path = ConversionDetailPath { id: path.id }.to_string();
    let product_info = ProductInfo { codes: product_codes, names: product_names, specs: product_specs, units: product_units };
    let content = conversion_detail_page(
        &conversion, &items, &detail_path,
        &wh_name, &operator_name, &product_info,
    );
    let page_html = admin_page(
        is_htmx,
        "形态转换详情",
        &claims,
        "inventory",
        "/admin/wms/conversions",
        "库存管理",
        None,
        content, &nav_filter,    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("WMS", "write")]
pub async fn post_conversion_action(
    path: ConversionDetailPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ConversionActionForm>,
) -> crate::errors::Result<axum::response::Response> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.form_conversion_service();

    match form.action.as_str() {
        "complete" => svc.complete(&service_ctx, &mut conn, path.id).await?,
        "cancel" => svc.cancel(&service_ctx, &mut conn, path.id).await?,
        _ => {}
    }

    let redirect_url = ConversionDetailPath { id: path.id }.to_string();
    let mut resp = axum::response::Response::default();
    resp.headers_mut().insert(
        axum::http::HeaderName::from_static("hx-redirect"),
        redirect_url.parse().unwrap(),
    );

    Ok(resp)
}

// ── Components ──

fn conversion_detail_page(
    conversion: &abt_core::wms::form_conversion::FormConversion,
    items: &[ConversionItem],
    detail_path: &str,
    wh_name: &str,
    operator_name: &str,
    product_info: &ProductInfo,
) -> Markup {
    let (status_label, status_class) = match conversion.status {
        ConversionStatus::Draft => ("草稿", "status-draft"),
        ConversionStatus::Completed => ("已完成", "status-completed"),
        ConversionStatus::Cancelled => ("已取消", "status-cancelled"),
    };

    let consume_items: Vec<_> = items.iter().filter(|i| i.direction == ConversionDir::Consume).collect();
    let produce_items: Vec<_> = items.iter().filter(|i| i.direction == ConversionDir::Produce).collect();

    html! {
        div {
            a href="/admin/wms/conversions" class="back-link" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回形态转换列表"
            }

            div class="detail-header" {
                div {
                    div class="detail-title-row" {
                        h1 class="detail-no font-mono" { (conversion.doc_number) }
                        span class=(format!("status-pill {status_class}")) { (status_label) }
                    }
                }
                div class="page-actions" {
                    (conversion_action_buttons(conversion.status, detail_path))
                }
            }

            // ── Workflow Steps ──
            (conversion_workflow_steps(conversion.status))

            // ── Info Card ──
            div class="info-card" {
                div class="info-card-title" { "转换信息" }
                div class="info-grid" {
                    div class="info-item" {
                        span class="info-label" { "转换单号" }
                        span class="info-value mono" { (conversion.doc_number) }
                    }
                    div class="info-item" {
                        span class="info-label" { "转换仓库" }
                        span class="info-value" { (wh_name) }
                    }
                    div class="info-item" {
                        span class="info-label" { "转换日期" }
                        span class="info-value mono" { (conversion.conversion_date.to_string()) }
                    }
                    div class="info-item" {
                        span class="info-label" { "操作员" }
                        span class="info-value" { (operator_name) }
                    }
                }
            }

            // ── Consume Items ──
            div class="info-card" {
                div class="info-card-title" {
                    "消耗物料 "
                    span class="status-pill status-cancelled" { "消耗" }
                }
                table class="data-table" {
                    thead {
                        tr {
                            th { "行号" }
                            th { "产品编码" }
                            th { "名称" }
                            th { "规格" }
                            th { "单位" }
                            th class="num-right" { "消耗数量" }
                            th class="num-right" { "单位成本" }
                            th { "批次号" }
                        }
                    }
                    tbody {
                        @for (i, item) in consume_items.iter().enumerate() {
                            tr {
                                td class="mono" { (i + 1) }
                                td class="mono" { (product_info.code(&item.product_id)) }
                                td { (product_info.name(&item.product_id)) }
                                td { (product_info.spec(&item.product_id)) }
                                td { (product_info.unit(&item.product_id)) }
                                td class="num-right" { (format!("{:.2}", item.quantity)) }
                                td class="num-right" { (format!("{:.2}", item.unit_cost)) }
                                td class="mono" {
                                    @if let Some(ref batch) = item.batch_no {
                                        (batch)
                                    } @else {
                                        "—"
                                    }
                                }
                            }
                        }
                        @if consume_items.is_empty() {
                            tr {
                                td colspan="8" class="empty-cell" {
                                    "暂无消耗物料"
                                }
                            }
                        }
                    }
                }
            }

            // ── Produce Items ──
            div class="info-card" {
                div class="info-card-title" {
                    "产出物料 "
                    span class="status-pill status-completed" { "产出" }
                }
                table class="data-table" {
                    thead {
                        tr {
                            th { "行号" }
                            th { "产品编码" }
                            th { "名称" }
                            th { "规格" }
                            th { "单位" }
                            th class="num-right" { "产出数量" }
                            th class="num-right" { "单位成本" }
                            th { "批次号" }
                        }
                    }
                    tbody {
                        @for (i, item) in produce_items.iter().enumerate() {
                            tr {
                                td class="mono" { (i + 1) }
                                td class="mono" { (product_info.code(&item.product_id)) }
                                td { (product_info.name(&item.product_id)) }
                                td { (product_info.spec(&item.product_id)) }
                                td { (product_info.unit(&item.product_id)) }
                                td class="num-right" { (format!("{:.2}", item.quantity)) }
                                td class="num-right" { (format!("{:.2}", item.unit_cost)) }
                                td class="mono" {
                                    @if let Some(ref batch) = item.batch_no {
                                        (batch)
                                    } @else {
                                        "—"
                                    }
                                }
                            }
                        }
                        @if produce_items.is_empty() {
                            tr {
                                td colspan="8" class="empty-cell" {
                                    "暂无产出物料"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn conversion_action_buttons(status: ConversionStatus, detail_path: &str) -> Markup {
    match status {
        ConversionStatus::Draft => {
            html! {
                button class="btn btn-default"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"cancel"}"#
                    hx-confirm="确定要取消此转换单吗？"
                    hx-redirect=(detail_path) {
                    (icon::x_icon("w-4 h-4"))
                    "取消"
                }
                button class="btn btn-primary"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"complete"}"#
                    hx-confirm="确定要完成形态转换吗？"
                    hx-redirect=(detail_path) {
                    (icon::check_circle_icon("w-4 h-4"))
                    "确认完成"
                }
            }
        }
        _ => html! {},
    }
}

fn conversion_workflow_steps(status: ConversionStatus) -> Markup {
    let steps = [
        ("草稿", ConversionStatus::Draft),
        ("已完成", ConversionStatus::Completed),
    ];

    let current_idx = match status {
        ConversionStatus::Draft => 0,
        ConversionStatus::Completed => 1,
        ConversionStatus::Cancelled => 0,
    };

    html! {
        div class="workflow-steps" {
            @for (i, (label, _)) in steps.iter().enumerate() {
                @if i > 0 {
                    div class=(if i <= current_idx { "wf-line completed" } else { "wf-line" }) {}
                }
                div class={
                    @if i < current_idx { "wf-step completed" }
                    @else if i == current_idx { "wf-step current" }
                    @else { "wf-step" }
                } {
                    span class="wf-dot" {}
                    (label)
                }
            }
        }
    }
}
