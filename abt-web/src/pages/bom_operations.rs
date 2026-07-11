// BOM 工序独立管理页 — 方案 A 表格内联编辑
//
// 工序（bom_operations）按 product_code 维护：一个产品的所有 BOM 版本
// （Draft / Published / 历史）共享同一套工序。因此工序编辑从 BOM 编辑页
// 迁出到本独立页 /admin/md/boms/{id}/operations，BOM 编辑页只保留只读摘要。
//
// 交互（方案 A）：
// - 表格内每行字段就地编辑；工序下拉变化 → outerHTML 刷新整行（带字典默认值 +
//   类别联动：检验→无产出+勾检验点，外协→勾委外）；其余字段 change/blur 静默保存。
// - SortableJS 拖拽排序（static/bom-operation.js，onEnd → htmx.ajax → reorder）。
//
// 参考三家 ERP：工序是产品维度主数据（非 BOM 单据级），产出品可选（ERPNext
// finished_good / Odoo byproduct.operation_id / OFBiz PRUNT_PROD_DELIV 均可选）。

use std::collections::HashMap;

use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;
use serde::Deserialize;

use abt_core::master_data::bom::BomQueryService;
use abt_core::master_data::bom_operation::{
    BomOperationService,
    model::{BomOperation, UpsertBomOperationReq},
};
use abt_core::master_data::labor_process_dict::{
    LaborProcessDictService,
    model::{LaborProcessDict, LaborProcessDictQuery},
};
use abt_core::master_data::product::{ProductService, model::Product};
use abt_core::master_data::routing::RoutingService;
use abt_core::master_data::routing::model::{Routing, RoutingQuery};
use abt_core::master_data::work_center::{WorkCenterService, model::WorkCenter};
use abt_core::shared::types::{DomainError, PageParams, PgPoolConn};

use abt_macros::require_permission;

use crate::components::combo_select::{combo_select, ComboHx, ComboOption};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::bom::{
    BomDetailPath, BomEditPath, BomListPath, BomOperationApplyPath, BomOperationDeletePath,
    BomOperationReorderPath, BomOperationsPath,
};
use crate::utils::RequestContext;

// ── Form ──

#[derive(Debug, Deserialize)]
pub struct UpsertBomOperationForm {
    #[serde(default)]
    pub step_order: Option<i32>,
    #[serde(default)]
    pub process_code: String,
    #[serde(default)]
    pub process_name: String,
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    pub work_center_id: Option<i64>,
    #[serde(default)]
    pub standard_time: Option<String>,
    #[serde(default)]
    pub standard_cost: Option<String>,
    #[serde(default)]
    pub allowed_loss_rate: Option<String>,
    #[serde(default)]
    pub is_outsourced: Option<bool>,
    #[serde(default)]
    pub is_inspection_point: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    pub output_product_id: Option<i64>,
    #[serde(default)]
    pub remark: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApplyRoutingToBomForm {
    pub routing_id: i64,
    #[serde(default)]
    pub force: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ReorderBomOperationForm {
    /// JSON: [{"step_order":1,"new_order":2}, ...]
    pub orders: String,
}

#[derive(Debug, Deserialize)]
struct ReorderItem {
    step_order: i32,
    new_order: i32,
}

// ── 辅助 ──

fn deserialize_optional_i64<'de, D>(de: D) -> std::result::Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(de)?;
    match opt {
        None => Ok(None),
        Some(ref s) if s.is_empty() => Ok(None),
        Some(s) => s.parse::<i64>().map(Some).map_err(serde::de::Error::custom),
    }
}

fn parse_dec_field(s: &Option<String>) -> abt_core::shared::types::Result<Option<Decimal>> {
    match s {
        Some(s) if !s.trim().is_empty() => s
            .trim()
            .parse::<Decimal>()
            .map(Some)
            .map_err(|_| DomainError::business_rule(format!("数值格式错误: {s}"))),
        _ => Ok(None),
    }
}

fn fmt_dec(d: Option<Decimal>) -> String {
    d.map(|v| v.to_string()).unwrap_or_default()
}

fn bom_op_to_req(op: &BomOperation) -> UpsertBomOperationReq {
    UpsertBomOperationReq {
        product_code: op.product_code.clone(),
        step_order: op.step_order,
        process_code: op.process_code.clone(),
        process_name: op.process_name.clone(),
        work_center_id: op.work_center_id,
        standard_time: op.standard_time,
        standard_cost: op.standard_cost,
        allowed_loss_rate: op.allowed_loss_rate,
        is_outsourced: op.is_outsourced,
        is_inspection_point: op.is_inspection_point,
        is_required: op.is_required,
        output_product_id: op.output_product_id,
        remark: op.remark.clone(),
    }
}

/// 取 BOM 根 product_code（空 → Err）
async fn require_bom_product_code(
    state: &crate::state::AppState,
    service_ctx: &abt_core::shared::types::ServiceContext,
    conn: &mut PgPoolConn,
    bom_id: i64,
) -> abt_core::shared::types::Result<String> {
    let bom = state.bom_query_service().get(service_ctx, conn, bom_id).await?;
    bom.product_code
        .clone()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| DomainError::business_rule("该 BOM 无根产品编码，无法配置工序"))
}

