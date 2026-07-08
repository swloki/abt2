use std::collections::{HashMap, HashSet};

use axum::extract::Query;
use axum::response::Html;
use maud::{Markup, html, PreEscaped};
use rust_decimal::Decimal;

use abt_core::master_data::bom::BomQueryService;
use abt_core::master_data::bom_routing_output::model::{StepWithOutput, UpsertBomOutputReq};
use abt_core::master_data::bom_routing_output::{BomRoutingOutputService, new_bom_routing_output_service};
use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::Product;
use abt_core::master_data::routing::RoutingService;
use abt_core::master_data::routing::model::*;
use abt_core::master_data::work_center::model::WorkCenter;
use abt_core::master_data::work_center::{new_work_center_service, service::WorkCenterService};
use abt_core::shared::identity::UserService;
use abt_core::shared::types::PageParams;

use abt_macros::require_permission;

use crate::components::{detail::detail_row, icon};
use crate::components::pagination::pagination;
use crate::components::product_picker;
use crate::layout::page::admin_page;
use crate::routes::routing::{
    RoutingBindBomPath, RoutingBomListPath, RoutingCopyPath, RoutingDeletePath, RoutingDetailPath,
    RoutingEditPath, RoutingListPath, RoutingOutputDeletePath, RoutingOutputEditPath,
    RoutingOutputUpsertPath, RoutingUnbindBomPath,
};
use crate::utils::RequestContext;

// ── Query Params ──

#[derive(Debug, serde::Deserialize, Clone, Default)]
pub struct BomPageParams {
 #[serde(default)]
 pub page: Option<u32>,
 #[serde(default)]
 pub keyword: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct BindBomForm {
 pub product_id: i64,
}

#[derive(Debug, serde::Deserialize)]
pub struct UnbindBomForm {
 pub product_code: String,
}

/// 覆盖层编辑分区 GET 参数（product_code 定位 BOM）。
#[derive(Debug, serde::Deserialize, Clone)]
pub struct OutputEditParams {
    pub product_code: String,
}

/// UPSERT 单道工序覆盖的 form（by product_code + step_order）。
#[derive(Debug, serde::Deserialize)]
pub struct OutputUpsertForm {
    pub product_code: String,
    pub step_order: i32,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub output_product_id: Option<i64>,
    /// 计件单价（字符串 → Decimal；空串 = None）
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub unit_price: Option<String>,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub work_center_id: Option<i64>,
}

/// 删除单道工序覆盖的 form。
#[derive(Debug, serde::Deserialize)]
pub struct OutputDeleteForm {
    pub product_code: String,
    pub step_order: i32,
}

// ── Handlers ──

#[require_permission("ROUTING", "read")]
pub async fn get_routing_detail(
 path: RoutingDetailPath,
 ctx: RequestContext,
 Query(qp): Query<BomPageParams>,
) -> crate::errors::Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.routing_service();

 let detail = svc.get_detail(&service_ctx, &mut conn, path.id).await?;
 let bom_page = PageParams::new(qp.page.unwrap_or(1), 10);
 let boms = svc.paginate_boms_by_routing(&service_ctx, &mut conn, path.id, qp.keyword.clone(), bom_page).await?;

 let creator_name = if let Some(uid) = detail.routing.operator_id {
 state.user_service()
 .get_users_by_ids(&service_ctx, &mut conn, vec![uid])
 .await
 .ok()
 .and_then(|users| users.into_iter().next())
 .map(|u| u.user.display_name.unwrap_or(u.user.username))
 } else {
 None
 };

 // 工作中心名称映射（详情表格展示用）
 let wc_map: HashMap<i64, String> = new_work_center_service(state.pool.clone())
 .list_active(&service_ctx, &mut conn)
 .await
 .unwrap_or_default()
 .into_iter()
 .map(|wc| (wc.id, wc.name))
 .collect();

