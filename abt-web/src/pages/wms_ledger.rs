use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use chrono::NaiveDate;
use maud::{html, Markup};
use serde::Deserialize;
use std::collections::HashMap;

use abt_core::master_data::product::ProductService;
use abt_core::shared::types::pagination::PageParams;
use abt_core::shared::types::{PgExecutor, ServiceContext};
use abt_core::wms::cycle_count::{CycleCount, CycleCountFilter, CycleCountService};
use abt_core::wms::enums::{CycleCountStatus, PickingStatus, PickingType};
use abt_core::wms::picking::{PickingFilter, PickingService, StockPicking, StockPickingItem};
use abt_core::wms::warehouse::{WarehouseFilter, WarehouseService};

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::components::tabs::{status_tabs_with_oob, TabItem};
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::shipping::ShippingDetailPath;
use crate::routes::wms_ledger::{LedgerItemRowsPath, LedgerPath};
use crate::state::AppState;
use crate::utils::{resolve_customer_names, RequestContext};
use abt_macros::require_permission;

const PAGE_SIZE: u32 = 20;

/// 单据台账查询参数。`type` 选单据类型；`status` 二级状态筛选；
/// keyword 模糊单号；date_from/date_to 计划/盘点日期范围。
#[derive(Debug, Deserialize, Default, Clone)]
pub struct LedgerQuery {
    /// arrival / outbound / transfer / requisition / cycle-count（缺省 arrival）
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub keyword: Option<String>,
    #[serde(default)]
    pub date_from: Option<String>,
    #[serde(default)]
    pub date_to: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
}

impl LedgerQuery {
    fn type_slug(&self) -> &str {
        match self.r#type.as_deref() {
            Some("outbound") | Some("transfer") | Some("requisition") | Some("cycle-count") => {
                self.r#type.as_deref().unwrap()
            }
            _ => "arrival",
        }
    }
    fn keyword_opt(&self) -> Option<String> {
        self.keyword
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
    }
}

// ── 类型 / 状态 映射 ──

/// 类型 slug → picking_types（None 表示盘点，走 cycle_count）
fn picking_types_for(slug: &str) -> Option<Vec<PickingType>> {
    match slug {
        "arrival" => Some(vec![PickingType::IncomingPurchase, PickingType::IncomingWorkOrder]),
        "outbound" => Some(vec![PickingType::OutgoingSales]),
        "transfer" => Some(vec![PickingType::InternalTransfer]),
        "requisition" => Some(vec![PickingType::InternalIssue]),
        _ => None,
    }
}

fn parse_picking_status(s: Option<&str>) -> Option<PickingStatus> {
    match s? {
        "draft" => Some(PickingStatus::Draft),
        "confirmed" => Some(PickingStatus::Confirmed),
        "done" => Some(PickingStatus::Done),
        "cancelled" => Some(PickingStatus::Cancelled),
        _ => None,
    }
}

fn parse_cc_status(s: Option<&str>) -> Option<CycleCountStatus> {
    match s? {
        "draft" => Some(CycleCountStatus::Draft),
        "counting" => Some(CycleCountStatus::Counting),
        "completed" => Some(CycleCountStatus::Completed),
        "adjusted" => Some(CycleCountStatus::Adjusted),
        "pending_review" => Some(CycleCountStatus::PendingReview),
        "cancelled" => Some(CycleCountStatus::Cancelled),
        _ => None,
    }
}

fn parse_date(s: Option<&str>) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s?.trim(), "%Y-%m-%d").ok()
}

fn type_label(slug: &str) -> &'static str {
    match slug {
        "arrival" => "收货",
        "outbound" => "出库",
        "transfer" => "调拨",
        "requisition" => "领料",
        "cycle-count" => "盘点",
        _ => "收货",
    }
}

// ── Handler ──

