use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::html;

use abt_core::master_data::product::ProductService;
use abt_core::qms::enums::{InspectionType, SpecStatus};
use abt_core::qms::inspection_specification::InspectionSpecificationService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::qms::{SpecDetailPath, SpecListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn inspection_type_label(t: &InspectionType) -> &'static str {
    match t {
        InspectionType::Iqc => "IQC (来料检验)",
        InspectionType::Ipqc => "IPQC (过程检验)",
        InspectionType::Fqc => "FQC (成品检验)",
        InspectionType::Oqc => "OQC (出货检验)",
    }
}

fn spec_status_label(s: &SpecStatus) -> (&'static str, &'static str) {
    match s {
        SpecStatus::Draft => ("草稿", "status-draft"),
        SpecStatus::Active => ("生效", "status-active"),
        SpecStatus::Inactive => ("停用", "status-inactive"),
    }
}

// ── Handler ──

#[require_permission("QMS", "read")]
pub async fn get_detail(path: SpecDetailPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    let svc = state.inspection_specification_service();
    let spec = svc.get(&service_ctx, &mut conn, path.id).await?;

    let product_name = state
        .product_service()
        .get(&service_ctx, &mut conn, spec.product_id)
        .await
        .map(|p| p.pdt_name)
        .unwrap_or_else(|_| "—".into());

    let (status_text, status_class) = spec_status_label(&spec.status);

    let content = html! { div {
        div class="flex items-center justify-between mb-6" {
            div class="flex items-center justify-between mb-6-left" {
                a class="inline-flex items-center gap-2 text-sm text-text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", SpecListPath::PATH)) { "\u{2190} 返回列表" }
                h1 class="text-xl font-bold text-fg tracking-tight" {
                    "单号 " (spec.doc_number)
                    " "
                    span class=(format!("status-pill {status_class}")) { (status_text) }
                }
            }
        }

        // ── 基本信息 ──
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
            h3 { "基本信息" }
            div class="grid gap-4" {
                div class="flex flex-col gap-1" { label { "产品" } span { (product_name) } }
                div class="flex flex-col gap-1" { label { "检验类型" } span { (inspection_type_label(&spec.inspection_type)) } }
                div class="flex flex-col gap-1" { label { "版本" } span class="font-mono tabular-nums" { "V" (spec.version) } }
                div class="flex flex-col gap-1" { label { "状态" } span { (status_text) } }
                div class="flex flex-col gap-1" { label { "创建时间" } span { (spec.created_at.format("%Y-%m-%d %H:%M")) } }
            }
        }

        // ── 抽样方案 ──
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
            h3 { "抽样方案" }
            div class="grid gap-4" {
                div class="flex flex-col gap-1" { label { "检验水平" } span { (&spec.sample_plan.level) } }
                div class="flex flex-col gap-1" { label { "AQL" } span class="font-mono tabular-nums" { (spec.sample_plan.aql.to_string()) } }
                div class="flex flex-col gap-1" { label { "抽样模式" } span { (&spec.sample_plan.mode) } }
            }
        }

        // ── 检验项目 ──
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" {
            h3 { "检验项目" }
            @if spec.check_items.is_empty() {
                p { "暂无检验项目" }
            } @else {
                table class="data-table" {
                    thead {
                        tr {
                            th { "序号" }
                            th { "检验项目" }
                            th { "检验标准" }
                            th { "公差" }
                            th { "检验方法" }
                        }
                    }
                    tbody {
                        @for (i, item) in spec.check_items.iter().enumerate() {
                            tr {
                                td class="font-mono tabular-nums" { (i + 1) }
                                td { (&item.item) }
                                td { (&item.standard) }
                                td { (&item.tolerance) }
                                td { (&item.method) }
                            }
                        }
                    }
                }
            }
        }
    }};

    let current_path = SpecDetailPath { id: path.id }.to_string();
    let html = admin_page(
        is_htmx,
        "检验规格详情",
        &claims,
        "quality",
        &current_path,
        "质量管理",
        Some(SpecListPath::PATH),
        content, &nav_filter,    );
    Ok(Html(html.into_string()))
}