 let content = routing_detail_page(&detail, &boms, &qp.keyword, &creator_name, &wc_map);
 let detail_path_str = RoutingDetailPath { id: path.id }.to_string();
 let page_html = admin_page(
 is_htmx,
 &format!("{} - 工艺路线详情", detail.routing.name),
 &claims,
 "production",
 &detail_path_str,
 "主数据管理",
 Some(&detail.routing.name),
 content, &nav_filter, );

 Ok(Html(page_html.into_string()))
}

#[require_permission("ROUTING", "read")]
pub async fn get_routing_bom_list(
 path: RoutingBomListPath,
 ctx: RequestContext,
 Query(qp): Query<BomPageParams>,
) -> crate::errors::Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let svc = state.routing_service();
 let page = PageParams::new(qp.page.unwrap_or(1), 10);
 let boms = svc.paginate_boms_by_routing(&service_ctx, &mut conn, path.id, qp.keyword.clone(), page).await?;
 Ok(Html(bom_list_fragment(path.id, &qp.keyword, &boms, None).into_string()))
}

#[require_permission("ROUTING", "update")]
pub async fn bind_bom(
 path: RoutingBindBomPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<BindBomForm>,
) -> crate::errors::Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let product = state.product_service().get(&service_ctx, &mut conn, form.product_id).await?;
 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 // 唯一性：一个 BOM 只能关联一个 routing。已关联到其他 routing → 回滚并回显错误到关联 BOM 列表（不 toast）。
 match state.routing_service()
     .set_bom_routing(&service_ctx, &mut tx, product.product_code.clone(), path.id).await
 {
     Ok(()) => {
         tx.commit().await
             .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
     }
     Err(e) => {
         let _ = tx.rollback().await;
         let boms = state.routing_service().paginate_boms_by_routing(&service_ctx, &mut conn, path.id, None, PageParams::new(1, 10)).await?;
         let raw = format!("{e}");
         let msg = ["Business rule: ", "Validation: ", "Unauthorized: ", "Permission denied: "]
             .iter()
             .find_map(|p| raw.strip_prefix(p))
             .unwrap_or(&raw)
             .to_string();
         return Ok(Html(bom_list_fragment(path.id, &None, &boms, Some(&msg)).into_string()));
     }
 }
 let boms = state.routing_service().paginate_boms_by_routing(&service_ctx, &mut conn, path.id, None, PageParams::new(1, 10)).await?;
 Ok(Html(bom_list_fragment(path.id, &None, &boms, None).into_string()))
}

#[require_permission("ROUTING", "update")]
pub async fn unbind_bom(
 path: RoutingUnbindBomPath,
 ctx: RequestContext,
 axum::Form(form): axum::Form<UnbindBomForm>,
) -> crate::errors::Result<Html<String>> {
 let RequestContext { mut conn, state, service_ctx, .. } = ctx;
 let mut tx = state.pool.begin().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 state.routing_service().delete_bom_routing(&service_ctx, &mut tx, form.product_code).await?;
 tx.commit().await
     .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
 let boms = state.routing_service().paginate_boms_by_routing(&service_ctx, &mut conn, path.id, None, PageParams::new(1, 10)).await?;
 Ok(Html(bom_list_fragment(path.id, &None, &boms, None).into_string()))
}

// ── 产出/计件覆盖层（per-BOM）──

