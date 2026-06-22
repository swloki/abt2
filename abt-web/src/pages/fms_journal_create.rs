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
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Forms ──

#[derive(Debug, Deserialize)]
pub struct JournalCreateForm {
    pub journal_type: i16,
    pub direction: i16,
    pub bank_account: String,
    pub counterparty_type: i16,
    pub counterparty_id: Option<i64>,
    pub source_type: Option<i16>,
    pub source_id: Option<i64>,
    pub transaction_date: String,
    pub period: String,
    pub remark: String,
    pub lines_json: String,
}

#[derive(Debug, Deserialize)]
struct LineJson {
    account_code: String,
    debit_amount: f64,
    credit_amount: f64,
    remark: String,
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
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let journal_type = JournalType::from_i16(form.journal_type)
        .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效日记账类型".into()))?;
    let direction = CashDirection::from_i16(form.direction)
        .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效方向".into()))?;
    let counterparty_type = CounterpartyType::from_i16(form.counterparty_type)
        .ok_or_else(|| abt_core::shared::types::DomainError::Validation("无效往来方类型".into()))?;

    let transaction_date = chrono::NaiveDate::parse_from_str(&form.transaction_date, "%Y-%m-%d")
        .map_err(|_| abt_core::shared::types::DomainError::Validation("无效交易日期".into()))?;

    // Parse journal lines from JSON
    let lines_data: Vec<LineJson> = serde_json::from_str(&form.lines_json)
        .map_err(|_| abt_core::shared::types::DomainError::Validation("无效分录数据格式".into()))?;

    if lines_data.is_empty() {
        return Err(abt_core::shared::types::DomainError::Validation("至少需要一条分录".into()).into());
    }

    let lines: Vec<abt_core::fms::cash_journal::model::CashJournalLineInput> = lines_data
        .into_iter()
        .map(|l| abt_core::fms::cash_journal::model::CashJournalLineInput {
            account_code: l.account_code,
            debit_amount: rust_decimal::Decimal::try_from(l.debit_amount).unwrap_or(rust_decimal::Decimal::ZERO),
            credit_amount: rust_decimal::Decimal::try_from(l.credit_amount).unwrap_or(rust_decimal::Decimal::ZERO),
            cost_center: None,
            profit_center: None,
            remark: l.remark,
        })
        .collect();

    // Validate balanced: total debit == total credit
    let total_debit: rust_decimal::Decimal = lines.iter().map(|l| l.debit_amount).sum();
    let total_credit: rust_decimal::Decimal = lines.iter().map(|l| l.credit_amount).sum();
    if total_debit != total_credit {
        return Err(abt_core::shared::types::DomainError::Validation(
            format!("借贷不平衡：借方 ¥{total_debit}，贷方 ¥{total_credit}")
        ).into());
    }
    if total_debit == rust_decimal::Decimal::ZERO {
        return Err(abt_core::shared::types::DomainError::Validation("金额不能为零".into()).into());
    }

    let counterparty_id = form.counterparty_id.unwrap_or(0);
    let counterparty = abt_core::fms::enums::CounterpartyRef::from_parts(counterparty_type, counterparty_id);

    let source_type: DocumentType = match form.source_type {
        Some(1) => DocumentType::SalesOrder,
        Some(2) => DocumentType::PurchaseOrder,
        Some(3) => DocumentType::ExpenseReimbursement,
        _ => DocumentType::CashJournal,
    };
    let source_id = form.source_id.unwrap_or(0);

    let req = CreateCashJournalReq {
        journal_type,
        direction,
        amount: total_debit,
        counterparty,
        source_type,
        source_id,
        bank_account: form.bank_account,
        transaction_date,
        period: form.period,
        remark: form.remark,
        lines,
    };

    let svc = state.cash_journal_service();
    svc.create(&service_ctx, &mut conn, req).await?;

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

    // Account picker config (reused for each line)
    let acct_picker = EntityPickerConfig {
        modal_id: "acct-picker",
        title: "选择会计科目",
        search_label: "关键词",
        search_placeholder: "搜索科目编码或名称…",
        search_path: JournalSearchAccountPath::PATH,
        search_param: "q",
        target_id: "acct-id",
        display_id: "acct-display",
        event_name: "accountSelected",
        extra_include: None,
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
                // ── Section 2: 借贷分录（多行动态）──
                div class="form-section" {
                    div class="flex items-center justify-between text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                    {
                        span class="flex items-center gap-2" {
                            (icon::dollar_icon("w-4 h-4"))
                            " 借贷分录"
                        }
                        span class="text-sm font-normal text-muted" {
                            "借方合计："
                            strong id="total-debit-display" class="text-accent font-mono" {
                                "¥0.00"
                            }
                            "  贷方合计："
                            strong id="total-credit-display" class="text-success font-mono" {
                                "¥0.00"
                            }
                            "  差额："
                            strong id="balance-display" class="text-danger font-mono" { "¥0.00" }
                        }
                    }
                    div class="overflow-x-auto" {
                        table class="w-full border-separate border-spacing-0 min-w-[700px]" {
                            thead {
                                tr {
                                    th  class="text-left text-xs font-semibold text-fg-2 px-3 py-2 border-b border-border-soft uppercase tracking-wide w-[180px]"
                                    {
                                        "科目 "
                                        span class="text-danger" { "*" }
                                    }
                                    th  class="w-[140px] text-right text-xs font-semibold text-fg-2 px-3 py-2 border-b border-border-soft uppercase tracking-wide"
                                    { "借方金额" }
                                    th  class="w-[140px] text-right text-xs font-semibold text-fg-2 px-3 py-2 border-b border-border-soft uppercase tracking-wide"
                                    { "贷方金额" }
                                    th  class="text-left text-xs font-semibold text-fg-2 px-3 py-2 border-b border-border-soft uppercase tracking-wide"
                                    { "备注" }
                                    th class="w-[44px] px-3 py-2 border-b border-border-soft" {}
                                }
                            }
                            tbody id="journal-lines" {}
                        }
                    }
                    // 添加分录行
                    div class="py-4" {
                        button
                            type="button"
                            class="flex items-center justify-center gap-2 w-full py-3 border-1.5 border-dashed border-border text-accent text-sm font-medium cursor-pointer rounded-md hover:border-accent hover:bg-[rgba(37,99,235,0.04)] transition-all duration-200"
                            onclick="addJournalLine()"
                        { (icon::plus_icon("w-4 h-4")) "添加分录行" }
                    }
                }

