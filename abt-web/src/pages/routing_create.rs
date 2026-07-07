use std::collections::{HashMap, HashSet};

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html, PreEscaped};
use serde::Deserialize;

use abt_core::master_data::bom::BomQueryService;
use abt_core::master_data::labor_process_dict::LaborProcessDictService;
use abt_core::master_data::labor_process_dict::model::LaborProcessDictQuery;
use abt_core::master_data::product::ProductService;
use abt_core::master_data::routing::RoutingService;
use abt_core::master_data::work_center::WorkCenterService;
use abt_core::master_data::routing::model::{
    CreateRoutingReq, RoutingDetail, RoutingStep, RoutingStepInput, UpdateRoutingReq,
};
use abt_core::shared::types::{DomainError, PageParams, PgExecutor};
use abt_macros::require_permission;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::routing::{
    RoutingCopyPath, RoutingCreatePath, RoutingDetailPath, RoutingEditPath, RoutingListPath,
    RoutingOutputSearchPath,
};
use crate::utils::RequestContext;

// ── Form request ──

#[derive(Debug, Deserialize)]
pub struct RoutingCreateForm {
 pub name: String,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub description: Option<String>,
 pub steps_json: String,
 /// 新建/复制时关联的主产品 code（同时建立 bom_routings 关联；产出品候选集由此产品 BOM 派生）。Issue #212
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub bind_product_code: Option<String>,
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

/// 解析 steps_json → RoutingStepInput，含产出品/计件单价校验（create 与 update 共用）。
///
/// `allowed` = 关联产品 BOM 非叶子节点 product_id 集合（产出品候选集，Issue #212）。
/// 非空时校验每道工序产出品 ∈ allowed；为空（如关联产品无已发布 BOM）则跳过归属校验，
/// 仅前端 picker 限定，避免无 BOM 新产品无法配工序。
fn parse_steps(form: &RoutingCreateForm, allowed: &HashSet<i64>) -> crate::errors::Result<Vec<RoutingStepInput>> {
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
 // Issue #212: 产出品必须 ∈ 关联产品 BOM 非叶子节点（成品/半成品），防绕过前端 picker
 if let Some(pid) = s.product_id {
 if !allowed.is_empty() && !allowed.contains(&pid) {
 return Err(DomainError::validation(format!(
 "工序 {} 的产出品不在关联产品 BOM 物料项内（仅可选关联 BOM 的成品/半成品）", i + 1)).into());
 }
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

/// 计算工序产出品候选集 = 关联产品 BOM 非叶子节点 product_id 集合（Issue #212）。
///
/// - `routing_id`（编辑模式）：取该 routing 所有关联 BOM 的非叶子节点并集（多产品共享）
/// - `bind_product_code`（新建/复制模式）：取该单产品 BOM 非叶子节点
/// 二者二选一；均无则返回空集（`parse_steps` 对空集跳过归属校验，仅靠前端限定）。
async fn compute_output_candidates(
 state: &crate::state::AppState,
 service_ctx: &abt_core::shared::types::ServiceContext,
 db: PgExecutor<'_>,
 routing_id: Option<i64>,
 bind_product_code: Option<&str>,
) -> HashSet<i64> {
 let codes: Vec<String> = if let Some(rid) = routing_id {
 state.routing_service()
 .list_boms_by_routing(service_ctx, db, rid)
 .await.unwrap_or_default()
 .into_iter().map(|br| br.product_code).collect()
 } else if let Some(pc) = bind_product_code.filter(|s| !s.trim().is_empty()) {
 vec![pc.to_string()]
 } else {
 return HashSet::new();
 };
 state.bom_query_service()
 .list_non_leaf_product_ids_by_product_codes(service_ctx, db, &codes)
 .await.unwrap_or_default()
 .into_iter().collect()
}

// ── 产出品候选搜索端点（Issue #212）──

#[derive(Debug, Deserialize)]
pub struct RoutingOutputSearchParams {
 pub routing_id: Option<i64>,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub product_code: Option<String>,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub name: Option<String>,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub code: Option<String>,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub target_id: Option<String>,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub display_id: Option<String>,
 #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
 pub modal_id: Option<String>,
}

/// 工序产出品专用搜索：候选集限定为关联产品 BOM 的非叶子节点（成品/半成品）。Issue #212。
///
/// 复用 `product_picker_results` 渲染；候选集可能很大（如 routing#1 = 1776 个），
/// 由后端按 `routing_id`/`product_code` 自算 + 名称/编码过滤，避免前端传超长 bom_product_ids 串。
#[require_permission("ROUTING", "read")]
pub async fn get_routing_output_search(
 ctx: RequestContext,
 Query(params): Query<RoutingOutputSearchParams>,
) -> Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let candidate_ids: Vec<i64> = {
 let codes: Vec<String> = if let Some(rid) = params.routing_id {
 state.routing_service()
 .list_boms_by_routing(&service_ctx, &mut conn, rid)
 .await.unwrap_or_default()
 .into_iter().map(|br| br.product_code).collect()
 } else if let Some(ref pc) = params.product_code {
 vec![pc.clone()]
 } else {
 Vec::new()
 };
 if codes.is_empty() {
 Vec::new()
 } else {
 state.bom_query_service()
 .list_non_leaf_product_ids_by_product_codes(&service_ctx, &mut conn, &codes)
 .await.unwrap_or_default()
 }
 };
 let products = if candidate_ids.is_empty() {
 Vec::new()
 } else {
 state.product_service()
 .get_by_ids(&service_ctx, &mut conn, candidate_ids)
 .await.unwrap_or_default()
 .into_iter()
 .filter(|p| {
 let nm = params.name.as_ref().map_or(true, |n| p.pdt_name.contains(n.as_str()));
 let cm = params.code.as_ref().map_or(true, |c| p.product_code.contains(c.as_str()));
 nm && cm
 })
 .collect()
 };
 let target = params.target_id.as_deref().unwrap_or("step-product-id-0");
 let display = params.display_id.as_deref().unwrap_or("step-product-display-0");
 let modal = params.modal_id.as_deref().unwrap_or("routing-output-modal");
 Ok(Html(
 crate::components::product_picker::product_picker_results(&products, target, display, modal)
 .into_string(),
 ))
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
 let content = routing_form_page(
 &processes, &products, &work_centers, None, FormMode::New,
 // Issue #212：新建无 routing_id，产出品候选由用户在基本信息区选关联产品后动态拉取
 None, None, 0,
 );
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
 // Issue #212：编辑模式产出品候选 = 该 routing 所有关联 BOM 非叶子节点并集（picker 按 routing_id 动态拉取）
 let bound_bom_count = state.routing_service()
 .list_boms_by_routing(&service_ctx, &mut conn, path.id)
 .await.unwrap_or_default().len();
 let edit_path_str = RoutingEditPath { id: path.id }.to_string();
 let content = routing_form_page(
 &processes, &products, &work_centers, Some(&detail), FormMode::Edit,
 Some(path.id), None, bound_bom_count,
 );
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
 // Issue #212：复制模式新 routing 尚未创建，关联产品由用户在基本信息区选（同新建）
 let content = routing_form_page(
 &processes, &products, &work_centers, Some(&detail), FormMode::Copy,
 None, None, 0,
 );
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
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;

 if form.name.trim().is_empty() {
 return Err(DomainError::validation("路线名称不能为空").into());
 }

 // Issue #212：产出品候选集 = 关联产品 BOM 非叶子节点；提交前校验产出品 ∈ 候选集
 let allowed = compute_output_candidates(&state, &service_ctx, &mut conn, None, form.bind_product_code.as_deref()).await;
 let steps = parse_steps(&form, &allowed)?;

 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

 let create_req = CreateRoutingReq {
 name: form.name.trim().to_string(),
 description: form.description.filter(|d| !d.trim().is_empty()),
 steps,
 };

 let svc = state.routing_service();
 let id = svc.create(&service_ctx, &mut tx, create_req).await?;
 // Issue #212：新建时同步建立 BOM 关联（产出品过滤源）；失败（如已关联其他 routing）抛错 → 事务回滚
 if let Some(pc) = form.bind_product_code.as_ref().filter(|s| !s.trim().is_empty()) {
 svc.set_bom_routing(&service_ctx, &mut tx, pc.trim().to_string(), id).await?;
 }
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
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;

 if form.name.trim().is_empty() {
 return Err(DomainError::validation("路线名称不能为空").into());
 }

 // Issue #212：编辑模式产出品候选 = 该 routing 所有关联 BOM 非叶子节点并集；校验产出品 ∈ 候选集
 let allowed = compute_output_candidates(&state, &service_ctx, &mut conn, Some(path.id), None).await;
 let steps = parse_steps(&form, &allowed)?;

 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

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

/// Issue #212：工序产出品专用 picker modal。
///
/// 与通用 `product_picker_modal_deferred` 的区别：搜索端点为 `RoutingOutputSearchPath`
/// （候选集 = 关联产品 BOM 非叶子节点，后端按 `routing_id`/`product_code` 自算 + 名称/编码过滤，
/// 避免前端传超长 `bom_product_ids` 串——routing#1 候选集达 1776 个）。
///
/// 多行共用一个 modal：`target_id`/`display_id` 由 `openProductPicker(idx)` 动态写入 hidden input，
/// 搜索 bar 的 `hx-include=".routing-output-search-bar"` 携带上下文与动态 id。
///
/// `routing_id`（编辑模式）与 `product_code`（新建/复制模式）二选一渲染为 hidden —— 不渲染的那个
/// 字段不在表单内，`hx-include` 不会带上，handler `RoutingOutputSearchParams` 收到 `None`，据此走对应分支。
fn routing_output_picker_modal(routing_id: Option<i64>, product_code: &str) -> Markup {
 use crate::components::overlay::modal_shell;
 let search_path = RoutingOutputSearchPath::PATH;
 let close_hs = "on click remove .is-open from #routing-product-modal";
 modal_shell("routing-product-modal", "z-[1100]", html! {
 div class="bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl" {
 // ── Header ──
 div class="px-6 py-5 border-b border-border-soft flex items-center gap-3 shrink-0" {
 h2 class="text-lg font-semibold m-0" { "选择产出品" }
 span class="text-xs text-muted" { "（仅关联 BOM 的成品/半成品）" }
 button
 class="ml-auto bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg transition-colors"
 _=(close_hs)
 { "×" }
 }
 // ── Body ──
 div class="overflow-y-auto flex-1 min-h-0 p-6" {
 // ── Search Bar（含 hidden 上下文，hx-include=".routing-output-search-bar" 一并带上）──
 div class="routing-output-search-bar flex gap-4 mb-4 pb-4 border-b border-border-soft" {
 @if let Some(rid) = routing_id {
 input type="hidden" name="routing_id" value=(rid) {};
 } @else {
 input type="hidden" name="product_code" id="output-ctx-product-code" value=(product_code) {};
 }
 input type="hidden" name="target_id" id="output-target-id" {};
 input type="hidden" name="display_id" id="output-display-id" {};
 input type="hidden" name="modal_id" value="routing-product-modal" {};
 div class="flex-1 flex flex-col gap-1" {
 label class="text-xs font-medium text-fg-2" { "产品名称" }
 input
 class="product-search-input w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
 type="text" name="name" placeholder="输入产品名称…"
 hx-get=(search_path)
 hx-trigger="keyup changed delay:300ms"
 hx-sync="this:replace"
 hx-target="#routing-output-results"
 hx-swap="innerHTML"
 hx-include=".routing-output-search-bar" {};
 }
 div class="flex-1 flex flex-col gap-1" {
 label class="text-xs font-medium text-fg-2" { "产品编码" }
 input
 class="product-search-input w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
 type="text" name="code" placeholder="输入产品编码…"
 hx-get=(search_path)
 hx-trigger="keyup changed delay:300ms"
 hx-sync="this:replace"
 hx-target="#routing-output-results"
 hx-swap="innerHTML"
 hx-include=".routing-output-search-bar" {};
 }
 }
 // ── Results（deferred：不带 intersect once，首次加载由 openProductPicker htmx.ajax 拉取）──
 div id="routing-output-results" class="max-h-[400px] overflow-y-auto" {
 div class="flex items-center justify-center py-8 text-muted text-sm" { "加载中…" }
 }
 }
 }
 })
}

fn routing_form_page(
 processes: &[abt_core::master_data::labor_process_dict::model::LaborProcessDict],
 products: &[abt_core::master_data::product::model::Product],
 work_centers: &[abt_core::master_data::work_center::model::WorkCenter],
 existing: Option<&RoutingDetail>,
 mode: FormMode,
 // Issue #212：产出品候选上下文
 routing_id: Option<i64>,
 bind_product_code: Option<String>,
 bound_bom_count: usize,
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

 // Issue #212：产出品候选上下文（编辑模式用 routing_id；新建/复制用 bind_product_code）
 let is_edit = mode == FormMode::Edit;
 let output_routing_id: Option<i64> = if is_edit { routing_id } else { None };
 let output_product_code: String = if !is_edit {
 bind_product_code.clone().unwrap_or_default()
 } else {
 String::new()
 };
 // 关联产品名回填（新建/复制：初始通常为空，用户选后 JS 填）
 let bind_product_name: String = bind_product_code
 .as_deref()
 .and_then(|c| products.iter().find(|p| p.product_code == c).map(|p| p.pdt_name.as_str()))
 .unwrap_or("")
 .to_string();
 // Issue #212：产出品专用搜索端点（注入 JS）
 let output_search_path: &str = RoutingOutputSearchPath::PATH;

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
                    // Issue #212：关联产品（产出品过滤源）
                    div class="form-field" {
                        @if is_edit {
                            label { "关联 BOM" }
                            div class="flex items-center gap-2 text-sm text-fg-2 mt-1" {
                                span { (format!("已关联 {} 个产品", bound_bom_count)) }
                                span class="text-muted text-xs" { "（产出品候选来自这些 BOM）" }
                            }
                            a class="text-xs text-accent hover:underline mt-1 inline-block"
                                href=(RoutingDetailPath { id: output_routing_id.unwrap_or(0) }.to_string())
                            { "在详情页管理关联 →" }
                        } @else {
                            label {
                                "关联产品（BOM） "
                                span class="text-danger" { "*" }
                            }
                            input type="hidden" name="bind_product_code" id="bind-product-code"
                                value=(bind_product_code.as_deref().unwrap_or("")) {};
                            div class="flex items-center gap-1 mt-1" {
                                span id="bind-product-display"
                                    class=(format!(
                                        "flex-1 text-sm truncate px-2 py-[5px] border border-border rounded-sm {}",
                                        if bind_product_name.is_empty() { "text-muted" } else { "text-fg" }
                                    ))
                                { (if bind_product_name.is_empty() { "未选择" } else { bind_product_name.as_str() }) }
                                button type="button" onclick="openBindProductPicker()"
                                    class="shrink-0 text-xs text-accent px-2 py-[3px] border border-border rounded-sm cursor-pointer hover:bg-accent-bg whitespace-nowrap"
                                { "选择" }
                            }
                            p class="text-muted text-xs mt-1" { "工序产出品将从该产品 BOM 的成品/半成品中选取" }
                        }
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
    // Issue #212：产出品专用 picker（候选集 = 关联 BOM 非叶子节点，后端按 routing_id/product_code 过滤）
    (routing_output_picker_modal(output_routing_id, &output_product_code))
    // 关联产品 picker（基本信息区，全量产品搜索，新建/复制用）
    (crate::components::product_picker::product_picker_modal(
        "bind-product-modal", "bind-product-code", "bind-product-display",
    ))
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

// Issue #212：产出品搜索端点（候选集后端按 routing_id/product_code 过滤）
const outputSearchPath = "{output_search_path}";
// 产出品选择（步骤行共用弹窗，openProductPicker 动态指向当前行）
let editingProductIdx = null;
function openProductPicker(idx) {{
 editingBindProduct = false;
 editingProductIdx = idx;
 const modal = document.getElementById('routing-product-modal');
 // 上下文：编辑模式用 routing_id，新建/复制用 product_code（hidden 二选一渲染）
 const ridInput = modal.querySelector('input[name=routing_id]');
 const pcInput = modal.querySelector('input[name=product_code]');
 const rid = ridInput ? ridInput.value : '';
 const pc = pcInput ? pcInput.value : '';
 if (!rid && !pc) {{
 alert('请先在基本信息区选择关联产品');
 editingProductIdx = null;
 return;
 }}
 modal.querySelector('input[name=target_id]').value = 'step-product-id-' + idx;
 modal.querySelector('input[name=display_id]').value = 'step-product-display-' + idx;
 modal.querySelectorAll('.product-search-input').forEach(i => i.value = '');
 const values = {{ target_id: 'step-product-id-' + idx, display_id: 'step-product-display-' + idx, modal_id: 'routing-product-modal', name: '', code: '' }};
 if (rid) values.routing_id = rid; else values.product_code = pc;
 modal.classList.add('is-open');
 htmx.ajax('GET', outputSearchPath, {{ target: '#routing-output-results', swap: 'innerHTML', values: values }});
}}
// 关联产品选择（新建/复制：基本信息区，全量产品搜索 → 建立 BomRouting 关联）
let editingBindProduct = false;
function openBindProductPicker() {{
 editingProductIdx = null;
 editingBindProduct = true;
 const modal = document.getElementById('bind-product-modal');
 modal.querySelectorAll('.product-search-input').forEach(i => i.value = '');
 modal.classList.add('is-open');
 htmx.ajax('GET', '/api/products/search', {{
 target: '#product-search-results', swap: 'innerHTML',
 values: {{ target_id: 'bind-product-code', display_id: 'bind-product-display', modal_id: 'bind-product-modal', name: '', code: '' }}
 }});
}}
// productSelected 统一监听：区分产出品（editingProductIdx）与关联产品（editingBindProduct）
document.body.addEventListener('productSelected', (e) => {{
 if (editingProductIdx !== null) {{
 syncFromDom();
 // 产品名由事件 detail 携带（picker 搜索结果直达，绕过仅含前 N 个产品的 productMap）
 steps[editingProductIdx].product_name = (e.detail && e.detail.productName) || '';
 editingProductIdx = null;
 renderSteps();
 }} else if (editingBindProduct) {{
 const code = (e.detail && e.detail.productCode) || '';
 const name = (e.detail && e.detail.productName) || '';
 document.querySelector('#bind-product-code').value = code;
 const disp = document.querySelector('#bind-product-display');
 disp.textContent = name || '未选择';
 disp.classList.remove('text-muted'); disp.classList.add('text-fg');
 // 同步产出品 picker 上下文（product_code hidden）
 const pcHidden = document.querySelector('#output-ctx-product-code');
 if (pcHidden) pcHidden.value = code;
 // 清空已有产出品（旧产出品可能不在新关联 BOM 内，提示重选）
 syncFromDom();
 steps.forEach(s => {{ s.product_id = ''; s.product_name = ''; }});
 renderSteps();
 editingBindProduct = false;
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