/// 工序字典：code → LaborProcessDict（用于 upsert 时查默认值/类别）
async fn load_process_map(
    state: &crate::state::AppState,
    service_ctx: &abt_core::shared::types::ServiceContext,
    conn: &mut PgPoolConn,
) -> Result<HashMap<String, LaborProcessDict>> {
    let items = state
        .labor_process_dict_service()
        .list(service_ctx, conn, LaborProcessDictQuery::default(), PageParams::new(1, 500))
        .await?
        .items;
    Ok(items.into_iter().map(|p| (p.code.clone(), p)).collect())
}

async fn load_active_work_centers(
    state: &crate::state::AppState,
    service_ctx: &abt_core::shared::types::ServiceContext,
    conn: &mut PgPoolConn,
) -> Vec<WorkCenter> {
    state
        .work_center_service()
        .list_active(service_ctx, conn)
        .await
        .unwrap_or_default()
}

async fn load_non_leaf_products(
    state: &crate::state::AppState,
    service_ctx: &abt_core::shared::types::ServiceContext,
    conn: &mut PgPoolConn,
    product_code: &str,
) -> Vec<Product> {
    let ids = state
        .bom_query_service()
        .list_non_leaf_product_ids_by_product_codes(service_ctx, conn, &[product_code.to_string()])
        .await
        .unwrap_or_default();
    if ids.is_empty() {
        Vec::new()
    } else {
        state
            .product_service()
            .get_by_ids(service_ctx, conn, ids)
            .await
            .unwrap_or_default()
    }
}

// ── Handlers ──

