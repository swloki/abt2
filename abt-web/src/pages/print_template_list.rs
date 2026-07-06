use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::print_template::{
    PrintTemplate, PrintTemplateQuery, PrintTemplateService,
};

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::layout::page::admin_page;
use crate::routes::print_template::*;
use crate::utils::RequestContext;

use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct PrintTemplateQueryParams {
    pub document_type: Option<String>,
    pub keyword: Option<String>,
    pub page: Option<u32>,
}

// ── Handlers ──

#[require_permission("USER", "read")]
pub async fn list(
    _path: PrintTemplateListPath,
    ctx: RequestContext,
    Query(params): Query<PrintTemplateQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext {
        mut conn, state, claims, ..
    } = ctx;

    let svc = state.print_template_service();
    let filter = PrintTemplateQuery {
        document_type: params.document_type.clone(),
        keyword: params.keyword.clone(),
    };
    let page = abt_core::shared::types::PageParams {
        page: params.page.unwrap_or(1),
        page_size: 20,
    };
    let result = svc.list(&mut conn, filter, page).await?;

    let content = list_page(&result.items, result.total, result.page, result.page_size, &params);
    let page_html = admin_page(
        is_htmx,
        "打印模板",
        &claims,
        "system",
        PrintTemplateListPath::PATH,
        "系统管理",
        Some("打印模板"),
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("USER", "delete")]
pub async fn delete(
    path: PrintTemplateDeletePath,
    ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext {
        mut conn, state, service_ctx, ..
    } = ctx;

    state
        .print_template_service()
        .delete(&service_ctx, &mut conn, path.id)
        .await?;

    Ok([("HX-Redirect", PrintTemplateListPath::PATH.to_string())])
}

#[require_permission("USER", "create")]
pub async fn set_default(
    path: PrintTemplateSetDefaultPath,
    ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext {
        mut conn, state, service_ctx, ..
    } = ctx;

    state
        .print_template_service()
        .set_default(&service_ctx, &mut conn, path.id)
        .await?;

    Ok([("HX-Redirect", PrintTemplateListPath::PATH.to_string())])
}

// ── UI ──

const DOCUMENT_TYPES: &[(&str, &str)] = &[
    ("delivery_note", "送货单"),
    ("quotation", "报价单"),
    ("sales_order", "销售订单"),
    ("purchase_order", "采购订单"),
];

fn doc_type_label(dt: &str) -> &str {
    DOCUMENT_TYPES
        .iter()
        .find(|(k, _)| *k == dt)
        .map(|(_, label)| *label)
        .unwrap_or(dt)
}

fn list_page(
    items: &[PrintTemplate],
    total: u64,
    page: u32,
    page_size: u32,
    _params: &PrintTemplateQueryParams,
) -> Markup {
    let total_pages = ((total as f64) / (page_size as f64)).ceil() as u32;
    let total_pages = total_pages.max(1);

    html! {
        div class="flex flex-col gap-5" {
            // ── Header ──
            div class="flex items-center justify-between" {
                div class="flex items-center gap-4" {
                    h1 class="text-xl font-bold text-fg tracking-tight" { "打印模板" }
                    a
                        class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                        href=(PrintTemplateCreatePath::PATH)
                    { (icon::plus_icon("w-4 h-4")) "新建模板" }
                }
            }

            // ── Table ──
            @if items.is_empty() {
                div class="data-card text-center py-12 text-muted text-sm" {
                    p { "暂无打印模板" }
                    p class="mt-2" { "点击「新建模板」创建第一个打印模板" }
                }
            } @else {
                div class="data-card overflow-hidden !p-0" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "模板名称" }
                                th { "单据类型" }
                                th { "默认" }
                                th { "创建时间" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            @for item in items {
                                tr {
                                    td class="font-medium" { (item.name) }
                                    td {
                                        span class="inline-flex items-center px-2 py-0.5 rounded text-xs bg-surface border border-border" {
                                            (doc_type_label(&item.document_type))
                                        }
                                    }
                                    td {
                                        @if item.is_default {
                                            span class="text-success text-xs font-medium" { "默认" }
                                        } @else {
                                            span class="text-muted text-xs" { "—" }
                                        }
                                    }
                                    td class="text-sm text-muted" {
                                        (item.created_at.format("%Y-%m-%d %H:%M").to_string())
                                    }
                                    td {
                                        div class="flex items-center gap-2" {
                                            a
                                                class="inline-flex items-center gap-1 text-xs text-accent hover:text-accent-hover cursor-pointer no-underline"
                                                href=(PrintTemplateEditPath { id: item.id }.to_string())
                                            { (icon::edit_icon("w-3.5 h-3.5")) "编辑" }
                                            @if !item.is_default {
                                                form
                                                    hx-post=(PrintTemplateSetDefaultPath { id: item.id }.to_string())
                                                    hx-confirm="设为默认模板？"
                                                {
                                                    button
                                                        class="inline-flex items-center gap-1 text-xs text-fg-2 hover:text-accent cursor-pointer border-none bg-transparent"
                                                { (icon::check_circle_icon("w-3.5 h-3.5")) "设默认" }
                                                }
                                            }
                                            form
                                                hx-post=(PrintTemplateDeletePath { id: item.id }.to_string())
                                                hx-confirm="确定删除此模板？"
                                            {
                                                button
                                                    class="inline-flex items-center gap-1 text-xs text-red-500 hover:text-red-600 cursor-pointer border-none bg-transparent"
                                                { (icon::trash_icon("w-3.5 h-3.5")) "删除" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                @if total_pages > 1 {
                    (pagination(PrintTemplateListPath::PATH, "#data-card", "#filter-form", total, page, total_pages))
                }
            }
        }
    }
}