/// 装载覆盖层编辑上下文：工序步骤(+覆盖) + 产出品候选(该 BOM 非叶子节点) + 工作中心列表。
async fn load_output_ctx(
    state: &crate::state::AppState,
    service_ctx: &abt_core::shared::types::ServiceContext,
    db: abt_core::shared::types::PgExecutor<'_>,
    product_code: &str,
) -> crate::errors::Result<(Vec<StepWithOutput>, Vec<Product>, Vec<WorkCenter>)> {
    let steps = new_bom_routing_output_service(state.pool.clone())
        .list_steps_with_output(service_ctx, db, product_code.to_string())
        .await?;
    let candidates: Vec<Product> = {
        let ids = state.bom_query_service()
            .list_non_leaf_product_ids_by_product_codes(service_ctx, db, &[product_code.to_string()])
            .await
            .unwrap_or_default();
        if ids.is_empty() {
            Vec::new()
        } else {
            state.product_service()
                .get_by_ids(service_ctx, db, ids)
                .await
                .unwrap_or_default()
        }
    };
    let work_centers = new_work_center_service(state.pool.clone())
        .list_active(service_ctx, db)
        .await
        .unwrap_or_default();
    Ok((steps, candidates, work_centers))
}

/// 渲染某 BOM 的产出/计件覆盖编辑分区（drawer body）。
#[require_permission("ROUTING", "read")]
pub async fn get_routing_output_edit(
    path: RoutingOutputEditPath,
    ctx: RequestContext,
    Query(qp): Query<OutputEditParams>,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let (steps, candidates, work_centers) =
        load_output_ctx(&state, &service_ctx, &mut conn, &qp.product_code).await?;
    Ok(Html(
        output_edit_fragment(path.id, &qp.product_code, &steps, &candidates, &work_centers)
            .into_string(),
    ))
}

/// UPSERT 单道工序的产出覆盖（自替换该行；产出品非法时回显错误到行内）。
#[require_permission("ROUTING", "update")]
pub async fn upsert_routing_output(
    path: RoutingOutputUpsertPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<OutputUpsertForm>,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    // 顶部装载：校验 + 出错重渲染 + 成功重渲染共用 candidates/work_centers。
    let (steps_before, candidates, work_centers) =
        load_output_ctx(&state, &service_ctx, &mut conn, &form.product_code).await?;

    // 校验产出品 ∈ 该 BOM 非叶子节点（空 = 不指定，允许）。
    let candidate_id_set: HashSet<i64> = candidates.iter().map(|p| p.product_id).collect();
    if let Some(pid) = form.output_product_id {
        if !candidate_id_set.contains(&pid) {
            let step = steps_before.iter().find(|s| s.step_order == form.step_order);
            return Ok(Html(
                output_step_row(
                    path.id, &form.product_code, step, &candidates, &work_centers,
                    Some("产出品必须为该 BOM 的非叶子节点"),
                )
                .into_string(),
            ));
        }
    }

    let unit_price = form.unit_price.as_deref().and_then(|v| v.trim().parse::<Decimal>().ok());
    let req = UpsertBomOutputReq {
        product_code: form.product_code.clone(),
        routing_id: path.id,
        step_order: form.step_order,
        output_product_id: form.output_product_id,
        unit_price,
        work_center_id: form.work_center_id,
    };

    let mut tx = state.pool.begin().await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    new_bom_routing_output_service(state.pool.clone())
        .upsert_output(&service_ctx, &mut tx, req).await?;
    tx.commit().await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

    // 重查该 step（反映 upsert 后真实状态），渲染单行自替换。
    let refreshed = new_bom_routing_output_service(state.pool.clone())
        .list_steps_with_output(&service_ctx, &mut conn, form.product_code.clone()).await?;
    let step = refreshed.iter().find(|s| s.step_order == form.step_order);
    Ok(Html(
        output_step_row(path.id, &form.product_code, step, &candidates, &work_centers, None)
            .into_string(),
    ))
}

/// 删除单道工序的产出覆盖（回退模板默认），自替换该行。
#[require_permission("ROUTING", "update")]
pub async fn delete_routing_output(
    path: RoutingOutputDeletePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<OutputDeleteForm>,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;

    let mut tx = state.pool.begin().await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;
    new_bom_routing_output_service(state.pool.clone())
        .delete_output(&service_ctx, &mut tx, form.product_code.clone(), form.step_order).await?;
    tx.commit().await
        .map_err(|e| abt_core::shared::types::error::DomainError::Internal(e.into()))?;

    let (steps, candidates, work_centers) =
        load_output_ctx(&state, &service_ctx, &mut conn, &form.product_code).await?;
    let step = steps.iter().find(|s| s.step_order == form.step_order);
    Ok(Html(
        output_step_row(path.id, &form.product_code, step, &candidates, &work_centers, None)
            .into_string(),
    ))
}

