//! 供应商搜索控件（纯 HTMX、自给自足、只依赖自身 URL）
//!
//! - **一个 URL = 一个组件**：`/api/supplier-search` 根据参数渲染**整个组件**
//!   （隐藏 input + 只读显示框 + 弹出面板 + 搜索框 + 结果区）。宿主页面 SSR
//!   调用 `supplier_search_field()` 直接渲染，客户端 HTMX 加载同一 URL 也能
//!   拿到完整组件——两端共享同一段 `render_component` 代码，行为完全一致。
//! - **搜索**：面板内搜索框 `hx-get` 自身 URL（带 q）→ 渲染完整组件 →
//!   `hx-select=#results_id` + `hx-swap=outerHTML` 由 HTMX 自动切出结果区替换。
//!   同一个 URL、同一段渲染，服务器不做"部分渲染"特判。
//! - **选中**：极简 hyperscript，运行时读 `my @data-name`（不拼字符串，兼容带引号的名字），
//!   写入隐藏 input 并 `trigger change` → 宿主表单按供应商名筛选。
//! - **面板开关 / 清除**：hyperscript（纯前端 UI 状态，项目惯例）。
//!
//! 默认（空查询）加载 5 个；输入关键词时放宽到 50。

use axum::routing::get;
use axum::Router;
use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::fms::cash_journal::model::CounterpartyResult;
use abt_core::fms::cash_journal::CashJournalService;
use abt_core::fms::enums::CounterpartyType;

use crate::errors::Result;
use crate::utils::RequestContext;

use super::icon;

// ── 自身端点（组件唯一依赖的 URL）──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/api/supplier-search")]
pub struct SupplierSearchPath;

#[derive(Debug, Deserialize)]
pub struct SupplierSearchParams {
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub q: Option<String>,
    pub input_id: String,
    pub display_id: String,
    pub panel_id: String,
    pub results_id: String,
    /// 隐藏 input 的 name（完整渲染需要；搜索请求可不传，客户端只取结果区不受影响）。
    #[serde(default)]
    pub name: Option<String>,
    /// 占位符（完整渲染需要；搜索请求可不传）。
    #[serde(default)]
    pub placeholder: Option<String>,
    /// 当前选中值（隐藏 input 的回显）；搜索请求可不传，端点用空串渲染——
    /// 客户端 `hx-select=#results_id + hx-swap=outerHTML` 只替换结果区，不影响隐藏 input。
    #[serde(default)]
    pub value: Option<String>,
}

pub fn router() -> Router<crate::state::AppState> {
    Router::new().route(SupplierSearchPath::PATH, get(search_suppliers))
}

/// HTMX: 渲染**整个**供应商搜索组件（field + 结果区）。
///
/// 客户端搜索请求通过 `hx-select=#results_id` + `hx-swap=outerHTML` 由 HTMX
/// 自动切出结果区替换——服务器始终返回完整组件，不按请求类型做特判。
///
/// 始终返回 5 条。
pub async fn search_suppliers(
    ctx: RequestContext,
    Query(p): Query<SupplierSearchParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let kw = p.q.as_deref().unwrap_or("");
    let limit: i64 = 5;
    let items: Vec<CounterpartyResult> = state
        .cash_journal_service()
        .search_counterparties(&service_ctx, &mut conn, CounterpartyType::Supplier, kw, limit)
        .await
        .unwrap_or_default();
    let value = p.value.as_deref().unwrap_or("");
    Ok(Html(
        render_component(
            &p.input_id,
            &p.display_id,
            &p.panel_id,
            &p.results_id,
            p.name.as_deref().unwrap_or(""),
            value,
            p.placeholder.as_deref().unwrap_or(""),
            Some(&items),
        )
        .into_string(),
    ))
}

// ── 完整组件渲染（SSR 与端点共用同一段代码）──

