use std::collections::HashMap;

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html, PreEscaped};
use serde::Deserialize;

use abt_core::master_data::labor_process_dict::LaborProcessDictService;
use abt_core::master_data::labor_process_dict::model::LaborProcessDictQuery;
use abt_core::master_data::product::ProductService;
use abt_core::master_data::routing::RoutingService;
use abt_core::master_data::work_center::WorkCenterService;
use abt_core::master_data::routing::model::{CreateRoutingReq, RoutingStepInput};
use abt_core::shared::types::{DomainError, PageParams};
use abt_macros::require_permission;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::routing::{RoutingCreatePath, RoutingDetailPath, RoutingListPath};
use crate::utils::RequestContext;

// ── Form request ──

#[derive(Debug, Deserialize)]
pub struct RoutingCreateForm {
 pub name: String,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub description: Option<String>,
 pub steps_json: String,
}

#[derive(Debug, Deserialize)]
struct StepWeb {
 process_code: String,
 is_required: bool,
 remark: Option<String>,
 // steps_json 是 JSON（integer/null），用 serde default 直接解析；empty_as_none 只接 string 会报「expected string」
 #[serde(default)]
 product_id: Option<i64>,
 #[serde(default)]
 work_center_id: Option<i64>,
 #[serde(default)]
 unit_price: Option<String>,
 #[serde(default)]
 standard_time: Option<String>,
 #[serde(default)]
 is_outsourced: bool,
}

// ── Handlers ──