/// GET 工序管理页（is_htmx 分流：整页 / content 片段）。
/// 局部刷新（删除/导入/重排后）由 #ops-table 监听 bomOpChanged 自刷新。
#[require_permission("BOM", "read")]
pub async fn get_operation_page(path: BomOperationsPath, ctx: RequestContext) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    let bom = state.bom_query_service().get(&service_ctx, &mut conn, path.id).await?;
    let product_code = bom.product_code.clone().unwrap_or_default();
    let operations = if product_code.is_empty() {
        Vec::new()
    } else {
        state
            .bom_operation_service()
            .list_operations(&service_ctx, &mut conn, product_code.clone())
            .await
            .unwrap_or_default()
    };
    let processes: Vec<LaborProcessDict> = state
        .labor_process_dict_service()
        .list(&service_ctx, &mut conn, LaborProcessDictQuery::default(), PageParams::new(1, 500))
        .await?
        .items;
    let work_centers = load_active_work_centers(&state, &service_ctx, &mut conn).await;
    let nl_products = load_non_leaf_products(&state, &service_ctx, &mut conn, &product_code).await;
    let routings = state
        .routing_service()
        .list(&service_ctx, &mut conn, RoutingQuery { keyword: None, bom_keyword: None }, PageParams::new(1, 200))
        .await
        .map(|r| r.items)
        .unwrap_or_default();

    let current_path = BomOperationsPath { id: path.id }.to_string();
    let content = operation_page_content(
        path.id,
        &product_code,
        &bom.bom_name,
        &operations,
        &processes,
        &work_centers,
        &nl_products,
        &routings,
    );

    let page_html = admin_page(
        is_htmx,
        "工序管理",
        &claims,
        "md",
        &current_path,
        "主数据管理",
        Some("工序管理"),
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

/// POST upsert 工序（整行）。
/// - 选工序时（process_code 非空）从字典带默认工作中心/工时。
/// - 返回刷新后的整行 tr（outerHTML 替换），让默认值即时可见。
#[require_permission("BOM", "update")]
pub async fn upsert_bom_operation(
    path: BomOperationsPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<UpsertBomOperationForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let product_code = require_bom_product_code(&state, &service_ctx, &mut conn, path.id).await?;

    let process_map = load_process_map(&state, &service_ctx, &mut conn).await?;
    let pdef = form.process_code.as_str().split_once("·").map(|(c, _)| c.trim()).unwrap_or(form.process_code.as_str());
    let pdef = process_map.get(pdef);

    // 默认值填充（用户未填才带默认）
    let work_center_id = form.work_center_id.or_else(|| pdef.and_then(|p| p.default_work_center_id));
    let standard_time = parse_dec_field(&form.standard_time)?.or_else(|| pdef.and_then(|p| p.default_standard_time));
    let standard_cost = parse_dec_field(&form.standard_cost)?;
    let allowed_loss_rate = parse_dec_field(&form.allowed_loss_rate)?.unwrap_or(Decimal::ZERO);
    let process_name = if form.process_name.trim().is_empty() {
        pdef.map(|p| p.name.clone()).unwrap_or_default()
    } else {
        form.process_name.clone()
    };

    let is_inspection_point = form.is_inspection_point.unwrap_or(false);
    let is_outsourced = form.is_outsourced.unwrap_or(false);
    let output_product_id = form.output_product_id;

    // 产出品校验（非 None 时须 ∈ 非叶子节点）
    if let Some(pid) = output_product_id {
        let candidates = state
            .bom_query_service()
            .list_non_leaf_product_ids_by_product_codes(&service_ctx, &mut conn, std::slice::from_ref(&product_code))
            .await?;
        if !candidates.contains(&pid) {
            return Err(DomainError::business_rule("产出品必须属于该产品 BOM 的非叶子节点（成品/半成品）").into());
        }
    }

    // step_order：0（新增/缺省）→ 末尾追加；否则用 form 值
    let step_order = match form.step_order.unwrap_or(0) {
        0 => state
            .bom_operation_service()
            .count_operations(&service_ctx, &mut conn, product_code.clone())
            .await? as i32
            + 1,
        v => v,
    };

    let req = UpsertBomOperationReq {
        product_code: product_code.clone(),
        step_order,
        process_code: form.process_code,
        process_name,
        work_center_id,
        standard_time,
        standard_cost,
        allowed_loss_rate,
        is_outsourced,
        is_inspection_point,
        is_required: true,
        output_product_id,
        remark: form.remark.filter(|s| !s.is_empty()),
    };

    let mut tx = state.pool.begin().await.map_err(|e| DomainError::Internal(e.into()))?;
    state.bom_operation_service().upsert_operation(&service_ctx, &mut tx, req).await?;
    tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;

    // 返回刷新行（含联动后状态）
    let op = state
        .bom_operation_service()
        .find_operation(&service_ctx, &mut conn, product_code, step_order)
        .await?
        .ok_or_else(|| DomainError::not_found("BomOperation"))?;
    let processes: Vec<LaborProcessDict> = state
        .labor_process_dict_service()
        .list(&service_ctx, &mut conn, LaborProcessDictQuery::default(), PageParams::new(1, 500))
        .await?
        .items;
    let work_centers = load_active_work_centers(&state, &service_ctx, &mut conn).await;
    let nl_products = load_non_leaf_products(&state, &service_ctx, &mut conn, &op.product_code).await;
    Ok(Html(operation_row(path.id, &op, &processes, &work_centers, &nl_products).into_string()))
}

/// POST 删除工序（级联清计件单价由 service 内部处理）。广播 bomOpChanged → #ops-table 刷新。
#[require_permission("BOM", "update")]
pub async fn delete_bom_operation(path: BomOperationDeletePath, ctx: RequestContext) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let product_code = require_bom_product_code(&state, &service_ctx, &mut conn, path.id).await?;
    let mut tx = state.pool.begin().await.map_err(|e| DomainError::Internal(e.into()))?;
    state
        .bom_operation_service()
        .delete_operation(&service_ctx, &mut tx, product_code, path.step_order)
        .await?;
    tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
    Ok(([("HX-Trigger", "bomOpChanged")], Html(String::new())))
}

