use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::html;
use rust_decimal::Decimal;

use abt_core::qms::enums::{MRBDisposition, MRBStatus, ResponsibleParty};
use abt_core::qms::inspection_result::InspectionResultService;
use abt_core::qms::mrb::MrbService;
use abt_core::master_data::product::ProductService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::qms::{MrbDetailPath, MrbListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Helpers ──

fn disposition_label(d: &MRBDisposition) -> (&'static str, &'static str) {
    match d {
        MRBDisposition::Scrap => ("报废", "status-danger"),
        MRBDisposition::Return => ("退货", "status-warning"),
        MRBDisposition::Degrade => ("降级", "status-purple"),
        MRBDisposition::Rework => ("返工", "status-info"),
    }
}

fn responsible_party_label(r: &ResponsibleParty) -> (&'static str, &'static str) {
    match r {
        ResponsibleParty::Internal => ("内部", "status-active"),
        ResponsibleParty::Supplier => ("供应商", "status-info"),
        ResponsibleParty::Customer => ("客户", "status-purple"),
    }
}

fn status_label(s: &MRBStatus) -> (&'static str, &'static str) {
    match s {
        MRBStatus::Draft => ("草稿", "status-draft"),
        MRBStatus::UnderReview => ("审批中", "status-warning"),
        MRBStatus::Approved => ("已批准", "status-active"),
        MRBStatus::Completed => ("已完成", "status-info"),
    }
}

fn fmt_cost(v: Decimal) -> String {
    if v.is_zero() {
        "—".into()
    } else {
        format!("¥{}", v)
    }
}

// ── Handler ──

#[require_permission("QMS", "read")]
pub async fn get_detail(path: MrbDetailPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    let svc = state.mrb_service();
    let mrb = svc.get(&service_ctx, &mut conn, path.id).await?;

    // Resolve product name
    let product_name = state
        .product_service()
        .get(&service_ctx, &mut conn, mrb.product_id)
        .await
        .map(|p| p.pdt_name)
        .unwrap_or_else(|_| "—".into());

    // Resolve linked inspection result doc number
    let inspection_doc = state
        .inspection_result_service()
        .get(&service_ctx, &mut conn, mrb.inspection_result_id)
        .await
        .map(|r| r.doc_number)
        .unwrap_or_else(|_| "—".into());

    let (status_text, status_class) = status_label(&mrb.status);
    let (disp_text, disp_class) = disposition_label(&mrb.disposition);
    let (party_text, party_class) = responsible_party_label(&mrb.responsible_party);

    let content = html! { div {
        div class="page-header" {
            div class="page-header-left" {
                a class="back-link" href=(MrbListPath::PATH) { "\u{2190} 返回列表" }
                h1 class="page-title" {
                    "MRB单号 " (&mrb.doc_number)
                    " "
                    span class=(format!("status-pill {status_class}")) { (status_text) }
                }
            }
        }

        // ── 基本信息 ──
        div class="info-card" {
            h3 { "基本信息" }
            div class="info-grid" {
                div class="info-item" {
                    label { "关联检验结果单号" }
                    span class="mono" { (inspection_doc) }
                }
                div class="info-item" { label { "产品" } span { (product_name) } }
                div class="info-item" {
                    label { "处置方式" }
                    span class=(format!("status-pill {disp_class}")) { (disp_text) }
                }
                div class="info-item" {
                    label { "责任方" }
                    span class=(format!("status-pill {party_class}")) { (party_text) }
                }
                div class="info-item" { label { "成本影响" } span class="mono num-right" { (fmt_cost(mrb.cost_impact)) } }
            }
        }

        // ── 缺陷描述 ──
        div class="info-card" {
            h3 { "缺陷描述" }
            p style="white-space: pre-wrap;" { (&mrb.defect_description) }
        }

        // ── 备注 ──
        div class="info-card" {
            h3 { "备注" }
            p style="white-space: pre-wrap;" { (mrb.remark.as_str().is_empty().then(|| "—").unwrap_or(&mrb.remark)) }
        }

        // ── 其他信息 ──
        div class="info-card" {
            h3 { "其他信息" }
            div class="info-grid" {
                div class="info-item" { label { "创建时间" } span { (mrb.created_at.format("%Y-%m-%d %H:%M")) } }
            }
        }
    }};

    let current_path = MrbDetailPath { id: path.id }.to_string();
    let html = admin_page(
        is_htmx,
        "MRB评审详情",
        &claims,
        "quality",
        &current_path,
        "质量管理",
        Some(MrbListPath::PATH),
        content,
    );
    Ok(Html(html.into_string()))
}
