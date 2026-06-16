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
        div class="flex items-center justify-between mb-6" {
            div class="flex items-center justify-between mb-6-left" { a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", InspectionListPath::PATH)) { "\u{2190} 返回列表" } h1 class="text-xl font-bold text-fg tracking-tight" { "新建检验" } }
        }
        form hx-post=(InspectionCreatePath::PATH) hx-swap="none" {
            div class="form-section" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "检验信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" { label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "工单ID" } input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" name="work_order_id" required; }
                    div class="form-field" { label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "产品ID" } input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" name="product_id" required; }
                    div class="form-field" { label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "工序ID" } input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" name="routing_id"; }
                    div class="form-field" { label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "检验类型" } select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" name="inspection_type" {
                        option value="1" { "首检" } option value="2" { "巡检" } option value="3" { "完工检" }
                    }}
                    div class="form-field" { label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "样本数量" } input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" step="0.01" name="sample_qty" required; }
                    div class="form-field" { label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "检验日期" } input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="date" name="inspection_date" required; }
                    div class="form-field span-2" { label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "处置意见" } input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="disposition"; }
                }
            }
            div class="flex items-center justify-end gap-3 pt-4 border-t border-border-soft" {
                a class="btn bg-white text-fg border border-border hover:bg-surface" href=(format!("{}?restore=true", InspectionListPath::PATH)) { "取消" }
                button type="submit" class="btn bg-accent text-accent-on border-none hover:bg-accent-hover" { "提交" }
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
