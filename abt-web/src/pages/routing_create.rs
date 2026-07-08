use std::collections::HashMap;

use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{Markup, html, PreEscaped};
use serde::Deserialize;

use abt_core::master_data::labor_process_dict::LaborProcessDictService;
use abt_core::master_data::labor_process_dict::model::LaborProcessDictQuery;
use abt_core::master_data::routing::RoutingService;
use abt_core::master_data::work_center::WorkCenterService;
use abt_core::master_data::routing::model::{
    BomRouting, CreateRoutingReq, RoutingDetail, RoutingStep, RoutingStepInput, UpdateRoutingReq,
};
use abt_core::shared::types::{DomainError, PageParams, PaginatedResult};
use abt_macros::require_permission;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::routing::{
    RoutingBoundBomsPath, RoutingCopyPath, RoutingCreatePath, RoutingDetailPath, RoutingEditPath,
    RoutingListPath,
};
use crate::utils::RequestContext;

// ── Form request ──

#[derive(Debug, Deserialize)]
pub struct RoutingCreateForm {
    pub name: String,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub description: Option<String>,
    pub steps_json: String,
    /// 新建/复制时关联的主产品 code（同时建立 bom_routings 关联）。产出品/计件价已下沉到
    /// per-BOM 覆盖层（bom_routing_outputs），在 routing 详情页按 BOM 维护，不在模板编辑。
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub bind_product_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StepWeb {
    process_code: String,
    is_required: bool,
    remark: Option<String>,
    #[serde(default)]
    work_center_id: Option<i64>,
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
    work_center_id: String,
    standard_time: String,
    is_outsourced: bool,
}

impl JsStep {
    fn from_step(s: &RoutingStep) -> Self {
        Self {
            process_code: s.process_code.clone(),
            is_required: s.is_required,
            remark: s.remark.clone().unwrap_or_default(),
            work_center_id: s.work_center_id.map(|id| id.to_string()).unwrap_or_default(),
            standard_time: s.standard_time.map(|d| d.to_string()).unwrap_or_default(),
            is_outsourced: s.is_outsourced,
        }
    }
}

/// 解析 steps_json → RoutingStepInput（工艺模板只含可共享的工艺属性：工序/工作中心/工时/委外/必经/备注）。
/// 产出品与计件单价已下沉到 per-BOM 覆盖层 bom_routing_outputs，不在此校验。
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
            work_center_id: s.work_center_id,
            standard_time: s.standard_time.and_then(|v| v.trim().parse::<rust_decimal::Decimal>().ok()),
            is_outsourced: s.is_outsourced,
            ..Default::default()
        })
        .collect();

    for (i, s) in steps.iter().enumerate() {
        if s.process_code.trim().is_empty() {
            return Err(DomainError::validation(format!("工序 {} 未选择工序名称", i + 1)).into());
        }
    }

    Ok(steps)
}

/// 加载工序步骤下拉数据（工序字典 / 工作中心），create/edit/copy 共用
async fn load_step_options(
    state: &crate::state::AppState,
    service_ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
) -> crate::errors::Result<(
    Vec<abt_core::master_data::labor_process_dict::model::LaborProcessDict>,
    Vec<abt_core::master_data::work_center::model::WorkCenter>,
)> {
    let processes = state
        .labor_process_dict_service()
        .list(service_ctx, db, LaborProcessDictQuery::default(), PageParams::new(1, 500))
        .await?
        .items;
    let work_centers = abt_core::master_data::work_center::new_work_center_service(state.pool.clone())
        .list_active(service_ctx, db)
        .await
        .unwrap_or_default();
    Ok((processes, work_centers))
}

#[derive(Debug, Deserialize)]
pub struct BoundBomPageParams {
    #[serde(default)]
    pub page: Option<u32>,
}