// ── Components ──

/// id → 名称（无则 —）
fn map_name(id: Option<i64>, map: &HashMap<i64, String>) -> String {
 id.and_then(|i| map.get(&i)).cloned().unwrap_or_else(|| "—".to_string())
}

/// Option<Decimal> → 格式化（无则 —）
fn fmt_opt_decimal(v: Option<rust_decimal::Decimal>) -> String {
 v.map(crate::utils::fmt_qty).unwrap_or_else(|| "—".into())
}

fn routing_detail_page(
 detail: &RoutingDetail,
 boms: &abt_core::shared::types::PaginatedResult<BomRouting>,
 keyword: &Option<String>,
 creator_name: &Option<String>,
 wc_map: &HashMap<i64, String>,
) -> Markup {
 let routing = &detail.routing;
 let steps = &detail.steps;
 let list_path = RoutingListPath;
 let delete_path = RoutingDeletePath { id: routing.id };

 let required_count = steps.iter().filter(|s| s.is_required).count();
 let step_count = steps.len();

 html! {
    div {
        // ── Detail Top ──
        div class="flex justify-between items-start" {
            div class="flex items-center gap-5" {
                div class="w-10 h-10 grid place-items-center rounded-full bg-accent text-white" {
                    (icon::clipboard_list_icon("w-5 h-5"))
                }
                div {
                    h1 class="text-xl font-bold" { (routing.name) }
                    div class="flex gap-4 text-muted text-xs" {
                        span { "工序: " (step_count) }
                        span { "必经: " (required_count) }
                        span { "关联BOM: " (boms.total) }
                        @if let Some(dt) = routing.created_at {
                            span { "创建: " (dt.format("%Y-%m-%d")) }
                        }
                    }
                }
            }
            div class="flex gap-3" {
                a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    href=(format!("{list_path}?restore=true"))
                { (icon::arrow_left_icon("w-4 h-4")) " 返回列表" }
                a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    href=(RoutingEditPath { id: routing.id }.to_string())
                { (icon::edit_icon("w-4 h-4")) " 编辑" }
                a   class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    href=(RoutingCopyPath { id: routing.id }.to_string())
                { (icon::copy_icon("w-4 h-4")) " 复制" }
                button
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90"
                    hx-confirm=({
                        format!(
                            "确定要删除工艺路线 {} 吗？此操作不可撤销。",
                            routing.name,
                        )
                    })
                    hx-post=(delete_path.to_string())
                    hx-swap="none"
                { (icon::trash_icon("w-4 h-4")) " 删除" }
            }
        }
        // ── 基本信息 ──
        div class="data-card" {
            div class="flex items-center justify-between text-sm font-semibold mb-4 pb-2 border-b border-border-soft"
            { "基本信息" }
            div class="grid gap-5 grid-cols-3" {
                ({
                    detail_row(
                        "编码",
                        html! {
                            span class = "font-mono tabular-nums" { (routing.code) }
                        },
                    )
                })
                ({
                    detail_row(
                        "名称",
                        html! {
                            (routing.name)
                        },
                    )
                })
                ({
                    detail_row(
                        "描述",
                        html! {
                            (routing.description.as_deref().unwrap_or("—"))
                        },
                    )
                })
                ({
                    detail_row(
                        "创建人",
                        html! {
                            @ if let Some(name) = creator_name { (name) } @ else { "—"
                            }
                        },
                    )
                })
                @if let Some(dt) = routing.created_at {
                    ({
                        detail_row(
                            "创建时间",
                            html! {
                                (dt.format("%Y-%m-%d %H:%M"))
                            },
                        )
                    })
                } @else { ({
                    detail_row(
                        "创建时间",
                        html! {
                            "—"
                        },
                    )
                }) }
                @if let Some(dt) = routing.updated_at {
                    ({
                        detail_row(
                            "更新时间",
                            html! {
                                (dt.format("%Y-%m-%d %H:%M"))
                            },
                        )
                    })
                } @else { ({
                    detail_row(
                        "更新时间",
                        html! {
                            "—"
                        },
                    )
                }) }
            }
        }
        // ── 工序流程 ──
        div class="data-card" {
            div class="flex items-center justify-between text-sm font-semibold mb-4 pb-2 border-b border-border-soft"
            {
                span { "工序流程" }
                span class="text-muted font-normal text-xs" { "（共 " (step_count) " 道工序）" }
            }
            @if steps.is_empty() {
                div class="text-center p-6 text-muted text-sm" { "暂无工序步骤" }
            } @else {
                table class="data-table" {
                    thead {
                        tr {
                            th class="w-[60px]" { "序号" }
                            th { "工序名称" }
                            th class="w-[140px]" { "工作中心" }
                            th class="w-[90px] text-right" { "标准工时" }
                            th class="w-[60px] text-center" { "委外" }
                            th class="w-[70px] text-center" { "必经" }
                            th { "备注" }
                        }
                    }
                    tbody {
                        @for step in steps {
                            tr {
                                td class="font-mono tabular-nums" { (step.step_order) }
                                td { (step.process_name.as_deref().unwrap_or(&step.process_code)) }
                                td class="text-fg-2" { (map_name(step.work_center_id, wc_map)) }
                                td class="text-right font-mono tabular-nums text-fg-2" { (fmt_opt_decimal(step.standard_time)) }
                                td class="text-center" {
                                    @if step.is_outsourced {
                                        span class="text-accent" { "✓" }
                                    } @else {
                                        span class="text-muted" { "—" }
                                    }
                                }
                                td class="text-center" {
                                    @if step.is_required {
                                        span class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap bg-warn-bg text-warn" { "必经" }
                                    } @else {
                                        span class="inline-flex items-center gap-[5px] rounded-full text-xs font-medium whitespace-nowrap bg-surface text-muted" { "选检" }
                                    }
                                }
                                td class="text-fg-2" { (step.remark.as_deref().unwrap_or("—")) }
                            }
                        }
                    }
                }
            }
        }
        // ── 关联BOM ──
        div class="data-card routing-bom-card" {
            div class="flex items-center justify-between text-sm font-semibold mb-4 pb-2 border-b border-border-soft"
            {
                span { "关联BOM" }
                button type="button" onclick="openBomPicker()"
                    class="inline-flex items-center gap-1 py-1.5 px-3 rounded-sm bg-accent text-accent-on text-xs font-medium cursor-pointer border-none hover:bg-accent-hover"
                { (icon::plus_icon("w-3.5 h-3.5")) "添加BOM" }
            }
            (bom_list_fragment(routing.id, keyword, boms, None))
        }
        // 产品选择弹窗（关联 BOM 用）+ 桥接 hidden input/display（picker 选中后填这俩，JS 读 product_id 触发 bind）
        input type="hidden" id="bind-product-id";
        span id="bind-product-display" class="hidden" {};
        (product_picker::product_picker_modal("routing-bom-modal", "bind-product-id", "bind-product-display"))
        (bom_picker_js(routing.id))
        // 产出/计件覆盖层编辑 drawer（关联 BOM 行「维护产出」按钮 hx-get 加载 body，settle 后打开）
        (output_edit_drawer())
    }
}
}
// ── 关联 BOM 列表片段（自包含 #routing-bom-list，搜索/分页/bind/unbind 都刷新它）──