/// 渲染完整供应商搜索组件：外层 div + 隐藏 input + 只读显示框 + 弹出面板（搜索框 + 结果区）。
///
/// - **SSR**：宿主页面通过 `supplier_search_field()` 调用（`items=None`，占位"输入关键词搜索…"）。
/// - **端点**：HTMX 请求 `/api/supplier-search` 调用（`items=Some(搜索结果)`）。
///
/// 同一段渲染代码保证 SSR 与客户端行为完全一致——这是"一个 URL = 一个组件"的基础。
fn render_component(
    input_id: &str,
    display_id: &str,
    panel_id: &str,
    results_id: &str,
    name: &str,
    value: &str,
    placeholder: &str,
    items: Option<&[CounterpartyResult]>,
) -> Markup {
    let clear_id = format!("{}-clear", input_id);
    let q_id = format!("{}-q", panel_id);
    let display_empty = value.is_empty();
    // 点显示框：开关面板 + 触发搜索框加载（loadResults 由搜索框监听，统一由 input 驱动搜索）
    let open_hs = format!(
        "on click toggle .invisible on #{p} then trigger loadResults on #{q}",
        p = panel_id,
        q = q_id
    );
    // ✕ 清除：清空隐藏 input + 还原显示框 + 隐藏自己 + 触发 change（让宿主表单恢复）
    let clear_hs = format!(
        "on click halt the event \
         then put '' into #{i}'s value \
         then put '{ph}' into #{d} \
         then add .hidden to #{c} \
         then trigger change on #{i}",
        i = input_id,
        ph = placeholder,
        d = display_id,
        c = clear_id
    );

    html! {
        div class="relative w-52 min-w-[208px]"
            _=(format!("on click from elsewhere add .invisible to #{}", panel_id))
        {
            input type="hidden" name=(name) id=(input_id) value=(value) {};
            // 只读显示框（点击展开 + 加载结果）
            div class="flex items-center w-full border border-border rounded-sm bg-white cursor-pointer text-sm transition-colors duration-150 hover:border-accent"
                _=(open_hs)
            {
                span id=(display_id)
                    class=(format!(
                        "flex-1 pl-3.5 pr-1 py-1.5 truncate {}",
                        if display_empty { "text-muted" } else { "text-fg" }
                    ))
                {
                    @if display_empty { (placeholder) } @else { (value) }
                }
                // ✕ 清除按钮（始终渲染；空值时 hidden，选中后由结果点击移除 hidden）
                span id=(clear_id.as_str())
                    class=(format!(
                        "px-1 py-1.5 text-muted text-xs cursor-pointer hover:text-danger {}",
                        if display_empty { "hidden" } else { "" }
                    ))
                    _=(clear_hs)
                { "✕" }
                span class="px-2 py-1.5 text-muted text-xs pointer-events-none" { "▾" }
            }
            // 弹出面板
            div id=(panel_id)
                class="absolute left-0 top-full mt-0.5 w-72 bg-white border border-border rounded-sm shadow-[var(--shadow-card)] z-30 invisible transition-all duration-150"
                _="on click halt"
            {
                div class="flex items-center gap-2 p-2 border-b border-border-soft" {
                    (icon::search_icon("w-3.5 h-3.5 text-muted shrink-0"))
                    input
                        class="flex-1 py-1 text-sm bg-transparent text-fg outline-none min-w-0"
                        type="text"
                        id=(q_id.as_str())
                        name="q"
                        placeholder=(format!("搜索{}…", placeholder))
                        autocomplete="off"
                        hx-get=(SupplierSearchPath::PATH)
                        hx-trigger="keyup changed delay:300ms, loadResults"
                        hx-target=(format!("#{}", results_id))
                        hx-select=(format!("#{}", results_id))
                        hx-swap="outerHTML"
                        hx-vals=(vals_json(input_id, display_id, panel_id, results_id, name, placeholder).as_str())
                        _="on keyup halt on change halt on input halt"
                        hx-include="this";
                }
                // 结果区（打开面板时由 loadResults 触发 hx-get 加载；搜索时由上方 input 的 hx-get 更新）
                (results_region(results_id, input_id, display_id, panel_id, items))
            }
        }
    }
}

