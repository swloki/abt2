use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::html;

use abt_core::master_data::product::ProductService;
use abt_core::qms::enums::{
    InspectionResultType, InspectionSourceType, InspectionStatus, InspectionType,
};
use abt_core::qms::inspection_result::InspectionResultService;
use abt_core::qms::inspection_specification::InspectionSpecificationService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::qms::{MrbCreatePath, ResultDetailPath, ResultListPath};
use crate::utils::{fmt_qty, RequestContext};
use abt_macros::require_permission;

// ── Helpers ──

fn inspection_type_label(t: &InspectionType) -> (&'static str, &'static str) {
    match t {
        InspectionType::Iqc => ("IQC (来料检验)", "status-active"),
        InspectionType::Ipqc => ("IPQC (过程检验)", "status-completed"),
        InspectionType::Fqc => ("FQC (成品检验)", "status-draft"),
        InspectionType::Oqc => ("OQC (出货检验)", "status-warning"),
    }
}

fn source_type_label(s: &InspectionSourceType) -> &'static str {
    match s {
        InspectionSourceType::ArrivalNotice => "来料通知",
        InspectionSourceType::WorkOrderRouting => "工单工序",
        InspectionSourceType::ShippingRequest => "发货单",
        InspectionSourceType::OutsourcingOrder => "委外单",
        InspectionSourceType::ProductionReceipt => "完工入库",
    }
}

fn result_type_label(r: &InspectionResultType) -> (&'static str, &'static str) {
    match r {
        InspectionResultType::Pass => ("合格", "status-active"),
        InspectionResultType::Fail => ("不合格", "status-danger"),
        InspectionResultType::Conditional => ("让步接收", "status-info"),
    }
}

fn status_label(s: &InspectionStatus) -> (&'static str, &'static str) {
    match s {
        InspectionStatus::Pending => ("待检验", "status-draft"),
        InspectionStatus::Completed => ("已完成", "status-completed"),
        InspectionStatus::Dispositioned => ("已处置", "status-active"),
    }
}

// ── Handler ──

#[require_permission("QMS", "read")]
pub async fn get_detail(path: ResultDetailPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    let svc = state.inspection_result_service();
    let result = svc.get(&service_ctx, &mut conn, path.id).await?;

    let spec_svc = state.inspection_specification_service();
    let product_id = spec_svc
        .get(&service_ctx, &mut conn, result.spec_id)
        .await
        .map(|s| s.product_id)
        .unwrap_or(0);
    let product_name = state
        .product_service()
        .get(&service_ctx, &mut conn, product_id)
        .await
        .map(|p| p.pdt_name)
        .unwrap_or_else(|_| "—".into());

    let (status_text, status_class) = status_label(&result.status);
    let (type_text, type_class) = inspection_type_label(&result.inspection_type);
    let (result_text, result_class) = result_type_label(&result.result);

    let content = html! { div {
        div class="flex items-center justify-between mb-6" {
            div class="flex items-center justify-between mb-6-left" {
                a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", ResultListPath::PATH)) { "\u{2190} 返回列表" }
                h1 class="text-xl font-bold text-fg tracking-tight" {
                    "单号 " (&result.doc_number)
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
                div class="flex flex-col gap-1" {
                    label { "检验类型" }
                    span class=(format!("status-pill {type_class}")) { (type_text) }
                }
                div class="flex flex-col gap-1" { label { "来源类型" } span { (source_type_label(&result.source_type)) } }
                div class="flex flex-col gap-1" { label { "批次号" } span class="mono" { (&result.batch_no) } }
                div class="flex flex-col gap-1" {
                    label { "检验日期" }
                    span { (result.inspection_date.map(|d| d.to_string()).unwrap_or_else(|| "—".into())) }
                }
            }
        }

        // ── 抽样结果 ──
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
            h3 { "抽样结果" }
            div class="grid gap-4" {
                div class="flex flex-col gap-1" { label { "抽样数量" } span class="mono num-right" { (fmt_qty(result.sample_qty)) } }
                div class="flex flex-col gap-1" { label { "合格数量" } span class="mono num-right" { (fmt_qty(result.qualified_qty)) } }
                div class="flex flex-col gap-1" { label { "不合格数量" } span class="mono num-right" { (fmt_qty(result.unqualified_qty)) } }
                div class="flex flex-col gap-1" {
                    label { "检验结果" }
                    span class=(format!("status-pill {result_class}")) { (result_text) }
                }
            }
        }

        // ── 检验项目结果 ──
        div class="data-card bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]-scroll" {
            h3 { "检验项目结果" }
            @if result.check_results.is_empty() {
                p { "暂无检验项目结果" }
            } @else {
                table class="data-table" {
                    thead {
                        tr {
                            th { "序号" }
                            th { "检验项目" }
                            th { "实测值" }
                            th { "是否合格" }
                            th { "备注" }
                        }
                    }
                    tbody {
                        @for (i, cr) in result.check_results.iter().enumerate() {
                            tr {
                                td class="mono" { (i + 1) }
                                td { (&cr.item) }
                                td class="mono" { (&cr.measured) }
                                td {
                                    @if cr.pass {
                                        span class="status-pill status-active" { "合格" }
                                    } @else {
                                        span class="status-pill status-danger" { "不合格" }
                                    }
                                }
                                td { (cr.remark.as_deref().unwrap_or("—")) }
                            }
                        }
                    }
                }
            }
        }

        // ── 其他信息 ──
        div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
            h3 { "其他信息" }
            div class="grid gap-4" {
                div class="flex flex-col gap-1" { label { "创建时间" } span { (result.created_at.format("%Y-%m-%d %H:%M")) } }
                div class="flex flex-col gap-1" { label { "更新时间" } span { (result.updated_at.format("%Y-%m-%d %H:%M")) } }
            }
        }

        // ── 操作按钮 ──
        @if result.status == InspectionStatus::Pending {
            div class="flex items-center justify-end gap-3 pt-4 border-t border-border-soft" {
                a class="btn bg-accent text-accent-on border-none hover:bg-accent-hover" href=(format!("{}?restore=true", ResultListPath::PATH)) {
                    "记录结果"
                }
            }
        }
        @if result.status == InspectionStatus::Completed && result.result == InspectionResultType::Fail {
            div class="flex items-center justify-end gap-3 pt-4 border-t border-border-soft" {
                a class="btn bg-danger text-white border-none hover:opacity-90-ghost" href=(MrbCreatePath::PATH) {
                    "创建MRB"
                }
            }
        }
    }};

    let current_path = ResultDetailPath { id: path.id }.to_string();
    let html = admin_page(
        is_htmx,
        "检验结果详情",
        &claims,
        "quality",
        &current_path,
        "质量管理",
        Some(ResultListPath::PATH),
        content, &nav_filter,    );
    Ok(Html(html.into_string()))
}
