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
use abt_core::master_data::routing::model::{
    CreateRoutingReq, RoutingDetail, RoutingStep, RoutingStepInput, UpdateRoutingReq,
};
use abt_core::shared::types::{DomainError, PageParams};
use abt_macros::require_permission;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::routing::{
    RoutingCopyPath, RoutingCreatePath, RoutingDetailPath, RoutingEditPath, RoutingListPath,
};
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

// ── Form mode ──

#[derive(Clone, Copy, PartialEq)]
enum FormMode {
 New,
 Edit,
 Copy,
}

/// JS 端步骤行回填结构（字段名与注入的 JS `steps` 数组对齐，供编辑/复制预填）
#[derive(serde::Serialize)]
struct JsStep {
 process_code: String,
 is_required: bool,
 remark: String,
 product_id: String,
 product_name: String,
 work_center_id: String,
 unit_price: String,
 standard_time: String,
 is_outsourced: bool,
}

impl JsStep {
 fn from_step(s: &RoutingStep) -> Self {
 Self {
 process_code: s.process_code.clone(),
 is_required: s.is_required,
 remark: s.remark.clone().unwrap_or_default(),
 product_id: s.product_id.map(|id| id.to_string()).unwrap_or_default(),
 product_name: s.product_name.clone().unwrap_or_default(),
 work_center_id: s.work_center_id.map(|id| id.to_string()).unwrap_or_default(),
 unit_price: s.unit_price.map(|d| d.to_string()).unwrap_or_default(),
 standard_time: s.standard_time.map(|d| d.to_string()).unwrap_or_default(),
 is_outsourced: s.is_outsourced,
 }
 }
}

/// 解析 steps_json → RoutingStepInput，含产出品/计件单价校验（create 与 update 共用）
fn parse_steps(form: &RoutingCreateForm) -> crate::errors::Result<Vec<RoutingStepInput>> {
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
 // BOM 人工成本依赖 routing 模板的产出品 + 计件单价，提交前校验非空
 for (i, s) in steps.iter().enumerate() {
 if s.product_id.is_none() {
 return Err(DomainError::validation(format!("工序 {} 未配置产出品（BOM 人工成本与工序级领料依赖）", i + 1)).into());
 }
 if s.unit_price.is_none_or(|p| p <= rust_decimal::Decimal::ZERO) {
 return Err(DomainError::validation(format!("工序 {} 未配置计件单价（BOM 人工成本依据）", i + 1)).into());
 }
 }

 Ok(steps)
}

