use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::purchase::approval::{PurchaseApprovalRule, PurchaseApprovalService, RuleUpsertRequest};
use abt_core::shared::types::DomainError;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::purchase_approval_rules::{
    ApprovalRulesPath, RuleCreatePath, RuleDeletePath, RuleEditPath,
};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form data ──

#[derive(Debug, Deserialize)]
pub struct RuleFormData {
    pub name: String,
    pub min_amount: String,
    pub max_amount: Option<String>,
    pub approver_role: String,
    pub approver_id: Option<String>,
    pub sort_order: String,
    #[serde(default)]
    pub is_active: Option<String>, // "on" when checked
}

fn parse_rule_form(form: &RuleFormData) -> std::result::Result<RuleUpsertRequest, DomainError> {
    let min_amount: rust_decimal::Decimal = form.min_amount.parse()
        .map_err(|_| DomainError::validation("无效最低金额"))?;
    let max_amount = form.max_amount.as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse().ok());
    let approver_id = form.approver_id.as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse().ok());
    let sort_order: i32 = form.sort_order.parse()
        .map_err(|_| DomainError::validation("无效排序值"))?;

    Ok(RuleUpsertRequest {
        name: form.name.clone(),
        min_amount,
        max_amount,
        approver_role: form.approver_role.clone(),
        approver_id,
        sort_order,
        is_active: form.is_active.as_deref() == Some("on"),
    })
}