                input type="hidden" name="lines_json" id="lines-json-input";
                // Hidden field for counterparty_id comes from entity_picker
                input type="hidden" name="source_id" id="source-id" value="0";
            }
        }
        // ── Entity Picker Modals ──
        (entity_picker::entity_picker_modal(&cp_picker))
        (entity_picker::entity_picker_modal(&acct_picker))
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

// ── Journal Lines ──
function addJournalLine(accountCode, debitAmount, creditAmount, remarkText) {
    var tbody = document.getElementById('journal-lines');
    var row = document.createElement('tr');
    row.className = 'journal-line-row';
    row.innerHTML =
        '<td class="px-3 py-2 border-b border-border-soft">' +
          '<input type="text" class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent font-mono" ' +
            'data-field="account_code" placeholder="科目编码" oninput="calcJournalTotal()">' +
        '</td>' +
        '<td class="px-3 py-2 border-b border-border-soft">' +
          '<input type="number" step="any" class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent font-mono text-right" ' +
            'data-field="debit_amount" placeholder="0.00" value="' + (debitAmount || '') + '" oninput="calcJournalTotal()">' +
        '</td>' +
        '<td class="px-3 py-2 border-b border-border-soft">' +
          '<input type="number" step="any" class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent font-mono text-right" ' +
            'data-field="credit_amount" placeholder="0.00" value="' + (creditAmount || '') + '" oninput="calcJournalTotal()">' +
        '</td>' +
        '<td class="px-3 py-2 border-b border-border-soft">' +
          '<input type="text" class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent" ' +
            'data-field="remark" placeholder="备注" value="' + (remarkText || '') + '">' +
        '</td>' +
        '<td class="px-3 py-2 border-b border-border-soft">' +
          '<button type="button" class="w-7 h-7 border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:bg-[rgba(220,38,38,0.08)] hover:text-danger transition-all"' +
            ' onclick="this.closest(\'tr\').remove();calcJournalTotal()">' +
            '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><path d="M18 6L6 18M6 6l12 12"/></svg>' +
          '</button>' +
        '</td>';
    tbody.appendChild(row);
}

function calcJournalTotal() {
    var totalDebit = 0, totalCredit = 0;
    document.querySelectorAll('#journal-lines tr').forEach(function(row) {
        var dEl = row.querySelector('[data-field="debit_amount"]');
        var cEl = row.querySelector('[data-field="credit_amount"]');
        if (dEl && dEl.value) totalDebit += parseFloat(dEl.value) || 0;
        if (cEl && cEl.value) totalCredit += parseFloat(cEl.value) || 0;
    });

    var balance = totalDebit - totalCredit;

    var tDisplay = document.getElementById('total-debit-display');
    if (tDisplay) tDisplay.textContent = '¥' + totalDebit.toFixed(2);

    var cDisplay = document.getElementById('total-credit-display');
    if (cDisplay) cDisplay.textContent = '¥' + totalCredit.toFixed(2);

    var bDisplay = document.getElementById('balance-display');
    if (bDisplay) {
        bDisplay.textContent = '¥' + balance.toFixed(2);
        bDisplay.className = balance === 0 ? 'text-success font-mono' : 'text-danger font-mono';
    }
}

// Collect lines before submit
document.getElementById('journal-create-form').addEventListener('htmx:configRequest', function(e) {
    var lines = [];
    document.querySelectorAll('#journal-lines tr').forEach(function(row) {
        var account = row.querySelector('[data-field="account_code"]');
        var debit = row.querySelector('[data-field="debit_amount"]');
        var credit = row.querySelector('[data-field="credit_amount"]');
        var remark = row.querySelector('[data-field="remark"]');
        if (account && account.value) {
            lines.push({
                account_code: account.value,
                debit_amount: parseFloat(debit ? debit.value : 0) || 0,
                credit_amount: parseFloat(credit ? credit.value : 0) || 0,
                remark: remark ? remark.value : ''
            });
        }
    });
    var json = JSON.stringify(lines);
    document.getElementById('lines-json-input').value = json;
    if (e && e.detail && e.detail.parameters) e.detail.parameters['lines_json'] = json;
});

// Add 2 default lines (debit + credit)
addJournalLine('', '', '', '借方');
addJournalLine('', '', '', '贷方');
</script>"#,
            )
        })
    }
}