/// 编辑页关联 BOM drawer 分页端点（每页 10 条，返回 `#bound-boms-list` 片段）。
#[require_permission("ROUTING", "read")]
pub async fn get_routing_bound_boms(
    path: RoutingBoundBomsPath,
    ctx: RequestContext,
    Query(qp): Query<BoundBomPageParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let boms = state.routing_service()
        .paginate_boms_by_routing(&service_ctx, &mut conn, path.id, None, PageParams::new(qp.page.unwrap_or(1), 10))
        .await?;
    Ok(Html(bound_boms_list_fragment(path.id, &boms).into_string()))
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

    let (processes, work_centers) = load_step_options(&state, &service_ctx, &mut conn).await?;
    let content = routing_form_page(
        &processes, &work_centers, None, FormMode::New, None, None, "", 0,
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
    let (processes, work_centers) = load_step_options(&state, &service_ctx, &mut conn).await?;
    // 关联 BOM 取首页（10 条）+ 总数：first 用于基本信息区展示，count 用于判断是否渲染更多 drawer
    let bound_page = state.routing_service()
        .paginate_boms_by_routing(&service_ctx, &mut conn, path.id, None, PageParams::new(1, 10))
        .await?;
    let first_bound_name: String = bound_page.items.first()
        .map(|b| b.product_name.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| b.product_code.clone()))
        .unwrap_or_default();
    let bound_count = bound_page.total as usize;
    let edit_path_str = RoutingEditPath { id: path.id }.to_string();
    let content = routing_form_page(
        &processes, &work_centers, Some(&detail), FormMode::Edit,
        Some(path.id), None, &first_bound_name, bound_count,
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
    let (processes, work_centers) = load_step_options(&state, &service_ctx, &mut conn).await?;
    let content = routing_form_page(
        &processes, &work_centers, Some(&detail), FormMode::Copy, None, None, "", 0,
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
    let RequestContext { state, service_ctx, .. } = ctx;

    if form.name.trim().is_empty() {
        return Err(DomainError::validation("路线名称不能为空").into());
    }

    let steps = parse_steps(&form)?;

    let mut tx = state.pool.begin().await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

    let create_req = CreateRoutingReq {
        name: form.name.trim().to_string(),
        description: form.description.filter(|d| !d.trim().is_empty()),
        steps,
    };

    let svc = state.routing_service();
    let id = svc.create(&service_ctx, &mut tx, create_req).await?;
    // 新建时同步建立 BOM 关联；失败（如已关联其他 routing）抛错 → 事务回滚
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
    let RequestContext { state, service_ctx, .. } = ctx;

    if form.name.trim().is_empty() {
        return Err(DomainError::validation("路线名称不能为空").into());
    }

    let steps = parse_steps(&form)?;

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

/// 编辑模式关联 BOM 列表 drawer（基本信息区"更多"按钮就地展开，不跳详情页）。
fn bound_boms_drawer(routing_id: i64) -> Markup {
    let bound_boms_path = RoutingBoundBomsPath { id: routing_id }.to_string();
    html! {
        div id="bound-boms-drawer"
            class="drawer-overlay fixed inset-0 z-[90] flex justify-end bg-slate-900/40"
            _="on click[me is event.target] remove .open\non keydown[event.key is 'Escape'] from body remove .open"
        {
            div class="drawer-panel bg-bg h-full w-[480px] max-w-[92vw] shadow-lg flex flex-col" {
                div class="flex items-center gap-3 px-5 py-4 border-b border-border-soft shrink-0" {
                    span class="text-sm font-semibold text-fg" { "关联 BOM" }
                    span class="text-muted text-xs font-normal" { "产出品/计件价按 BOM 维护" }
                    button type="button"
                        class="ml-auto text-muted hover:text-fg text-xl leading-none bg-transparent border-none cursor-pointer"
                        _="on click remove .open from closest .drawer-overlay"
                    { "×" }
                }
                div class="overflow-y-auto flex-1" {
                    div id="bound-boms-list"
                        hx-get=(&bound_boms_path)
                        hx-trigger="load"
                        hx-target="this"
                        hx-select="#bound-boms-list"
                        hx-swap="outerHTML" {
                        div class="flex items-center justify-center py-8 text-muted text-sm" { "加载中…" }
                    }
                }
            }
        }
    }
}

/// drawer 关联 BOM 列表片段（`#bound-boms-list`）：当前页 10 行 + pagination 控件。
fn bound_boms_list_fragment(routing_id: i64, boms: &PaginatedResult<BomRouting>) -> Markup {
    let path = RoutingBoundBomsPath { id: routing_id }.to_string();
    html! {
        div id="bound-boms-list" {
            @if boms.items.is_empty() {
                div class="text-center py-8 text-muted text-sm" { "暂无关联 BOM" }
            } @else {
                @for bom in &boms.items {
                    div class="p-3 border-b border-border-soft" {
                        div class="text-sm text-fg truncate" { (bom.product_name.as_deref().unwrap_or("—")) }
                        div class="text-xs text-muted font-mono mt-0.5" { (bom.product_code.as_str()) }
                    }
                }
                (pagination(&path, "#bound-boms-list", "", boms.total, boms.page, boms.total_pages))
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn routing_form_page(
    processes: &[abt_core::master_data::labor_process_dict::model::LaborProcessDict],
    work_centers: &[abt_core::master_data::work_center::model::WorkCenter],
    existing: Option<&RoutingDetail>,
    mode: FormMode,
    routing_id: Option<i64>,
    bind_product_code: Option<String>,
    first_bound_name: &str,
    bound_count: usize,
) -> Markup {
    let process_map: HashMap<&str, &str> = processes
        .iter()
        .map(|p| (p.code.as_str(), p.name.as_str()))
        .collect();

    let process_map_json = serde_json::to_string(&process_map).unwrap_or_else(|_| "{}".into());

    // 工作中心映射（id → name，注入 JS 渲染下拉）
    let work_center_map: HashMap<String, String> = work_centers
        .iter()
        .map(|wc| (wc.id.to_string(), wc.name.clone()))
        .collect();
    let work_center_map_json = serde_json::to_string(&work_center_map).unwrap_or_else(|_| "{}".into());

    let is_edit = mode == FormMode::Edit;
    // 关联产品名回显由通用 picker 选中后 JS 填充，模板渲染初始为"未选择"
    let bind_product_name = String::new();

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
            work_center_id: String::new(),
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
                        div class="form-field" {
                            @if is_edit {
                                label { "关联 BOM" }
                                @if bound_count == 0 {
                                    div class="text-sm text-muted mt-1" { "未关联 BOM" }
                                } @else {
                                    div class="flex items-center gap-2 text-sm text-fg-2 mt-1" {
                                        span class="truncate" { (first_bound_name) }
                                        @if bound_count > 1 {
                                            span class="text-muted text-xs whitespace-nowrap" { (format!("等 {} 个", bound_count)) }
                                            button type="button"
                                                class="text-xs text-accent hover:underline cursor-pointer bg-transparent border-none whitespace-nowrap p-0"
                                                _="on click add .open to #bound-boms-drawer"
                                            { "更多" }
                                        }
                                    }
                                }
                            } @else {
                                label { "关联产品（BOM）" }
                                input type="hidden" name="bind_product_code" id="bind-product-code"
                                    value=(bind_product_code.as_deref().unwrap_or("")) {};
                                div class="flex items-center gap-1 mt-1" {
                                    span id="bind-product-display"
                                        class="flex-1 text-sm truncate px-2 py-[5px] border border-border rounded-sm text-muted"
                                    { "未选择" }
                                    button type="button" onclick="openBindProductPicker()"
                                        class="shrink-0 text-xs text-accent px-2 py-[3px] border border-border rounded-sm cursor-pointer hover:bg-accent-bg whitespace-nowrap"
                                    { "选择" }
                                }
                                p class="text-muted text-xs mt-1" { "建立 BOM 关联后，产出品/计件价在该 routing 详情页按 BOM 维护" }
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
                    @if is_edit && bound_count > 0 {
                        div class="mx-5 mb-3 p-2.5 rounded-sm bg-warn-bg text-warn text-xs flex items-start gap-2" {
                            (icon::alert_triangle_icon("w-4 h-4 shrink-0 mt-0.5"))
                            span {
                                "该路线已关联 BOM 并可能存在产出覆盖，"
                                strong { "删除或重排已有工序将被拒绝" }
                                "（仅允许在末尾追加新工序）；如需调整已有工序，请先到详情页清除相关 BOM 的产出覆盖。"
                            }
                        }
                    }
                    div class="overflow-x-auto" {
                        table class="data-table min-w-[760px]" {
                            thead {
                                tr {
                                    th class="w-[50px] text-center" { "排序" }
                                    th class="w-[220px]" { "工序名称" }
                                    th class="w-[160px]" { "工作中心" }
                                    th class="w-[110px]" { "标准工时" }
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
        // 编辑模式关联 BOM 列表 drawer（多个时点"更多"就地展开）
        @if is_edit && bound_count > 1 {
            (bound_boms_drawer(routing_id.unwrap_or(0)))
        }
        // 关联产品 picker（基本信息区，全量产品搜索，新建/复制用）
        (crate::components::product_picker::product_picker_modal(
            "bind-product-modal", "bind-product-code", "bind-product-display",
        ))
        script {
            ({
                PreEscaped(
                    format!(
                        r#"
const processMap = {process_map_json};
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
 work_center_id: s.work_center_id && s.work_center_id !== '' ? Number(s.work_center_id) : null,
 standard_time: s.standard_time || null,
 is_outsourced: !!s.is_outsourced,
 }}))
 );
}}

function addStep() {{
 steps.push({{ process_code: '', is_required: true, remark: '', work_center_id: '', standard_time: '', is_outsourced: false }});
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
 const opHidden = row.querySelector('.cat-select input[type=hidden]');
 if (opHidden) steps[idx].process_code = opHidden.value;
 if (selects[0]) steps[idx].work_center_id = selects[0].value;
 if (checkboxes[0]) steps[idx].is_outsourced = checkboxes[0].checked;
 if (checkboxes[1]) steps[idx].is_required = checkboxes[1].checked;
 if (inputs[0]) steps[idx].standard_time = inputs[0].value;
 if (inputs[1]) steps[idx].remark = inputs[1].value;
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
 let st = step.standard_time || '';
 let rem = step.remark || '';
 html += '<tr>' +
 '<td class="text-muted text-xs text-center">' + (idx + 1) + '</td>' +
 '<td>' +
 '<div class="cat-select relative">' +
 '<input type="hidden" class="step-process-code" value="' + (step.process_code || '') + '" onchange="onStepChange(' + idx + ')">' +
 '<button type="button" class="cat-trigger w-full flex items-center justify-between gap-2 px-2 py-[5px] border border-border rounded-sm text-[13px] bg-white text-fg cursor-pointer hover:border-[rgba(37,99,235,0.3)]" onclick="toggleOpCombo(this)">' +
 '<span class="cat-label truncate flex-1 text-left ' + (step.process_code ? '' : 'text-muted') + '">' + opLabel + '</span>' +
 '<svg class="w-3.5 h-3.5 text-muted shrink-0" viewBox="0 0 24 24 fill="none" stroke="currentColor" stroke-width="2"><path d="M19 9l-7 7-7-7"></path></svg>' +
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
 '<td><select onchange="onStepChange(' + idx + ')" class="w-full text-[13px] rounded-sm px-2 py-[5px] border border-border">' + wcopts + '</select></td>' +
 '<td><input type="number" step="any" onchange="onStepChange(' + idx + ')" value="' + st + '" placeholder="0.00" class="w-full text-[13px] rounded-sm px-2 py-[5px] border border-border font-mono text-right"></td>' +
 '<td class="text-center"><input type="checkbox" onchange="onStepChange(' + idx + ')" class="cursor-pointer w-[18px] h-[18px] accent-accent"' + chk_out + '></td>' +
 '<td class="text-center"><input type="checkbox" onchange="onStepChange(' + idx + ')" class="cursor-pointer w-[18px] h-[18px] accent-accent"' + chk_req + '></td>' +
 '<td><input type="text" onchange="onStepChange(' + idx + ')" value="' + rem + '" placeholder="备注" class="w-full text-[13px] rounded-sm px-2 py-[5px] border border-border"></td>' +
 '<td><button type="button" class="w-[28px] h-[28px] border-none text-muted rounded-sm cursor-pointer grid place-items-center" onclick="removeStep(' + idx + ')" title="删除"><svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg></button></td>' +
 '</tr>';
 }});
 document.querySelector('#routing-steps-body').innerHTML = html;
}}

// 工序搜索下拉开关
function toggleOpCombo(trigger) {{
 const wrapper = trigger.closest('.cat-select');
 const dropdown = wrapper.querySelector('.cat-dropdown');
 const backdrop = wrapper.querySelector('.cat-backdrop');
 if (dropdown.style.display !== 'none') {{ closeOpCombo(trigger); return; }}
 const r = trigger.getBoundingClientRect();
 dropdown.style.left = r.left + 'px';
 dropdown.style.top = (r.bottom + 4) + 'px';
 dropdown.style.display = 'block';
 if (r.bottom + 304 > window.innerHeight && r.top > 324) {{
 dropdown.style.top = (r.top - dropdown.offsetHeight - 4) + 'px';
 }}
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

// 关联产品选择（新建/复制：基本信息区，全量产品搜索 → 建立 BomRouting 关联）
let editingBindProduct = false;
function openBindProductPicker() {{
 editingBindProduct = true;
 const modal = document.getElementById('bind-product-modal');
 modal.querySelectorAll('.product-search-input').forEach(i => i.value = '');
 modal.classList.add('is-open');
 htmx.ajax('GET', '/api/products/search', {{
 target: '#product-search-results', swap: 'innerHTML',
 values: {{ target_id: 'bind-product-code', display_id: 'bind-product-display', modal_id: 'bind-product-modal', name: '', code: '' }}
 }});
}}
document.body.addEventListener('productSelected', (e) => {{
 if (editingBindProduct) {{
 const code = (e.detail && e.detail.productCode) || '';
 const name = (e.detail && e.detail.productName) || '';
 document.querySelector('#bind-product-code').value = code;
 const disp = document.querySelector('#bind-product-display');
 disp.textContent = name || '未选择';
 disp.classList.remove('text-muted'); disp.classList.add('text-fg');
 editingBindProduct = false;
 }}
}});

// 页面加载渲染初始工序行
renderSteps();
"#,
                    ),
                )
            })
        }
    }
}