// ══════════════════════════════════════════════════════════════════
//  Handlers
// ══════════════════════════════════════════════════════════════════

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_list(
    _path: ApprovalRulesPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, mut conn, state, service_ctx, .. } = ctx;

    let svc = state.purchase_approval_service();
    let rules = svc.list_rules(&service_ctx, &mut conn).await.unwrap_or_default();

    let content = list_page(&rules);
    let page_html = admin_page(
        is_htmx,
        "审批规则管理",
        &claims,
        "purchase",
        ApprovalRulesPath::PATH,
        "采购管理",
        Some("审批规则"),
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

// ── Modal: create form ──

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_create_modal(
    _path: RuleCreatePath,
    _ctx: RequestContext,
) -> Result<Html<String>> {
    let html = rule_form(ApprovalRulesPath::PATH, None);
    Ok(Html(html.into_string()))
}

// ── Modal: edit form ──

#[require_permission("PURCHASE_ORDER", "read")]
pub async fn get_edit_modal(
    path: RuleEditPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_approval_service();
    let rule = svc.get_rule(&service_ctx, &mut conn, path.id).await?;

    let action_url = RuleEditPath { id: path.id }.to_string();
    let html = rule_form(&action_url, Some(&rule));
    Ok(Html(html.into_string()))
}

// ── Create ──

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn create_rule(
    _path: ApprovalRulesPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<RuleFormData>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_approval_service();
    let req = parse_rule_form(&form)?;
    svc.create_rule(&service_ctx, &mut conn, req).await?;

    Ok((
        [
            ("HX-Trigger", r#"{"rulesUpdated":"", "closeRuleModal":""}"#),
            ("Content-Type", "text/html"),
        ],
        Html(String::new()),
    ))
}

// ── Update ──

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn update_rule(
    path: RuleEditPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<RuleFormData>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_approval_service();
    let req = parse_rule_form(&form)?;
    svc.update_rule(&service_ctx, &mut conn, path.id, req).await?;

    Ok((
        [
            ("HX-Trigger", r#"{"rulesUpdated":"", "closeRuleModal":""}"#),
            ("Content-Type", "text/html"),
        ],
        Html(String::new()),
    ))
}

// ── Delete ──

#[require_permission("PURCHASE_ORDER", "update")]
pub async fn delete_rule(
    path: RuleDeletePath,
    ctx: RequestContext,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.purchase_approval_service();
    svc.delete_rule(&service_ctx, &mut conn, path.id).await?;

    // Re-query and return updated table fragment
    let rules = svc.list_rules(&service_ctx, &mut conn).await.unwrap_or_default();
    let html = table_fragment(&rules);
    Ok(([("Content-Type", "text/html")], Html(html.into_string())))
}

// ══════════════════════════════════════════════════════════════════
//  Rendering
// ══════════════════════════════════════════════════════════════════

fn list_page(rules: &[PurchaseApprovalRule]) -> Markup {
    use maud::PreEscaped;
    html! {
        div {
            div class="flex items-center justify-between mb-6" {
                div class="flex items-center justify-between mb-6-left" {
                    h1 class="text-xl font-bold text-fg tracking-tight" { "审批规则管理" }
                }
                div class="flex gap-3" {
                    button type="button" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                        hx-get=(RuleCreatePath::PATH)
                        hx-target="#rule-modal"
                        hx-swap="innerHTML"
                        _="on 'htmx:afterRequest' add .is-open to #rule-modal" {
                        "+ 新建规则"
                    }
                }
            }
            (table_fragment(rules))
            (rule_modal_shell())
            (PreEscaped(r#"<script>
                document.body.addEventListener('rulesUpdated', function() {
                    var card = document.getElementById('rules-data-card');
                    if (card) {
                        htmx.ajax('GET', window.location.href, {target: '#rules-data-card', swap: 'outerHTML'});
                    }
                });
            </script>"#))
        }
    }
}

fn table_fragment(rules: &[PurchaseApprovalRule]) -> Markup {
    html! {
        div id="rules-data-card" {
            @if !rules.is_empty() {
                (ladder_vis(rules))
            }
            (data_card(rules))
        }
    }
}

fn ladder_vis(rules: &[PurchaseApprovalRule]) -> Markup {
    // Compute the overall amount range
    let min_all: rust_decimal::Decimal = rules.iter()
        .map(|r| r.min_amount)
        .min()
        .unwrap_or(rust_decimal::Decimal::ZERO);
    let max_all: rust_decimal::Decimal = rules.iter()
        .filter_map(|r| r.max_amount)
        .fold(rust_decimal::Decimal::ZERO, |a, b| if b > a { b } else { a });

    // If all rules have no max, use a reasonable upper bound
    let range = if max_all > min_all {
        max_all - min_all
    } else {
        rust_decimal::Decimal::ONE
    };

    let colors = ["#165DFF", "#0FC6C2", "#FF7D00", "#F53F3F", "#722ED1", "#14C9C9"];

    html! {
        div class="data-card" {
            div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "金额阶梯" }
            div style="padding:var(--space-4) var(--space-4) var(--space-2)" {
                div style="position:relative;height:40px;margin-bottom:4px" {
                    @for (i, rule) in rules.iter().enumerate() {
                        @if rule.is_active {
                            (ladder_bar(rule, i, min_all, range, &colors))
                        }
                    }
                }
                // Labels below bars
                div style="display:flex;justify-content:space-between;font-size:var(--text-xs);color:var(--text-muted);padding:0" {
                    span { (crate::utils::fmt_qty(min_all)) }
                    @for rule in rules {
                        @if rule.is_active && rule.max_amount.is_some() {
                            span { (crate::utils::fmt_qty(rule.max_amount.unwrap())) }
                        }
                    }
                }
            }
            // Legend
            div style="display:flex;gap:var(--space-4);flex-wrap:wrap;padding:var(--space-2) var(--space-4)" {
                @for (i, rule) in rules.iter().enumerate() {
                    @if rule.is_active {
                        div style="display:flex;align-items:center;gap:var(--space-1);font-size:var(--text-xs)" {
                            span style=(format!("display:inline-block;width:10px;height:10px;border-radius:2px;background:{}", colors[i % colors.len()])) {}
                            span { (&rule.name) }
                        }
                    }
                }
            }
        }
    }
}

fn ladder_bar(
    rule: &PurchaseApprovalRule,
    i: usize,
    min_all: rust_decimal::Decimal,
    range: rust_decimal::Decimal,
    colors: &[&str],
) -> Markup {
    use rust_decimal::Decimal;

    let left_pct = ((rule.min_amount - min_all) / range * Decimal::ONE_HUNDRED).to_string();
    let width_pct = match rule.max_amount {
        Some(max) => ((max - rule.min_amount) / range * Decimal::ONE_HUNDRED).to_string(),
        None => "5".to_string(), // narrow bar for unlimited
    };
    let color = colors[i % colors.len()];

    html! {
        div style=(format!(
            "position:absolute;left:{}%;width:{}%;top:4px;height:24px;background:{};border-radius:4px;opacity:0.85;display:flex;align-items:center;justify-content:center;overflow:hidden",
            left_pct, width_pct, color
        )) {
            span style="color:#fff;font-size:10px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;padding:0 4px" {
                (&rule.name)
                @if let Some(max) = rule.max_amount {
                    " " (crate::utils::fmt_qty(rule.min_amount)) "–" (crate::utils::fmt_qty(max))
                } @else {
                    " ≥" (crate::utils::fmt_qty(rule.min_amount))
                }
            }
        }
    }
}

fn data_card(rules: &[PurchaseApprovalRule]) -> Markup {
    html! {
        div class="data-card" {
            @if rules.is_empty() {
                (empty_state())
            } @else {
                div class="overflow-x-auto" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "排序" }
                                th { "规则名称" }
                                th class="text-right text-[13px]" { "最低金额" }
                                th class="text-right text-[13px]" { "最高金额" }
                                th { "审批角色" }
                                th { "审批人ID" }
                                th { "状态" }
                                th class="!text-right" { "操作" }
                            }
                        }
                        tbody {
                            @for rule in rules {
                                (row_tr(rule))
                            }
                        }
                    }
                }
            }
        }
    }
}

