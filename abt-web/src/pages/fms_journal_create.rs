use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::fms::cash_journal::model::CreateCashJournalReq;
use abt_core::fms::cash_journal::CashJournalService;
use abt_core::fms::enums::{CashDirection, CounterpartyType, JournalType};
use abt_core::shared::enums::document_type::DocumentType;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms::{JournalCreatePath, JournalListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form ──

#[derive(Debug, Deserialize)]
pub struct JournalCreateForm {
    pub journal_type: i16,
    pub direction: i16,
    pub amount: String,
    pub bank_account: String,
    pub counterparty_type: i16,
    pub counterparty_name: Option<String>,
    pub source_no: Option<String>,
    pub transaction_date: String,
    pub period: String,
    pub remark: String,
}

// ── Handlers ──

#[require_permission("FMS", "write")]
pub async fn get_create(
    _path: JournalCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, .. } = ctx;

    let content = journal_create_page();
    let page_html = admin_page(
        is_htmx,
        "新建日记账",
        &claims,
        "finance",
        JournalCreatePath::PATH,
        "财务管理",
        Some(JournalListPath::PATH),
        content, &nav_filter,    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("FMS", "write")]
pub async fn create(
    _path: JournalCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<JournalCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let journal_type = JournalType::from_i16(form.journal_type)
        .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效日记账类型".into()))?;
    let direction = CashDirection::from_i16(form.direction)
        .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效方向".into()))?;
    let counterparty_type = CounterpartyType::from_i16(form.counterparty_type)
        .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效往来方类型".into()))?;

    let amount: rust_decimal::Decimal = form.amount.parse()
        .map_err(|_| abt_core::shared::types::DomainError::Validation("无效金额".into()))?;
    let transaction_date = chrono::NaiveDate::parse_from_str(&form.transaction_date, "%Y-%m-%d")
        .map_err(|_| abt_core::shared::types::DomainError::Validation("无效交易日期".into()))?;

    let req = CreateCashJournalReq {
        journal_type,
        direction,
        amount,
        counterparty: abt_core::fms::enums::CounterpartyRef::from_parts(counterparty_type, 0),
        source_type: DocumentType::CashJournal,
        source_id: 0,
        bank_account: form.bank_account,
        transaction_date,
        period: form.period,
        remark: form.remark,
        lines: vec![],
    };

    let svc = state.cash_journal_service();
    let _id = svc.create(&service_ctx, &mut conn, req).await?;

    Ok(
        axum::response::Response::builder()
            .header("HX-Redirect", JournalListPath::PATH)
            .body(axum::body::Body::empty())
            .unwrap(),
    )
}

// ── Page ──

fn journal_create_page() -> Markup {
    html! {
        div {
            // 页面头部
            div class="page-header" {
                div class="page-header-left" {
                    a class="back-link" href=(JournalListPath::PATH) {
                        "\u{2190} 返回列表"
                    }
                    h1 class="page-title" { "新建出纳日记账" }
                }
            }

            form hx-post=(JournalCreatePath::PATH) hx-swap="none" {
                // ── 基本信息 ──
                div class="form-section" {
                    div class="form-section-title" { "基本信息" }
                    div class="form-grid" {
                        // 日记账类型
                        div class="form-field" {
                            label class="form-label" {
                                "日记账类型 "
                                span style="color:var(--danger)" { "*" }
                            }
                            select class="form-select" name="journal_type" required {
                                option value="" disabled selected { "请选择类型" }
                                option value="1" { "销售回款" }
                                option value="2" { "采购付款" }
                                option value="3" { "费用报销" }
                                option value="4" { "工资支付" }
                                option value="5" { "其他" }
                            }
                        }
                        // 收付方向
                        div class="form-field" {
                            label class="form-label" {
                                "收付方向 "
                                span style="color:var(--danger)" { "*" }
                            }
                            select class="form-select" name="direction" required {
                                option value="1" { "流入 (Inflow)" }
                                option value="2" { "流出 (Outflow)" }
                            }
                        }
                        // 金额
                        div class="form-field" {
                            label class="form-label" {
                                "金额 "
                                span style="color:var(--danger)" { "*" }
                            }
                            input class="form-input" type="number" name="amount" step="0.01" min="0" required placeholder="0.00" style="font-family:var(--font-mono);text-align:right";
                        }
                        // 银行账户
                        div class="form-field" {
                            label class="form-label" {
                                "银行账户 "
                                span style="color:var(--danger)" { "*" }
                            }
                            input class="form-input" type="text" name="bank_account" required placeholder="银行账号";
                        }
                        // 往来方类型
                        div class="form-field" {
                            label class="form-label" {
                                "往来方类型 "
                                span style="color:var(--danger)" { "*" }
                            }
                            select class="form-select" name="counterparty_type" required {
                                option value="1" { "客户" }
                                option value="2" { "供应商" }
                                option value="3" { "员工" }
                                option value="4" { "其他" }
                            }
                        }
                        // 往来方
                        div class="form-field" {
                            label class="form-label" {
                                "往来方 "
                                span style="color:var(--danger)" { "*" }
                            }
                            input class="form-input" type="text" name="counterparty_name" placeholder="搜索选择往来方…";
                        }
                        // 来源单据
                        div class="form-field" {
                            label class="form-label" { "来源单据" }
                            input class="form-input" type="text" name="source_no" placeholder="关联来源单号（可选）";
                        }
                        // 交易日期
                        div class="form-field" {
                            label class="form-label" {
                                "交易日期 "
                                span style="color:var(--danger)" { "*" }
                            }
                            input class="form-input" type="date" name="transaction_date" required;
                        }
                        // 期间
                        div class="form-field" {
                            label class="form-label" {
                                "期间 "
                                span style="color:var(--danger)" { "*" }
                            }
                            input class="form-input" type="month" name="period" required;
                        }
                        // 备注（占满整行）
                        div class="form-field field-full" {
                            label class="form-label" { "备注" }
                            textarea class="form-input" name="remark" placeholder="填写备注信息…" rows="3" {}
                        }
                    }
                }

                // ── 操作栏 ──
                div class="create-action-bar" {
                    a class="btn btn-default" href=(JournalListPath::PATH) { "取消" }
                    div style="display:flex;gap:var(--space-3)" {
                        button type="button" class="btn btn-default" { "保存草稿" }
                        button type="submit" class="btn btn-primary" { "提交" }
                    }
                }
            }
        }
    }
}