fn bom_list_fragment(
 routing_id: i64,
 keyword: &Option<String>,
 boms: &abt_core::shared::types::PaginatedResult<BomRouting>,
 error: Option<&str>,
) -> Markup {
 let list_path = RoutingBomListPath { id: routing_id }.to_string();
 let unbind_path = RoutingUnbindBomPath { id: routing_id }.to_string();
 html! {
    div id="routing-bom-list" {
        @if let Some(msg) = error {
            div class="mb-3 p-2.5 rounded-sm bg-danger-bg text-danger text-xs" { (msg) }
        }
        div class="mb-3" {
            input type="text" name="keyword" id="routing-bom-keyword"
                class="w-full max-w-xs px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                placeholder="按产品名称或编码搜索…"
                value=(keyword.as_deref().unwrap_or(""))
                hx-get=(&list_path)
                hx-trigger="keyup changed delay:300ms"
                hx-target="#routing-bom-list"
                hx-select="#routing-bom-list"
                hx-swap="outerHTML"
                hx-include="this";
        }
        @if boms.items.is_empty() {
            div class="text-center p-6 text-muted text-sm" { "暂无关联BOM" }
        } @else {
            table class="data-table" {
                thead {
                    tr {
                        th { "产品编码" }
                        th { "产品名称" }
                        th style="width:160px" { "关联时间" }
                        th class="w-[150px]" { "操作" }
                    }
                }
                tbody {
                    @for bom in &boms.items {
                        tr {
                            td class="font-mono tabular-nums" { (bom.product_code) }
                            td { (bom.product_name.as_deref().unwrap_or("—")) }
                            td {
                                @if let Some(dt) = bom.created_at { (dt.format("%Y-%m-%d %H:%M")) } @else { "—" }
                            }
                            td {
                                div class="flex items-center gap-2" {
                                    button type="button"
                                        class="text-accent text-xs hover:underline cursor-pointer bg-transparent border-none whitespace-nowrap"
                                        hx-get=(RoutingOutputEditPath { id: routing_id }.to_string())
                                        hx-vals=(serde_json::json!({ "product_code": bom.product_code }).to_string())
                                        hx-target="#routing-output-body"
                                        hx-swap="innerHTML"
                                    { "维护产出" }
                                    button type="button"
                                        class="text-danger text-xs hover:underline cursor-pointer bg-transparent border-none"
                                        hx-post=(&unbind_path)
                                        hx-vals=(serde_json::json!({ "product_code": bom.product_code }).to_string())
                                        hx-confirm=(format!("确定取消产品 {} 的工艺路线关联吗？", bom.product_code))
                                        hx-target="#routing-bom-list"
                                        hx-select="#routing-bom-list"
                                        hx-swap="outerHTML"
                                    { "取消" }
                                }
                            }
                        }
                    }
                }
            }
            ({
                pagination(&list_path, "#routing-bom-list", "#routing-bom-keyword", boms.total, boms.page, boms.total_pages)
            })
        }
    }
}
}

