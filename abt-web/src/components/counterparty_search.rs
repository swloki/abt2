//! 往来方搜索控件（搜索型 select：只读框 + 弹出搜索面板）
//!
//! 完全自洽：使用方只需调 counterparty_search_input()（FMS 台账：按名称 keyword 过滤）
//! 或 counterparty_search_field()（配置化：可按 id 存储 + 选中后级联 HTMX 重搜），
//! 所有交互（展开/搜索/选中/清除/关闭/提交）都封装在组件内。
//!
//! 关键设计：panel 用 `invisible`（visibility:hidden）而非 `hidden`（display:none），
//! 这样 HTMX 的 `load` trigger 能在页面加载时正常初始化和触发。

use axum::Router;
use axum::extract::Query;
use axum::response::Html;
use axum::routing::get;
use axum_extra::routing::TypedPath;
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::fms::cash_journal::CashJournalService;
use abt_core::fms::cash_journal::model::CounterpartyResult;
use abt_core::fms::enums::CounterpartyType;

use crate::errors::Result;
use crate::utils::RequestContext;

use super::icon;

// ── 通用往来方搜索端点（控件自带 router，参数化 input/display/panel id + store_id）──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/api/counterparties/search")]
pub struct CounterpartySearchPath;

#[derive(Debug, Deserialize)]
pub struct CounterpartySearchParams {
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub q: Option<String>,
    /// `supplier` | `customer`（缺省 supplier）
    #[serde(default)]
    pub kind: Option<String>,
    pub input_id: String,
    pub display_id: String,
    pub panel_id: String,
    /// true→选中存 id（render_counterparty_results_by_id），否则存 name
    #[serde(default)]
    pub store_id: Option<bool>,
}

pub fn router() -> Router<crate::state::AppState> {
    Router::new().route(
        CounterpartySearchPath::PATH,
        get(search_counterparties_endpoint),
    )
}

/// HTMX/JS: 按 keyword 搜索供应商/客户，渲染结果列表（ids 与 store_id 由 query 参数指定）
pub async fn search_counterparties_endpoint(
    ctx: RequestContext,
    Query(params): Query<CounterpartySearchParams>,
) -> Result<Html<String>> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let kw = params.q.as_deref().unwrap_or("");
    let kind = match params.kind.as_deref() {
        Some("customer") => CounterpartyType::Customer,
        _ => CounterpartyType::Supplier,
    };
    let items: Vec<CounterpartyResult> = state
        .cash_journal_service()
        .search_counterparties(&service_ctx, &mut conn, kind, kw, 50)
        .await
        .unwrap_or_default();
    let markup = if params.store_id.unwrap_or(false) {
        render_counterparty_results_by_id(
            &items,
            &params.input_id,
            &params.display_id,
            &params.panel_id,
            "未找到匹配结果",
        )
    } else {
        render_counterparty_results(
            &items,
            &params.input_id,
            &params.display_id,
            &params.panel_id,
            "未找到匹配结果",
        )
    };
    Ok(Html(markup.into_string()))
}

// ── 配置化字段（PO 弹窗等需要按 id 存储 + 选中级联重搜的场景）──

/// 选中一项后，在隐藏 input 上挂载的级联 HTMX（触发外部列表重搜，如采购订单）。
pub struct CpCascade {
    pub hx_get: String,
    pub hx_trigger: String,
    pub hx_target: String,
    pub hx_include: String,
    pub hx_swap: String,
}

/// 搜索型 select 字段的完整配置。
pub struct CpSearchField<'a> {
    pub input_id: &'a str,
    pub display_id: &'a str,
    pub panel_id: &'a str,
    /// 搜索端点 URL，可带固定 query（cpSearch 追加 `&q=` 或 `?q=`）。
    pub search_path: &'a str,
    pub placeholder: &'a str,
    /// 隐藏 input 当前值（store_id=true 时为 id 字符串，否则为 name）。
    pub hidden_value: &'a str,
    /// 只读框展示文本；空串时显示 placeholder。
    pub display_value: &'a str,
    /// 隐藏 input 的 name（FMS 台账为 "keyword"，PO 弹窗为 "supplier_id"）。
    pub name: &'a str,
    /// true→选中存 `item.id`，false→存 `item.name`。
    pub store_id: bool,
    /// 外层容器宽度类（默认 "w-52 min-w-[208px]"，PO 弹窗用 "w-full"）。
    pub width_class: &'a str,
    /// 选中后隐藏 input 上的级联 HTMX；None 则仅 trigger change。
    pub cascade: Option<&'a CpCascade>,
}