// ── SSR 入口（宿主页面调用一次）──

/// 渲染供应商搜索字段：隐藏 input + 只读显示框 + 弹出面板（搜索框 + 结果区）。
///
/// 宿主页面 SSR 时调用，内部委托 `render_component`（`items=None`，占位"输入关键词搜索…"）。
/// 渲染结果与端点 `/api/supplier-search` 返回的完全一致——同一段代码。
///
/// - `input_id`：隐藏 input 的 id（宿主表单字段的 name 由 `name` 指定）
/// - `display_id`：只读显示框里展示选中名称的 span 的 id
/// - `panel_id` / `results_id`：面板与结果区的 id
/// - `name`：隐藏 input 的 name（台账用 "keyword"）
/// - `value`：当前选中的供应商名（用于回显）
/// - `placeholder`：占位符，如 "供应商"
pub fn supplier_search_field(
    input_id: &str,
    display_id: &str,
    panel_id: &str,
    results_id: &str,
    name: &str,
    value: &str,
    placeholder: &str,
) -> Markup {
    render_component(input_id, display_id, panel_id, results_id, name, value, placeholder, None)
}

// ── 结果区（render_component 内部使用；端点搜索结果通过 hx-select 切出此区域）──

/// 渲染结果区 `#results_id`。
/// `items = None`：初始占位（组件首次渲染）；`Some(...)`：端点返回的搜索结果。
fn results_region(
    results_id: &str,
    input_id: &str,
    display_id: &str,
    panel_id: &str,
    items: Option<&[CounterpartyResult]>,
) -> Markup {
    let clear_id = format!("{}-clear", input_id);
    // 选中：从 my @data-name 读名称（运行时取属性，不拼字符串）→ 写隐藏 input + 显示框 + 关面板 + 触发 change
    let select_hs = format!(
        "on click set #{i}'s value to my @data-name \
         then put my @data-name into #{d} \
         then remove .hidden from #{c} \
         then add .invisible to #{p} \
         then trigger change on #{i}",
        i = input_id,
        d = display_id,
        c = clear_id,
        p = panel_id
    );
    html! {
        div id=(results_id) {
            @match items {
                None => {
                    div class="px-3 py-4 text-xs text-muted text-center" { "输入关键词搜索…" }
                }
                Some(list) if list.is_empty() => {
                    div class="px-3 py-4 text-xs text-muted text-center" { "未找到匹配供应商" }
                }
                Some(list) => {
                    @for item in list {
                        div data-name=(item.name.as_str())
                            class="px-3 py-2 text-sm cursor-pointer hover:bg-accent-bg border-b border-border-soft last:border-b-0"
                            _=(select_hs.as_str())
                        {
                            div class="font-medium text-fg" { (item.name.as_str()) }
                            div class="text-xs text-muted" { (item.code.as_str()) }
                        }
                    }
                }
            }
        }
    }
}

/// 把组件实例的不变参数编成 hx-vals 的 JSON（供搜索框 hx-get 携带）。
///
/// `q` 由 `hx-include="this"` 带上（搜索框自身），`value` 不需要——
/// 客户端 `hx-select + hx-swap=outerHTML` 只替换结果区，隐藏 input 由 hyperscript 管理。
fn vals_json(
    input_id: &str,
    display_id: &str,
    panel_id: &str,
    results_id: &str,
    name: &str,
    placeholder: &str,
) -> String {
    format!(
        "{{\"input_id\":\"{i}\",\"display_id\":\"{d}\",\"panel_id\":\"{p}\",\"results_id\":\"{r}\",\"name\":\"{n}\",\"placeholder\":\"{ph}\"}}",
        i = input_id,
        d = display_id,
        p = panel_id,
        r = results_id,
        n = name,
        ph = placeholder
    )
}
