use axum::extract::Query;
use axum::response::Html;
use maud::{Markup, html, PreEscaped};

use abt_core::master_data::product::ProductService;
use abt_core::master_data::routing::RoutingService;
use abt_core::master_data::routing::model::*;
use abt_core::master_data::work_center::{new_work_center_service, service::WorkCenterService};
use abt_core::shared::identity::UserService;
use abt_core::shared::types::PageParams;
use std::collections::HashMap;

use abt_macros::require_permission;

use crate::components::{detail::detail_row, icon};
use crate::components::pagination::pagination;
use crate::components::product_picker;
use crate::layout::page::admin_page;
use crate::routes::routing::{
    RoutingBindBomPath, RoutingBomListPath, RoutingCopyPath, RoutingDeletePath, RoutingDetailPath,
    RoutingEditPath, RoutingListPath, RoutingUnbindBomPath,
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

 // 工序产出品 / 工作中心名称映射（详情表格展示用）
 let pids: Vec<i64> = detail.steps.iter().filter_map(|s| s.product_id).collect();
 let product_map: HashMap<i64, String> = if pids.is_empty() {
 HashMap::new()
 } else {
 state.product_service()
 .get_by_ids(&service_ctx, &mut conn, pids)
 .await
 .unwrap_or_default()
 .into_iter()
 .map(|p| (p.product_id, p.pdt_name))
 .collect()
 };
 let wc_map: HashMap<i64, String> = new_work_center_service(state.pool.clone())
 .list_active(&service_ctx, &mut conn)
 .await
 .unwrap_or_default()
 .into_iter()
 .map(|wc| (wc.id, wc.name))
 .collect();

 let content = routing_detail_page(&detail, &boms, &qp.keyword, &creator_name, &product_map, &wc_map);
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
 product_map: &HashMap<i64, String>,
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
                            th class="w-[180px]" { "产出品" }
                            th class="w-[140px]" { "工作中心" }
                            th class="w-[100px] text-right" { "计件单价" }
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
                                td class="text-fg-2" { (map_name(step.product_id, product_map)) }
                                td class="text-fg-2" { (map_name(step.work_center_id, wc_map)) }
                                td class="text-right font-mono tabular-nums text-fg-2" { (fmt_opt_decimal(step.unit_price)) }
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
                        th class="w-[80px]" { "操作" }
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
