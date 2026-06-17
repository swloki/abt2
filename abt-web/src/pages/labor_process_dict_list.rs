use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::labor_process_dict::model::*;
use abt_core::master_data::labor_process_dict::LaborProcessDictService;
use abt_core::shared::types::{DomainError, PageParams};

use crate::components::icon;
use crate::components::export_button;
use crate::components::pagination::pagination;
use crate::layout::page::admin_page;
use crate::routes::labor_process_dict::{
    ProcessDictCreatePath, ProcessDictDeletePath, ProcessDictListPath,
};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ProcessDictQueryParams {
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Form ──

#[derive(Debug, Deserialize)]
pub struct ProcessDictCreateForm {
    pub name: String,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub sort_order: Option<i32>,
}

// ── Handlers ──

#[require_permission("LABOR_PROCESS_DICT", "read")]
pub async fn get_process_dict_list(
    _path: ProcessDictListPath,
    ctx: RequestContext,
    Query(params): Query<ProcessDictQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_create = ctx.has_permission("LABOR_PROCESS_DICT", "create").await;
    let can_delete = ctx.has_permission("LABOR_PROCESS_DICT", "delete").await;
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;
    let svc = state.labor_process_dict_service();

    let filter = LaborProcessDictQuery {
        keyword: params.keyword.clone(),
    };
    let page = PageParams::new(params.page.unwrap_or(1), 20);

    let result = svc.list(&service_ctx, &mut conn, filter, page).await?;


    let content = process_dict_list_page(&result, &params, can_create, can_delete);
    let page_html = admin_page(
        is_htmx,
        "工序字典管理",
        &claims,
        "md",
        ProcessDictListPath::PATH,
        "主数据管理",
        Some("工序字典管理"),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("LABOR_PROCESS_DICT", "create")]
pub async fn get_process_dict_create(
    _path: ProcessDictCreatePath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { claims, .. } = ctx;


    let content = process_dict_form_page(None);
    let page_html = admin_page(
        is_htmx,
        "新建工序",
        &claims,
        "md",
        ProcessDictCreatePath::PATH,
        "主数据管理",
        Some("新建工序"),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("LABOR_PROCESS_DICT", "create")]
pub async fn post_process_dict_create(
    _path: ProcessDictCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ProcessDictCreateForm>,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    if form.name.trim().is_empty() {
        return Err(DomainError::validation("工序名称不能为空").into());
    }

    let req = CreateLaborProcessDictReq {
        name: form.name.trim().to_string(),
        description: form.description.filter(|d| !d.trim().is_empty()),
        sort_order: form.sort_order.unwrap_or(0),
    };

    let svc = state.labor_process_dict_service();
    let _id = svc.create(&service_ctx, &mut conn, req).await?;

    Ok((
        [("HX-Redirect", ProcessDictListPath::PATH)],
        Html(String::new()),
    ))
}

#[require_permission("LABOR_PROCESS_DICT", "delete")]
pub async fn delete_process_dict(
    path: ProcessDictDeletePath,
    ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.labor_process_dict_service();

    svc.delete(&service_ctx, &mut conn, path.id).await?;

    Ok((
        [("HX-Trigger", "{\"processDictDeleted\":true}")],
        Html(String::new()),
    ))
}

// ── Components ──

fn process_dict_list_page(
    result: &abt_core::shared::types::PaginatedResult<LaborProcessDict>,
    params: &ProcessDictQueryParams,
    can_create: bool,
    can_delete: bool,
) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                h1 class="text-xl font-bold text-fg tracking-tight" { "工序字典管理" }
                div class="flex gap-3" {
                    (export_button::export_button("导出工序字典", "labor-process-dict"))
                    @if can_create {
                        a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" href=(ProcessDictCreatePath::PATH) {
                            (icon::plus_icon("w-4 h-4"))
                            "新建工序"
                        }
                    }
                }
            }

            // ── Tabs + Filter + Data Table (HTMX panel) ──
            (process_dict_table_fragment(result, params, can_delete))

        }
    }
}

