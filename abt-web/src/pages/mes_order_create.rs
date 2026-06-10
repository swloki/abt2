use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::work_order::WorkOrderService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_order::{OrderCreatePath, OrderListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[derive(Debug, Deserialize)]
pub struct OrderCreateForm {
    pub product_id: String,
    pub planned_qty: String,
    pub scheduled_start: String,
    pub scheduled_end: String,
    pub work_center_id: Option<String>,
    pub remark: Option<String>,
}

#[require_permission("WORK_ORDER", "create")]
pub async fn get_order_create(
    _path: OrderCreatePath, ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, .. } = ctx;
    let content = order_create_page();
    Ok(Html(admin_page(is_htmx, "新建工单", &claims, "production", OrderCreatePath::PATH, "生产管理", Some(OrderListPath::PATH), content, &nav_filter).into_string()))
}

#[require_permission("WORK_ORDER", "create")]
pub async fn create_order(
    _path: OrderCreatePath, ctx: RequestContext,
    axum::Form(form): axum::Form<OrderCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.work_order_service();
    let req = abt_core::mes::work_order::CreateWorkOrderReq {
        plan_item_id: None,
        product_id: form.product_id.parse().map_err(|_| abt_core::shared::types::DomainError::Validation("无效产品ID".into()))?,
        bom_snapshot_id: None,
        routing_id: None,
        planned_qty: form.planned_qty.parse().map_err(|_| abt_core::shared::types::DomainError::Validation("无效数量".into()))?,
        scheduled_start: form.scheduled_start.parse().map_err(|_| abt_core::shared::types::DomainError::Validation("无效开始日期".into()))?,
        scheduled_end: form.scheduled_end.parse().map_err(|_| abt_core::shared::types::DomainError::Validation("无效结束日期".into()))?,
        work_center_id: form.work_center_id.and_then(|s| s.parse().ok()),
        sales_order_id: None,
        remark: form.remark,
    };
    let _id = svc.create(&service_ctx, &mut conn, req).await?;
    Ok(axum::response::Response::builder().header("HX-Redirect", OrderListPath::PATH).body(axum::body::Body::empty()).unwrap())
}

fn order_create_page() -> Markup {
    html! { div {
        div class="page-header" {
            div class="page-header-left" { a class="back-link" href=(OrderListPath::PATH) { "\u{2190} 返回列表" } h1 class="page-title" { "新建工单" } }
        }
        form hx-post=(OrderCreatePath::PATH) hx-swap="none" {
            div class="form-section" {
                div class="form-section-title" { "基本信息" }
                div class="form-grid" {
                    div class="form-field" { label class="form-label" { "产品ID" } input class="form-input" type="number" name="product_id" required; }
                    div class="form-field" { label class="form-label" { "计划数量" } input class="form-input" type="number" step="0.01" name="planned_qty" required; }
                    div class="form-field" { label class="form-label" { "开始日期" } input class="form-input" type="date" name="scheduled_start" required; }
                    div class="form-field" { label class="form-label" { "结束日期" } input class="form-input" type="date" name="scheduled_end" required; }
                    div class="form-field" { label class="form-label" { "工作中心ID" } input class="form-input" type="number" name="work_center_id"; }
                    div class="form-field span-2" { label class="form-label" { "备注" } textarea class="form-input" name="remark" rows="2" {}; }
                }
            }
            div class="create-action-bar" {
                a class="btn btn-default" href=(OrderListPath::PATH) { "取消" }
                button type="submit" class="btn btn-primary" { "提交" }
            }
        }
    }}
}
