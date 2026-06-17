use axum::extract::Query;
use axum::response::Html;
use maud::{Markup, html};

use abt_core::master_data::routing::RoutingService;
use abt_core::master_data::routing::model::*;
use abt_core::shared::identity::UserService;
use abt_core::shared::types::PageParams;

use abt_macros::require_permission;

use crate::components::{detail::detail_row, icon};
use crate::components::pagination::htmx_pagination;
use crate::layout::page::admin_page;
use crate::routes::routing::{RoutingDeletePath, RoutingDetailPath, RoutingListPath};
use crate::utils::RequestContext;

// ── Query Params ──

#[derive(Debug, serde::Deserialize, Clone, Default)]
pub(crate) struct BomPageParams {
    pub page: Option<u32>,
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
    let boms = svc.paginate_boms_by_routing(&service_ctx, &mut conn, path.id, bom_page).await?;

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

    let content = routing_detail_page(&detail, &boms, &creator_name);
    let detail_path_str = RoutingDetailPath { id: path.id }.to_string();
    let page_html = admin_page(
        is_htmx,
        &format!("{} - 工艺路线详情", detail.routing.name),
        &claims,
        "md",
        &detail_path_str,
        "主数据管理",
        Some(&detail.routing.name),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("ROUTING", "update")]
pub async fn update_routing(
    _path: RoutingDetailPath,
    _ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    Ok(Html("<p>Update routing placeholder</p>".into()))
}


// ── Components ──

fn routing_detail_page(
    detail: &RoutingDetail,
    boms: &abt_core::shared::types::PaginatedResult<BomRouting>,
    creator_name: &Option<String>,
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
                    div class="customer-inline-grid place-items-center rounded-full text-white font-semibold shrink-0 select-none" style="background:var(--color-primary-light,#e0e7ff)" {
                        (icon::clipboard_list_icon("w-5 h-5"))
                    }
                    div {
                        h1 class="text-xl font-bold" {
                            (routing.name)
                        }
                        div class="flex gap-4 text-text-muted text-xs" {
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
                    a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(format!("{list_path}?restore=true")) {
                        (icon::arrow_left_icon("w-4 h-4"))
                        " 返回列表"
                    }
                    a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href="#" {
                        (icon::edit_icon("w-4 h-4"))
                        " 编辑"
                    }
                    a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href="#" {
                        (icon::copy_icon("w-4 h-4"))
                        " 复制"
                    }
                    button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative bg-danger text-white border-none hover:opacity-90-ghost"
                        hx-confirm=(format!("确定要删除工艺路线 {} 吗？此操作不可撤销。", routing.name))
                        hx-post=(delete_path.to_string())
                        hx-target="body"
                        hx-swap="outerHTML" {
                        (icon::trash_icon("w-4 h-4"))
                        " 删除"
                    }
                }
            }

            // ── 基本信息 ──
            div class="bg-white border border-border-soft rounded p-5" {
                div class="flex items-center justify-between text-sm font-semibold mb-4 pb-2 border-b border-border-soft" { "基本信息" }
                div class="grid gap-5" style="grid-template-columns:repeat(3,1fr)" {
                    (detail_row("编码", html! { span class="font-mono tabular-nums" { (routing.id) } }))
                    (detail_row("名称", html! { (routing.name) }))
                    (detail_row("描述", html! { (routing.description.as_deref().unwrap_or("—")) }))
                    (detail_row("创建人", html! {
                        @if let Some(name) = creator_name {
                            (name)
                        } @else {
                            "—"
                        }
                    }))
                    @if let Some(dt) = routing.created_at {
                        (detail_row("创建时间", html! { (dt.format("%Y-%m-%d %H:%M")) }))
                    } @else {
                        (detail_row("创建时间", html! { "—" }))
                    }
                    @if let Some(dt) = routing.updated_at {
                        (detail_row("更新时间", html! { (dt.format("%Y-%m-%d %H:%M")) }))
                    } @else {
                        (detail_row("更新时间", html! { "—" }))
                    }
                }
            }

            // ── 工序流程 ──
            div class="bg-white border border-border-soft rounded p-5" style="margin-top:var(--space-5)" {
                div class="flex items-center justify-between text-sm font-semibold mb-4 pb-2 border-b border-border-soft" {
                    span { "工序流程" }
                    span style="color:var(--text-tertiary);font-weight:400;font-size:12px" {
                        "（共 " (step_count) " 道工序）"
                    }
                }
                @if steps.is_empty() {
                    div class="text-center p-6 text-text-muted text-sm" { "暂无工序步骤" }
                } @else {
                    table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:text-left [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" {
                        thead {
                            tr {
                                th style="width:60px" { "序号" }
                                th style="width:120px" { "工序代码" }
                                th { "工序名称" }
                                th style="width:80px" { "是否必经" }
                                th { "备注" }
                            }
                        }
                        tbody {
                            @for step in steps {
                                tr {
                                    td class="font-mono tabular-nums" { (step.step_order) }
                                    td class="font-mono tabular-nums" { (step.process_code) }
                                    td { (step.process_name.as_deref().unwrap_or(&step.process_code)) }
                                    td {
                                        @if step.is_required {
                                            span class="inline-flex items-center gap-[5px] rounded-full text-[12px] font-medium whitespace-nowrap bg-[#fff8eb] text-[#d46b08]" { "必经" }
                                        } @else {
                                            span class="inline-flex items-center gap-[5px] rounded-full text-[12px] font-medium whitespace-nowrap bg-surface text-text-muted" { "选检" }
                                        }
                                    }
                                    td { (step.remark.as_deref().unwrap_or("—")) }
                                }
                            }
                        }
                    }
                }
            }

            // ── 关联BOM ──
            div class="bg-white border border-border-soft rounded p-5 routing-bom-card" style="margin-top:var(--space-5)"
                hx-select=".routing-bom-card" hx-target=".routing-bom-card" hx-swap="outerHTML"
                hx-push-url="true" {
                div class="flex items-center justify-between text-sm font-semibold mb-4 pb-2 border-b border-border-soft" { "关联BOM" }
                (bom_table_fragment(routing.id, boms))
            }
        }
    }
}
fn bom_table_fragment(routing_id: i64, boms: &abt_core::shared::types::PaginatedResult<BomRouting>) -> Markup {
    let base_path = RoutingDetailPath { id: routing_id }.to_string();
    html! {
        @if boms.items.is_empty() {
            div class="text-center p-6 text-text-muted text-sm" { "暂无关联BOM" }
        } @else {
            table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:text-left [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none]" {
                thead {
                    tr {
                        th style="width:60px" { "ID" }
                        th { "产品编码" }
                        th style="width:160px" { "关联时间" }
                    }
                }
                tbody {
                    @for bom in &boms.items {
                        tr {
                            td class="font-mono tabular-nums" { (bom.id) }
                            td class="font-mono tabular-nums" { (bom.product_code) }
                            td {
                                @if let Some(dt) = bom.created_at {
                                    (dt.format("%Y-%m-%d %H:%M"))
                                } @else {
                                    "—"
                                }
                            }
                        }
                    }
                }
            }
            (htmx_pagination(&base_path, boms.total, boms.page, boms.total_pages, ".routing-bom-card", "outerHTML"))
        }
    }
}
// ── Helpers ──
