use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::purchase::supplier_price::SupplierPriceService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/supplier-prices")]
pub struct SupplierPricesPath;

#[derive(Debug, Deserialize)]
pub struct PriceQuery {
    pub product_id: Option<i64>,
    pub supplier_id: Option<i64>,
}

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_supplier_prices(
    _path: SupplierPricesPath,
    ctx: RequestContext,
    axum::extract::Query(params): axum::extract::Query<PriceQuery>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;
    let svc = state.supplier_price_service();

    let prices = if let Some(pid) = params.product_id {
        svc.list_by_product(&service_ctx, &mut conn, pid).await.unwrap_or_default()
    } else if let Some(sid) = params.supplier_id {
        svc.list_by_supplier(&service_ctx, &mut conn, sid).await.unwrap_or_default()
    } else {
        Vec::new()
    };

    let content = prices_page(&prices);
    let page_html = admin_page(
        is_htmx, "供应商价格目录", &claims, "purchase",
        SupplierPricesPath::PATH,
        "采购管理", Some("供应商价格"), content, &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

#[derive(Debug, Deserialize)]
pub struct PriceForm {
    pub supplier_id: String,
    pub product_id: String,
    pub price: String,
    pub currency_code: Option<String>,
}

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn create_price(
    _path: SupplierPricesPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<PriceForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.supplier_price_service();
    let supplier_id: i64 = form.supplier_id.parse()
        .map_err(|_| abt_core::shared::types::DomainError::validation("无效供应商ID"))?;
    let product_id: i64 = form.product_id.parse()
        .map_err(|_| abt_core::shared::types::DomainError::validation("无效产品ID"))?;
    let price: rust_decimal::Decimal = form.price.parse()
        .map_err(|_| abt_core::shared::types::DomainError::validation("无效价格"))?;
    let currency = form.currency_code.unwrap_or_else(|| "CNY".into());
    svc.create_price(&service_ctx, &mut conn, supplier_id, product_id, price, currency).await?;
    let redirect = SupplierPricesPath.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/supplier-prices/{id}/delete")]
pub struct PriceDeletePath { pub id: i64 }

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn delete_price(
    path: PriceDeletePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.supplier_price_service();
    svc.delete_price(&service_ctx, &mut conn, path.id).await?;
    let redirect = SupplierPricesPath.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

fn prices_page(prices: &[abt_core::purchase::supplier_price::model::SupplierProductPrice]) -> Markup {
    html! {
        div {
            div class="page-header" {
                h1 class="page-title" { "供应商价格目录" }
            }
            div class="data-card" style="margin-bottom:var(--space-4)" {
                div class="form-section-title" { "新建价格记录" }
                form hx-post=(SupplierPricesPath::PATH) hx-swap="none" {
                    div class="form-grid" {
                        div class="form-field" { label { "供应商ID" } input type="number" name="supplier_id" required class="form-input" {} }
                        div class="form-field" { label { "产品ID" } input type="number" name="product_id" required class="form-input" {} }
                        div class="form-field" { label { "价格" } input type="number" step="0.0001" name="price" required class="form-input" {} }
                        div class="form-field" {
                            label { "币种" }
                            select name="currency_code" class="form-select" {
                                option value="CNY" selected { "CNY" }
                                option value="USD" { "USD" }
                                option value="EUR" { "EUR" }
                            }
                        }
                    }
                    div style="padding:var(--space-3)" {
                        button type="submit" class="btn btn-primary" { "创建价格" }
                    }
                }
            }
            div class="data-card" {
                div class="form-section-title" { "价格列表" }
                @if prices.is_empty() {
                    p style="color:var(--text-tertiary);padding:var(--space-4)" { "暂无价格记录。可通过 URL 参数 ?product_id=X 或 ?supplier_id=X 筛选查看。" }
                } @else {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "供应商ID" }
                                th { "产品ID" }
                                th style="text-align:right" { "价格" }
                                th { "币种" }
                                th style="text-align:right" { "起订量" }
                                th { }
                            }
                        }
                        tbody {
                            @for p in prices {
                                tr {
                                    td { (p.supplier_id) }
                                    td { (p.product_id) }
                                    td style="text-align:right" { (p.price) }
                                    td { (&p.currency_code) }
                                    td style="text-align:right" { (p.min_order_qty) }
                                    td {
                                        button class="btn btn-sm btn-danger"
                                            hx-post=(PriceDeletePath { id: p.id }.to_string())
                                            hx-confirm="确认删除？" { "删除" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
