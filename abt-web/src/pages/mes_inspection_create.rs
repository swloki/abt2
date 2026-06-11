use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::html;
use serde::Deserialize;

use abt_core::mes::production_inspection::ProductionInspectionService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_inspection::{InspectionCreatePath, InspectionListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[derive(Debug, Deserialize)]
pub struct InspectionCreateForm {
    pub work_order_id: i64,
    pub product_id: i64,
    pub routing_id: Option<i64>,
    pub inspection_type: i16,
    pub sample_qty: rust_decimal::Decimal,
    pub inspection_date: chrono::NaiveDate,
    pub disposition: Option<String>,
    pub remark: Option<String>,
}

#[require_permission("INSPECTION", "create")]
pub async fn get_inspection_create(_path: InspectionCreatePath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, .. } = ctx;
    let content = html! { div {
        div class="page-header" {
            div class="page-header-left" { a class="back-link" href=(InspectionListPath::PATH) { "\u{2190} 返回列表" } h1 class="page-title" { "新建检验" } }
        }
        form hx-post=(InspectionCreatePath::PATH) hx-swap="none" {
            div class="form-section" {
                div class="form-section-title" { "检验信息" }
                div class="form-grid" {
                    div class="form-field" { label class="form-label" { "工单ID" } input class="form-input" type="number" name="work_order_id" required; }
                    div class="form-field" { label class="form-label" { "产品ID" } input class="form-input" type="number" name="product_id" required; }
                    div class="form-field" { label class="form-label" { "工序ID" } input class="form-input" type="number" name="routing_id"; }
                    div class="form-field" { label class="form-label" { "检验类型" } select class="form-select" name="inspection_type" {
                        option value="1" { "首检" } option value="2" { "巡检" } option value="3" { "完工检" }
                    }}
                    div class="form-field" { label class="form-label" { "样本数量" } input class="form-input" type="number" step="0.01" name="sample_qty" required; }
                    div class="form-field" { label class="form-label" { "检验日期" } input class="form-input" type="date" name="inspection_date" required; }
                    div class="form-field span-2" { label class="form-label" { "处置意见" } input class="form-input" type="text" name="disposition"; }
                }
            }
            div class="create-action-bar" {
                a class="btn btn-default" href=(InspectionListPath::PATH) { "取消" }
                button type="submit" class="btn btn-primary" { "提交" }
            }
        }
    }};
    Ok(Html(admin_page(is_htmx, "新建检验", &claims, "production", InspectionCreatePath::PATH, "生产管理", Some(InspectionListPath::PATH), content, &nav_filter).into_string()))
}

#[require_permission("INSPECTION", "create")]
pub async fn create_inspection(
    _path: InspectionCreatePath, ctx: RequestContext, axum::Form(form): axum::Form<InspectionCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.production_inspection_service();
    let insp_type = abt_core::mes::enums::InspectionType::from_i16(form.inspection_type)
        .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效检验类型".into()))?;
    let req = abt_core::mes::production_inspection::CreateInspectionReq {
        work_order_id: form.work_order_id,
        product_id: form.product_id,
        routing_id: form.routing_id,
        inspection_type: insp_type,
        sample_qty: form.sample_qty,
        inspection_date: form.inspection_date,
        disposition: form.disposition,
        remark: form.remark,
    };
    let _id = svc.create(&service_ctx, &mut conn, req).await?;
    Ok(axum::response::Response::builder().header("HX-Redirect", InspectionListPath::PATH).body(axum::body::Body::empty()).unwrap())
}