/// 产品选择弹窗桥接 JS（打开弹窗 + 选中后 POST bind 刷新列表）
fn bom_picker_js(routing_id: i64) -> Markup {
 let bind_path = RoutingBindBomPath { id: routing_id }.to_string();
 html! {
    script {
        (PreEscaped(
            format!(
                r#"
function openBomPicker() {{
 const modal = document.getElementById('routing-bom-modal');
 modal.querySelectorAll('.product-search-input').forEach(i => i.value = '');
 modal.classList.add('is-open');
 htmx.ajax('GET', '/api/products/search', {{
 target: '#product-search-results', swap: 'innerHTML',
 values: {{ target_id: 'bind-product-id', display_id: 'bind-product-display', modal_id: 'routing-bom-modal', name: '', code: '' }}
 }});
}}
document.body.addEventListener('productSelected', () => {{
 const pid = document.querySelector('#bind-product-id').value;
 if (pid) {{
 htmx.ajax('POST', '{bind_path}', {{
 target: '#routing-bom-list', swap: 'outerHTML',
 values: {{ product_id: pid }}
 }});
 }}
}});
"#,
                bind_path = bind_path,
            ),
        ))
    }
 }
}

// ── Helpers ──

// ── 产出/计件覆盖层组件（per-BOM）──

