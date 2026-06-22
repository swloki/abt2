use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::gl::account::model::{CreateGlAccountReq, GlAccountFilter};
use abt_core::gl::account::GlAccountService;
use abt_core::gl::enums::{AccountType, BalanceDirection};

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::gl::{GlAccountCreatePath, GlAccountListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form ──

#[derive(Debug, Deserialize)]
pub struct AccountCreateForm {
    pub code: String,
    pub name: String,
    pub account_type: i16,
    pub parent_id: Option<i64>,
    pub balance_direction: i16,
    pub is_detail: Option<String>,      // checkbox："on" 或 None
    pub reconcile: Option<String>,      // checkbox
    pub opening_balance: Option<String>,
    pub currency: String,
}

// ── Handlers ──

#[require_permission("GL", "create")]
pub async fn get_create(_path: GlAccountCreatePath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;

    // 拉取可作为父科目的候选（is_detail=false 的汇总科目）
    let svc = state.gl_account_service();
    let all = svc
        .list(
            &service_ctx,
            &mut conn,
            GlAccountFilter::default(),
            abt_core::shared::types::PageParams::new(1, 200),
        )
        .await?;
    let parent_candidates: Vec<&abt_core::gl::account::model::GlAccount> =
        all.items.iter().filter(|a| !a.is_detail).collect();

    let content = account_create_page(&parent_candidates);
    let page_html = admin_page(
        is_htmx,
        "新建科目",
        &claims,
        "gl",
        GlAccountCreatePath::PATH,
        "总账管理",
        Some(GlAccountListPath::PATH),
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("GL", "create")]
pub async fn create(
    _path: GlAccountCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<AccountCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    let account_type = AccountType::from_i16(form.account_type)
        .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效科目类型".into()))?;
    let balance_direction = BalanceDirection::from_i16(form.balance_direction)
        .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效余额方向".into()))?;

    let opening_balance: rust_decimal::Decimal = form
        .opening_balance
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("0")
        .parse()
        .map_err(|_| {
            abt_core::shared::types::DomainError::Validation("无效期初余额".into())
        })?;

    let req = CreateGlAccountReq {
        code: form.code,
        name: form.name,
        account_type,
        parent_id: form.parent_id,
        is_detail: form.is_detail.as_deref() == Some("on"),
        balance_direction,
        reconcile: form.reconcile.as_deref() == Some("on"),
        opening_balance,
        currency: form.currency,
    };

    let svc = state.gl_account_service();
    let _id = svc.create(&service_ctx, &mut conn, req).await?;

    Ok(axum::response::Response::builder()
        .header("HX-Redirect", GlAccountListPath::PATH)
        .body(axum::body::Body::empty())
        .unwrap())
}

// ── Page ──

fn account_create_page(
    parent_candidates: &[&abt_core::gl::account::model::GlAccount],
) -> Markup {
    html! {
        div {
            a   href=(format!("{}?restore=true", GlAccountListPath::PATH))
                class="inline-flex items-center gap-1 text-sm text-muted hover:text-fg mb-4"
            { (icon::chevron_left_icon("w-4 h-4")) "返回列表" }
            h1 class="text-xl font-bold text-fg tracking-tight mb-6" { "新建科目" }

            form id="gl-account-create-form" hx-post=(GlAccountCreatePath::PATH) hx-swap="none" {
                div class="form-section" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                    { (icon::clipboard_document_icon("w-4 h-4")) " 基本信息" }
                    div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                        // 科目编码
                        div class="form-field" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            {
                                "科目编码 "
                                span class="text-danger" { "*" }
                            }
                            input
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent font-mono"
                                type="text"
                                name="code"
                                required
                                placeholder="如 1001";
                        }
                        // 科目名称
                        div class="form-field" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            {
                                "科目名称 "
                                span class="text-danger" { "*" }
                            }
                            input
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent"
                                type="text"
                                name="name"
                                required
                                placeholder="如 库存现金";
                        }
                        // 科目类型
                        div class="form-field" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            {
                                "科目类型 "
                                span class="text-danger" { "*" }
                            }
                            select
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent"
                                name="account_type"
                                required
                            {
                                option value="" disabled selected { "请选择类型" }
                                option value="1" { "资产" }
                                option value="2" { "负债" }
                                option value="3" { "权益" }
                                option value="4" { "收入" }
                                option value="5" { "成本" }
                                option value="6" { "费用" }
                            }
                        }
                        // 余额方向
                        div class="form-field" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            {
                                "余额方向 "
                                span class="text-danger" { "*" }
                            }
                            select
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent"
                                name="balance_direction"
                                required
                            {
                                option value="1" { "借 (Debit)" }
                                option value="2" { "贷 (Credit)" }
                            }
                        }
                        // 父科目
                        div class="form-field" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            { "父科目" }
                            select
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent"
                                name="parent_id"
                            {
                                option value="" { "（无，顶级科目）" }
                                @for p in parent_candidates {
                                    @let v = format!("{}", p.id);
                                    option value=(v) { (format!("{} {}", p.code, p.name)) }
                                }
                            }
                        }
                        // 币种
                        div class="form-field" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            { "币种" }
                            input
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent font-mono"
                                type="text"
                                name="currency"
                                value="CNY";
                        }
                        // 期初余额
                        div class="form-field" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            { "期初余额" }
                            input
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent font-mono text-right"
                                type="number"
                                step="any"
                                name="opening_balance"
                                value="0";
                        }
                        // 复选项：明细科目 / 需辅助核算
                        div class="form-field" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            { "科目属性" }
                            div class="flex items-center gap-6 h-[38px]" {
                                label
                                    class="inline-flex items-center gap-2 text-sm text-fg-2 cursor-pointer"
                                {
                                    input
                                        type="checkbox"
                                        name="is_detail"
                                        checked
                                        value="on"
                                        class="w-4 h-4 accent-[var(--accent)]";
                                    "明细科目（可被凭证引用）"
                                }
                            }
                        }
                        div class="form-field field-full" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            { "辅助核算" }
                            label
                                class="inline-flex items-center gap-2 text-sm text-fg-2 cursor-pointer"
                            {
                                input
                                    type="checkbox"
                                    name="reconcile"
                                    value="on"
                                    class="w-4 h-4 accent-[var(--accent)]";
                                "启用辅助核算（客户/供应商/部门等维度）"
                            }
                        }
                    }
                }
                // ── Action Bar ──
                div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft"
                {
                    a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                        href=(format!("{}?restore=true", GlAccountListPath::PATH))
                    { "取消" }
                    button
                        type="submit"
                        form="gl-account-create-form"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                    { (icon::check_circle_icon("w-4 h-4")) "保存" }
                }
            }
        }
    }
}
