use axum::routing::get;
use axum::Router;
use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use super::overlay::modal_shell;
use abt_core::shared::identity::{UserService, model::UserWithRoles};

use crate::errors::Result;
use crate::state::AppState;
use crate::utils::RequestContext;

// ── Typed Path ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/api/workers/search")]
pub struct WorkerSearchPath;

// ── Search Params ──

#[derive(Debug, Deserialize)]
pub struct WorkerSearchParams {
    pub name: Option<String>,
    // fill-input 模式参数
    pub target_id: Option<String>,
    pub display_id: Option<String>,
    pub modal_id: Option<String>,
    /// 部门编码，默认生产部 SHENGCHAN
    pub department_code: Option<String>,
    // add-row 模式参数（选工人加行到报工表格）
    pub item_row_path: Option<String>,
    pub tbody_id: Option<String>,
}

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new().route(WorkerSearchPath::PATH, get(search_workers))
}

// ── Search Handler ──

pub async fn search_workers(
    ctx: RequestContext,
    Query(params): Query<WorkerSearchParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let dept_code = params.department_code.as_deref().unwrap_or("SHENGCHAN");
    let users = state
        .user_service()
        .list_users_by_departments(&service_ctx, &mut conn, &[dept_code])
        .await
        .unwrap_or_default();
    let needle = params.name.as_deref().unwrap_or("").trim();
    let filtered: Vec<&UserWithRoles> = users
        .iter()
        .filter(|u| {
            if needle.is_empty() {
                true
            } else {
                u.user.username.contains(needle)
                    || u
                        .user
                        .display_name
                        .as_deref()
                        .is_some_and(|d| d.contains(needle))
            }
        })
        .collect();
    let modal_id = params.modal_id.as_deref().unwrap_or("worker-picker-modal");
    // add-row 模式（选工人加行到表格）优先于 fill-input 模式
    if let Some(row_path) = &params.item_row_path {
        let tbody = params.tbody_id.as_deref().unwrap_or("report-workers-tbody");
        Ok(Html(worker_picker_results_for_table(&filtered, row_path, tbody, modal_id).into_string()))
    } else {
        let target = params.target_id.as_deref().unwrap_or("worker_id");
        let display = params.display_id.as_deref().unwrap_or("worker-display");
        Ok(Html(worker_picker_results(&filtered, target, display, modal_id).into_string()))
    }
}

// ── Modal Component ──

/// 报工人选择弹窗（默认生产部人员）。
///
/// 选中后：填充 hidden input（target_id=user_id）+ 显示姓名（display_id）+ 关弹窗。
pub fn worker_picker_modal(modal_id: &str, target_id: &str, display_id: &str) -> Markup {
    let close_hs = format!("on click remove .is-open from #{}", modal_id);
    modal_shell(modal_id, "z-[1100]", html! {
        div class="bg-bg rounded-xl w-[520px] max-h-[80vh] flex flex-col overflow-hidden shadow-xl" {
            // ── Header ──
            div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                h2 class="text-lg font-semibold m-0" { "选择报工人（生产部）" }
                button
                    class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg transition-colors"
                    _=(close_hs)
                { "×" }
            }
            // ── Body ──
            div class="overflow-y-auto flex-1 min-h-0 p-6" {
                div class="worker-search-bar flex gap-4 mb-4 pb-4 border-b border-border-soft" {
                    input type="hidden" name="target_id" value=(target_id);
                    input type="hidden" name="display_id" value=(display_id);
                    input type="hidden" name="modal_id" value=(modal_id);
                    input type="hidden" name="department_code" value="SHENGCHAN";
                    div class="flex-1 flex flex-col gap-1" {
                        label class="text-xs font-medium text-fg-2" { "姓名 / 账号" }
                        input
                            class="worker-search-input w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent"
                            type="text"
                            name="name"
                            placeholder="输入姓名或账号…"
                            hx-get=(WorkerSearchPath::PATH)
                            hx-trigger="keyup changed delay:300ms"
                            hx-sync="this:replace"
                            hx-target="#worker-search-results"
                            hx-swap="innerHTML"
                            hx-include=".worker-search-bar" {}
                    }
                }
                div id="worker-search-results"
                    class="max-h-[400px] overflow-y-auto"
                    hx-get=(WorkerSearchPath::PATH)
                    hx-trigger="intersect once"
                    hx-swap="innerHTML"
                    hx-include=".worker-search-bar"
                {
                    div class="flex items-center justify-center py-8 text-muted text-sm" {
                        "加载中…"
                    }
                }
            }
        }
    })
}