/// 详情页覆盖层编辑 drawer（空壳；body 由「维护产出」按钮 `hx-get` 动态加载，
/// 内容 settle 后 `add .open` 显示）。`.drawer-overlay` 默认 `display:none`，`.open` 显示。
fn output_edit_drawer() -> Markup {
    html! {
        div id="routing-output-drawer"
            class="drawer-overlay fixed inset-0 z-[90] flex justify-end bg-slate-900/40"
            _="on click[me is event.target] remove .open\non keydown[event.key is 'Escape'] from body remove .open"
        {
            div class="drawer-panel bg-bg h-full w-[660px] max-w-[92vw] shadow-lg flex flex-col" {
                div class="flex items-center gap-3 px-5 py-4 border-b border-border-soft shrink-0" {
                    span class="text-sm font-semibold text-fg" { "产出 / 计件维护" }
                    button type="button"
                        class="ml-auto text-muted hover:text-fg text-xl leading-none bg-transparent border-none cursor-pointer"
                        _="on click remove .open from closest .drawer-overlay"
                    { "×" }
                }
                // body：内容 settle 后打开 drawer（等表单就位再显示，避免空壳闪烁）
                div id="routing-output-body"
                    class="overflow-y-auto flex-1"
                    _="on htmx:afterSettle add .open to #routing-output-drawer"
                {}
            }
        }
    }
}

/// 覆盖层编辑分区（drawer body）：该 BOM 的工序列表 + per-step 编辑行。
fn output_edit_fragment(
    routing_id: i64,
    product_code: &str,
    steps: &[StepWithOutput],
    candidates: &[Product],
    work_centers: &[WorkCenter],
) -> Markup {
    html! {
        div id="routing-output-edit" class="p-5" {
            div class="mb-4 pb-3 border-b border-border-soft" {
                div class="text-xs text-muted mb-0.5" { "产品编码" }
                div class="text-sm font-mono text-fg-2" { (product_code) }
                p class="text-xs text-muted mt-2 leading-relaxed" {
                    "产出品候选限定为该 BOM 的非叶子节点；工作中心留空则沿用模板默认。"
                }
            }
            @if steps.is_empty() {
                div class="text-center py-10 text-muted text-sm" {
                    "该 BOM 绑定的 routing 无工序步骤，或尚未绑定 routing。"
                }
            } @else {
                div class="flex flex-col gap-3" {
                    @for step in steps {
                        (output_step_row(routing_id, product_code, Some(step), candidates, work_centers, None))
                    }
                }
            }
        }
    }
}

