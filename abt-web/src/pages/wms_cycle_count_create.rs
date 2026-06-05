use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::wms::cycle_count::model::CreateCycleCountReq;
use abt_core::wms::cycle_count::CycleCountService;

use crate::layout::page::admin_page;
use crate::routes::wms_cycle_count::{CycleCountCreatePath, CycleCountListPath};
use crate::utils::RequestContext;

use abt_macros::require_permission;

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub struct CreateCycleCountForm {
    pub warehouse_id: i64,
    pub zone_id: Option<i64>,
    pub count_date: String,
    pub is_blind: Option<String>,
    pub remark: Option<String>,
    pub action: Option<String>,
}

// ── Handlers ──

#[require_permission("WMS", "read")]
pub async fn get_cycle_count_create(
    _path: CycleCountCreatePath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let claims = ctx.claims;

    let content = cycle_count_create_form();
    let page_html = admin_page(
        is_htmx,
        "新建盘点",
        &claims,
        "inventory",
        CycleCountListPath::PATH,
        "库存管理",
        Some("新建盘点"),
        content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("WMS", "write")]
pub async fn create_cycle_count(
    _path: CycleCountCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<CreateCycleCountForm>,
) -> crate::errors::Result<axum::response::Response> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.cycle_count_service();

    let count_date = chrono::NaiveDate::parse_from_str(&form.count_date, "%Y-%m-%d")
        .map_err(|e| crate::errors::WebError::from(abt_core::shared::types::DomainError::Validation(format!("无效日期格式: {e}"))))?;

    let is_blind = form.is_blind.as_deref() == Some("on");

    let req = CreateCycleCountReq {
        warehouse_id: form.warehouse_id,
        zone_id: form.zone_id,
        count_date,
        is_blind,
        remark: form.remark,
        items: vec![],
    };

    let id = svc.create(&service_ctx, &mut conn, req).await?;

    if form.action.as_deref() == Some("start") {
        svc.start_count(&service_ctx, &mut conn, id).await?;
    }

    let mut resp = axum::response::Response::default();
    resp.headers_mut().insert(
        axum::http::header::LOCATION,
        CycleCountListPath::PATH.parse().unwrap(),
    );
    resp.headers_mut().insert(
        "HX-Redirect",
        CycleCountListPath::PATH.parse().unwrap(),
    );
    *resp.status_mut() = axum::http::StatusCode::SEE_OTHER;

    Ok(resp)
}

// ── Components ──

fn cycle_count_create_form() -> Markup {
    html! {
        div class="data-card" {
            form method="POST" action=(CycleCountCreatePath::PATH)
                hx-post=(CycleCountCreatePath::PATH)
                hx-redirect=(CycleCountListPath::PATH) {

                div class="wms-form-section" {
                    div class="wms-form-grid" {
                        div class="form-field" {
                            label class="form-label" { "仓库" }
                            select class="form-select" name="warehouse_id" required {
                                option value="" { "请选择仓库" }
                            }
                        }
                        div class="form-field" {
                            label class="form-label" { "库区" }
                            select class="form-select" name="zone_id" {
                                option value="" { "全部库区" }
                            }
                        }
                        div class="form-field" {
                            label class="form-label" { "盘点日期" }
                            input class="form-input" type="date" name="count_date" required;
                        }
                        div class="form-field" {
                            label class="form-label" { "盲盘模式" }
                            label style="display:flex;align-items:center;gap:var(--space-2);cursor:pointer;padding-top:var(--space-2)" {
                                input type="checkbox" name="is_blind";
                                "开启盲盘（隐藏系统数量）"
                            }
                        }
                        div class="form-field" style="grid-column:1/-1" {
                            label class="form-label" { "备注" }
                            textarea class="form-input" name="remark" rows="3" placeholder="可选备注…" {}
                        }
                    }
                }

                div class="create-action-bar" {
                    a class="btn btn-default" href=(CycleCountListPath::PATH) { "取消" }
                    button type="submit" class="btn btn-default" name="action" value="draft" {
                        "保存草稿"
                    }
                    button type="submit" class="btn btn-primary" name="action" value="start" {
                        "开始盘点"
                    }
                }
            }
        }
    }
}
