use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup, PreEscaped};
use serde::Deserialize;

use abt_core::fms::cash_journal::model::CreateCashJournalReq;
use abt_core::fms::cash_journal::CashJournalService;
use abt_core::fms::enums::{CashDirection, CounterpartyType, JournalType};
use abt_core::shared::enums::document_type::DocumentType;

use crate::components::{entity_picker, icon};
use crate::components::entity_picker::{EntityPickerConfig, EntityPickerItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms::{JournalCreatePath, JournalListPath, JournalSearchAccountPath, JournalSearchCpPath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Forms ──

#[derive(Debug, Deserialize)]
pub struct JournalCreateForm {
    pub journal_type: i16,
    pub direction: i16,
    pub bank_account: String,
    pub counterparty_type: i16,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub counterparty_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub source_type: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub source_id: Option<i64>,
    pub transaction_date: String,
    pub period: String,
    pub remark: String,
    pub amount: String,
    #[serde(default = "default_currency")]
    pub currency: String,
    #[serde(default = "default_exchange_rate")]
    pub exchange_rate: String,
}

fn default_currency() -> String {
    "CNY".to_string()
}
fn default_exchange_rate() -> String {
    "1".to_string()
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchQuery {
    pub q: Option<String>,
    pub counterparty_type: Option<i16>,
}

// ── Search Handlers (for entity_picker) ──

#[require_permission("FMS", "read")]
pub async fn search_counterparty(
    _path: JournalSearchCpPath,
    ctx: RequestContext,
    Query(query): Query<SearchQuery>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.cash_journal_service();
    let keyword = query.q.as_deref().unwrap_or("");

    let cp_type = match query.counterparty_type {
        Some(1) => CounterpartyType::Customer,
        Some(2) => CounterpartyType::Supplier,
        Some(3) => CounterpartyType::Employee,
        _ => CounterpartyType::Other,
    };

    let items: Vec<EntityPickerItem> = svc
        .search_counterparties(&service_ctx, &mut conn, cp_type, keyword, 20)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            EntityPickerItem::new(r.id, r.name)
                .sub(format!("编码: {}", r.code))
        })
        .collect();

    Ok(Html(entity_picker::entity_picker_results(&items).into_string()))
}

#[require_permission("FMS", "read")]
pub async fn search_account(
    _path: JournalSearchAccountPath,
    ctx: RequestContext,
    Query(query): Query<SearchQuery>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.cash_journal_service();
    let keyword = query.q.as_deref().unwrap_or("");

    let items: Vec<EntityPickerItem> = svc
        .search_accounts(&service_ctx, &mut conn, keyword, 20)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            EntityPickerItem::new(r.id, format!("{} {}", r.code, r.name))
                .sub(format!("科目编码: {}", r.code))
        })
        .collect();

    Ok(Html(entity_picker::entity_picker_results(&items).into_string()))
}

// ── Page Handlers ──

#[require_permission("FMS", "read")]
pub async fn get_create(
    _path: JournalCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, .. } = ctx;

    let content = journal_create_page();
    let page_html = admin_page(
        is_htmx, "新建日记账", &claims, "finance", JournalCreatePath::PATH,
        "财务管理", Some(JournalListPath::PATH), content, &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("FMS", "create")]
pub async fn create(
    _path: JournalCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<JournalCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    let mut tx = state.pool.begin().await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

    let journal_type = JournalType::from_i16(form.journal_type)
        .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效日记账类型".into()))?;
    let direction = CashDirection::from_i16(form.direction)
        .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效方向".into()))?;
    let counterparty_type = CounterpartyType::from_i16(form.counterparty_type)
        .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效往来方类型".into()))?;

    let transaction_date = chrono::NaiveDate::parse_from_str(&form.transaction_date, "%Y-%m-%d")
        .map_err(|_| abt_core::shared::types::DomainError::Validation("无效交易日期".into()))?;

    let amount = form.amount.trim().parse::<rust_decimal::Decimal>()
        .map_err(|_| abt_core::shared::types::DomainError::Validation("无效金额".into()))?;

    let counterparty_id = form.counterparty_id.unwrap_or(0);
    let counterparty = abt_core::fms::enums::CounterpartyRef::from_parts(counterparty_type, counterparty_id);

    let source_type: DocumentType = match form.source_type {
        Some(1) => DocumentType::SalesOrder,
        Some(2) => DocumentType::PurchaseOrder,
        Some(3) => DocumentType::ExpenseReimbursement,
        _ => DocumentType::CashJournal,
    };
    let source_id = form.source_id.unwrap_or(0);

    // 多币种（issue #69）：CNY 强制汇率 = 1；非 CNY 解析汇率
    let currency = form.currency.trim().to_uppercase();
    let exchange_rate = if currency == "CNY" {
        rust_decimal::Decimal::ONE
    } else {
        form.exchange_rate
            .trim()
            .parse::<rust_decimal::Decimal>()
            .map_err(|_| abt_core::shared::types::DomainError::Validation("无效汇率".into()))?
    };

    // 出纳日记账简化为简单收付款单：金额取自表单，不再强制借贷分录（write_off 按 header amount 核销）
    let req = CreateCashJournalReq {
        journal_type,
        direction,
        amount,
        counterparty,
        source_type,
        source_id,
        bank_account: form.bank_account,
        transaction_date,
        period: form.period,
        remark: form.remark,
        currency,
        exchange_rate,
        lines: vec![],
    };

    let svc = state.cash_journal_service();
    svc.create(&service_ctx, &mut tx, req).await?;
    tx.commit().await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

    Ok(axum::response::Response::builder()
        .header("HX-Redirect", JournalListPath::PATH)
        .body(axum::body::Body::empty())
        .unwrap())
}