/// 单道工序的产出覆盖编辑行（自包含：`hx-target="closest .output-step-row"` + outerHTML 自替换）。
/// 保存（upsert）与清除（delete）都自替换本行，符合组件化三原则。
fn output_step_row(
    routing_id: i64,
    product_code: &str,
    step: Option<&StepWithOutput>,
    candidates: &[Product],
    work_centers: &[WorkCenter],
    error: Option<&str>,
) -> Markup {
    let upsert_path = RoutingOutputUpsertPath { id: routing_id }.to_string();
    let delete_path = RoutingOutputDeletePath { id: routing_id }.to_string();
    let Some(step) = step else {
        return html! { div class="output-step-row" { "工序不存在" } };
    };
    let cur_output = step.output_product_id;
    // 当前产出品是否在合法候选(该 BOM 非叶子节点)内。历史脏数据(迁移 063 灌入、
    // 不在该 BOM 树的 product_id)不在候选内，select 需额外渲染该值让用户可见、可清/可改。
    let cur_in_candidates = cur_output
        .map_or(false, |id| candidates.iter().any(|p| p.product_id == id));
    let cur_price: String = step.unit_price.map(|d| d.to_string()).unwrap_or_default();
    let cur_wc = step.work_center_override_id;
    html! {
        div class="output-step-row border border-border-soft rounded-md p-3 bg-surface" {
            div class="flex items-center gap-2 mb-2" {
                span class="text-xs font-mono text-muted tabular-nums" { (step.step_order) }
                span class="text-sm font-medium text-fg" {
                    (step.process_name.as_deref().unwrap_or(&step.process_code))
                }
                @if step.has_override() {
                    span class="ml-auto inline-flex items-center gap-1 text-xs text-accent" {
                        (icon::check_circle_icon("w-3.5 h-3.5")) "已覆盖"
                    }
                }
            }
            @if let Some(msg) = error {
                div class="mb-2 p-2 rounded-sm bg-danger-bg text-danger text-xs" { (msg) }
            }
            form hx-post=(&upsert_path) hx-target="closest .output-step-row" hx-swap="outerHTML" {
                input type="hidden" name="product_code" value=(product_code);
                input type="hidden" name="step_order" value=(step.step_order);
                div class="grid grid-cols-3 gap-3" {
                    div {
                        label class="text-xs text-fg-2 block mb-1" { "产出品" }
                        select name="output_product_id"
                            class="w-full px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg" {
                            option value="" selected[cur_output.is_none()] { "— 不指定 —" }
                            // 当前产出品不在合法候选(历史脏数据)时，额外渲染该值让用户知情、可清/可改。
                            // output_product_name 由 list_steps_with_output JOIN products 取得（即使不在候选也有名）。
                            @if let Some(pid) = cur_output {
                                @if !cur_in_candidates {
                                    option value=(pid) selected {
                                        "⚠ " (step.output_product_name.as_deref().unwrap_or("未知")) " · " (pid) "（不在该 BOM 合法候选）"
                                    }
                                }
                            }
                            @for p in candidates {
                                option value=(p.product_id)
                                    selected[cur_output == Some(p.product_id)]
                                { (p.pdt_name.as_str()) " · " (p.product_code.as_str()) }
                            }
                        }
                    }
                    div {
                        label class="text-xs text-fg-2 block mb-1" { "计件单价" }
                        input type="number" step="any" name="unit_price"
                            value=(cur_price)
                            placeholder="0.00"
                            class="w-full px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg font-mono text-right";
                    }
                    div {
                        label class="text-xs text-fg-2 block mb-1" { "工作中心覆盖" }
                        select name="work_center_id"
                            class="w-full px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg" {
                            option value="" { "— 用模板 —" }
                            @for wc in work_centers {
                                option value=(wc.id) selected[cur_wc == Some(wc.id)] { (wc.name.as_str()) }
                            }
                        }
                    }
                }
                div class="flex items-center justify-end gap-2 mt-3" {
                    button type="submit"
                        class="inline-flex items-center gap-1 px-3 py-1 rounded-sm bg-accent text-accent-on text-xs font-medium border-none cursor-pointer hover:bg-accent-hover"
                    { (icon::save_icon("w-3.5 h-3.5")) "保存" }
                    @if step.has_override() {
                        button type="button"
                            class="px-3 py-1 rounded-sm text-danger text-xs font-medium border border-danger/30 bg-transparent cursor-pointer hover:bg-danger-bg"
                            hx-post=(&delete_path)
                            hx-vals=(serde_json::json!({ "product_code": product_code, "step_order": step.step_order }).to_string())
                            hx-target="closest .output-step-row"
                            hx-swap="outerHTML"
                            hx-confirm="清除该工序的产出覆盖（回退模板默认）？"
                        { "清除覆盖" }
                    }
                }
            }
        }
    }
}