/// 单据台账唯一 GET：单端点 list（整页 / htmx 片段，由 admin_page(is_htmx) 切换）。
/// 类型 tab 决定走 picking_service（前 4 类，按 picking_types）或 cycle_count_service（盘点）。
#[require_permission("INVENTORY", "read")]
pub async fn get_ledger_list(
    _path: LedgerPath,
    Query(q): Query<LedgerQuery>,
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

    let type_slug = q.type_slug().to_string();
    let page = q.page.unwrap_or(1).max(1);
    let card =
        render_ledger_card(&state, &service_ctx, &mut conn, &q, &type_slug, page).await?;

    let content = if is_htmx {
        card
    } else {
        html! {
            div class="flex items-center justify-between mb-4 flex-wrap gap-4" {
                div {
                    h1 class="text-xl font-bold text-fg tracking-tight" { "单据台账" }
                    p class="text-sm text-muted mt-1" { "按类型 / 状态 / 单号 / 日期 检索全部 WMS 单据" }
                }
            }
            (card)
        }
    };

    let page_html = admin_page(
        is_htmx,
        "单据台账",
        &claims,
        "inventory",
        LedgerPath::PATH,
        "库存管理",
        Some("单据台账"),
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

/// 行内展开：按需加载某个作业单据的明细行（Issue #225）。
/// 返回单个 `<tr class="row-detail">`，前端 `hx-swap="afterend"` 注入到该单据行之后。
/// 失败时渲染同结构「加载失败」行（带同 id，收起可正常移除），避免裸文本泄进表格。
#[require_permission("INVENTORY", "read")]
pub async fn get_ledger_items(
    path: LedgerItemRowsPath,
    Query(q): Query<LedgerQuery>,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let colspan = picking_colspan(q.type_slug());
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    let html = match state
        .picking_service()
        .list_items(&service_ctx, &mut conn, path.id)
        .await
    {
        Ok(items) => {
            let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
            let (codes, names, specs, units) = if product_ids.is_empty() {
                (
                    HashMap::new(),
                    HashMap::new(),
                    HashMap::new(),
                    HashMap::new(),
                )
            } else {
                let products = state
                    .product_service()
                    .get_by_ids(&service_ctx, &mut conn, product_ids)
                    .await
                    .unwrap_or_default();
                (
                    products
                        .iter()
                        .map(|p| (p.product_id, p.product_code.clone()))
                        .collect(),
                    products
                        .iter()
                        .map(|p| (p.product_id, p.pdt_name.clone()))
                        .collect(),
                    products
                        .iter()
                        .map(|p| (p.product_id, p.meta.specification.clone()))
                        .collect(),
                    products
                        .iter()
                        .map(|p| (p.product_id, p.unit.clone()))
                        .collect(),
                )
            };
            render_picking_items_detail(&items, path.id, &codes, &names, &specs, &units, colspan)
                .into_string()
        }
        Err(_) => render_items_error_row(path.id, colspan).into_string(),
    };
    Ok(Html(html))
}

async fn render_ledger_card(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    q: &LedgerQuery,
    type_slug: &str,
    page: u32,
) -> Result<Markup> {
    let type_tabs = type_tab_items();

    let warehouses = state
        .warehouse_service()
        .list(ctx, db, WarehouseFilter::default(), 1, 200)
        .await
        .map(|r| r.items)
        .unwrap_or_default();
    let wh_map: HashMap<i64, String> = warehouses
        .iter()
        .map(|w| (w.id, w.name.clone()))
        .collect();

    let body = if type_slug == "cycle-count" {
        let filter = CycleCountFilter {
            doc_number: q.keyword_opt(),
            status: parse_cc_status(q.status.as_deref()),
            date_from: parse_date(q.date_from.as_deref()),
            date_to: parse_date(q.date_to.as_deref()),
            ..Default::default()
        };
        let result = state
            .cycle_count_service()
            .list(ctx, db, filter, page, PAGE_SIZE)
            .await?;
        let html = html! {
            (render_cycle_count_table(&result.items, &wh_map))
            (maybe_pagination(result.total, result.page, result.total_pages))
        };
        html
    } else {
        let picking_types = picking_types_for(type_slug).unwrap_or_default();
        let filter = PickingFilter {
            doc_number: q.keyword_opt(),
            picking_types: Some(picking_types),
            status: parse_picking_status(q.status.as_deref()),
            date_from: parse_date(q.date_from.as_deref()),
            date_to: parse_date(q.date_to.as_deref()),
            ..Default::default()
        };
        let result = state
            .picking_service()
            .list(ctx, db, filter, PageParams::new(page, PAGE_SIZE))
            .await?;
        // 仅出库需客户名；其他类型用仓库名/工单号
        let customer_names = if type_slug == "outbound" {
            resolve_customer_names(
                &state.customer_service(),
                ctx,
                db,
                result.items.iter().filter_map(|i| i.partner_id),
            )
            .await
        } else {
            HashMap::new()
        };
        let html = html! {
            (render_picking_table(&result.items, type_slug, &customer_names, &wh_map))
            (maybe_pagination(result.total, result.page, result.total_pages))
        };
        html
    };

    Ok(html! {
        div id="ledger-card" class="bg-bg border border-border-soft rounded-lg shadow-card overflow-hidden" {
            // card 头（类型图标 + 标题 + 描述），对齐 MES render_card_shell 范式
            (ledger_card_head(type_slug))
            // 类型 tab（5）；状态走过滤栏 select（对齐 Odoo filter 范式，select 语义更轻）
            (status_tabs_with_oob(
                LedgerPath::PATH, "#ledger-card", "#ledger-filter", "",
                &type_tabs, type_slug, "type",
            ))
            (render_ledger_filter(q, type_slug))
            div class="p-4" {
                (body)
            }
        }
    })
}

/// card 头：类型图标 + 标题 + 描述，对齐 MES `render_card_shell` 的 card 头范式。随 type_slug 切换。
fn ledger_card_head(type_slug: &str) -> Markup {
    let (title, icon_mkp, desc): (&str, Markup, &str) = match type_slug {
        "arrival" => ("收货", icon::truck_icon("w-[15px] h-[15px]"), "采购 PO / 生产工单 收货入库单据"),
        "outbound" => ("出库", icon::upload_icon("w-[15px] h-[15px]"), "销售订单 发货出库单据"),
        "transfer" => ("调拨", icon::arrow_left_right_icon("w-[15px] h-[15px]"), "仓间库存调拨单据"),
        "requisition" => ("领料", icon::clipboard_list_icon("w-[15px] h-[15px]"), "生产工单 领料发料单据"),
        "cycle-count" => ("盘点", icon::clipboard_document_icon("w-[15px] h-[15px]"), "库存盘点 审批调整单据"),
        _ => ("收货", icon::truck_icon("w-[15px] h-[15px]"), "收货单据"),
    };
    html! {
        div class="flex items-center gap-3 px-5 py-3 border-b border-border-soft" {
            div class="relative w-7 h-7 rounded-md grid place-items-center bg-surface text-fg-2 shrink-0" {
                (icon_mkp)
            }
            span class="font-semibold text-fg shrink-0" { (title) }
            span class="text-xs text-muted font-mono flex-1 min-w-0 truncate" { (desc) }
        }
    }
}

/// 类型 tab（5）：收货 / 出库 / 调拨 / 领料 / 盘点
fn type_tab_items() -> [TabItem; 5] {
    [
        ("arrival", "收货"),
        ("outbound", "出库"),
        ("transfer", "调拨"),
        ("requisition", "领料"),
        ("cycle-count", "盘点"),
    ]
    .map(|(v, l)| TabItem {
        value: v.into(),
        label: l,
        count: None,
    })
}

/// 状态 tab：picking 4 态 / cycle_count 6 态
fn status_tab_items(type_slug: &str) -> Vec<TabItem> {
    static PICKING: &[(&str, &str)] = &[
        ("", "全部"),
        ("draft", "草稿"),
        ("confirmed", "已确认"),
        ("done", "已完成"),
        ("cancelled", "已取消"),
    ];
    static CC: &[(&str, &str)] = &[
        ("", "全部"),
        ("draft", "草稿"),
        ("counting", "盘点中"),
        ("completed", "已完成"),
        ("adjusted", "已调整"),
        ("pending_review", "待审批"),
        ("cancelled", "已取消"),
    ];
    let pairs = if type_slug == "cycle-count" {
        CC
    } else {
        PICKING
    };
    pairs
        .iter()
        .map(|&(v, l)| TabItem {
            value: v.into(),
            label: l,
            count: None,
        })
        .collect()
}

/// 过滤栏：单号搜索（防抖）+ 状态 select（按类型动态）+ 日期范围。
/// hidden type 携带当前 tab；status 由 select 提交（搜索/切状态时不丢上下文）。
fn render_ledger_filter(q: &LedgerQuery, type_slug: &str) -> Markup {
    let kw = q.keyword.as_deref().unwrap_or("");
    let df = q.date_from.as_deref().unwrap_or("");
    let dt = q.date_to.as_deref().unwrap_or("");
    let active_status = q.status.as_deref().unwrap_or("");
    html! {
        form id="ledger-filter"
            class="flex items-center gap-3 flex-wrap px-4 py-3 border-b border-border-soft"
            hx-get=(LedgerPath::PATH)
            hx-trigger="change, keyup changed delay:300ms from:.ledger-search"
            hx-target="#ledger-card"
            hx-select="#ledger-card"
            hx-swap="outerHTML"
            hx-include="#ledger-filter" {
            input type="hidden" name="type" value=(type_slug);
            div class="relative" {
                (icon::search_icon("w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"));
                input class="ledger-search w-[220px] pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                    type="text" name="keyword" placeholder="搜索单号"
                    value=(kw);
            }
            // 状态 select（picking 4 态 / 盘点 6 态，按当前类型动态）
            select name="status"
                class="px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent cursor-pointer" {
                @for opt in status_tab_items(type_slug) {
                    option value=(opt.value) selected[opt.value.as_str() == active_status] { (opt.label) }
                }
            }
            input class="px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                type="date" name="date_from" value=(df);
            span class="text-muted text-sm" { "~" }
            input class="px-2 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                type="date" name="date_to" value=(dt);
        }
    }
}

// ── 表格渲染 ──

const TH: &str = "text-left text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft";

fn th(label: &str) -> Markup {
    html! { th class=(TH) { (label) } }
}

fn empty_state(msg: &str) -> Markup {
    html! { div class="mt-2 p-4 text-center text-sm text-muted bg-surface rounded-md" { (msg) } }
}

fn maybe_pagination(total: u64, page: u32, total_pages: u32) -> Markup {
    if total_pages > 1 {
        html! {
            div class="mt-3" {
                (pagination(
                    LedgerPath::PATH, "#ledger-card", "#ledger-filter",
                    total, page, total_pages,
                ))
            }
        }
    } else {
        html! {}
    }
}

fn wh_name(map: &HashMap<i64, String>, id: Option<i64>) -> String {
    id.and_then(|i| map.get(&i).cloned())
        .unwrap_or_else(|| "—".into())
}

fn picking_status_badge(s: PickingStatus) -> (&'static str, &'static str) {
    match s {
        PickingStatus::Draft => ("草稿", "bg-surface text-muted"),
        PickingStatus::Confirmed => ("已确认", "bg-accent-bg text-accent"),
        PickingStatus::Done => ("已完成", "bg-success-bg text-success"),
        PickingStatus::Cancelled => ("已取消", "bg-danger-bg text-danger"),
    }
}

fn cc_status_badge(s: CycleCountStatus) -> (&'static str, &'static str) {
    match s {
        CycleCountStatus::Draft => ("草稿", "bg-surface text-muted"),
        CycleCountStatus::Counting => ("盘点中", "bg-warn-bg text-warn"),
        CycleCountStatus::Completed => ("已完成", "bg-accent-bg text-accent"),
        CycleCountStatus::Adjusted => ("已调整", "bg-accent-bg text-accent"),
        CycleCountStatus::Cancelled => ("已取消", "bg-danger-bg text-danger"),
        CycleCountStatus::PendingReview => ("待审批", "bg-warn-bg text-warn"),
    }
}

fn arrival_source_label(p: &StockPicking) -> String {
    match p.picking_type {
        PickingType::IncomingPurchase => "采购收货".into(),
        PickingType::IncomingWorkOrder => "生产入库".into(),
        _ => p.source_type.clone(),
    }
}

fn render_picking_table(
    items: &[StockPicking],
    type_slug: &str,
    customer_names: &HashMap<i64, String>,
    wh_map: &HashMap<i64, String>,
) -> Markup {
    if items.is_empty() {
        return empty_state(&format!("暂无{}单", type_label(type_slug)));
    }
    html! {
        table class="w-full border-collapse" {
            thead {
                tr {
                    (th(""))
                    (th("单号"))
                    @if type_slug == "arrival" {
                        (th("来源"))
                        (th("仓库"))
                    } @else if type_slug == "outbound" {
                        (th("客户"))
                    } @else if type_slug == "transfer" {
                        (th("来源仓 → 目标仓"))
                    } @else if type_slug == "requisition" {
                        (th("工单"))
                        (th("仓库"))
                    }
                    (th("日期"))
                    (th("状态"))
                }
            }
            tbody {
                @for p in items {
                    (render_picking_row(p, type_slug, customer_names, wh_map))
                }
            }
        }
    }
}

fn render_picking_row(
    p: &StockPicking,
    type_slug: &str,
    customer_names: &HashMap<i64, String>,
    wh_map: &HashMap<i64, String>,
) -> Markup {
    let (st_label, st_cls) = picking_status_badge(p.status);
    let date = p
        .scheduled_date
        .map(|d| d.format("%m-%d").to_string())
        .unwrap_or_else(|| "—".into());
    // 出库单号跳发货 detail；其他类型单号纯文本（独立 detail 后续补）
    let no_cell: Markup = if type_slug == "outbound" {
        let url = ShippingDetailPath { id: p.id }.to_string();
        html! {
            td class="py-3 px-3" {
                a class="text-sm font-mono text-accent font-semibold no-underline hover:underline cursor-pointer"
                    href=(url) { (p.doc_number) }
            }
        }
    } else {
        html! { td class="py-3 px-3 text-sm font-mono text-accent font-semibold" { (p.doc_number) } }
    };
    html! {
        tr class="border-b border-border-soft last:border-b-0 [&.open_.ledger-chev]:rotate-90" {
            (ledger_expand_cell(p.id, type_slug))
            (no_cell)
            @if type_slug == "arrival" {
                td class="py-3 px-3 text-sm text-fg-2" { (arrival_source_label(p)) }
                td class="py-3 px-3 text-sm text-fg-2" { (wh_name(wh_map, p.to_warehouse_id)) }
            } @else if type_slug == "outbound" {
                td class="py-3 px-3 text-sm text-fg-2" {
                    (p.partner_id
                        .and_then(|c| customer_names.get(&c).map(|s| s.as_str()))
                        .unwrap_or("—"))
                }
            } @else if type_slug == "transfer" {
                td class="py-3 px-3 text-sm text-fg-2" {
                    (wh_name(wh_map, p.from_warehouse_id)) " → " (wh_name(wh_map, p.to_warehouse_id))
                }
            } @else if type_slug == "requisition" {
                td class="py-3 px-3 text-sm font-mono text-fg-2" { "WO-" (p.work_order_id.unwrap_or(0)) }
                td class="py-3 px-3 text-sm text-fg-2" { (wh_name(wh_map, p.from_warehouse_id)) }
            }
            td class="py-3 px-3 text-sm font-mono text-muted" { (date) }
            td class="py-3 px-3" {
                span class=(format!(
                    "inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium {st_cls}"
                )) { (st_label) }
            }
        }
    }
}

fn render_cycle_count_table(items: &[CycleCount], wh_map: &HashMap<i64, String>) -> Markup {
    if items.is_empty() {
        return empty_state("暂无盘点单");
    }
    html! {
        table class="w-full border-collapse" {
            thead {
                tr {
                    (th("单号"))
                    (th("仓库"))
                    (th("盘点日期"))
                    (th("状态"))
                }
            }
            tbody {
                @for c in items {
                    (render_cycle_count_row(c, wh_map))
                }
            }
        }
    }
}

fn render_cycle_count_row(c: &CycleCount, wh_map: &HashMap<i64, String>) -> Markup {
    let (st_label, st_cls) = cc_status_badge(c.status);
    html! {
        tr class="border-b border-border-soft last:border-b-0" {
            td class="py-3 px-3 text-sm font-mono text-accent font-semibold" { (c.doc_number) }
            td class="py-3 px-3 text-sm text-fg-2" {
                (wh_map.get(&c.warehouse_id).map(|s| s.as_str()).unwrap_or("—"))
            }
            td class="py-3 px-3 text-sm font-mono text-muted" { (c.count_date.format("%m-%d")) }
            td class="py-3 px-3" {
                span class=(format!(
                    "inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium {st_cls}"
                )) { (st_label) }
            }
        }
    }
}

// =============================================================================
// 行内展开明细（Issue #225）：行首 chevron → 懒加载 `<tr class="row-detail">` 明细表
// =============================================================================

/// 行首展开单元格：chevron 按钮。点击 → toggle 行 `.open`：
/// - 展开（变 .open）：派发自定义 `loadItem` 事件 → htmx 按 `hx-trigger="loadItem"` 拉明细，
///   `afterend` 注入 `<tr id="picking-items-{id}">` 到本行之后；
/// - 收起（去 .open）：直接 `remove #picking-items-{id}`，不派发事件 → 不重复请求。
///
/// 用自定义事件触发（而非默认 click），彻底避免「重复点击产生重复明细行 / 请求竞态」。
fn ledger_expand_cell(picking_id: i64, type_slug: &str) -> Markup {
    let items_url = format!(
        "{}?type={}",
        LedgerItemRowsPath { id: picking_id },
        type_slug
    );
    // toggle 后按「现在是否 .open」分流：展开→拉取，收起→移除明细行（id 精确匹配，无 query 歧义）
    let hs = format!(
        "on click\n  toggle .open on closest <tr/>\n  if (closest <tr/> matches .open)\n    trigger loadItem on me\n  else\n    remove #picking-items-{}\n  end",
        picking_id
    );
    html! {
        td class="py-3 px-2 w-8" {
            button type="button"
                class="ledger-expand inline-flex items-center justify-center w-6 h-6 text-muted hover:text-fg hover:bg-surface rounded-sm transition-colors cursor-pointer"
                title="查看明细"
                hx-get=(items_url)
                hx-target="closest <tr/>"
                hx-swap="afterend"
                hx-trigger="loadItem"
                _=(hs) {
                (icon::chevron_right_icon("w-4 h-4 ledger-chev transition-transform duration-150"))
            }
        }
    }
}

/// 各类型 picking 行总列数（含行首 chevron 列），供明细 `<td colspan>` 跨满整行。
fn picking_colspan(type_slug: &str) -> u32 {
    match type_slug {
        "outbound" | "transfer" => 5,
        _ => 6, // arrival / requisition / 兜底
    }
}

/// 数量格式化：去尾零（10.00→10、10.50→10.5、100→100）。
fn fmt_qty(d: rust_decimal::Decimal) -> String {
    d.normalize().to_string()
}

/// 单据明细渲染：`<tr class="row-detail"><td colspan> ...明细表... </td></tr>`。
/// id 与展开按钮的 `remove #picking-items-{id}` 对应，确保收起可移除。
fn render_picking_items_detail(
    items: &[StockPickingItem],
    picking_id: i64,
    codes: &HashMap<i64, String>,
    names: &HashMap<i64, String>,
    specs: &HashMap<i64, String>,
    units: &HashMap<i64, String>,
    colspan: u32,
) -> Markup {
    let ith = "text-left text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft";
    let ith_r = "text-right text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft";
    html! {
        tr id=(format!("picking-items-{}", picking_id)) class="row-detail" {
            td colspan=(colspan) class="p-0 border-none bg-surface-raised" {
                div class="p-4 border-t border-dashed border-border-soft" {
                    @if items.is_empty() {
                        div class="text-center text-sm text-muted py-4" { "暂无明细" }
                    } @else {
                        table class="w-full text-sm border-collapse" {
                            thead {
                                tr {
                                    th class=(ith) { "商品编码" }
                                    th class=(ith) { "商品名称" }
                                    th class=(ith) { "规格" }
                                    th class=(ith) { "单位" }
                                    th class=(ith_r) { "计划数量" }
                                    th class=(ith_r) { "实际数量" }
                                    th class=(ith) { "批次号" }
                                }
                            }
                            tbody {
                                @for it in items {
                                    tr class="border-b border-border-soft last:border-b-0" {
                                        td class="py-2 px-3 font-mono text-fg" {
                                            (codes.get(&it.product_id).map(|s| s.as_str()).unwrap_or("—"))
                                        }
                                        td class="py-2 px-3 text-fg" {
                                            (names.get(&it.product_id).map(|s| s.as_str()).unwrap_or("—"))
                                        }
                                        td class="py-2 px-3 text-fg-2" {
                                            (specs.get(&it.product_id).map(|s| s.as_str()).unwrap_or("—"))
                                        }
                                        td class="py-2 px-3 text-muted" {
                                            (units.get(&it.product_id).map(|s| s.as_str()).unwrap_or("—"))
                                        }
                                        td class="py-2 px-3 text-right font-mono text-muted" { (fmt_qty(it.qty_requested)) }
                                        td class="py-2 px-3 text-right font-mono text-fg font-semibold" { (fmt_qty(it.qty_done)) }
                                        td class="py-2 px-3 font-mono text-fg-2" {
                                            (it.batch_no.as_deref().unwrap_or("—"))
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 明细加载失败的兜底行（带同 id，收起可正常移除）。
fn render_items_error_row(picking_id: i64, colspan: u32) -> Markup {
    html! {
        tr id=(format!("picking-items-{}", picking_id)) class="row-detail" {
            td colspan=(colspan) class="p-0 border-none" {
                div class="p-4 text-center text-sm text-danger" { "加载明细失败，请重试" }
            }
        }
    }
}