/// POST 拖拽批量重排。orders JSON → 按新顺序 replace_operations（step_order 重编 1..N）。
#[require_permission("BOM", "update")]
pub async fn reorder_bom_operation(
    path: BomOperationReorderPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ReorderBomOperationForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let product_code = require_bom_product_code(&state, &service_ctx, &mut conn, path.id).await?;

    let orders: Vec<ReorderItem> = serde_json::from_str(&form.orders)
        .map_err(|e| DomainError::business_rule(format!("顺序数据格式错误: {e}")))?;
    let mut new_order_map: HashMap<i32, i32> =
        orders.into_iter().map(|o| (o.step_order, o.new_order)).collect();

    let mut ops = state
        .bom_operation_service()
        .list_operations(&service_ctx, &mut conn, product_code.clone())
        .await?;
    ops.sort_by_key(|op| new_order_map.remove(&op.step_order).unwrap_or(op.step_order));
    let reqs: Vec<UpsertBomOperationReq> = ops
        .iter()
        .enumerate()
        .map(|(i, op)| {
            let mut r = bom_op_to_req(op);
            r.step_order = (i as i32) + 1;
            r
        })
        .collect();

    let mut tx = state.pool.begin().await.map_err(|e| DomainError::Internal(e.into()))?;
    state
        .bom_operation_service()
        .replace_operations(&service_ctx, &mut tx, product_code, reqs)
        .await?;
    tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
    Ok(([("HX-Trigger", "bomOpChanged")], Html(String::new())))
}

/// POST 从 routing 模板 copy-on-write 拷贝工序。force 守卫拒绝覆盖已有工序。
#[require_permission("BOM", "update")]
pub async fn apply_routing_to_bom(
    path: BomOperationApplyPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ApplyRoutingToBomForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let product_code = require_bom_product_code(&state, &service_ctx, &mut conn, path.id).await?;
    let mut tx = state.pool.begin().await.map_err(|e| DomainError::Internal(e.into()))?;
    state
        .bom_operation_service()
        .apply_routing_to_bom(&service_ctx, &mut tx, product_code, form.routing_id, form.force.unwrap_or(false))
        .await?;
    tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
    Ok(([("HX-Trigger", "bomOpChanged")], Html(String::new())))
}

// ── 渲染 ──

