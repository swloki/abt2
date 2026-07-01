use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::html;
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

#[require_permission("INSPECTION", "read")]
pub async fn get_inspection_detail(path: InspectionDetailPath, ctx: RequestContext) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
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

 let content = html! {
    div {
        a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150 mb-4"
            href=(format!("{}?restore=true", InspectionListPath::PATH))
        { "← 返回列表" }
        // 标题行：单号 + 结果 pill
        div class="flex items-center justify-between flex-wrap gap-3 mb-5" {
            h1 class="text-xl font-bold text-fg tracking-tight" {
                "检验单 " span class="font-mono" { (insp.doc_number) }
            }
            span
                style=(format!(
                    "display:inline-flex;align-items:center;padding:3px 10px;border-radius:var(--radius-pill);font-size:var(--text-xs);font-weight:600;background:{};color:{}",
                    rb, rc,
                ))
            { (rl) }
        }
        // 基本信息（多列网格）
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
            div class="text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "基本信息" }
            div class="grid grid-cols-2 lg:grid-cols-4 gap-5" {
                div {
                    div class="text-xs text-muted mb-1.5" { "单号" }
                    div class="text-sm text-fg font-mono tabular-nums" { (insp.doc_number) }
                }
                div {
                    div class="text-xs text-muted mb-1.5" { "工单" }
                    div class="text-sm text-fg font-mono" { (wo) }
                }
                div {
                    div class="text-xs text-muted mb-1.5" { "产品" }
                    div class="text-sm text-fg" { (product) }
                }
                div {
                    div class="text-xs text-muted mb-1.5" { "检验类型" }
                    div class="text-sm text-fg" { (type_label) }
                }
                div {
                    div class="text-xs text-muted mb-1.5" { "样本数量" }
                    div class="text-sm text-fg font-mono tabular-nums" {
                        (crate::utils::fmt_qty(insp.sample_qty))
                    }
                }
                div {
                    div class="text-xs text-muted mb-1.5" { "合格数量" }
                    div class="text-sm text-success font-mono tabular-nums" {
                        (crate::utils::fmt_qty(insp.qualified_qty))
                    }
                }
                div {
                    div class="text-xs text-muted mb-1.5" { "不合格数量" }
                    div class="text-sm text-danger font-mono tabular-nums" {
                        (crate::utils::fmt_qty(insp.unqualified_qty))
                    }
                }
                div {
                    div class="text-xs text-muted mb-1.5" { "检验员" }
                    div class="text-sm text-fg" { (inspector) }
                }
                div {
                    div class="text-xs text-muted mb-1.5" { "检验日期" }
                    div class="text-sm text-fg font-mono" { (insp.inspection_date) }
                }
            }
        }
        // 记录检验结果
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
            div class="text-sm font-semibold text-fg mb-4 pb-3 border-b border-border-soft" { "记录检验结果" }
            form
                hx-post=(InspectionRecordResultPath { inspection_id: insp.id }.to_string())
                hx-swap="none"
                class="flex items-center gap-3"
            {
                select
                    class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent w-[200px]"
                    name="result"
                {
                    option value="1" { "合格" }
                    option value="2" { "不合格" }
                    option value="3" { "让步接收" }
                }
                button
                    type="submit"
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                { "提交" }
            }
        }
    }
};
 Ok(Html(admin_page(is_htmx, "检验详情", &claims, "production", &format!("/admin/mes/inspections/{}", path.id), "生产管理", Some(InspectionListPath::PATH), content, &nav_filter).into_string()))
}

#[require_permission("INSPECTION", "update")]
pub async fn record_result(
 path: InspectionRecordResultPath, ctx: RequestContext, axum::Form(form): axum::Form<RecordResultForm>,
) -> Result<impl IntoResponse> {
 let RequestContext { state, service_ctx, .. } = ctx;
 let result = abt_core::mes::enums::InspectionResultType::from_i16(form.result)
 .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效检验结果".into()))?;
 let mut tx = state.pool.begin().await
 .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 state.production_inspection_service().record_result(&service_ctx, &mut tx, path.inspection_id, result).await?;
 tx.commit().await
 .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 Ok(axum::response::Response::builder().header("HX-Redirect", &format!("/admin/mes/inspections/{}", path.inspection_id)).body(axum::body::Body::empty()).unwrap())
}
