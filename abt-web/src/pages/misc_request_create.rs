use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::purchase::misc_request::MiscellaneousRequestService;
use abt_core::purchase::misc_request::model::*;
use abt_core::shared::types::DomainError;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::misc_request::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form request ──

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct MiscCreateForm {
    pub department: Option<String>,
    pub purpose: String,
    pub request_date: String,
    pub remark: Option<String>,
    pub items_json: String,
}

#[derive(Debug, Deserialize)]
struct ItemWeb {
    item_name: String,
    specification: Option<String>,
    quantity: String,
    unit: String,
    estimated_price: Option<String>,
    item_remark: Option<String>,
}

// ── Handlers ──

#[require_permission("MISC_REQUEST", "create")]
pub async fn get_misc_create(
    _path: MiscCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, .. } = ctx;

    let content = misc_create_page();
    let page_html = admin_page(
        is_htmx,
        "新建零星请购",
        &claims,
        "purchase",
        MiscCreatePath::PATH,
        "采购管理",
        Some("新建零星请购"),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

/// HTMX: return an empty item row fragment
#[require_permission("MISC_REQUEST", "create")]
pub async fn get_misc_item_row(
    _ctx: RequestContext,
) -> Result<Html<String>> {
    Ok(Html(empty_row_fragment().into_string()))
}

#[require_permission("MISC_REQUEST", "create")]
pub async fn create_misc(
    _path: MiscCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<MiscCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;
    let svc = state.misc_request_service();

    let request_date = chrono::NaiveDate::parse_from_str(&form.request_date, "%Y-%m-%d")
        .map_err(|e| DomainError::validation(format!("无效请购日期格式: {e}")))?;

    let web_items: Vec<ItemWeb> = serde_json::from_str(&form.items_json)
        .map_err(|e| DomainError::validation(format!("无效明细数据: {e}")))?;

    let items: Vec<CreateMiscItemRequest> = web_items
        .into_iter()
        .enumerate()
        .map(|(idx, item)| CreateMiscItemRequest {
            line_no: (idx as i32) + 1,
            item_name: item.item_name,
            specification: item.specification,
            quantity: item
                .quantity
                .parse()
                .unwrap_or(rust_decimal::Decimal::ZERO),
            unit: item.unit,
            estimated_price: item.estimated_price.and_then(|s| s.parse().ok()),
            remark: item.item_remark,
        })
        .collect();

    let department_id = claims
        .department_ids
        .first()
        .copied()
        .unwrap_or(1);

    let create_req = CreateMiscRequestRequest {
        department_id,
        request_date,
        purpose: form.purpose,
        remark: form.remark.unwrap_or_default(),
        items,
    };

    let id = svc.create(&service_ctx, &mut conn, create_req, None).await?;

    let redirect = MiscDetailPath { id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn misc_create_page() -> Markup {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    html! {
        div id="misc-app" {
            // ── Page Header ──
            div class="page-header" {
                a class="back-link" href=(format!("{}?restore=true", MiscListPath::PATH)) {
                    (icon::arrow_left_icon("w-4 h-4"))
                    "返回零星请购列表"
                }
                h1 class="page-title" { "新建零星请购" }
            }

            form id="misc-form"
                  hx-post=(MiscCreatePath::PATH)
                  hx-swap="none" {
                input type="hidden" id="items-json" name="items_json" value="[]";

            // ── Basic Info ──
            div class="form-section" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "基本信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label { "请购部门" }
                        select name="department" {
                            option value="" { "请选择部门" }
                            option value="行政部" { "行政部" }
                            option value="IT部" { "IT部" }
                            option value="生产部" { "生产部" }
                            option value="品质部" { "品质部" }
                            option value="研发部" { "研发部" }
                            option value="财务部" { "财务部" }
                            option value="人事部" { "人事部" }
                            option value="市场部" { "市场部" }
                        }
                    }
                    div class="form-field" {
                        label { "请购用途" span style="color:var(--danger)" { "*" } }
                        input type="text" name="purpose" required placeholder="输入请购用途" {}
                    }
                    div class="form-field" {
                        label { "请购日期" }
                        input type="date" name="request_date" value=(today) {}
                    }
                    div class="form-field span-2" {
                        label { "备注" }
                        textarea name="remark" placeholder="输入请购相关备注信息…" {}
                    }
                }
            }

            // ── Line Items ──
            div class="data-card" style="padding:0;overflow:hidden;margin-bottom:var(--space-4)" {
                div style="padding:var(--space-5) var(--space-5) var(--space-3);display:flex;justify-content:space-between;align-items:center" {
                    span class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" style="margin:0;padding:0;border:none" { "请购明细" }
                    button type="button" class="btn btn-sm btn-primary"
                        hx-get=(MiscItemRowPath::PATH)
                        hx-target="#misc-item-tbody"
                        hx-swap="beforeend" {
                        (icon::plus_icon("w-3.5 h-3.5"))
                        "添加行"
                    }
                }
                div style="overflow-x:auto" {
                    table class="data-table" style="min-width:900px" {
                        thead {
                            tr {
                                th style="width:36px;text-align:center" { "#" }
                                th { "物品名称" }
                                th { "规格型号" }
                                th style="width:100px;text-align:right" { "数量" }
                                th style="width:80px;text-align:center" { "单位" }
                                th style="width:120px;text-align:right" { "预估单价" }
                                th style="width:120px;text-align:right" { "预估金额" }
                                th { "备注" }
                                th style="width:36px" { }
                            }
                        }
                        tbody id="misc-item-tbody" { }
                    }
                }
                div class="add-row-bar" {
                    button type="button" class="btn-add-row"
                        hx-get=(MiscItemRowPath::PATH)
                        hx-target="#misc-item-tbody"
                        hx-swap="beforeend" {
                        (icon::plus_icon("w-3.5 h-3.5"))
                        "添加行"
                    }
                }
            }

            // ── Action Bar ──
            div class="create-action-bar" {
                a class="btn btn-default" href=(format!("{}?restore=true", MiscListPath::PATH)) { "取消" }
                div style="display:flex;gap:var(--space-3)" {
                    button type="button" class="btn btn-default" { "保存草稿" }
                    button type="submit" class="btn btn-primary" { "提交请购" }
                }
            }
            script {
                (maud::PreEscaped("document.currentScript.parentElement.addEventListener('submit', function(ev){
                    var items=[];
                    document.querySelectorAll('#misc-item-tbody tr').forEach(function(r){
                        var o={};
                        r.querySelectorAll('input,select,textarea').forEach(function(e){if(e.name)o[e.name]=e.value});
                        items.push(o)
                    });
                    document.querySelector('#items-json').value=JSON.stringify(items)
                })"))
            }
            }

        }
    }
}

fn empty_row_fragment() -> Markup {
    html! {
        tr oninput="if(!event.target.classList.contains('num-input'))return;const r=this,q=parseFloat(r.querySelector('[name=quantity]').value)||0,p=parseFloat(r.querySelector('[name=estimated_price]').value)||0;r.querySelector('.line-subtotal').textContent=(q*p).toFixed(2)" {
            td class="line-num" { }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="item_name" required placeholder="物品名称" style="width:100%;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="specification" placeholder="规格型号" style="width:100%;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" step="any" min="0" name="quantity" placeholder="0" style="width:90px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="unit" placeholder="单位" style="width:70px;text-align:center;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] num-input" type="number" step="any" min="0" name="estimated_price" placeholder="0.00" style="width:110px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td class="line-subtotal mono" style="text-align:right" { "0.00" }
            td { input class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)]" type="text" name="item_remark" placeholder="备注" style="width:100%;padding:5px 8px;font-size:13px;border:1px solid var(--border);border-radius:var(--radius-sm)" {} }
            td { button type="button" class="btn-remove-row" title="删除行"
                _="on click remove closest <tr/>" {
                (icon::x_icon("w-3.5 h-3.5"))
            } }
        }
    }
}