fn operation_page_content(
    bom_id: i64,
    product_code: &str,
    bom_name: &str,
    operations: &[BomOperation],
    processes: &[LaborProcessDict],
    work_centers: &[WorkCenter],
    nl_products: &[Product],
    routings: &[Routing],
) -> Markup {
    let inspect_cnt = operations.iter().filter(|o| o.is_inspection_point).count();
    let outsource_cnt = operations.iter().filter(|o| o.is_outsourced).count();
    let has_ops = !operations.is_empty();
    html! {
        // ── 面包屑 + 跳转 ──
        div class="flex items-center justify-between mb-4 flex-wrap gap-2" {
            a class="text-sm text-muted hover:text-accent no-underline" href=(BomListPath::PATH) {
                "← 返回 BOM 列表"
            }
            div class="flex items-center gap-2" {
                a class="inline-flex items-center gap-1.5 py-1.5 px-3 text-xs font-medium rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent no-underline cursor-pointer"
                    href=(BomDetailPath { id: bom_id }.to_string()) { "查看 BOM" }
                a class="inline-flex items-center gap-1.5 py-1.5 px-3 text-xs font-medium rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:text-accent no-underline cursor-pointer"
                    href=(BomEditPath { id: bom_id }.to_string()) { "编辑 BOM 物料" }
            }
        }
        // ── 标题 + 共享提示 ──
        div class="mb-4" {
            h1 class="text-xl font-bold text-fg tracking-tight flex items-center gap-2 flex-wrap" {
                span class="font-mono text-sm font-normal text-muted" { (product_code) }
                span { (bom_name) }
            }
            div class="mt-1.5 text-xs text-warn bg-warn-50 border border-warn-200 rounded-sm px-3 py-1.5 inline-block" {
                "工序按产品维护：该产品所有 BOM 版本（Draft / Published）共享同一套工序。"
            }
            @if has_ops {
                div class="mt-2 text-xs text-muted" {
                    "共 " (operations.len()) " 道 · 检验点 " (inspect_cnt) " · 委外 " (outsource_cnt)
                }
            }
        }
        // ── 从工艺路线加载 ──
        @if !routings.is_empty() {
            div class="mb-4 flex items-center gap-2.5 flex-wrap" {
                span class="text-xs text-muted whitespace-nowrap" { "从工艺路线加载：" }
                form class="flex items-center gap-2 flex-wrap"
                    hx-post=(BomOperationApplyPath { id: bom_id }.to_string())
                    hx-swap="none" {
                    div class="min-w-[220px]" {
                        (combo_select(
                            "routing_id",
                            &routings.iter().map(|r| ComboOption {
                                value: r.id.to_string(),
                                label: format!("{} · {}", r.code, r.name),
                            }).collect::<Vec<_>>(),
                            None,
                            "— 选择工艺路线 —",
                            "搜索工艺路线…",
                            None,
                        ))
                    }
                    @if has_ops {
                        button type="submit"
                            class="inline-flex items-center gap-1 py-1.5 px-3 text-xs font-medium rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
                            hx-confirm="该产品已有工序，加载将覆盖现有工序，是否继续？"
                            hx-disabled-elt="this" {
                            "加载工序"
                        }
                    } @else {
                        button type="submit"
                            class="inline-flex items-center gap-1 py-1.5 px-3 text-xs font-medium rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
                            hx-disabled-elt="this" {
                            "加载工序"
                        }
                    }
                    @if has_ops {
                        label class="inline-flex items-center gap-1 text-xs text-muted cursor-pointer" {
                            input type="checkbox" name="force" value="true" class="accent-accent" {};
                            "覆盖现有工序"
                        }
                    }
                }
                div class="text-xs text-muted" {
                    "拷贝后与模板解耦，后续修改模板不影响本 BOM。"
                }
            }
        }
        // ── 工序表格（#ops-table 监听 bomOpChanged 自刷新）──
        div id="ops-table"
            hx-get=(BomOperationsPath { id: bom_id }.to_string())
            hx-trigger="bomOpChanged from:body"
            hx-target="this"
            hx-select="#ops-table"
            hx-swap="outerHTML" {
            (operations_table(bom_id, operations, processes, work_centers, nl_products))
        }
        // 拖拽脚本（仅渲染一次即可，但放在此处随 #ops-table 刷新无害——Sortable 会对新 tbody 重新初始化）
        script type="text/javascript" src="/bom-operation.js?v=20260709" {}
    }
}

