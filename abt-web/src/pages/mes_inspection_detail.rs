use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::mes::production_inspection::ProductionInspectionService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_inspection::{InspectionDetailPath, InspectionListPath, InspectionRecordResultPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

fn insp_result_label(r: &abt_core::mes::enums::InspectionResultType) -> (&'static str, &'static str, &'static str) {
    match r {
        abt_core::mes::enums::InspectionResultType::Pass => ("合格", "rgba(82,196,26,0.08)", "var(--success)"),
        abt_core::mes::enums::InspectionResultType::Fail => ("不合格", "rgba(245,63,63,0.06)", "#f53f3f"),
        abt_core::mes::enums::InspectionResultType::Conditional => ("让步接收", "rgba(250,140,22,0.08)", "#fa8c16"),
    }
}

#[derive(Debug, Deserialize)]
pub struct RecordResultForm {
    pub result: i16,
}

#[require_permission("MES", "read")]
pub async fn get_inspection_detail(path: InspectionDetailPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.production_inspection_service();
    let insp = svc.find_by_id(&service_ctx, &mut conn, path.id).await?;
    let lookups = svc.get_detail_lookups(&mut conn, &insp).await?;

    let type_label = match insp.inspection_type {
        abt_core::mes::enums::InspectionType::FirstArticle => "首检",
        abt_core::mes::enums::InspectionType::InProcess => "巡检",
        _ => "完工检",
    };
    let (rl, rb, rc) = insp_result_label(&insp.result);

    let wo = lookups.wo_doc_number.as_deref().unwrap_or("—");
    let product = lookups.product_name.as_deref().unwrap_or("—");
    let inspector = lookups.inspector_name.as_deref().unwrap_or("—");

    let content = html! { div {
        div class="page-header" {
            div class="page-header-left" { a class="back-link" href=(InspectionListPath::PATH) { "\u{2190} 返回列表" } h1 class="page-title" { "检验 " (insp.doc_number) } }
        }
        div class="info-card" {
            div class="info-grid" {
                div class="info-item" { label { "单号" } span class="mono" { (insp.doc_number) } }
                div class="info-item" { label { "工单" } span { (wo) } }
                div class="info-item" { label { "产品" } span { (product) } }
                div class="info-item" { label { "检验类型" } span { (type_label) } }
                div class="info-item" { label { "样本数量" } span class="mono" { (crate::utils::fmt_qty(insp.sample_qty)) } }
                div class="info-item" { label { "合格数量" } span class="mono" { (crate::utils::fmt_qty(insp.qualified_qty)) } }
                div class="info-item" { label { "不合格数量" } span class="mono" { (crate::utils::fmt_qty(insp.unqualified_qty)) } }
                div class="info-item" { label { "结果" } span style=(format!("display:inline-flex;padding:2px 8px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:500;background:{};color:{}", rb, rc)) { (rl) } }
                div class="info-item" { label { "检验员" } span { (inspector) } }
                div class="info-item" { label { "检验日期" } span { (insp.inspection_date) } }
            }
        }
        div class="form-section" style="margin-top:var(--space-6)" {
            div class="form-section-title" { "记录检验结果" }
            form hx-post=(format!("/admin/mes/inspections/{}/record-result", insp.id)) hx-swap="none" {
                select class="form-select" name="result" style="width:200px;display:inline-block" {
                    option value="1" { "合格" } option value="2" { "不合格" } option value="3" { "让步接收" }
                }
                button type="submit" class="btn btn-primary" style="margin-left:var(--space-3)" { "提交" }
            }
        }
    }};
    Ok(Html(admin_page(is_htmx, "检验详情", &claims, "production", &format!("/admin/mes/inspections/{}", path.id), "生产管理", Some(InspectionListPath::PATH), content).into_string()))
}

#[require_permission("MES", "write")]
pub async fn record_result(
    path: InspectionRecordResultPath, ctx: RequestContext, axum::Form(form): axum::Form<RecordResultForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let result = abt_core::mes::enums::InspectionResultType::from_i16(form.result)
        .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效检验结果".into()))?;
    state.production_inspection_service().record_result(&service_ctx, &mut conn, path.inspection_id, result).await?;
    Ok(axum::response::Response::builder().header("HX-Redirect", &format!("/admin/mes/inspections/{}", path.inspection_id)).body(axum::body::Body::empty()).unwrap())
}
