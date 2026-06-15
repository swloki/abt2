use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::purchase::approval::PurchaseApprovalService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/approval-rules")]
pub struct ApprovalRulesPath;

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_approval_rules(
    _path: ApprovalRulesPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;

    let svc = state.purchase_approval_service();
    let rules = svc.list_rules(&service_ctx, &mut conn).await.unwrap_or_default();

    let content = rules_page(&rules);
    let page_html = admin_page(
        is_htmx, "审批规则管理", &claims, "purchase",
        ApprovalRulesPath::PATH,
        "采购管理", Some("审批规则"), content, &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

#[derive(Debug, Deserialize)]
pub struct RuleForm {
    pub name: String,
    pub min_amount: String,
    pub max_amount: Option<String>,
    pub approver_role: String,
    pub approver_id: Option<String>,
    pub sort_order: Option<String>,
}

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn create_rule(
    _path: ApprovalRulesPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<RuleForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_approval_service();
    let min_amount: rust_decimal::Decimal = form.min_amount.parse()
        .map_err(|_| abt_core::shared::types::DomainError::validation("无效金额"))?;
    let max_amount = form.max_amount.filter(|s| !s.is_empty())
        .and_then(|s| s.parse().ok());
    let approver_id = form.approver_id.and_then(|s| s.parse().ok());
    let sort_order = form.sort_order.and_then(|s| s.parse().ok()).unwrap_or(10);

    svc.create_rule(&service_ctx, &mut conn, form.name, min_amount, max_amount,
        form.approver_role, approver_id, sort_order).await?;
    let redirect = ApprovalRulesPath.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/purchase/approval-rules/{id}/delete")]
pub struct RuleDeletePath { pub id: i64 }

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn delete_rule(
    path: RuleDeletePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_approval_service();
    svc.delete_rule(&service_ctx, &mut conn, path.id).await?;
    let redirect = ApprovalRulesPath.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

fn rules_page(rules: &[abt_core::purchase::approval::model::PurchaseApprovalRule]) -> Markup {
    html! {
        div {
            div class="page-header" {
                h1 class="page-title" { "审批规则管理" }
            }
            // Create form
            div class="data-card" style="margin-bottom:var(--space-4)" {
                div class="form-section-title" { "新建规则" }
                form hx-post=(ApprovalRulesPath::PATH) hx-swap="none" {
                    div class="form-grid" {
                        div class="form-field" {
                            label { "规则名称" }
                            input type="text" name="name" required class="form-input" {}
                        }
                        div class="form-field" {
                            label { "最低金额" }
                            input type="number" step="0.01" name="min_amount" required class="form-input" {}
                        }
                        div class="form-field" {
                            label { "最高金额（空=不限）" }
                            input type="number" step="0.01" name="max_amount" class="form-input" {}
                        }
                        div class="form-field" {
                            label { "审批角色" }
                            input type="text" name="approver_role" placeholder="如 manager" class="form-input" {}
                        }
                        div class="form-field" {
                            label { "审批人ID（可选）" }
                            input type="number" name="approver_id" class="form-input" {}
                        }
                        div class="form-field" {
                            label { "排序" }
                            input type="number" name="sort_order" value="10" class="form-input" {}
                        }
                    }
                    div style="padding:var(--space-3)" {
                        button type="submit" class="btn btn-primary" { "创建规则" }
                    }
                }
            }
            // Rules table
            div class="data-card" {
                div class="form-section-title" { "现有规则" }
                @if rules.is_empty() {
                    p style="color:var(--text-tertiary);padding:var(--space-4)" { "暂无审批规则，提交订单将直接确认" }
                } @else {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "名称" }
                                th style="text-align:right" { "最低金额" }
                                th style="text-align:right" { "最高金额" }
                                th { "审批角色" }
                                th { "排序" }
                                th { }
                            }
                        }
                        tbody {
                            @for rule in rules {
                                tr {
                                    td { (&rule.name) }
                                    td style="text-align:right" { (rule.min_amount) }
                                    td style="text-align:right" { (rule.max_amount.map(|m| m.to_string()).unwrap_or_else(|| "不限".into())) }
                                    td { (&rule.approver_role) }
                                    td { (rule.sort_order) }
                                    td {
                                        button class="btn btn-sm btn-danger"
                                            hx-post=(RuleDeletePath { id: rule.id }.to_string())
                                            hx-confirm="确认删除？" { "删除" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