// ── Page ──

fn journal_create_page() -> Markup {
    // Counterparty picker config
    let cp_picker = EntityPickerConfig {
        modal_id: "cp-picker",
        title: "选择往来方",
        search_label: "关键词",
        search_placeholder: "搜索名称或编码…",
        search_path: JournalSearchCpPath::PATH,
        search_param: "q",
        target_id: "cp-id",
        display_id: "cp-display",
        event_name: "counterpartySelected",
        extra_include: Some("#cp-type"),
    };

    html! {
        div {
            // 返回链接
            a   href=(format!("{}?restore=true", JournalListPath::PATH))
                class="inline-flex items-center gap-1 text-sm text-muted hover:text-fg mb-4"
            { (icon::chevron_left_icon("w-4 h-4")) "返回列表" }
            // 标题
            h1 class="text-xl font-bold text-fg tracking-tight mb-6" { "新建出纳日记账" }

            form id="journal-create-form" hx-post=(JournalCreatePath::PATH) hx-swap="none" {
                // ── Section 1: 基本信息 ──
                div class="form-section" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                    { (icon::clipboard_document_icon("w-4 h-4")) " 基本信息" }
                    div class="grid grid-cols-2 gap-4 gap-x-6" {
                        // 日记账类型
                        div class="form-field" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            {
                                "日记账类型 "
                                span class="text-danger" { "*" }
                            }
                            select
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent"
                                name="journal_type"
                                required
                            {
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
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            {
                                "收付方向 "
                                span class="text-danger" { "*" }
                            }
                            select
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent"
                                name="direction"
                                required
                            {
                                option value="1" { "流入 (Inflow)" }
                                option value="2" { "流出 (Outflow)" }
                            }
                        }
                        // 金额 + 币种（issue #69 多币种）
                        div class="form-field" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            {
                                "金额 "
                                span class="text-danger" { "*" }
                            }
                            div class="flex gap-2" {
                                input
                                    class="flex-1 min-w-0 px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent font-mono text-right"
                                    type="number"
                                    step="any"
                                    min="0"
                                    name="amount"
                                    id="amount"
                                    required
                                    placeholder="0.00"
                                    _="on input call cjCalcCny()";
                                select
                                    class="!w-24 shrink-0 px-2 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent"
                                    name="currency"
                                    id="currency"
                                    _="on change call cjUpdateRate() then call cjCalcCny()"
                                {
                                    option value="CNY" selected { "CNY ¥" }
                                    option value="USD" { "USD $" }
                                    option value="EUR" { "EUR €" }
                                    option value="HKD" { "HKD HK$" }
                                }
                            }
                        }
                        // 汇率（CNY 时只读固定 1，issue #69）
                        div class="form-field" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            {
                                "汇率"
                            }
                            input
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-surface text-fg-2 font-mono text-right outline-none"
                                type="number"
                                step="any"
                                min="0"
                                name="exchange_rate"
                                id="exchange_rate"
                                value="1"
                                readonly
                                _="on input call cjCalcCny()";
                        }
                        // 折合人民币（只读实时计算，issue #69）
                        div class="form-field" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            {
                                "折合人民币"
                            }
                            div
                                class="w-full px-3 py-2 border border-border-soft rounded-sm text-sm bg-surface text-fg font-mono text-right"
                                id="amount_cny"
                            { "¥0.00" }
                        }
                        // 银行账户（预设 + 手动）
                        div class="form-field" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            {
                                "银行账户 "
                                span class="text-danger" { "*" }
                            }
                            select
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent mb-2"
                                id="bank-preset"
                                _="on change if my value is not '' put my value into #bank-account's value"
                            {
                                option value="" { "手动输入" }
                                option value="工商银行 6222021234567890" { "工商银行 6222...7890" }
                                option value="建设银行 6227001234567890" { "建设银行 6227...7890" }
                                option value="招商银行 6225881234567890" { "招商银行 6225...7890" }
                                option value="中国银行 6217001234567890" { "中国银行 6217...7890" }
                            }
                            input
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent"
                                type="text"
                                name="bank_account"
                                id="bank-account"
                                required
                                placeholder="或手动输入银行账号";
                        }
                        // 交易日期
                        div class="form-field" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            {
                                "交易日期 "
                                span class="text-danger" { "*" }
                            }
                            input
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent"
                                type="date"
                                name="transaction_date"
                                required;
                        }
                        // 期间
                        div class="form-field" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            {
                                "期间 "
                                span class="text-danger" { "*" }
                            }
                            input
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent"
                                type="month"
                                name="period"
                                required;
                        }
                        // 往来方类型
                        div class="form-field" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            {
                                "往来方类型 "
                                span class="text-danger" { "*" }
                            }
                            select
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent"
                                name="counterparty_type"
                                id="cp-type"
                                required
                                _="on change put '' into #cp-display's innerHTML then put '' into #cp-id's value"
                            {
                                option value="" disabled selected { "请选择" }
                                option value="1" { "客户" }
                                option value="2" { "供应商" }
                                option value="3" { "员工" }
                            }
                        }
                        // 往来方选择器
                        div class="form-field" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            {
                                "往来方 "
                                span class="text-danger" { "*" }
                            }
                            ({
                                entity_picker::entity_picker_field(
                                    "counterparty_id",
                                    "cp-id",
                                    "cp-display",
                                    "cp-picker",
                                    "选择往来方",
                                    true,
                                    "搜索选择往来方…",
                                )
                            })
                        }
                        // 来源类型
                        div class="form-field" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            { "来源单据" }
                            select
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent"
                                name="source_type"
                                id="source-type"
                            {
                                option value="" { "无关联" }
                                option value="1" { "销售订单" }
                                option value="2" { "采购订单" }
                                option value="3" { "费用报销" }
                            }
                        }
                        // 来源单据ID（可选）
                        div class="form-field" {
                            label
                                class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap"
                            { "来源单号" }
                            input
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent"
                                type="text"
                                name="source_id"
                                id="source-id"
                                placeholder="输入来源单号（可选）";
                        }
                    }
                    // 备注（跨列）
                    div class="form-field mt-4" {
                        label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                            "备注"
                        }
                        textarea
                            class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent"
                            name="remark"
                            placeholder="填写备注信息…"
                            rows="2" {}
                    }
                }
                // 借贷分录已移除：简化为简单收付款单（金额在基本信息区），write_off 按 header amount 核销（issue #78）
            }
        }
        // ── Entity Picker Modals ──
        (entity_picker::entity_picker_modal(&cp_picker))
        // ── Action Bar ──
        div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft"
        {
            a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                href=(format!("{}?restore=true", JournalListPath::PATH))
            { "取消" }
            button
                type="button"
                class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                _="on click trigger submit on #journal-create-form"
            { (icon::check_circle_icon("w-4 h-4")) "提交" }
        }
        // ── JS: Dynamic journal lines + counterparty picker ──
        ({
            PreEscaped(
                r#"<script>
// ── Counterparty Picker ──
(function() {
    // Re-open the cp picker with the current counterparty_type filter
    var cpTypeSelect = document.getElementById('cp-type');
    var cpPickerBtn = document.querySelector('[data-modal-id="cp-picker"]');
    if (cpTypeSelect && cpPickerBtn) {
        cpPickerBtn.addEventListener('click', function() {
            // The search URL already includes type via hx-include, so HTMX will
            // automatically append ?q=...&type=... from #cp-type
        });
    }
})();

// ── 多币种折算（issue #69）──
function cjUpdateRate() {
    var cur = document.getElementById('currency').value;
    var rate = document.getElementById('exchange_rate');
    if (!rate) return;
    if (cur === 'CNY') {
        rate.value = '1';
        rate.readOnly = true;
        rate.classList.add('bg-surface', 'text-fg-2');
        rate.classList.remove('bg-white', 'text-fg');
    } else {
        rate.readOnly = false;
        rate.classList.remove('bg-surface', 'text-fg-2');
        rate.classList.add('bg-white', 'text-fg');
    }
}
function cjCalcCny() {
    var amt = parseFloat(document.getElementById('amount').value) || 0;
    var rate = parseFloat(document.getElementById('exchange_rate').value) || 0;
    var box = document.getElementById('amount_cny');
    if (box) box.textContent = '¥' + (amt * rate).toFixed(2);
}
// 初始化
cjUpdateRate();
cjCalcCny();
</script>"#,
            )
        })
    }
}