/// 渲染人员搜索结果（点击行填充 hidden input + 显示姓名）。
pub fn worker_picker_results(
    users: &[&UserWithRoles],
    target_id: &str,
    display_id: &str,
    modal_id: &str,
) -> Markup {
    let click_hs = format!(
        "on click set #{}'s value to my @data-uid then put my @data-uname into #{} then remove .is-open from #{} then send workerSelected to body",
        target_id, display_id, modal_id
    );
    html! {
        @if users.is_empty() {
            div class="flex flex-col items-center justify-center py-12 text-muted" {
                p class="mt-2 text-sm" { "未找到匹配的人员" }
            }
        } @else {
            div class="py-2" {
                @for u in users {
                    @let display_name = u.user.display_name.as_deref().unwrap_or(u.user.username.as_str());
                    div class="flex items-center justify-between p-3 border-b border-border-soft cursor-pointer hover:bg-accent-bg transition-colors"
                        data-uid=(u.user.user_id)
                        data-uname=(display_name)
                        _=(click_hs)
                    {
                        div class="flex-1 min-w-0" {
                            div class="text-sm font-medium text-fg" { (display_name) }
                            div class="text-xs text-muted font-mono mt-0.5" { (u.user.username.as_str()) }
                        }
                    }
                }
            }
        }
    }
}

/// 报工人选择弹窗（add-row 模式：选工人 → 加行到报工表格）。
/// `item_row_path` — 加工人的行端点（?worker_id=X），`tbody_id` — 报工表格 tbody。
pub fn worker_picker_modal_with_search(modal_id: &str, item_row_path: &str, tbody_id: &str) -> Markup {
    let close_hs = format!("on click remove .is-open from #{}", modal_id);
    modal_shell(modal_id, "z-[1200]", html! {
        div class="bg-bg rounded-xl w-[520px] max-h-[80vh] flex flex-col overflow-hidden shadow-xl" {
            div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                h2 class="text-lg font-semibold m-0" { "选择报工人（生产部）" }
                button class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg transition-colors"
                    _=(close_hs) { "×" }
            }
            div class="overflow-y-auto flex-1 min-h-0 p-6" {
                div class="worker-search-bar flex gap-4 mb-4 pb-4 border-b border-border-soft" {
                    input type="hidden" name="item_row_path" value=(item_row_path);
                    input type="hidden" name="tbody_id" value=(tbody_id);
                    input type="hidden" name="modal_id" value=(modal_id);
                    input type="hidden" name="department_code" value="SHENGCHAN";
                    div class="flex-1 flex flex-col gap-1" {
                        label class="text-xs font-medium text-fg-2" { "姓名 / 账号" }
                        input class="worker-search-input w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                            type="text" name="name" placeholder="输入姓名或账号…"
                            hx-get=(WorkerSearchPath::PATH) hx-trigger="keyup changed delay:300ms"
                            hx-sync="this:replace" hx-target="#worker-search-results" hx-swap="innerHTML"
                            hx-include=".worker-search-bar" {}
                    }
                }
                div id="worker-search-results" class="max-h-[400px] overflow-y-auto"
                    hx-get=(WorkerSearchPath::PATH) hx-trigger="intersect once" hx-swap="innerHTML"
                    hx-include=".worker-search-bar" {
                    div class="flex items-center justify-center py-8 text-muted text-sm" { "加载中…" }
                }
            }
        }
    })
}

/// 渲染人员搜索结果（点击整行 → 加到报工表格）。
pub fn worker_picker_results_for_table(
    users: &[&UserWithRoles],
    item_row_path: &str,
    tbody_id: &str,
    modal_id: &str,
) -> Markup {
    html! {
        @if users.is_empty() {
            div class="flex flex-col items-center justify-center py-12 text-muted" {
                p class="mt-2 text-sm" { "未找到匹配的人员" }
            }
        } @else {
            div class="py-2" {
                @for u in users {
                    @let display_name = u.user.display_name.as_deref().unwrap_or(u.user.username.as_str());
                    div class="flex items-center p-3 border-b border-border-soft cursor-pointer hover:bg-accent-bg transition-colors"
                        hx-get=(format!("{}?worker_id={}", item_row_path, u.user.user_id))
                        hx-target=(format!("#{}", tbody_id))
                        hx-swap="beforeend"
                        _=(format!("on 'htmx:afterRequest' remove .is-open from #{}", modal_id))
                    {
                        div class="flex-1 min-w-0" {
                            div class="text-sm font-medium text-fg" { (display_name) }
                            div class="text-xs text-muted font-mono mt-0.5" { (u.user.username.as_str()) }
                        }
                        span class="text-xs text-accent font-medium shrink-0" { "点击添加" }
                    }
                }
            }
        }
    }
}
