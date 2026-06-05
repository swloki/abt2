use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use crate::errors::Result;
use crate::routes::wms_conversion::ConversionCreatePath;
use crate::utils::RequestContext;
use abt_macros::require_permission;
use crate::layout::page::admin_page;

use crate::components::icon;

#[require_permission("WMS", "write")]
pub async fn get_conversion_create(
    _path: ConversionCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let claims = ctx.claims;
    let content = conversion_create_page();
    let page_html = admin_page(
        is_htmx,
        "新建形态转换单",
        &claims,
        "inventory",
        "/admin/wms/conversions/create",
        "库存管理",
        None,
        content,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("WMS", "write")]
pub async fn create_conversion(
    _path: ConversionCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let claims = ctx.claims;
    let content = conversion_create_page();
    let page_html = admin_page(
        is_htmx,
        "新建形态转换单",
        &claims,
        "inventory",
        "/admin/wms/conversions/create",
        "库存管理",
        None,
        content,
    );
    Ok(Html(page_html.into_string()))
}

fn conversion_create_page() -> Markup {
    html! {
        div {
            a href="/admin/wms/conversions" class="back-link" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回形态转换列表"
            }

            div class="page-header" {
                h1 class="page-title" { "新建形态转换单" }
            }

            div class="workflow-steps" {
                div class="wf-step current" { span class="wf-dot" {} "草稿" }
                div class="wf-line" {}
                div class="wf-step" { span class="wf-dot" {} "已完成" }
            }

            form hx-post=(ConversionCreatePath::PATH) hx-swap="none" {
                // ── Basic Info ──
                div class="wms-form-section" {
                    h3 class="form-section-title" { "转换信息" }
                    div class="wms-form-grid" {
                        div class="form-field" {
                            label class="form-label" { "仓库" }
                            select class="form-select" name="warehouse_id" required {}
                        }
                        div class="form-field" {
                            label class="form-label" { "转换日期" }
                            input class="form-input" type="date" name="conversion_date" required {}
                        }
                        div class="form-field" style="grid-column:span 2" {
                            label class="form-label" { "备注" }
                            input class="form-input" type="text" name="remark" {}
                        }
                    }
                }

                // ── Consume Items ──
                div class="wms-form-section" {
                    h3 class="form-section-title" {
                        "消耗物料 "
                        span style="display:inline-flex;align-items:center;padding:3px 10px;border-radius:9999px;font-size:12px;font-weight:600;background:#fff2f0;color:var(--danger)" { "消耗" }
                    }
                    div class="data-card" {
                        table class="data-table" {
                            thead {
                                tr {
                                    th { "行号" }
                                    th { "产品" }
                                    th class="num-right" { "数量" }
                                    th class="num-right" { "单位成本" }
                                    th { "批次号" }
                                    th { "操作" }
                                }
                            }
                            tbody id="consume-items" {}
                        }
                    }
                    button type="button" class="btn btn-default" style="margin-top:var(--space-3)"
                        onclick="addConversionLine('consume')" {
                        (icon::plus_icon("w-4 h-4"))
                        "添加消耗行"
                    }
                }

                // ── Produce Items ──
                div class="wms-form-section" {
                    h3 class="form-section-title" {
                        "产出物料 "
                        span style="display:inline-flex;align-items:center;padding:3px 10px;border-radius:9999px;font-size:12px;font-weight:600;background:#f0fff0;color:var(--success)" { "产出" }
                    }
                    div class="data-card" {
                        table class="data-table" {
                            thead {
                                tr {
                                    th { "行号" }
                                    th { "产品" }
                                    th class="num-right" { "数量" }
                                    th class="num-right" { "单位成本" }
                                    th { "批次号" }
                                    th { "操作" }
                                }
                            }
                            tbody id="produce-items" {}
                        }
                    }
                    button type="button" class="btn btn-default" style="margin-top:var(--space-3)"
                        onclick="addConversionLine('produce')" {
                        (icon::plus_icon("w-4 h-4"))
                        "添加产出行"
                    }
                }

                // ── Actions ──
                div class="create-action-bar" {
                    a href="/admin/wms/conversions" class="btn btn-default" { "取消" }
                    button type="submit" class="btn btn-default" name="action" value="draft" { "保存草稿" }
                    button type="submit" class="btn btn-primary" name="action" value="submit" { "提交" }
                }
            }
        }
    }
}
