use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;
use serde::Deserialize;

use abt_core::master_data::price::ProductPriceService;
use abt_core::master_data::price::model::{PriceQuery, PriceType};
use abt_core::master_data::product::ProductService;
use abt_core::shared::identity::UserService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::layout::page::admin_page;
use crate::routes::product::{PriceHistoryDetailPath, PriceHistoryListPath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// ── Query Params ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct PriceHistoryQueryParams {
    #[serde(default, deserialize_with = "empty_as_none")]
    pub keyword: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub date_from: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub date_to: Option<String>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

// ── Enriched Row (owned) ──

#[allow(dead_code)]
pub struct PriceHistoryRow {
    pub log_id: i64,
    pub product_id: i64,
    pub product_code: String,
    pub product_name: String,
    pub price_type: PriceType,
    pub old_price: Option<Decimal>,
    pub new_price: Decimal,
    pub operator_name: String,
    pub remark: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ── Handlers ──

#[require_permission("PRODUCT", "read")]
pub async fn get_price_history_list(
    _path: PriceHistoryListPath,
    ctx: RequestContext,
    Query(params): Query<PriceHistoryQueryParams>,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    let (rows, total, page, total_pages) = fetch_enriched_rows(&state, &service_ctx, &mut conn, &params).await?;


    let content = price_history_page(&rows, total, page, total_pages, &params);
    let page_html = admin_page(
        is_htmx,
        "价格变更记录",
        &claims,
        "md",
        PriceHistoryListPath::PATH,
        "主数据管理",
        Some("价格变更记录"),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

// ── Data Fetching ──

async fn fetch_enriched_rows(
    state: &crate::state::AppState,
    service_ctx: &abt_core::shared::types::ServiceContext,
    conn: abt_core::shared::types::PgExecutor<'_>,
    params: &PriceHistoryQueryParams,
) -> crate::errors::Result<(Vec<PriceHistoryRow>, u64, u32, u32)> {
    let price_svc = state.product_price_service();
    let product_svc = state.product_service();
    let user_svc = state.user_service();
    let date_from = params.date_from.as_deref()
        .and_then(|s| s.parse::<chrono::NaiveDate>().ok())
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc());
    let date_to = params.date_to.as_deref()
        .and_then(|s| s.parse::<chrono::NaiveDate>().ok())
        .and_then(|d| d.succ_opt().and_then(|d| d.and_hms_opt(0, 0, 0)))
        .map(|d| d.and_utc());
    let query = PriceQuery {
        product_id: None,
        price_type: None,
        keyword: params.keyword.clone(),
        date_from,
        date_to,
    };
    let page_num = params.page.unwrap_or(1);
    let result = price_svc.list_price_history(service_ctx, conn, query, PageParams::new(page_num, 20)).await?;

    let total = result.total;
    let page = result.page;
    let total_pages = result.total_pages;

    // Collect unique product IDs and operator IDs
    let product_ids: Vec<i64> = result.items.iter().map(|e| e.product_id).collect();
    let operator_ids: Vec<i64> = result.items.iter()
        .filter_map(|e| e.operator_id)
        .collect();

    // Batch fetch products
    let product_map: HashMap<i64, (String, String)> = if product_ids.is_empty() {
        HashMap::new()
    } else {
        product_svc.get_by_ids(service_ctx, conn, product_ids)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|p| (p.product_id, (p.product_code, p.pdt_name)))
            .collect()
    };

    // Batch fetch users
    let user_map: HashMap<i64, String> = if operator_ids.is_empty() {
        HashMap::new()
    } else {
        user_svc.get_users_by_ids(service_ctx, conn, operator_ids)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|u| (u.user.user_id, u.user.display_name.unwrap_or(u.user.username)))
            .collect()
    };

    // Build enriched rows
    let rows: Vec<PriceHistoryRow> = result.items.into_iter().map(|entry| {
        let (product_code, product_name) = product_map
            .get(&entry.product_id)
            .cloned()
            .unwrap_or(("—".into(), "—".into()));
        let operator_name = entry.operator_id
            .and_then(|id| user_map.get(&id).cloned())
            .unwrap_or("—".into());
        PriceHistoryRow {
            log_id: entry.log_id,
            product_id: entry.product_id,
            product_code,
            product_name,
            price_type: entry.price_type,
            old_price: entry.old_price,
            new_price: entry.new_price,
            operator_name,
            remark: entry.remark,
            created_at: entry.created_at,
        }
    }).collect();

    Ok((rows, total, page, total_pages))
}

// ── Page ──

fn price_history_page(rows: &[PriceHistoryRow], total: u64, page: u32, total_pages: u32, params: &PriceHistoryQueryParams) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="page-header" {
                h1 class="page-title" { "价格变更记录" }
            }

            // ── Stats Row ──
            div class="customer-stats" {
                div class="stat-card" {
                    div class="stat-icon blue" { (icon::currency_icon("w-5 h-5")) }
                    div {
                        div class="stat-value" { (total) }
                        div class="stat-label" { "总变更次数" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon green" { (icon::trending_up_icon("w-5 h-5")) }
                    div {
                        div class="stat-value" { "—" }
                        div class="stat-label" { "平均涨幅" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon orange" { (icon::clock_icon("w-5 h-5")) }
                    div {
                        div class="stat-value" { "—" }
                        div class="stat-label" { "本月变更" }
                    }
                }
                div class="stat-card" {
                    div class="stat-icon red" { (icon::lock_icon("w-5 h-5")) }
                    div {
                        div class="stat-value" { "—" }
                        div class="stat-label" { "涉及产品数" }
                    }
                }
            }
            // ── Filter Bar + Table ──
            div class="customer-list-panel" {
                // ── Filter Bar ──
                form id="filter-form" class="filter-bar filter-form"
                    hx-get=(PriceHistoryListPath::PATH)
                    hx-trigger="change,keyup changed delay:300ms from:.search-input"
                    hx-target=".data-card"
                    hx-select=".data-card"
                    hx-swap="outerHTML"
                    hx-include="#filter-form"
                hx-push-url="true" {
                    div class="search-wrap" {
                        (icon::search_icon("w-4 h-4"))
                        input class="search-input" type="text" name="keyword"
                            placeholder="搜索产品名称 / 编码…"
                            value=(params.keyword.as_deref().unwrap_or(""));
                    }
                    input class="search-input" type="date" name="date_from"
                        style="width:150px;padding-left:12px"
                        value=(params.date_from.as_deref().unwrap_or(""))
                        title="开始日期";
                    span style="color:var(--muted);font-size:13px;line-height:36px" { "至" }
                    input class="search-input" type="date" name="date_to"
                        style="width:150px;padding-left:12px"
                        value=(params.date_to.as_deref().unwrap_or(""))
                        title="结束日期";
                    a href=(PriceHistoryListPath::PATH) class="btn btn-default" style="height:36px;text-decoration:none" { "重置" }
                }
                // ── Data Table ──
                (data_card(rows, total, page, total_pages))
            }

            // ── Detail Drawer Overlay ──
            div class="detail-overlay" id="detail-drawer"
                onclick="hsBackdropClose(this,event,'open')" {
                div class="detail-drawer" onclick="event.stopPropagation()" {
                    div class="detail-head" {
                        h2 { "变更详情" }
                        button class="detail-close" onclick="hsRemove(null,'#detail-drawer','open')" {
                            (icon::x_icon("w-4.5 h-4.5"))
                        }
                    }
                    div class="detail-body" id="detail-body" {
                    }
                }
            }
        }
    }
}

fn data_card(rows: &[PriceHistoryRow], total: u64, page: u32, total_pages: u32) -> Markup {
    html! {
        div class="data-card" {
            div class="data-card-scroll" {
                table class="data-table" style="width:100%;table-layout:fixed" {
                    thead {
                        tr {
                            th style="width:40px" { "#" }
                            th style="width:120px" { "产品编码" }
                            th style="width:22%" { "产品名称" }
                            th style="width:90px" class="num-right" { "原价格" }
                            th style="width:90px" class="num-right" { "新价格" }
                            th style="width:70px" class="num-right" { "变动" }
                            th style="width:60px" { "操作人" }
                            th style="width:110px" { "变更时间" }
                            th { "备注" }
                            th style="width:70px" { "操作" }
                        }
                    }
                    tbody {
                        @for (i, row) in rows.iter().enumerate() {
                            (price_history_row(i, row))
                        }
                        @if rows.is_empty() {
                            tr {
                                td colspan="10" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无价格变更记录"
                                }
                            }
                        }
                    }
                }
            }
            (pagination(PriceHistoryListPath::PATH, "", total, page, total_pages))
        }
    }
}
fn price_history_row(index: usize, row: &PriceHistoryRow) -> Markup {
    let old_str = row.old_price.map(|p| format!("{:.4}", p)).unwrap_or_else(|| "—".into());
    let new_str = format!("{:.4}", row.new_price);
    let (pct, is_up) = match row.old_price {
        Some(old) if !old.is_zero() => {
            let change = (row.new_price - old) / old * Decimal::from(100);
            let up = row.new_price >= old;
            (if change >= Decimal::ZERO { format!("+{:.1}%", change) } else { format!("{:.1}%", change) }, up)
        }
        _ => ("—".into(), true),
    };
    let tag_class = if is_up { "change-tag up" } else { "change-tag down" };
    let detail_path = PriceHistoryDetailPath { log_id: row.log_id };
    html! {
        tr style="cursor:pointer"
            hx-get=(detail_path.to_string())
            hx-target="#detail-body"
            hx-swap="innerHTML"
            hx-on::after-request="hsAdd(null,'#detail-drawer','open')" {
            td style="color:var(--muted)" { (index + 1) }
            td class="mono" { (row.product_code) }
            td style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap" title=(row.product_name) {
                a href="#" class="product-link" onclick="event.preventDefault()" { (row.product_name) }
            }
            td class="num-right" style="color:var(--muted)" { "¥ " (old_str) }
            td class="num-right" { strong { "¥ " (new_str) } }
            td class="num-right" {
                span class=(tag_class) { (pct) }
            }
            td { (row.operator_name) }
            td style="color:var(--muted);font-size:13px" { (row.created_at.format("%Y-%m-%d %H:%M")) }
            td style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap" title=(row.remark) {
                @if row.remark.is_empty() {
                    span style="color:var(--muted)" { "—" }
                } @else {
                    (row.remark)
                }
            }
            td {
                button class="btn btn-default" style="padding:4px 10px;font-size:12px"
                    onclick="event.stopPropagation()"
                    hx-get=(detail_path.to_string())
                    hx-target="#detail-body"
                    hx-swap="innerHTML"
                    onclick="halt(event)" hx-on::after-request="hsAdd(null,'#detail-drawer','open')" { "详情" }
            }
        }
    }
}
// ── Detail Drawer (HTMX) ──
#[require_permission("PRODUCT", "read")]
pub async fn get_price_history_detail(
    path: PriceHistoryDetailPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let price_svc = state.product_price_service();
    let product_svc = state.product_service();
    let user_svc = state.user_service();
    // Fetch all history entries and find the one matching log_id
    let query = PriceQuery { product_id: None, price_type: None, keyword: None, date_from: None, date_to: None };
    let result = price_svc.list_price_history(&service_ctx, &mut conn, query, PageParams::new(1, 1000)).await?;
    let entry = result.items.into_iter().find(|e| e.log_id == path.log_id)
        .ok_or_else(|| abt_core::shared::types::DomainError::NotFound("记录不存在".into()))?;
    // Enrich
    let products = product_svc.get_by_ids(&service_ctx, &mut conn, vec![entry.product_id]).await.unwrap_or_default();
    let (product_code, product_name) = products.first()
        .map(|p| (p.product_code.clone(), p.pdt_name.clone()))
        .unwrap_or(("—".into(), "—".into()));
    let operator_name = match entry.operator_id {
        Some(id) => {
            match user_svc.get_users_by_ids(&service_ctx, &mut conn, vec![id]).await {
                Ok(users) => users.into_iter().next()
                    .map(|u| u.user.display_name.unwrap_or(u.user.username))
                    .unwrap_or("—".into()),
                Err(_) => "—".into(),
            }
        }
        None => "—".into(),
    };
    let row = PriceHistoryRow {
        log_id: entry.log_id,
        product_id: entry.product_id,
        product_code,
        product_name,
        price_type: entry.price_type,
        old_price: entry.old_price,
        new_price: entry.new_price,
        operator_name,
        remark: entry.remark,
        created_at: entry.created_at,
    };
    Ok(Html(detail_content(&row).into_string()))
}
fn detail_content(row: &PriceHistoryRow) -> Markup {
    let old_str = row.old_price.map(|p| format!("¥ {:.4}", p)).unwrap_or_else(|| "—".into());
    let new_str = format!("¥ {:.4}", row.new_price);
    let (pct, is_up) = match row.old_price {
        Some(old) if !old.is_zero() => {
            let change = (row.new_price - old) / old * Decimal::from(100);
            let up = row.new_price >= old;
            (if change >= Decimal::ZERO { format!("+{:.1}%", change) } else { format!("{:.1}%", change) }, up)
        }
        _ => ("—".into(), true),
    };
    let tag_class = if is_up { "change-tag up" } else { "change-tag down" };
    html! {
        // ── 产品信息 ──
        div class="detail-section" {
            div class="detail-section-title" {
                (icon::box_icon("w-4 h-4"))
                "产品信息"
            }
            div class="detail-info-grid" {
                div class="detail-info-item" {
                    label { "产品名称" }
                    span { (row.product_name) }
                }
                div class="detail-info-item" {
                    label { "产品编码" }
                    span style="font-family:var(--font-mono)" { (row.product_code) }
                }
                div class="detail-info-item" {
                    label { "价格类型" }
                    span { (price_type_label(row.price_type)) }
                }
                div class="detail-info-item" {
                    label { "操作人" }
                    span { (row.operator_name) }
                }
            }
        }
        // ── 价格变动 ──
        div class="detail-section" {
            div class="detail-section-title" {
                (icon::currency_icon("w-4 h-4"))
                "价格变动"
            }
            div class="detail-price-box" {
                div class="detail-price-old" {
                    div class="label" { "原价格" }
                    div class="val" { (old_str) }
                }
                div class="detail-price-arrow" {
                    (icon::arrow_right_icon("w-6 h-6"))
                }
                div class="detail-price-new" {
                    div class="label" { "新价格" }
                    div class="val" { (new_str) }
                }
                div style="margin-left:auto" {
                    span class=(tag_class) style="font-size:14px;padding:4px 12px" { (pct) }
                }
            }
        }
        // ── 调价说明 ──
        div class="detail-section" {
            div class="detail-section-title" {
                (icon::comment_icon("w-4 h-4"))
                "调价说明"
            }
            div class="detail-remark-box" {
                @if row.remark.is_empty() { "—" } @else { (row.remark) }
            }
        }
        // ── 变更时间 ──
        div class="detail-section" {
            div class="detail-section-title" {
                (icon::clock_icon("w-4 h-4"))
                "变更时间"
            }
            div style="font-size:15px;color:var(--fg);font-weight:500" {
                (row.created_at.format("%Y-%m-%d %H:%M"))
            }
        }
    }
}
fn price_type_label(pt: PriceType) -> &'static str {
    match pt {
        PriceType::Purchase => "采购价",
        PriceType::Sales => "销售价",
        PriceType::StandardCost => "标准成本",
    }
}