/// 渲染搜索型 select 字段（配置化版本）。
pub fn counterparty_search_field(f: &CpSearchField) -> Markup {
    let q_id = format!("{}-q", f.panel_id);
    let list_id = format!("{}-list", f.panel_id);
    let clear_id = format!("{}-clear", f.input_id);
    let display_empty = f.display_value.is_empty();
    let open_hs = format!(
        "on click toggle .invisible on #{p} then call cpSearch('{s}', '{qi}', '{li}')",
        p = f.panel_id,
        s = f.search_path,
        qi = q_id,
        li = list_id
    );
    let clear_hs = format!(
        "on click halt the event \
 then put '' into #{ii}'s value \
 then put '{ph}' into #{di}'s innerHTML \
 then add .text-muted to #{di} \
 then add .hidden to #{ci} \
 then trigger change on #{ii}",
        ii = f.input_id,
        ph = f.placeholder,
        di = f.display_id,
        ci = clear_id
    );

    html! {
        div class=(format!("relative {}", f.width_class))
            _=(format!("on click from elsewhere add .invisible to #{}", f.panel_id))
        {
            // 隐藏 input：有级联时挂 hx-*（选中触发外部列表重搜），否则裸 hidden
            @if let Some(c) = f.cascade {
                input
                    type="hidden"
                    name=(f.name)
                    id=(f.input_id)
                    value=(f.hidden_value)
                    hx-get=(c.hx_get.as_str())
                    hx-trigger=(c.hx_trigger.as_str())
                    hx-target=(c.hx_target.as_str())
                    hx-include=(c.hx_include.as_str())
                    hx-swap=(c.hx_swap.as_str()) {}
            } @else {
                input type="hidden" name=(f.name) id=(f.input_id) value=(f.hidden_value) {}
            }
            // 只读显示框（点击展开 + 加载列表）
            div class="flex items-center w-full border border-border rounded-sm bg-white cursor-pointer text-sm transition-colors duration-150 hover:border-accent"
                _=(open_hs)
            {
                div id=(f.display_id)
                    class=({
                        format!(
                            "flex-1 pl-3.5 pr-1 py-1.5 truncate {}",
                            if display_empty { "text-muted" } else { "text-fg" },
                        )
                    })
                {
                    @if display_empty { (f.placeholder) } @else { (f.display_value) }
                }
                // ✕ 清除按钮（始终渲染；空值时 hidden，选中后由结果点击处理移除 hidden）
                span
                    id=(clear_id)
                    class=({
                        format!(
                            "cp-clear px-1 py-1.5 text-muted text-xs cursor-pointer hover:text-danger {}",
                            if f.hidden_value.is_empty() { "hidden" } else { "" },
                        )
                    })
                    _=(clear_hs)
                { "✕" }
                span class="px-2 py-1.5 text-muted text-xs pointer-events-none" { "▾" }
            }
            // 弹出面板
            div id=(f.panel_id)
                class="absolute left-0 top-full mt-0.5 w-72 bg-white border border-border rounded-sm shadow-[var(--shadow-card)] z-30 invisible transition-all duration-150"
                _="on click halt"
            {
                div class="flex items-center gap-2 p-2 border-b border-border-soft" {
                    (icon::search_icon("w-3.5 h-3.5 text-muted shrink-0"))
                    input
                        class="flex-1 py-1 text-sm bg-transparent text-fg outline-none min-w-0"
                        type="text"
                        id=(q_id)
                        placeholder=(format!("搜索{}…", f.placeholder))
                        autocomplete="off"
                        _=({
                            format!(
                                "on keyup debounced at 200ms call cpSearch('{s}', me.id, '{li}')",
                                s = f.search_path,
                                li = list_id,
                            )
                        });
                }
                div id=(list_id) class="max-h-[240px] overflow-y-auto" {}
            }
        }
    }
}

/// FMS 台账用的便捷封装：按 name 存、name="keyword"、固定宽度、无级联。
pub fn counterparty_search_input(
    input_id: &str,
    display_id: &str,
    panel_id: &str,
    search_path: &str,
    placeholder: &str,
    value: &str,
) -> Markup {
    counterparty_search_field(&CpSearchField {
        input_id,
        display_id,
        panel_id,
        search_path,
        placeholder,
        hidden_value: value,
        display_value: value,
        name: "keyword",
        store_id: false,
        width_class: "w-52 min-w-[208px]",
        cascade: None,
    })
}

pub fn render_counterparty_results(
    items: &[CounterpartyResult],
    input_id: &str,
    display_id: &str,
    panel_id: &str,
    empty_msg: &str,
) -> Markup {
    render_results_inner(items, input_id, display_id, panel_id, empty_msg, false)
}

/// 选中存 id（隐藏 input 值=item.id，只读框展示=item.name）—— PO 弹窗按 supplier_id 过滤用。
pub fn render_counterparty_results_by_id(
    items: &[CounterpartyResult],
    input_id: &str,
    display_id: &str,
    panel_id: &str,
    empty_msg: &str,
) -> Markup {
    render_results_inner(items, input_id, display_id, panel_id, empty_msg, true)
}

fn render_results_inner(
    items: &[CounterpartyResult],
    input_id: &str,
    display_id: &str,
    panel_id: &str,
    empty_msg: &str,
    store_id: bool,
) -> Markup {
    let clear_id = format!("{}-clear", input_id);
    // store_id=true：隐藏 input 存 id，只读框展示 name；否则两者都存 name
    html! {
        @if items.is_empty() {
            div class="px-3 py-4 text-xs text-muted text-center" { (empty_msg) }
        } @else {
            @for item in items {
                div class="px-3 py-2 text-sm cursor-pointer hover:bg-accent-bg border-b border-border-soft last:border-b-0"
                    _=({
                        format!(
                            "on click put {hv} into #{ii}'s value \
     then put '{name}' into #{di}'s innerHTML \
     then remove .text-muted from #{di} \
     then remove .hidden from #{ci} \
     then add .invisible to #{pi} \
     then trigger change on #{ii}",
                            hv = if store_id {
                                format!("'{}'", item.id)
                            } else {
                                format!("'{}'", item.name.replace('\'', "\\'"))
                            },
                            name = item.name.replace('\'', "\\'"),
                            ii = input_id,
                            di = display_id,
                            ci = clear_id,
                            pi = panel_id,
                        )
                    })
                {
                    div class="font-medium text-fg" { (item.name) }
                    div class="text-xs text-muted" { (item.code) }
                }
            }
        }
    }
}