/// 加载工序步骤下拉数据（工序字典 / 产出品 / 工作中心），create/edit/copy 共用
async fn load_step_options(
 state: &crate::state::AppState,
 service_ctx: &abt_core::shared::types::ServiceContext,
 db: abt_core::shared::types::PgExecutor<'_>,
) -> crate::errors::Result<(
 Vec<abt_core::master_data::labor_process_dict::model::LaborProcessDict>,
 Vec<abt_core::master_data::product::model::Product>,
 Vec<abt_core::master_data::work_center::model::WorkCenter>,
)> {
 let processes = state
 .labor_process_dict_service()
 .list(service_ctx, db, LaborProcessDictQuery::default(), PageParams::new(1, 500))
 .await?
 .items;
 let products = state
 .product_service()
 .list(
 service_ctx,
 db,
 abt_core::master_data::product::model::ProductQuery {
 name: None,
 code: None,
 status: None,
 owner_department_id: None,
 category_id: None,
 },
 PageParams::new(1, 500),
 )
 .await?
 .items;
 let work_centers = abt_core::master_data::work_center::new_work_center_service(state.pool.clone())
 .list_active(service_ctx, db)
 .await
 .unwrap_or_default();
 Ok((processes, products, work_centers))
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

 let (processes, products, work_centers) = load_step_options(&state, &service_ctx, &mut conn).await?;
 let content = routing_form_page(&processes, &products, &work_centers, None, FormMode::New);
 let page_html = admin_page(
 is_htmx,
 "新建工艺路线",
 &claims,
 "production",
 RoutingCreatePath::PATH,
 "主数据管理",
 Some("新建工艺路线"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

#[require_permission("ROUTING", "update")]
pub async fn get_routing_edit(
 path: RoutingEditPath,
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

 let detail = state.routing_service().get_detail(&service_ctx, &mut conn, path.id).await?;
 let (processes, products, work_centers) = load_step_options(&state, &service_ctx, &mut conn).await?;
 let edit_path_str = RoutingEditPath { id: path.id }.to_string();
 let content = routing_form_page(&processes, &products, &work_centers, Some(&detail), FormMode::Edit);
 let page_html = admin_page(
 is_htmx,
 "编辑工艺路线",
 &claims,
 "production",
 &edit_path_str,
 "主数据管理",
 Some("编辑工艺路线"),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

#[require_permission("ROUTING", "create")]
pub async fn get_routing_copy(
 path: RoutingCopyPath,
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

 let detail = state.routing_service().get_detail(&service_ctx, &mut conn, path.id).await?;
 let (processes, products, work_centers) = load_step_options(&state, &service_ctx, &mut conn).await?;
 let content = routing_form_page(&processes, &products, &work_centers, Some(&detail), FormMode::Copy);
 let page_html = admin_page(
 is_htmx,
 "复制工艺路线",
 &claims,
 "production",
 RoutingCreatePath::PATH,
 "主数据管理",
 Some("复制工艺路线"),
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

 let steps = parse_steps(&form)?;

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

#[require_permission("ROUTING", "update")]
pub async fn post_routing_update(
 path: RoutingEditPath,
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

 let steps = parse_steps(&form)?;
 let req = UpdateRoutingReq {
 name: Some(form.name.trim().to_string()),
 description: form.description.filter(|d| !d.trim().is_empty()),
 steps: Some(steps),
 };

 state.routing_service().update(&service_ctx, &mut tx, path.id, req).await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let redirect = RoutingDetailPath { id: path.id }.to_string();
 Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn routing_form_page(
 processes: &[abt_core::master_data::labor_process_dict::model::LaborProcessDict],
 products: &[abt_core::master_data::product::model::Product],
 work_centers: &[abt_core::master_data::work_center::model::WorkCenter],
 existing: Option<&RoutingDetail>,
 mode: FormMode,
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

 // 回填字段（编辑/复制模式预填；新建模式为空 + code 自动生成）
 let name_value: String = match (existing, mode) {
 (Some(d), FormMode::Copy) => format!("{}-副本", d.routing.name),
 (Some(d), _) => d.routing.name.clone(),
 (None, _) => String::new(),
 };
 let code_value: String = match existing {
 Some(d) => d.routing.code.clone(),
 None => "自动生成".to_string(),
 };
 let description_value = existing
 .and_then(|d| d.routing.description.clone())
 .unwrap_or_default();
 // 工序步骤：编辑/复制回填现有步骤，新建给一行空步骤
 let steps_json = match existing {
 Some(d) => serde_json::to_string(&d.steps.iter().map(JsStep::from_step).collect::<Vec<_>>())
 .unwrap_or_else(|_| "[]".into()),
 None => serde_json::to_string(&[JsStep {
 process_code: String::new(),
 is_required: true,
 remark: String::new(),
 product_id: String::new(),
 product_name: String::new(),
 work_center_id: String::new(),
 unit_price: String::new(),
 standard_time: String::new(),
 is_outsourced: false,
 }])
 .unwrap_or_else(|_| "[]".into()),
 };
 let title: &str = match mode {
 FormMode::New => "新建工艺路线",
 FormMode::Edit => "编辑工艺路线",
 FormMode::Copy => "复制工艺路线",
 };
 // 编辑提交到 /routings/{id}/edit（update）；新建/复制提交到 /routings/new（create）
 let post_url: String = match (existing, mode) {
 (Some(d), FormMode::Edit) => RoutingEditPath { id: d.routing.id }.to_string(),
 _ => RoutingCreatePath::PATH.to_string(),
 };

 html! {
    div id="routing-app" {
        // ── Page Header ──
        div class="flex items-center justify-between mb-6" {
            a   class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150"
                href=(format!("{}?restore=true", RoutingListPath::PATH))
            { (icon::arrow_left_icon("w-4 h-4")) "返回工艺路线列表" }
            h1 class="text-xl font-bold text-fg tracking-tight" { (title) }
        }

        form
            id="routing-form"
            hx-post=(post_url)
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
                        input type="text" name="name" required placeholder="请输入路线名称" value=(name_value) {}
                    }
                    div class="form-field" {
                        label { "路线编码" }
                        input type="text" value=(code_value) disabled class="!bg-surface !text-muted cursor-not-allowed" {}
                    }
                    div class="form-field field-full" {
                        label { "描述" }
                        textarea
                            name="description"
                            placeholder="请输入描述信息…"
                            class="w-full resize-y min-h-[80px]" { (description_value) }
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
                    table class="data-table min-w-[960px]" {
                        thead {
                            tr {
                                th class="w-[50px] text-center" { "排序" }
                                th class="w-[200px]" { "工序名称" }
                                th class="w-[200px]" { "产出品" }
                                th class="w-[140px]" { "工作中心" }
                                th class="w-[100px]" { "计件单价" }
                                th class="w-[90px]" { "标准工时" }
                                th class="w-[50px] text-center" { "委外" }
                                th class="w-[50px] text-center" { "必经" }
                                th class="w-[200px]" { "备注" }
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
            div class="sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft" {
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
    // 产品选择弹窗（步骤行产出品共用，openProductPicker 动态指向当前行）
    ({
        crate::components::product_picker::product_picker_modal_deferred(
            "routing-product-modal",
            "routing-product-target",
            "routing-product-display",
        )
    })
    // TODO: Rewrite routingForm() to a proper vanilla JS module with DOM-based state reading
    script {
        ({
            PreEscaped(
                format!(
                    r#"
const processMap = {process_map_json};
const productMap = {product_map_json};
const workCenterMap = {work_center_map_json};
let steps = {steps_json};

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
 steps.push({{ process_code: '', is_required: true, remark: '', product_id: '', product_name: '', work_center_id: '', unit_price: '', standard_time: '', is_outsourced: false }});
 syncFromDom();
 renderSteps();
}}

function removeStep(idx) {{
 if (steps.length <= 1) return;
 syncFromDom();
 steps.splice(idx, 1);
 renderSteps();
}}

function syncFromDom() {{
 const rows = document.querySelectorAll('#routing-steps-body tr');
 rows.forEach((row, idx) => {{
 if (!steps[idx]) return;
 const selects = row.querySelectorAll('select');
 const checkboxes = row.querySelectorAll('input[type="checkbox"]');
 const inputs = row.querySelectorAll('input[type="number"], input[type="text"]:not(.cat-search)');
 const productInput = row.querySelector('.step-product-id');
 const opHidden = row.querySelector('.cat-select input[type=hidden]');
 if (opHidden) steps[idx].process_code = opHidden.value;
 if (selects[0]) steps[idx].work_center_id = selects[0].value;
 if (productInput) steps[idx].product_id = productInput.value;
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
 // 工序搜索下拉选项（复用 .cat-* class + filterCatOptions/selectCat）；data-name 含 code+名称便于搜索
 let opOptionsHtml = Object.keys(processMap).map(function(code) {{
 let label = code + ' · ' + processMap[code];
 return '<button type="button" class="cat-option block w-full text-left px-3 py-1.5 text-[13px] text-fg-2 hover:bg-accent-bg hover:text-accent border-none bg-transparent cursor-pointer" data-id="' + code + '" data-name="' + label + '" onclick="selectCat(this)">' + label + '</button>';
 }}).join('');
 let opLabel = step.process_code ? (step.process_code + ' · ' + (processMap[step.process_code] || '')) : '请选择工序';
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
 // 产出品名优先取选中时缓存的 product_name（picker 搜索结果直达），回退 productMap（编辑回填的前 N 个）
 let pname = step.product_name || (step.product_id && productMap[step.product_id]) || '';
 html += '<tr>' +
 '<td class="text-muted text-xs text-center">' + (idx + 1) + '</td>' +
 '<td>' +
 '<div class="cat-select relative">' +
 '<input type="hidden" class="step-process-code" value="' + (step.process_code || '') + '" onchange="onStepChange(' + idx + ')">' +
 '<button type="button" class="cat-trigger w-full flex items-center justify-between gap-2 px-2 py-[5px] border border-border rounded-sm text-[13px] bg-white text-fg cursor-pointer hover:border-[rgba(37,99,235,0.3)]" onclick="toggleOpCombo(this)">' +
 '<span class="cat-label truncate flex-1 text-left ' + (step.process_code ? '' : 'text-muted') + '">' + opLabel + '</span>' +
 '<svg class="w-3.5 h-3.5 text-muted shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M19 9l-7 7-7-7"></path></svg>' +
 '</button>' +
 '<div class="cat-backdrop fixed inset-0 z-[999]" style="display:none" onclick="closeOpCombo(this)"></div>' +
 '<div class="cat-dropdown fixed z-[1000] w-80 bg-white border border-border rounded-sm shadow-[var(--shadow-card)]" style="display:none">' +
 '<div class="p-2 border-b border-border-soft">' +
 '<input type="text" placeholder="搜索工序编码或名称…" class="cat-search w-full px-2 py-1 border border-border rounded-sm text-sm outline-none focus:border-accent" oninput="filterCatOptions(this)">' +
 '</div>' +
 '<div class="cat-list max-h-[280px] overflow-y-auto py-1">' + opOptionsHtml + '</div>' +
 '</div>' +
 '</div>' +
 '</td>' +
 '<td>' +
 '<input type="hidden" id="step-product-id-' + idx + '" class="step-product-id" value="' + (step.product_id || '') + '">' +
 '<div class="flex items-center gap-1">' +
 '<span id="step-product-display-' + idx + '" class="flex-1 text-[13px] truncate ' + (pname ? '' : 'text-muted') + '">' + (pname || '未选择') + '</span>' +
 '<button type="button" onclick="openProductPicker(' + idx + ')" class="shrink-0 text-xs text-accent px-2 py-[3px] border border-border rounded-sm cursor-pointer hover:bg-accent-bg whitespace-nowrap">' + (pname ? '更换' : '选择') + '</button>' +
 '</div>' +
 '</td>' +
 '<td><select onchange="onStepChange(' + idx + ')" class="w-full text-[13px] rounded-sm px-2 py-[5px] border border-border">' + wcopts + '</select></td>' +
 '<td><input type="number" step="any" onchange="onStepChange(' + idx + ')" value="' + up + '" placeholder="0.00" class="w-full text-[13px] rounded-sm px-2 py-[5px] border border-border font-mono text-right"></td>' +
 '<td><input type="number" step="any" onchange="onStepChange(' + idx + ')" value="' + st + '" placeholder="0.00" class="w-full text-[13px] rounded-sm px-2 py-[5px] border border-border font-mono text-right"></td>' +
 '<td class="text-center"><input type="checkbox" onchange="onStepChange(' + idx + ')" class="cursor-pointer w-[18px] h-[18px] accent-accent"' + chk_out + '></td>' +
 '<td class="text-center"><input type="checkbox" onchange="onStepChange(' + idx + ')" class="cursor-pointer w-[18px] h-[18px] accent-accent"' + chk_req + '></td>' +
 '<td><input type="text" onchange="onStepChange(' + idx + ')" value="' + rem + '" placeholder="备注" class="w-full text-[13px] rounded-sm px-2 py-[5px] border border-border"></td>' +
 '<td><button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center" onclick="removeStep(' + idx + ')" title="删除"><svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg></button></td>' +
 '</tr>';
 }});
 document.querySelector('#routing-steps-body').innerHTML = html;
}}

// 工序搜索下拉开关：fixed 定位（步骤表在 overflow-hidden 卡片内，absolute 会被裁）
function toggleOpCombo(trigger) {{
 const wrapper = trigger.closest('.cat-select');
 const dropdown = wrapper.querySelector('.cat-dropdown');
 const backdrop = wrapper.querySelector('.cat-backdrop');
 if (dropdown.style.display !== 'none') {{ closeOpCombo(trigger); return; }}
 const r = trigger.getBoundingClientRect();
 dropdown.style.left = r.left + 'px';
 dropdown.style.top = (r.bottom + 4) + 'px';
 dropdown.style.display = 'block';
 // 向下溢出视口则向上展开
 if (r.bottom + 304 > window.innerHeight && r.top > 324) {{
 dropdown.style.top = (r.top - dropdown.offsetHeight - 4) + 'px';
 }}
 // 向右溢出视口则右对齐
 if (r.left + 320 > window.innerWidth) {{
 dropdown.style.left = Math.max(8, window.innerWidth - 328) + 'px';
 }}
 backdrop.style.display = 'block';
 const search = wrapper.querySelector('.cat-search');
 search.value = '';
 filterCatOptions(search);
 search.focus();
}}

function closeOpCombo(el) {{
 const wrapper = el.closest('.cat-select');
 if (!wrapper) return;
 const dropdown = wrapper.querySelector('.cat-dropdown');
 const backdrop = wrapper.querySelector('.cat-backdrop');
 if (dropdown) dropdown.style.display = 'none';
 if (backdrop) backdrop.style.display = 'none';
}}

function onStepChange(idx) {{
 syncFromDom();
 renderSteps();
}}

// 产出品选择（步骤行共用一个产品选择弹窗，openProductPicker 动态指向当前行）
let editingProductIdx = null;
function openProductPicker(idx) {{
 editingProductIdx = idx;
 const modal = document.getElementById('routing-product-modal');
 modal.querySelector('input[name=target_id]').value = 'step-product-id-' + idx;
 modal.querySelector('input[name=display_id]').value = 'step-product-display-' + idx;
 modal.querySelectorAll('.product-search-input').forEach(i => i.value = '');
 modal.classList.add('is-open');
 htmx.ajax('GET', '/api/products/search', {{
 target: '#product-search-results', swap: 'innerHTML',
 values: {{ target_id: 'step-product-id-' + idx, display_id: 'step-product-display-' + idx, modal_id: 'routing-product-modal', name: '', code: '' }}
 }});
}}
document.body.addEventListener('productSelected', (e) => {{
 if (editingProductIdx !== null) {{
 syncFromDom();
 // 产品名由事件 detail 携带（picker 搜索结果直达，绕过仅含前 N 个产品的 productMap）
 steps[editingProductIdx].product_name = (e.detail && e.detail.productName) || '';
 editingProductIdx = null;
 renderSteps();
 }}
}});

// 页面加载渲染初始工序行（含工作中心/单价/工时/委外）
renderSteps();
"#,
                ),
            )
        })
    }
}
}