fn process_dict_table_fragment(
    result: &abt_core::shared::types::PaginatedResult<LaborProcessDict>,
    params: &ProcessDictQueryParams,
    can_delete: bool,
) -> Markup {
    let query = build_query_string(params);
    let total_count = result.total;

    html! {
        div class="customer-list-panel" {
            // ── Filter Bar ──
            div class="flex items-center gap-3 mb-5 flex-wrap" {
                div class="inline-flex items-center gap-1 px-3 py-1 bg-surface rounded-full text-xs text-text-muted font-medium" { "全部 " span class="font-bold text-fg" { (total_count) } }
                div class="relative flex-1 max-w-xs" {
                    (icon::search_icon("w-4 h-4"))
                    input class="w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="text" name="keyword"
                        placeholder="搜索工序编码或名称…"
                        value=(params.keyword.as_deref().unwrap_or(""))
                        hx-get=(ProcessDictListPath::PATH)
                        hx-trigger="keyup changed delay:300ms, processDictDeleted from:body"
                        hx-sync="this:replace"
                        hx-target="#process-dict-table"
                        hx-select="#process-dict-table"
                        hx-swap="outerHTML";
                }
            }

            // ── Data Table ──
            div id="process-dict-table" class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" {
                div class="overflow-x-auto" {
                    table class="data-table w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:text-left [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none] [&_tbody_tr:hover_.row-actions]:opacity-100" {
                        thead {
                            tr {
                                th style="width:80px" { "编码" }
                                th { "工序名称" }
                                th { "描述" }
                                th style="width:80px;text-align:center" { "排序" }
                                th style="width:120px" { "创建时间" }
                                th style="width:80px" { "操作" }
                            }
                        }
                        tbody {
                            @for item in &result.items {
                                (process_dict_row(item, can_delete))
                            }
                            @if result.items.is_empty() {
                                tr {
                                    td colspan="6" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无工序字典数据"
                                    }
                                }
                            }
                        }
                    }
                }
                (pagination(ProcessDictListPath::PATH, &query, result.total, result.page, result.total_pages))
            }
        }
    }
}

fn process_dict_row(item: &LaborProcessDict, can_delete: bool) -> Markup {
    let delete_path = ProcessDictDeletePath { id: item.id };

    html! {
        tr {
            td {
                code style="font-size:12px;background:var(--surface);padding:2px 6px;border-radius:var(--radius-sm)" {
                    (&item.code)
                }
            }
            td {
                strong { (&item.name) }
            }
            td {
                @if let Some(ref desc) = item.description {
                    @if !desc.is_empty() {
                        (desc)
                    } @else {
                        span style="color:var(--muted)" { "—" }
                    }
                } @else {
                    span style="color:var(--muted)" { "—" }
                }
            }
            td style="text-align:center" {
                (item.sort_order)
            }
            td {
                @if let Some(ref created) = item.created_at {
                    (created.format("%Y-%m-%d"))
                } @else {
                    "—"
                }
            }
            td onclick="event.stopPropagation()" {
                div class="row-actions flex items-center gap-1 justify-end opacity-0 transition-opacity duration-150 [&_a]:w-[28px] [&_a]:h-[28px] [&_a]:grid [&_a]:place-items-center [&_a]:rounded-sm [&_a]:cursor-pointer [&_a]:bg-surface [&_a]:hover:bg-accent-bg [&_svg]:w-3.5 [&_svg]:h-3.5" {
                    @if can_delete {
                        button type="button" class="w-[28px] h-[28px] border-none bg-surface rounded-sm grid place-items-center cursor-pointer text-danger" title="删除"
                            hx-post=(delete_path)
                            hx-confirm=(format!("删除后无法恢复，确定要删除工序「{}」（{}）吗？", item.name, item.code))
                            hx-swap="none" {
                            (icon::trash_icon("w-4 h-4"))
                        }
                    }
                }
            }
        }
    }
}

fn process_dict_form_page(_existing: Option<&LaborProcessDict>) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                a class="inline-flex items-center gap-2 text-sm text-text-muted hover:text-accent transition-colors duration-150" href=(ProcessDictListPath::PATH) {
                    (icon::arrow_left_icon("w-4 h-4"))
                    "返回工序字典"
                }
                h1 class="text-xl font-bold text-fg tracking-tight" { "新建工序" }
            }

            form hx-post=(ProcessDictCreatePath::PATH)
                  hx-swap="none" {
                // ── Section: 基本信息 ──
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]" style="margin-bottom:var(--space-4)" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "基本信息" }
                    div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                        div class="form-field" {
                            label { "工序名称 " span style="color:var(--danger)" { "*" } }
                            input type="text" name="name" required placeholder="请输入工序名称，如：车削、铣削" {}
                        }
                        div class="form-field" {
                            label { "工序编码" }
                            input type="text" value="自动生成" readonly
                                style="background:var(--surface);color:var(--muted)" {}
                        }
                        div class="form-field" {
                            label { "排序" }
                            input type="number" name="sort_order" value="0" min="0"
                                placeholder="数字越小排越前" {}
                        }
                        div class="form-field field-full" {
                            label { "描述" }
                            textarea name="description" placeholder="请输入描述信息…"
                                style="width:100%;min-height:80px;resize:vertical" {}
                        }
                    }
                }

                // ── Action Bar ──
                div class="flex items-center justify-end gap-3 pt-4 border-t border-border-soft" {
                    a class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs" href=(ProcessDictListPath::PATH) { "取消" }
                    button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]" { "保存工序" }
                }
            }
        }
    }
}

// ── Helpers ──

fn build_query_string(params: &ProcessDictQueryParams) -> String {
    let mut q = vec![];
    if let Some(ref kw) = params.keyword {
        q.push(format!("keyword={kw}"));
    }
    q.join("&")
}
