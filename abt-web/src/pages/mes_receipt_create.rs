use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::html;
use serde::Deserialize;

use abt_core::mes::production_receipt::ProductionReceiptService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_receipt::{ReceiptCreatePath, ReceiptListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[derive(Debug, Deserialize)]
pub struct ReceiptCreateForm {
    pub work_order_id: i64,
    pub batch_id: Option<i64>,
    pub product_id: i64,
    pub received_qty: rust_decimal::Decimal,
    pub warehouse_id: i64,
    pub zone_id: Option<i64>,
    pub bin_id: Option<i64>,
    pub receipt_date: chrono::NaiveDate,
    pub remark: Option<String>,
}

#[require_permission("WORK_ORDER", "create")]
pub async fn get_receipt_create(_path: ReceiptCreatePath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, .. } = ctx;
    let content = html! { div {
        div class="page-header" {
            div class="page-header-left" { a class="back-link" href=(ReceiptListPath::PATH) { "\u{2190} 返回列表" } h1 class="page-title" { "新建入库" } }
        }
        form hx-post=(ReceiptCreatePath::PATH) hx-swap="none" {
            div class="form-section" {
                div class="form-section-title" { "入库信息" }
                div class="form-grid" {
                    div class="form-field" { label class="form-label" { "工单ID" } input class="form-input" type="number" name="work_order_id" required; }
                    div class="form-field" { label class="form-label" { "批次ID" } input class="form-input" type="number" name="batch_id"; }
                    div class="form-field" { label class="form-label" { "产品ID" } input class="form-input" type="number" name="product_id" required; }
                    div class="form-field" { label class="form-label" { "入库数量" } input class="form-input" type="number" step="0.01" name="received_qty" required; }
                    div class="form-field" { label class="form-label" { "仓库ID" } input class="form-input" type="number" name="warehouse_id" required; }
                    div class="form-field" { label class="form-label" { "库区ID" } input class="form-input" type="number" name="zone_id"; }
                    div class="form-field" { label class="form-label" { "储位ID" } input class="form-input" type="number" name="bin_id"; }
                    div class="form-field" { label class="form-label" { "入库日期" } input class="form-input" type="date" name="receipt_date" required; }
                }
            }
            div class="create-action-bar" {
                a class="btn btn-default" href=(ReceiptListPath::PATH) { "取消" }
                button type="submit" class="btn btn-primary" { "提交" }
            }
        }
    }};
    Ok(Html(admin_page(is_htmx, "新建入库", &claims, "production", ReceiptCreatePath::PATH, "生产管理", Some(ReceiptListPath::PATH), content, &nav_filter).into_string()))
}

#[require_permission("WORK_ORDER", "create")]
pub async fn create_receipt(
    _path: ReceiptCreatePath, ctx: RequestContext, axum::Form(form): axum::Form<ReceiptCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_receipt_service();
    let req = abt_core::mes::production_receipt::CreateReceiptReq {
        work_order_id: form.work_order_id,
        batch_id: form.batch_id,
        product_id: form.product_id,
        received_qty: form.received_qty,
        warehouse_id: form.warehouse_id,
        zone_id: form.zone_id,
        bin_id: form.bin_id,
        receipt_date: form.receipt_date,
        remark: form.remark,
    };
    let _id = svc.create(&service_ctx, &mut conn, req).await?;
    Ok(axum::response::Response::builder().header("HX-Redirect", ReceiptListPath::PATH).body(axum::body::Body::empty()).unwrap())
}