fn row_tr(rule: &PurchaseApprovalRule) -> Markup {
    html! {
        tr {
            td { (rule.sort_order) }
            td {
                span style="font-weight:500" { (&rule.name) }
            }
            td class="font-mono tabular-nums text-right text-[13px]" { (crate::utils::fmt_qty(rule.min_amount)) }
            td class="font-mono tabular-nums text-right text-[13px]" {
                (rule.max_amount.map(crate::utils::fmt_qty).unwrap_or_else(|| "不限".into()))
            }
            td { (&rule.approver_role) }
            td { (rule.approver_id.map(|id| id.to_string()).unwrap_or_else(|| "—".into())) }
            td {
                @if rule.is_active {
                    span class="inline-flex items-center gap-[5px] rounded-full text-[12px] font-medium whitespace-nowrap bg-[#f0fff0] text-[#389e0d]" { "启用" }
                } @else {
                    span class="inline-flex items-center gap-[5px] rounded-full text-[12px] font-medium whitespace-nowrap bg-[#fff2f0] text-[#cf1322]" { "停用" }
                }
            }
            td {
                div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg [&_svg]:w-3.5 [&_svg]:h-3.5" {
                    button type="button" class="btn inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative-sm inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs [&_svg]:w-4 [&_svg]:h-4"
                        hx-get=(RuleEditPath { id: rule.id }.to_string())
                        hx-target="#rule-modal"
                        hx-swap="innerHTML"
                        _="on 'htmx:afterRequest' add .is-open to #rule-modal" {
                        "编辑"
                    }
                    button class="btn inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative-sm bg-danger text-white border-none hover:opacity-90 [&_svg]:w-4 [&_svg]:h-4"
                        hx-post=(RuleDeletePath { id: rule.id }.to_string())
                        hx-confirm="确认删除此审批规则？"
                        hx-target="#rules-data-card"
                        hx-select="#rules-data-card"
                        hx-swap="outerHTML" {
                        "删除"
                    }
                }
            }
        }
    }
}