fn operations_table(
    bom_id: i64,
    operations: &[BomOperation],
    processes: &[LaborProcessDict],
    work_centers: &[WorkCenter],
    nl_products: &[Product],
) -> Markup {
    let post = BomOperationsPath { id: bom_id }.to_string();
    html! {
        div class="data-card overflow-hidden" {
            div class="flex justify-between items-center px-5 py-3.5 border-b border-border-soft" {
                h2 class="text-base font-semibold text-fg" { "工艺工序 " span class="text-xs font-normal text-muted" { "(" (operations.len()) " 道)" } }
                button type="button"
                    class="inline-flex items-center gap-1.5 py-1.5 px-3 text-xs font-medium rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover cursor-pointer"
                    hx-post=(post.clone())
                    hx-target="#ops-tbody"
                    hx-swap="beforeend"
                    hx-vals=(serde_json::json!({ "step_order": 0 }).to_string())
                { "+ 添加工序" }
            }
            @if operations.is_empty() {
                div class="p-8 text-center text-muted text-sm" { "暂无工序，点击「添加工序」手动添加，或从工艺路线模板拷贝。" }
            } @else {
                div class="overflow-x-auto" {
                    table class="w-full text-[13px] min-w-[880px]" {
                        thead {
                            tr class="bg-surface" {
                                th class="w-[28px] px-2 py-2 text-left text-xs font-semibold text-fg-2" { "" }
                                th class="w-[44px] px-2 py-2 text-left text-xs font-semibold text-fg-2" { "序号" }
                                th class="px-2 py-2 text-left text-xs font-semibold text-fg-2" { "工序" }
                                th class="w-[130px] px-2 py-2 text-left text-xs font-semibold text-fg-2" { "工作中心" }
                                th class="w-[160px] px-2 py-2 text-left text-xs font-semibold text-fg-2" { "产出品" }
                                th class="w-[120px] px-2 py-2 text-left text-xs font-semibold text-fg-2" { "备注" }
                                th class="w-[120px] px-2 py-2 text-center text-xs font-semibold text-fg-2" { "属性" }
                                th class="w-[50px] px-2 py-2 text-center text-xs font-semibold text-fg-2" { "" }
                            }
                        }
                        tbody id="ops-tbody" {
                            @for op in operations {
                                (operation_row(bom_id, op, processes, work_centers, nl_products))
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 单行可编辑工序。
/// 工序下拉 change → outerHTML 刷新整行（带默认值/类别联动）；
/// 其余字段 change/blur → 静默保存（hx-swap="none"），避免 DOM 抖动。
fn operation_row(
    bom_id: i64,
    op: &BomOperation,
    processes: &[LaborProcessDict],
    work_centers: &[WorkCenter],
    nl_products: &[Product],
) -> Markup {
    let step = op.step_order;
    let row_id = format!("op-row-{step}");
    let post = BomOperationsPath { id: bom_id }.to_string();
    let target = format!("#{row_id}");

    // 静默保存字段共用属性（hx-swap=none，不替换 DOM）
    let silent = format!("hx-post={post} hx-include=\"{target}\" hx-swap=\"none\"");
    let _ = silent; // 仅作注释，maud 里逐字段写属性

    html! {
        tr id=(row_id) data-step=(step) class="border-t border-border-soft hover:bg-surface/40" {
            // 序号 + 拖拽手柄 + hidden step_order
            td class="px-2 py-2 handle text-muted select-none cursor-grab" title="拖动排序" {
                "⟜"
                input type="hidden" name="step_order" value=(step) {};
            }
            td class="px-2 py-2 text-muted font-mono" { (step) }
            // 工序（可搜索下拉；选中 change → 刷新整行，带默认值联动）
            td class="px-2 py-2" {
                (combo_select(
                    "process_code",
                    &processes.iter().map(|p| ComboOption {
                        value: p.code.clone(),
                        label: p.name.clone(),
                    }).collect::<Vec<_>>(),
                    if op.process_code.is_empty() { None } else { Some(op.process_code.as_str()) },
                    "— 选择 —",
                    "搜索工序…",
                    Some(&ComboHx {
                        post: post.clone(),
                        target: target.clone(),
                        include: target.clone(),
                    }),
                ))
            }
            // 工作中心（change → 静默保存）
            td class="px-2 py-2" {
                select name="work_center_id" class="w-full px-1.5 py-1 border border-border rounded-sm bg-transparent text-fg-2 text-[13px] hover:border-border focus:border-accent focus:bg-bg cursor-pointer"
                    hx-post=(post.clone()) hx-include=(target.clone()) hx-swap="none" hx-trigger="change" {
                    option value="" selected[op.work_center_id.is_none()] { "—" }
                    @for w in work_centers {
                        option value=(w.id) selected[Some(w.id) == op.work_center_id] { (w.name) }
                    }
                }
            }
            // 产出品（change → 静默保存；检测工序选"无产出"）
            td class="px-2 py-2" {
                select name="output_product_id" class="w-full px-1.5 py-1 border border-border rounded-sm bg-transparent text-fg-2 text-[13px] hover:border-border focus:border-accent focus:bg-bg cursor-pointer"
                    hx-post=(post.clone()) hx-include=(target.clone()) hx-swap="none" hx-trigger="change" {
                    option value="" selected[op.output_product_id.is_none()] { "— 无产出 —" }
                    @for p in nl_products {
                        option value=(p.product_id) selected[Some(p.product_id) == op.output_product_id] {
                            (p.pdt_name)
                        }
                    }
                }
            }
            // 备注（blur changed → 静默保存）
            td class="px-2 py-2" {
                input type="text" name="remark"
                    value=(op.remark.as_deref().unwrap_or(""))
                    class="w-full px-1.5 py-1 border border-border rounded-sm bg-transparent text-fg-2 text-[13px] focus:border-accent focus:bg-bg"
                    hx-post=(post.clone()) hx-include=(target.clone()) hx-swap="none" hx-trigger="blur changed" {}
            }
            // 属性：仅「委外」可勾选（change → 静默保存）；
            // 检验点去掉 UI 编辑，用 hidden 保留原值，避免编辑他字段时被 hx-include 覆盖成 false
            td class="px-2 py-2 text-center whitespace-nowrap" {
                input type="hidden" name="is_inspection_point" value=(if op.is_inspection_point { "true" } else { "false" }) {};
                // 工时/标准成本/损耗：UI 不展示，用 hidden 保留原值，避免编辑他字段时 hx-include 提交清零（同 is_inspection_point 范式）
                input type="hidden" name="standard_time" value=(fmt_dec(op.standard_time)) {};
                input type="hidden" name="standard_cost" value=(fmt_dec(op.standard_cost)) {};
                input type="hidden" name="allowed_loss_rate" value=(fmt_dec(if op.allowed_loss_rate == Decimal::ZERO { None } else { Some(op.allowed_loss_rate) })) {};
                label class="inline-flex items-center gap-1 text-xs text-fg-2 cursor-pointer" {
                    input type="checkbox" value="true" name="is_outsourced" checked[op.is_outsourced] class="accent-accent"
                        hx-post=(post.clone()) hx-include=(target.clone()) hx-swap="none" hx-trigger="change" {};
                    "委外"
                }
            }
            // 删除（广播 bomOpChanged → #ops-table 刷新）
            td class="px-2 py-2 text-center" {
                button type="button" class="text-danger hover:bg-danger-50 rounded-sm px-1.5 py-0.5 text-xs cursor-pointer bg-transparent border-none"
                    hx-post=(BomOperationDeletePath { id: bom_id, step_order: step }.to_string())
                    hx-confirm="确认删除该工序？关联的计件单价将一并清除。"
                    hx-swap="none" { "✕" }
            }
        }
    }
}
