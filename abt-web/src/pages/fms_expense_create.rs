use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup, PreEscaped};
use crate::components::icon;
use serde::Deserialize;

use abt_core::fms::expense::ExpenseReimbursementService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms::{ExpenseCreatePath, ExpenseListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

/// Intermediate JSON struct for deserializing form items
#[derive(Debug, Deserialize)]
struct ItemJson {
    expense_type: i16,
    amount: f64,
    description: String,
    receipt_no: Option<String>,
}

// ── Form ──

#[derive(Debug, Deserialize)]
pub struct ExpenseCreateForm {
    expense_date: String,
    remark: String,
    items_json: String,
}

// ── Handlers ──

#[require_permission("FMS", "read")]
pub async fn get_create(
    _path: ExpenseCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let claims = ctx.claims.clone();
    let content = expense_create_page();
    let page_html = admin_page(
        is_htmx, "新建费用报销", &claims, "finance", ExpenseCreatePath::PATH,
        "费用报销", Some("新建"), content, &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("FMS", "create")]
pub async fn create(
    _path: ExpenseCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ExpenseCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let items: Vec<ItemJson> = match serde_json::from_str(&form.items_json) {
        Ok(v) => v,
        Err(_) => return Ok(([("HX-Redirect", ExpenseCreatePath::PATH.to_string())], axum::response::Html(String::new())).into_response()),
    };

    let req = abt_core::fms::expense::model::CreateExpenseReq {
        applicant_id: service_ctx.operator_id,
        department_id: None,
        expense_date: chrono::NaiveDate::parse_from_str(&form.expense_date, "%Y-%m-%d")
            .unwrap_or(chrono::Utc::now().date_naive()),
        remark: form.remark,
        items: items.into_iter().map(|i| abt_core::fms::expense::model::ExpenseItemInput {
            expense_type: abt_core::fms::enums::ExpenseType::from_i16(i.expense_type)
                .unwrap_or(abt_core::fms::enums::ExpenseType::Other),
            amount: rust_decimal::Decimal::try_from(i.amount).unwrap_or(rust_decimal::Decimal::ZERO),
            description: i.description,
            receipt_no: i.receipt_no,
            cost_center: None,
            profit_center: None,
        }).collect(),
    };

    let svc = state.expense_service();
    match svc.create(&service_ctx, &mut conn, req).await {
        Ok(_) => Ok(([("HX-Redirect", ExpenseListPath::PATH.to_string())], axum::response::Html(String::new())).into_response()),
        Err(_) => Ok(([("HX-Redirect", ExpenseCreatePath::PATH.to_string())], axum::response::Html(String::new())).into_response()),
    }
}

// ── Page ──

fn expense_create_page() -> Markup {
    html! {
        div {
            // 返回链接
            a href=(format!("{}?restore=true", ExpenseListPath::PATH))
                class="inline-flex items-center gap-1 text-sm text-muted hover:text-fg transition-colors duration-150 mb-6" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回列表"
            }

            // 标题
            div class="mb-6" {
                h1 class="text-2xl font-bold text-fg tracking-tight" { "新建费用报销" }
            }

            form id="expense-form" hx-post=(ExpenseCreatePath::PATH) hx-swap="none" {
                // ── Section 1: 报销信息 ──
                div class="form-section" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
                        (icon::clipboard_document_icon("w-4 h-4"))
                        " 报销信息"
                    }
                    div class="grid grid-cols-2 gap-4 gap-x-6" {
                        // 申请人
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                                "申请人 " span class="text-danger" { "*" }
                            }
                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-surface text-fg-2 cursor-not-allowed" type="text" value="Admin" readonly;
                        }
                        // 所属部门
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "所属部门" }
                            select class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent cursor-pointer" name="department" {
                                option value="" { "请选择" }
                                option value="production" { "生产部" }
                                option value="sales" { "销售部" }
                                option value="purchase" { "采购部" }
                                option value="admin" { "行政部" }
                                option value="finance" { "财务部" }
                            }
                        }
                        // 报销日期
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" {
                                "报销日期 " span class="text-danger" { "*" }
                            }
                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent cursor-pointer" type="date" name="expense_date" required;
                        }
                        // 备注
                        div class="form-field" {
                            label class="block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap" { "备注" }
                            input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="remark" placeholder="报销事由简述";
                        }
                    }
                }

                // ── Section 2: 费用明细 ──
                div class="form-section" {
                    div class="flex items-center justify-between text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" {
                        span class="flex items-center gap-2" {
                            (icon::dollar_icon("w-4 h-4"))
                            " 费用明细"
                        }
                        span class="text-sm font-normal text-muted" {
                            "合计："
                            strong id="totalDisplay" class="text-accent font-mono" { "¥0.00" }
                        }
                    }
                    div class="overflow-x-auto" {
                        table class="w-full border-separate border-spacing-0 min-w-[800px]" {
                            thead {
                                tr {
                                    th class="w-[140px] text-left text-xs font-semibold text-fg-2 px-3 py-2 border-b border-border-soft uppercase tracking-wide" { "费用类型 " span class="text-danger" { "*" } }
                                    th class="w-[140px] text-left text-xs font-semibold text-fg-2 px-3 py-2 border-b border-border-soft uppercase tracking-wide" { "金额 " span class="text-danger" { "*" } }
                                    th class="text-left text-xs font-semibold text-fg-2 px-3 py-2 border-b border-border-soft uppercase tracking-wide" { "说明" }
                                    th class="w-[160px] text-left text-xs font-semibold text-fg-2 px-3 py-2 border-b border-border-soft uppercase tracking-wide" { "发票号" }
                                    th class="w-[120px] text-left text-xs font-semibold text-fg-2 px-3 py-2 border-b border-border-soft uppercase tracking-wide" { "成本中心" }
                                    th class="w-[120px] text-left text-xs font-semibold text-fg-2 px-3 py-2 border-b border-border-soft uppercase tracking-wide" { "利润中心" }
                                    th class="w-[44px] px-3 py-2 border-b border-border-soft" {}
                                }
                            }
                            tbody id="expenseLines" {}
                        }
                    }
                    // 添加费用项
                    div class="py-4" {
                        button type="button"
                            class="flex items-center justify-center gap-2 w-full py-3 border-1.5 border-dashed border-border text-accent text-sm font-medium cursor-pointer rounded-md hover:border-accent hover:bg-[rgba(37,99,235,0.04)] transition-all duration-200"
                            onclick="addExpenseLine()" {
                            (icon::plus_icon("w-4 h-4"))
                            "添加费用项"
                        }
                    }
                }

                input type="hidden" name="items_json" id="items_json_input";
            }

            // ── Action Bar ──
            div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft" {
                a class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    href=(format!("{}?restore=true", ExpenseListPath::PATH)) {
                    "取消"
                }
                button type="button"
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    onclick="document.getElementById('expense-form').requestSubmit()" {
                    (PreEscaped(r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M19 21H5a2 2 0 01-2-2V5a2 2 0 012-2h11l5 5v11a2 2 0 01-2 2z"/><path d="M17 21v-8H7v8M7 3v5h8"/></svg>"#))
                    "保存草稿"
                }
                button type="button"
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                    _="on click trigger submit on #expense-form" {
                    (icon::check_circle_icon("w-4 h-4"))
                    "提交审批"
                }
            }
        }
        (PreEscaped(r#"<script>
function addExpenseLine() {
    var tbody = document.getElementById('expenseLines');
    var row = document.createElement('tr');
    row.innerHTML = '<td class="px-3 py-2 border-b border-border-soft"><select class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent" data-field="type">'
        + '<option value="">选择类型</option>'
        + '<option value="1">差旅</option><option value="2">办公</option>'
        + '<option value="3">交通</option><option value="4">餐饮</option>'
        + '<option value="5">招待</option><option value="6">其他</option></select></td>'
        + '<td class="px-3 py-2 border-b border-border-soft"><input type="number" step="any" class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent font-mono text-right" data-field="amount" placeholder="0.00" oninput="calcTotal()"></td>'
        + '<td class="px-3 py-2 border-b border-border-soft"><input type="text" class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent" data-field="description" placeholder="费用说明"></td>'
        + '<td class="px-3 py-2 border-b border-border-soft"><input type="text" class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent" data-field="receipt_no" placeholder="发票号"></td>'
        + '<td class="px-3 py-2 border-b border-border-soft"><select class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent" data-field="cost_center"><option value="">选择</option><option value="1">CC-001 生产部</option><option value="2">CC-002 销售部</option><option value="3">CC-003 管理部</option></select></td>'
        + '<td class="px-3 py-2 border-b border-border-soft"><select class="w-full px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent" data-field="profit_center"><option value="">选择</option><option value="1">PC-001 华南</option><option value="2">PC-002 华东</option><option value="3">PC-003 华北</option></select></td>'
        + '<td class="px-3 py-2 border-b border-border-soft"><button type="button" class="w-7 h-7 border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:bg-[rgba(220,38,38,0.08)] hover:text-danger transition-all" title="删除行" onclick="this.closest(\'tr\').remove();calcTotal()"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><path d="M18 6L6 18M6 6l12 12"/></svg></button></td>';
    tbody.appendChild(row);
}

function calcTotal() {
    var total = 0;
    document.querySelectorAll('#expenseLines input[data-field="amount"]').forEach(function(el) {
        if (el.value) total += parseFloat(el.value);
    });
    var display = document.getElementById('totalDisplay');
    if (display) display.textContent = '¥' + total.toFixed(2);
}

document.getElementById('expense-form').addEventListener('htmx:configRequest', function(e) {
    var rows = document.querySelectorAll('#expenseLines tr');
    var items = [];
    rows.forEach(function(row) {
        var typeVal = row.querySelector('[data-field="type"]');
        var amountVal = row.querySelector('[data-field="amount"]');
        var descVal = row.querySelector('[data-field="description"]');
        var receiptVal = row.querySelector('[data-field="receipt_no"]');
        if (amountVal && amountVal.value) {
            items.push({
                expense_type: parseInt(typeVal ? typeVal.value : '6'),
                amount: parseFloat(amountVal.value),
                description: descVal ? descVal.value : '',
                receipt_no: (receiptVal && receiptVal.value) ? receiptVal.value : null,
                cost_center: null,
                profit_center: null
            });
        }
    });
    var json = JSON.stringify(items);
    document.getElementById('items_json_input').value = json;
    // HTMX 在触发 configRequest 前已收集 form parameters（此时 items_json 为空），
    // 必须显式改写 parameters 才能让请求带上正确 items_json
    if (e && e.detail && e.detail.parameters) e.detail.parameters['items_json'] = json;
});

// 初始添加一行
addExpenseLine();
</script>"#))
    }
}