#[require_permission("ROUTING", "create")]
pub async fn get_routing_create(
 _path: RoutingCreatePath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext {
 mut conn,
 state,
 service_ctx,
 claims,
 ..
 } = ctx;

 let lpd_svc = state.labor_process_dict_service();
 let processes = lpd_svc
 .list(
 &service_ctx,
 &mut conn,
 LaborProcessDictQuery::default(),
 PageParams::new(1, 500),
 )
 .await?;
 let products = state.product_service()
 .list(&service_ctx, &mut conn,
 abt_core::master_data::product::model::ProductQuery { name: None, code: None, status: None, owner_department_id: None, category_id: None },
 PageParams::new(1, 500))
 .await?;
 let work_centers = abt_core::master_data::work_center::new_work_center_service(state.pool.clone())
 .list_active(&service_ctx, &mut conn).await.unwrap_or_default();
 let content = routing_create_page(&processes.items, &products.items, &work_centers);
 let page_html = admin_page(
 is_htmx,
 "新建工艺路线",
 &claims,
 "md",
 RoutingCreatePath::PATH,
 "主数据管理",
 Some("新建工艺路线"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

#[require_permission("ROUTING", "create")]
pub async fn post_routing_create(
 _path: RoutingCreatePath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<RoutingCreateForm>,
) -> Result<impl IntoResponse> {
 let RequestContext {
 state,
 service_ctx,
 ..
 } = ctx;

 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 if form.name.trim().is_empty() {
 return Err(DomainError::validation("路线名称不能为空").into());
 }

 let web_steps: Vec<StepWeb> = serde_json::from_str(&form.steps_json)
 .map_err(|e| DomainError::validation(format!("无效工序数据: {e}")))?;

 if web_steps.is_empty() {
 return Err(DomainError::validation("至少需要一道工序步骤").into());
 }

 let steps: Vec<RoutingStepInput> = web_steps
 .into_iter()
 .enumerate()
 .map(|(i, s)| RoutingStepInput {
 process_code: s.process_code,
 step_order: (i + 1) as i32,
 is_required: s.is_required,
 remark: s.remark.filter(|r| !r.trim().is_empty()),
 product_id: s.product_id,
 work_center_id: s.work_center_id,
 unit_price: s.unit_price.and_then(|v| v.trim().parse::<rust_decimal::Decimal>().ok()),
 standard_time: s.standard_time.and_then(|v| v.trim().parse::<rust_decimal::Decimal>().ok()),
 is_outsourced: s.is_outsourced,
 ..Default::default()
 })
 .collect();
 // BOM 人工成本依赖 routing 模板的产出品 + 计件单价，发布前校验非空
 for (i, s) in steps.iter().enumerate() {
 if s.product_id.is_none() {
 return Err(DomainError::validation(format!("工序 {} 未配置产出品（BOM 人工成本与工序级领料依赖）", i + 1)).into());
 }
 if s.unit_price.is_none_or(|p| p <= rust_decimal::Decimal::ZERO) {
 return Err(DomainError::validation(format!("工序 {} 未配置计件单价（BOM 人工成本依据）", i + 1)).into());
 }
 }

 let create_req = CreateRoutingReq {
 name: form.name.trim().to_string(),
 description: form.description.filter(|d| !d.trim().is_empty()),
 steps,
 };

 let svc = state.routing_service();
 let id = svc.create(&service_ctx, &mut tx, create_req).await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let redirect = RoutingDetailPath { id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn routing_create_page(
 processes: &[abt_core::master_data::labor_process_dict::model::LaborProcessDict],
 products: &[abt_core::master_data::product::model::Product],
 work_centers: &[abt_core::master_data::work_center::model::WorkCenter],
) -> Markup {
 let process_map: HashMap<&str, &str> = processes
 .iter()
 .map(|p| (p.code.as_str(), p.name.as_str()))
 .collect();

 let process_map_json = serde_json::to_string(&process_map).unwrap_or_else(|_| "{}".into());

 // 产出品映射（product_id → name，注入 JS 渲染下拉）
 let product_map: HashMap<String, String> = products
 .iter()
 .map(|p| (p.product_id.to_string(), p.pdt_name.clone()))
 .collect();
 let product_map_json = serde_json::to_string(&product_map).unwrap_or_else(|_| "{}".into());

 // 工作中心映射（id → name，注入 JS 渲染下拉）
 let work_center_map: HashMap<String, String> = work_centers
 .iter()
 .map(|wc| (wc.id.to_string(), wc.name.clone()))
 .collect();
 let work_center_map_json = serde_json::to_string(&work_center_map).unwrap_or_else(|_| "{}".into());

 html! {
    div id="routing-app" {
        // ── Page Header ──
        div class="flex items-center justify-between mb-6" {
            a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
                href=(format!("{}?restore=true", RoutingListPath::PATH))
            { (icon::arrow_left_icon("w-4 h-4")) "返回工艺路线列表" }
            h1 class="text-xl font-bold text-fg tracking-tight" { "新建工艺路线" }
        }

        form
            id="routing-form"
            hx-post=(RoutingCreatePath::PATH)
            hx-swap="none"
            onsubmit="syncFromDom(); document.querySelector('#routing-form input[name=steps_json]').value = getStepsJson()"
        {
            input type="hidden" name="steps_json";
            // ── Section: 基本信息 ──
            div class="data-card mb-4" {
                div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft"
                { "基本信息" }
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label {
                            "路线名称 "
                            span class="text-danger" { "*" }
                        }
                        input type="text" name="name" required placeholder="请输入路线名称" {}
                    }
                    div class="form-field" {
                        label { "路线编码" }
                        input type="text" value="自动生成" readonly class="bg-surface text-muted" {}
                    }
                    div class="form-field field-full" {
                        label { "描述" }
                        textarea
                            name="description"
                            placeholder="请输入描述信息…"
                            class="w-full resize-y min-h-[80px]" {}
                    }
                }
            }
            // ── Section: 工序步骤 ──
            div class="data-card p-0 overflow-hidden mb-4" {
                div class="p-5 pb-3 flex justify-between items-center" {
                    span class="flex items-center gap-2 text-sm font-semibold text-fg m-0 p-0" {
                        "工序步骤"
                    }
                    button
                        type="button"
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)] icon:w-4 icon:h-4"
                        onclick="addStep()"
                    { (icon::plus_icon("w-3.5 h-3.5")) "添加工序" }
                }
                div class="overflow-x-auto" {
                    table class="data-table min-w-[800px]" {
                        thead {
                            tr {
                                th class="w-[60px] text-center" { "排序" }
                                th class="w-[200px]" { "工序代码" }
                                th class="w-[160px]" { "工序名称" }
                                th class="w-[180px]" { "产出品" }
                                th class="w-[150px]" { "工作中心" }
                                th class="w-[100px]" { "计件单价" }
                                th class="w-[90px]" { "标准工时" }
                                th class="w-[60px] text-center" { "委外" }
                                th class="w-[60px] text-center" { "必经" }
                                th { "备注" }
                                th class="w-[50px]" {}
                            }
                        }
                        tbody id="routing-steps-body" {}
                    }
                }
                div class="p-3 flex items-center gap-2" {
                    button
                        type="button"
                        class="inline-flex items-center gap-2 rounded-sm text-accent text-sm cursor-pointer"
                        onclick="addStep()"
                    { (icon::plus_icon("w-3.5 h-3.5")) "添加工序" }
                }
            }
            // ── Action Bar ──
            div class="flex items-center justify-end gap-3 pt-4 border-t border-border-soft" {
                a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    href=(format!("{}?restore=true", RoutingListPath::PATH))
                { "取消" }
                button
                    type="submit"
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                { "保存路线" }
            }
        }
    }
    // TODO: Rewrite routingForm() to a proper vanilla JS module with DOM-based state reading
    script {
        ({
            PreEscaped(
                format!(
                    r#"
const processMap = {process_map_json};
const productMap = {product_map_json};
const workCenterMap = {work_center_map_json};
let steps = [
 {{ process_code: '', is_required: true, remark: '', product_id: '', work_center_id: '', unit_price: '', standard_time: '', is_outsourced: false }}
];

function getStepsJson() {{
 return JSON.stringify(
 steps
 .filter(s => s.process_code)
 .map((s, i) => ({{
 process_code: s.process_code,
 step_order: i + 1,
 is_required: s.is_required,
 remark: s.remark || null,
 product_id: s.product_id && s.product_id !== '' ? Number(s.product_id) : null,
 work_center_id: s.work_center_id && s.work_center_id !== '' ? Number(s.work_center_id) : null,
 unit_price: s.unit_price || null,
 standard_time: s.standard_time || null,
 is_outsourced: !!s.is_outsourced,
 }}))
 );
}}

function addStep() {{
 steps.push({{ process_code: '', is_required: true, remark: '', product_id: '', work_center_id: '', unit_price: '', standard_time: '', is_outsourced: false }});
 syncFromDom();
 renderSteps();
}}

function removeStep(idx) {{
 if (steps.length <= 1) return;
 syncFromDom();
 steps.splice(idx, 1);
 renderSteps();
}}

function getProcessName(code) {{
 return processMap[code] || '—';
}}

function syncFromDom() {{
 const rows = document.querySelectorAll('#routing-steps-body tr');
 rows.forEach((row, idx) => {{
 if (!steps[idx]) return;
 const selects = row.querySelectorAll('select');
 const checkboxes = row.querySelectorAll('input[type="checkbox"]');
 const inputs = row.querySelectorAll('input[type="number"], input[type="text"]');
 if (selects[0]) steps[idx].process_code = selects[0].value;
 if (selects[1]) steps[idx].product_id = selects[1].value;
 if (selects[2]) steps[idx].work_center_id = selects[2].value;
 if (checkboxes[0]) steps[idx].is_outsourced = checkboxes[0].checked;
 if (checkboxes[1]) steps[idx].is_required = checkboxes[1].checked;
 if (inputs[0]) steps[idx].unit_price = inputs[0].value;
 if (inputs[1]) steps[idx].standard_time = inputs[1].value;
 if (inputs[2]) steps[idx].remark = inputs[2].value;
 }});
}}

function renderSteps() {{
 let html = '';
 steps.forEach((step, idx) => {{
 let opts = '<option value="">-- 请选择 --</option>';
 for (let code in processMap) {{
 let sel = step.process_code === code ? ' selected' : '';
 opts += '<option value="' + code + '"' + sel + '>' + code + ' - ' + processMap[code] + '</option>';
 }}
 let popts = '<option value="">-- 无 --</option>';
 for (let pid in productMap) {{
 let sel = String(step.product_id) === pid ? ' selected' : '';
 popts += '<option value="' + pid + '"' + sel + '>' + productMap[pid] + '</option>';
 }}
 let wcopts = '<option value="">-- 无 --</option>';
 for (let wcid in workCenterMap) {{
 let sel = String(step.work_center_id) === wcid ? ' selected' : '';
 wcopts += '<option value="' + wcid + '"' + sel + '>' + workCenterMap[wcid] + '</option>';
 }}
 let chk_req = step.is_required ? ' checked' : '';
 let chk_out = step.is_outsourced ? ' checked' : '';
 let up = step.unit_price || '';
 let st = step.standard_time || '';
 let rem = step.remark || '';
 html += '<tr>' +
 '<td class="text-muted text-xs text-center">' + (idx + 1) + '</td>' +
 '<td><select onchange="onStepChange(' + idx + ')" class="w-full text-[13px] rounded-sm px-2 py-[5px] border border-border">' + opts + '</select></td>' +
 '<td class="text-[13px] px-2 py-[5px]">' + getProcessName(step.process_code) + '</td>' +
 '<td><select onchange="onStepChange(' + idx + ')" class="w-full text-[13px] rounded-sm px-2 py-[5px] border border-border">' + popts + '</select></td>' +
 '<td><select onchange="onStepChange(' + idx + ')" class="w-full text-[13px] rounded-sm px-2 py-[5px] border border-border">' + wcopts + '</select></td>' +
 '<td><input type="number" step="0.01" min="0" onchange="onStepChange(' + idx + ')" value="' + up + '" placeholder="0.00" class="w-full text-[13px] rounded-sm px-2 py-[5px] border border-border font-mono text-right"></td>' +
 '<td><input type="number" step="0.01" min="0" onchange="onStepChange(' + idx + ')" value="' + st + '" placeholder="0.00" class="w-full text-[13px] rounded-sm px-2 py-[5px] border border-border font-mono text-right"></td>' +
 '<td class="text-center"><input type="checkbox" onchange="onStepChange(' + idx + ')" class="cursor-pointer w-[18px] h-[18px] accent-accent"' + chk_out + '></td>' +
 '<td class="text-center"><input type="checkbox" onchange="onStepChange(' + idx + ')" class="cursor-pointer w-[18px] h-[18px] accent-accent"' + chk_req + '></td>' +
 '<td><input type="text" onchange="onStepChange(' + idx + ')" value="' + rem + '" placeholder="备注" class="w-full text-[13px] rounded-sm px-2 py-[5px] border border-border"></td>' +
 '<td><button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center" onclick="removeStep(' + idx + ')" title="删除"><svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg></button></td>' +
 '</tr>';
 }});
 document.querySelector('#routing-steps-body').innerHTML = html;
}}

function onStepChange(idx) {{
 syncFromDom();
 renderSteps();
}}

// 页面加载渲染初始工序行（含工作中心/单价/工时/委外）
renderSteps();
"#,
                ),
            )
        })
    }
}
}
