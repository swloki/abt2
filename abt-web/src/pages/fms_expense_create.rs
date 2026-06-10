use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup, PreEscaped};
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
        is_htmx,
        "新建费用报销",
        &claims,
        "finance",
        ExpenseCreatePath::PATH,
        "费用报销",
        Some("新建"),
        content, &nav_filter,    );
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
        Err(_) => return Ok(axum::response::Redirect::to(ExpenseCreatePath::PATH).into_response()),
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
        Ok(_) => Ok(axum::response::Redirect::to(ExpenseListPath::PATH).into_response()),
        Err(_) => Ok(axum::response::Redirect::to(ExpenseCreatePath::PATH).into_response()),
    }
}

// ── Page ──

fn expense_create_page() -> Markup {
    html! {
        div class="fms-form-page" {
            // 返回链接
            a href=(ExpenseListPath::PATH) class="back-link" style="display:inline-flex;align-items:center;gap:6px;font-size:14px;color:var(--muted);margin-bottom:var(--space-6)" {
                (PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16"><path d="M19 12H5M12 19l-7-7 7-7"/></svg>"#))
                "返回列表"
            }

            // 标题
            div class="detail-header" style="margin-bottom:var(--space-6)" {
                h1 class="detail-no" { "新建费用报销" }
            }

            form id="expense-form" hx-post=(ExpenseCreatePath::PATH) hx-swap="none" {
                // ── 报销信息 ──
                div class="info-card" {
                    div class="info-card-title" { "报销信息" }
                    div class="form-grid" {
                        div class="form-field" {
                            label { "申请人 " span style="color:var(--danger)" { "*" } }
                            input type="text" value="Admin" readonly style="background:var(--surface-raised)";
                        }
                        div class="form-field" {
                            label { "所属部门" }
                            select {
                                option value="" { "请选择" }
                                option value="production" { "生产部" }
                                option value="sales" { "销售部" }
                                option value="purchase" { "采购部" }
                                option value="admin" { "行政部" }
                                option value="finance" { "财务部" }
                            }
                        }
                        div class="form-field" {
                            label { "报销日期 " span style="color:var(--danger)" { "*" } }
                            input class="form-input" type="date" name="expense_date" required;
                        }
                        div class="form-field" {
                            label { "备注" }
                            input class="form-input" type="text" name="remark" placeholder="报销事由简述";
                        }
                    }
                }

                // ── 费用明细 ──
                div class="info-card" {
                    div class="info-card-title" style="display:flex;justify-content:space-between;align-items:center" {
                        span { "费用明细" }
                        span style="font-size:14px;font-weight:400;color:var(--muted)" { "合计：" strong id="totalDisplay" style="color:var(--accent);font-family:var(--font-mono)" { "¥0.00" } }
                    }
                    div class="data-card-scroll" {
                        table class="line-table" style="min-width:800px" {
                            thead {
                                tr {
                                    th style="width:140px" { "费用类型 " span style="color:var(--danger)" { "*" } }
                                    th style="width:140px" { "金额 " span style="color:var(--danger)" { "*" } }
                                    th { "说明" }
                                    th style="width:160px" { "发票号" }
                                    th style="width:120px" { "成本中心" }
                                    th style="width:120px" { "利润中心" }
                                    th style="width:44px" {}
                                }
                            }
                            tbody id="expenseLines" {}
                        }
                    }
                    div style="padding:var(--space-4) 0" {
                        button type="button" class="add-row-btn" onclick="addExpenseLine()" {
                            (PreEscaped(r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 4v16m8-8H4"/></svg>"#))
                            "添加费用项"
                        }
                    }
                }

                input type="hidden" name="items_json" id="items_json_input";

                // ── 操作栏 ──
                div class="action-bar" {
                    a class="btn btn-default" href=(ExpenseListPath::PATH) { "取消" }
                    button type="button" class="btn btn-default" onclick="document.getElementById('expense-form').querySelector('input[name=expense_date]').value;document.getElementById('expense-form').requestSubmit()" {
                        (PreEscaped(r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M19 21H5a2 2 0 01-2-2V5a2 2 0 012-2h11l5 5v11a2 2 0 01-2 2z"/><path d="M17 21v-8H7v8M7 3v5h8"/></svg>"#))
                        "保存草稿"
                    }
                    button type="submit" class="btn btn-primary" {
                        (PreEscaped(r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M5 12l5 5L20 7"/></svg>"#))
                        "提交审批"
                    }
                }
            }
        }
        (PreEscaped(r#"<script>
        function addExpenseLine() {
            var tbody = document.getElementById('expenseLines');
            var row = document.createElement('tr');
            row.innerHTML = '<td><select data-field="type">'
                + '<option value="">选择类型</option>'
                + '<option value="1">差旅</option><option value="2">办公</option>'
                + '<option value="3">交通</option><option value="4">餐饮</option>'
                + '<option value="5">招待</option><option value="6">其他</option></select></td>'
                + '<td><input type="number" class="num-input" data-field="amount" placeholder="0.00" step="0.01" oninput="calcTotal()"></td>'
                + '<td><input type="text" data-field="description" placeholder="费用说明"></td>'
                + '<td><input type="text" data-field="receipt_no" placeholder="发票号"></td>'
                + '<td><select data-field="cost_center"><option value="">选择</option><option value="1">CC-001 生产部</option><option value="2">CC-002 销售部</option><option value="3">CC-003 管理部</option></select></td>'
                + '<td><select data-field="profit_center"><option value="">选择</option><option value="1">PC-001 华南</option><option value="2">PC-002 华东</option><option value="3">PC-003 华北</option></select></td>'
                + '<td><button type="button" class="remove-row-btn" title="删除行" onclick="this.closest(\'tr\').remove();calcTotal()"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><path d="M18 6L6 18M6 6l12 12"/></svg></button></td>';
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

        document.getElementById('expense-form').addEventListener('htmx:configRequest', function() {
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
                        receipt_no: receiptVal ? receiptVal.value : null,
                        cost_center: null,
                        profit_center: null
                    });
                }
            });
            document.getElementById('items_json_input').value = JSON.stringify(items);
        });

        // 初始添加一行
        addExpenseLine();
        </script>"#))
    }
}