fn empty_state() -> Markup {
    html! {
        div style="text-align:center;padding:var(--space-12);color:var(--text-muted)" {
            p style="margin:0;font-size:var(--text-lg)" { "暂无审批规则" }
            p style="margin:var(--space-2) 0 0;font-size:var(--text-sm)" { "点击「+ 新建规则」添加金额阶梯审批规则" }
        }
    }
}

// ── Modal ──

fn rule_modal_shell() -> Markup {
    html! {
        div class="fixed z-[1000] grid place-items-center opacity-0" id="rule-modal"
            _="on closeRuleModal from body remove .is-open
               on click[me is event.target] remove .is-open" {
        }
    }
}

fn rule_form(action_url: &str, rule: Option<&PurchaseApprovalRule>) -> Markup {
    let is_edit = rule.is_some();
    let title = if is_edit { "编辑审批规则" } else { "新建审批规则" };

    let name = rule.map(|r| r.name.as_str()).unwrap_or("");
    let min_amt = rule.map(|r| r.min_amount.to_string()).unwrap_or_default();
    let max_amt = rule.and_then(|r| r.max_amount.map(|m| m.to_string())).unwrap_or_default();
    let role = rule.map(|r| r.approver_role.as_str()).unwrap_or("");
    let approver = rule.and_then(|r| r.approver_id.map(|id| id.to_string())).unwrap_or_default();
    let sort = rule.map(|r| r.sort_order.to_string()).unwrap_or_else(|| "10".into());
    let active = rule.map(|r| r.is_active).unwrap_or(true);

    let common_roles = ["manager", "director", "finance", "vp", "ceo"];

    html! {
        div class="bg-bg rounded-xl w-[680px] flex flex-col overflow-hidden opacity-0" _="on click halt" {
            div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                h2 { (title) }
                button class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg"
                    _="on click remove .is-open from #rule-modal" { "×" }
            }
            form hx-post=(action_url) hx-target="this" hx-swap="outerHTML"
                _="on 'htmx:afterRequest'[detail.successful] remove .is-open from #rule-modal" {

                div class="overflow-y-auto flex-1 min-h-0 p-6" {
                    div class="form-section" {
                        div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "规则信息" }
                        div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                            div class="form-field" {
                                label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "规则名称" span class="required" { "*" } }
                                input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="name"
                                    required value=(name)
                                    placeholder="如：小额审批、大额审批";
                            }
                            div class="form-field" {
                                label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "审批角色" span class="required" { "*" } }
                                input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="approver_role"
                                    required value=(role)
                                    placeholder="如 manager"
                                    list="common-roles";
                                // Datalist for common roles
                            }
                            div class="form-field" {
                                label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "最低金额" span class="required" { "*" } }
                                input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" step="0.01" min="0"
                                    name="min_amount" required value=(min_amt);
                            }
                            div class="form-field" {
                                label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "最高金额（空=不限）" }
                                input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" step="0.01" min="0"
                                    name="max_amount" value=(max_amt);
                            }
                            div class="form-field" {
                                label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "审批人ID（可选）" }
                                input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" step="1"
                                    name="approver_id" value=(approver);
                            }
                            div class="form-field" {
                                label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "排序" }
                                input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="number" step="1"
                                    name="sort_order" value=(sort);
                            }
                            div class="form-field" {
                                label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "启用状态" }
                                label style="display:flex;align-items:center;gap:var(--space-2);cursor:pointer" {
                                    input type="checkbox" name="is_active" checked[active] {};
                                    " 启用"
                                }
                            }
                        }
                    }
                    // Datalist for common approver roles
                    datalist id="common-roles" {
                        @for role_name in &common_roles {
                            option value=(*role_name) {}
                        }
                    }
                }

                div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 shrink-0" {
                    button type="button" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                        _="on click remove .is-open from #rule-modal" { "取消" }
                    button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" { "保存" }
                }
            }
        }
    }
}
