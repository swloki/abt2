use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::purchase::settings::{PurchaseSettingsService, model::UpdatePurchaseSettingsRequest};
use abt_core::shared::types::context::ServiceContext;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/settings")]
pub struct PurchaseSettingsPath;

#[require_permission("SUPPLIER", "read")]
pub async fn get_purchase_settings(
    _path: PurchaseSettingsPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;

    let svc = state.purchase_settings_service();
    let settings = svc.get(&service_ctx, &mut conn).await?;

    let content = settings_page(&settings);
    let page_html = admin_page(
        is_htmx, "采购参数配置", &claims, "purchase",
        PurchaseSettingsPath::PATH,
        "采购管理", Some("参数配置"), content, &nav_filter,
    );

    Ok(Html(page_html.into_string()))
}

#[derive(Debug, Deserialize)]
pub struct SettingsForm {
    pub over_delivery_allowance_pct: Option<String>,
    pub over_shortage_allowance_pct: Option<String>,
    pub maintain_same_rate: Option<String>,
    pub po_required_for_receipt: Option<String>,
    pub receipt_required_for_invoice: Option<String>,
    pub default_currency_code: Option<String>,
}

#[require_permission("SUPPLIER", "update")]
pub async fn update_purchase_settings(
    _path: PurchaseSettingsPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<SettingsForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_settings_service();

    let req = UpdatePurchaseSettingsRequest {
        over_delivery_allowance_pct: form.over_delivery_allowance_pct.and_then(|s| s.parse().ok()),
        over_shortage_allowance_pct: form.over_shortage_allowance_pct.and_then(|s| s.parse().ok()),
        maintain_same_rate: form.maintain_same_rate.map(|_| true),
        po_required_for_receipt: form.po_required_for_receipt.map(|_| true),
        receipt_required_for_invoice: form.receipt_required_for_invoice.map(|_| true),
        default_currency_code: form.default_currency_code,
        default_tax_rate_id: None,
    };

    svc.update(&service_ctx, &mut conn, req).await?;

    let redirect = PurchaseSettingsPath.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

fn settings_page(s: &abt_core::purchase::settings::model::PurchaseSettings) -> Markup {
    html! {
        div {
            div class="page-header" {
                h1 class="page-title" { "采购参数配置" }
            }
            form hx-post=(PurchaseSettingsPath::PATH) hx-swap="none" {
                div class="data-card" style="margin-bottom:var(--space-4)" {
                    div class="form-section-title" { "收货容差" }
                    div class="form-grid" {
                        div class="form-field" {
                            label { "超收容差百分比 (%)" }
                            input type="number" step="0.01" min="0" max="100"
                                name="over_delivery_allowance_pct"
                                value=(s.over_delivery_allowance_pct)
                                class="form-input" {}
                            span class="text-muted" style="font-size:var(--text-xs)" {
                                "收货数量超过订单数量的最大允许百分比，0 表示不允许超收"
                            }
                        }
                        div class="form-field" {
                            label { "超欠容差百分比 (%)" }
                            input type="number" step="0.01" min="0" max="100"
                                name="over_shortage_allowance_pct"
                                value=(s.over_shortage_allowance_pct)
                                class="form-input" {}
                            span class="text-muted" style="font-size:var(--text-xs)" {
                                "收货数量少于订单数量的最大允许百分比"
                            }
                        }
                    }
                }

                div class="data-card" style="margin-bottom:var(--space-4)" {
                    div class="form-section-title" { "业务规则" }
                    div class="form-grid" {
                        div class="form-field" {
                            label {
                                input type="checkbox" name="maintain_same_rate"
                                    checked[s.maintain_same_rate] {}
                                " 启用价格一致性校验"
                            }
                            span class="text-muted" style="font-size:var(--text-xs)" {
                                "确认订单时校验单价是否与关联报价单一致"
                            }
                        }
                        div class="form-field" {
                            label {
                                input type="checkbox" name="po_required_for_receipt"
                                    checked[s.po_required_for_receipt] {}
                                " 收货必须关联采购订单"
                            }
                        }
                        div class="form-field" {
                            label {
                                input type="checkbox" name="receipt_required_for_invoice"
                                    checked[s.receipt_required_for_invoice] {}
                                " 开票前必须完成收货"
                            }
                        }
                        div class="form-field" {
                            label { "默认币种" }
                            select name="default_currency_code" class="form-select" {
                                option value="CNY" selected[s.default_currency_code == "CNY"] { "CNY 人民币" }
                                option value="USD" selected[s.default_currency_code == "USD"] { "USD 美元" }
                                option value="EUR" selected[s.default_currency_code == "EUR"] { "EUR 欧元" }
                            }
                        }
                    }
                }

                div style="display:flex;gap:var(--space-3);padding:var(--space-4)" {
                    button type="submit" class="btn btn-primary" { "保存配置" }
                    a class="btn btn-default" href="/admin/purchase/orders" { "返回" }
                }
            }
        }
    }
}
