use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use rust_decimal::Decimal;

use abt_core::wms::warehouse::model::*;
use abt_core::wms::warehouse::WarehouseService;

use crate::components::icon;
use crate::errors::{Result, error_page};
use crate::layout::page::admin_page;
use crate::pages::wms_bin_list::{bin_status_class, bin_status_label};
use crate::routes::wms_bin::{BinDetailPath, BinListPath};
use crate::utils::RequestContext;

use abt_macros::require_permission;

// ── Handlers ──

#[require_permission("LOCATION", "read")]
pub async fn get_bin_detail(
    path: BinDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.warehouse_service();

    let bww = match svc.get_bin_with_warehouse(&service_ctx, &mut conn, path.id).await {
        Ok(bww) => bww,
        Err(e) => {
            if matches!(e, abt_core::shared::types::DomainError::NotFound(_)) {
                let content = error_page("储位未找到", &format!("储位 ID {} 不存在或已被删除", path.id));
                let page_html = admin_page(
                    is_htmx,
                    "储位未找到",
                    &claims,
                    "inventory",
                    &BinListPath.to_string(),
                    "库存管理",
                    Some("储位未找到"),
                    content, &nav_filter,                );
                return Ok(Html(page_html.into_string()));
            }
            return Err(e.into());
        }
    };
    let zones = svc.list_zones(&service_ctx, &mut conn, bww.warehouse_id).await?;
    let zone = zones.iter().find(|z| z.id == bww.bin.zone_id);
    let stats = svc.get_bin_inventory_stats(&service_ctx, &mut conn, path.id).await.ok();

    let content = bin_detail_page(&bww, zone, stats.as_ref());
    let detail_path_str = BinDetailPath { id: path.id }.to_string();
    let page_html = admin_page(
        is_htmx,
        &format!("{} - 储位详情", bww.bin.code),
        &claims,
        "inventory",
        &detail_path_str,
        "库存管理",
        Some(&bww.bin.code),
        content, &nav_filter,    );
    Ok(Html(page_html.into_string()))
}

// ── Helpers ──

fn temperature_label(req: &str) -> &str {
    match req {
        "ambient" => "常温",
        "cool" => "冷藏 (2~8°C)",
        "freeze" => "冷冻 (-18°C以下)",
        "constant" => "恒温",
        _ => "无要求",
    }
}

fn product_type_label(t: &str) -> &str {
    match t {
        "raw_material" => "原材料",
        "semi_finished" => "半成品",
        "finished" => "成品",
        "packaging" => "包材",
        "consumable" => "耗材",
        _ => t,
    }
}

fn product_type_color(t: &str) -> (&str, &str) {
    match t {
        "raw_material" => ("rgba(22,119,255,0.06)", "#1677ff"),
        "semi_finished" => ("rgba(82,196,26,0.06)", "#52c41a"),
        "finished" => ("rgba(114,46,209,0.06)", "#722ed1"),
        "packaging" => ("rgba(250,173,20,0.06)", "#d48806"),
        "consumable" => ("rgba(255,77,79,0.06)", "#ff4d4f"),
        _ => ("rgba(0,0,0,0.04)", "var(--muted)"),
    }
}

fn capacity_percent(stats: &BinInventoryStats, limit: Option<Decimal>) -> Option<Decimal> {
    limit.filter(|l| *l > Decimal::ZERO)
        .map(|l| (stats.total_quantity / l * Decimal::from(100)).min(Decimal::from(100)))
}

// ── Components ──

fn bin_detail_page(
    bww: &BinWithWarehouse,
    zone: Option<&Zone>,
    stats: Option<&BinInventoryStats>,
) -> Markup {
    let bin = &bww.bin;
    let status_label = bin_status_label(&bin.status);
    let status_class = bin_status_class(&bin.status);
    let detail_path = BinDetailPath { id: bin.id };

    let zone_name = zone.map(|z| z.name.as_str()).unwrap_or("—");
    let zone_code = zone.map(|z| z.code.as_str()).unwrap_or("—");

    let (used_qty, capacity_pct) = match stats {
        Some(s) => {
            let pct = capacity_percent(s, bin.capacity_limit);
            (format!("{:.2}", s.total_quantity), pct)
        }
        None => ("—".to_string(), None),
    };

    html! {
        div {
            // ── Back Link ──
            a class="back-link" href=(BinListPath::PATH) {
                (icon::arrow_left_icon("w-4 h-4"))
                "返回储位管理列表"
            }

            // ── Detail Header ──
            div class="detail-header" style="display:flex;align-items:flex-start;justify-content:space-between;margin-bottom:var(--space-5)" {
                div {
                    div style="display:flex;align-items:center;gap:var(--space-3)" {
                        h1 class="detail-no" style="font-size:var(--text-xl);font-weight:700;margin:0;font-family:var(--font-mono)" {
                            (bin.code)
                        }
                        span class=(format!("status-pill {status_class}")) { (status_label) }
                    }
                    div style="margin-top:var(--space-2);font-size:13px;color:var(--muted)" {
                        (bww.warehouse_name) " · " (zone_name)
                    }
                }
            }

            // ── Tabs ──
            div class="detail-tabs" style="display:flex;gap:var(--space-1);margin-bottom:var(--space-5);border-bottom:1px solid var(--border-soft);padding-bottom:0" {
                button class="detail-tab active" style="padding:var(--space-2) var(--space-4);font-size:var(--text-sm);cursor:pointer;border:none;background:none;color:var(--accent);border-bottom:2px solid var(--accent)" onclick="switchTab('info',this)" { "基本信息" }
                button class="detail-tab" style="padding:var(--space-2) var(--space-4);font-size:var(--text-sm);cursor:pointer;border:none;background:none;color:var(--muted);border-bottom:2px solid transparent" onclick="switchTab('stock',this)" { "库存明细" }
                button class="detail-tab" style="padding:var(--space-2) var(--space-4);font-size:var(--text-sm);cursor:pointer;border:none;background:none;color:var(--muted);border-bottom:2px solid transparent" onclick="switchTab('history',this)" { "操作历史" }
            }

            // ── Tab: 基本信息 ──
            div.tab-panel id="tab-info" {
                // Info card
                div class="info-card" {
                    div class="info-card-title" { "储位信息" }
                    div class="info-grid" {
                        div class="info-item" {
                            span class="info-label" { "储位编码" }
                            span class="info-value" style="font-family:var(--font-mono)" { (bin.code) }
                        }
                        div class="info-item" {
                            span class="info-label" { "储位名称" }
                            span class="info-value" { (bin.name) }
                        }
                        div class="info-item" {
                            span class="info-label" { "所属仓库" }
                            span class="info-value" { (bww.warehouse_name) }
                        }
                        div class="info-item" {
                            span class="info-label" { "所属库区" }
                            span class="info-value" { (zone_name) }
                        }
                        div class="info-item" {
                            span class="info-label" { "储位状态" }
                            span class="info-value" {
                                span class=(format!("status-pill {status_class}")) { (status_label) }
                            }
                        }
                        div class="info-item" {
                            span class="info-label" { "容量上限" }
                            span class="info-value" style="font-family:var(--font-mono)" {
                                @if let Some(cap) = &bin.capacity_limit {
                                    (format!("{:.2}", cap))
                                } @else {
                                    "—"
                                }
                            }
                        }
                        div class="info-item" {
                            span class="info-label" { "已用容量" }
                            span class="info-value" style="font-family:var(--font-mono);color:var(--warn)" { (used_qty) }
                        }
                        div class="info-item" {
                            span class="info-label" { "温控要求" }
                            span class="info-value" {
                                (bin.temperature_req.as_deref().map(temperature_label).unwrap_or("无要求"))
                            }
                        }
                        div class="info-item" {
                            span class="info-label" { "允许物料类型" }
                            span class="info-value" {
                                @if let Some(types) = &bin.allowed_product_types {
                                    @for t in types {
                                        @let (bg, fg) = product_type_color(t);
                                        span class="status-pill" style=(format!("background:{bg};color:{fg};margin-right:4px")) {
                                            (product_type_label(t))
                                        }
                                    }
                                } @else {
                                    "—"
                                }
                            }
                        }
                        div class="info-item" {
                            span class="info-label" { "创建时间" }
                            span class="info-value" style="font-family:var(--font-mono)" {
                                (bin.created_at.format("%Y-%m-%d %H:%M"))
                            }
                        }
                        div class="info-item" {
                            span class="info-label" { "最后更新" }
                            span class="info-value" style="font-family:var(--font-mono)" {
                                (bin.updated_at.format("%Y-%m-%d %H:%M"))
                            }
                        }
                    }
                }

                // Coordinates card
                div class="info-card" style="margin-top:var(--space-4)" {
                    div class="info-card-title" { "储位坐标" }
                    div style="display:flex;gap:var(--space-4);margin-top:var(--space-3)" {
                        div style="text-align:center;flex:1;background:var(--surface);border:1px solid var(--border-soft);border-radius:var(--radius-md);padding:var(--space-4)" {
                            div style="font-size:var(--text-xl);font-weight:700;font-family:var(--font-mono);color:var(--fg)" {
                                (bin.row_no.as_deref().unwrap_or("—"))
                            }
                            div style="font-size:var(--text-xs);color:var(--muted);margin-top:var(--space-1)" { "行号 (Row)" }
                        }
                        div style="text-align:center;flex:1;background:var(--surface);border:1px solid var(--border-soft);border-radius:var(--radius-md);padding:var(--space-4)" {
                            div style="font-size:var(--text-xl);font-weight:700;font-family:var(--font-mono);color:var(--fg)" {
                                (bin.column_no.as_deref().unwrap_or("—"))
                            }
                            div style="font-size:var(--text-xs);color:var(--muted);margin-top:var(--space-1)" { "列号 (Column)" }
                        }
                        div style="text-align:center;flex:1;background:var(--surface);border:1px solid var(--border-soft);border-radius:var(--radius-md);padding:var(--space-4)" {
                            div style="font-size:var(--text-xl);font-weight:700;font-family:var(--font-mono);color:var(--fg)" {
                                (bin.layer_no.as_deref().unwrap_or("—"))
                            }
                            div style="font-size:var(--text-xs);color:var(--muted);margin-top:var(--space-1)" { "层号 (Layer)" }
                        }
                        div style="text-align:center;flex:1;background:var(--surface);border:1px solid var(--border-soft);border-radius:var(--radius-md);padding:var(--space-4)" {
                            div style="font-size:var(--text-xl);font-weight:700;font-family:var(--font-mono);color:var(--fg)" {
                                @if let Some(pct) = capacity_pct {
                                    (format!("{}%", pct.round()))
                                } @else {
                                    "—"
                                }
                            }
                            div style="font-size:var(--text-xs);color:var(--muted);margin-top:var(--space-1)" { "容量使用率" }
                        }
                    }
                    @if let Some(pct) = capacity_pct {
                        div style="max-width:400px;margin-top:var(--space-4)" {
                            div style="height:8px;background:var(--border-soft);border-radius:4px;overflow:hidden" {
                                div style=(format!("width:{}%;background:var(--warn);height:100%;border-radius:4px;transition:width 0.3s", pct.round())) {}
                            }
                        }
                    }
                }
            }

            // ── Tab: 库存明细 ──
            div.tab-panel id="tab-stock" style="display:none" {
                div class="data-card" {
                    div class="data-card-scroll" {
                        table class="data-table" {
                            thead {
                                tr {
                                    th { "产品编码" }
                                    th { "产品名称" }
                                    th { "批次号" }
                                    th class="num-right" { "数量" }
                                    th class="num-right" { "单位成本" }
                                    th { "入库日期" }
                                    th { "有效期" }
                                    th { "状态" }
                                }
                            }
                            tbody {
                                tr {
                                    td colspan="8" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无库存数据"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Tab: 操作历史 ──
            div.tab-panel id="tab-history" style="display:none" {
                div class="data-card" {
                    div class="data-card-scroll" {
                        table class="data-table" {
                            thead {
                                tr {
                                    th { "时间" }
                                    th { "事务类型" }
                                    th { "关联单号" }
                                    th { "产品" }
                                    th class="num-right" { "变动数量" }
                                    th { "操作员" }
                                    th { "备注" }
                                }
                            }
                            tbody {
                                tr {
                                    td colspan="7" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无操作历史"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Tab switch script ──
            script {
                r#"
                function switchTab(tabId, btn) {
                    document.querySelectorAll('.tab-panel').forEach(function(p) {
                        p.style.display = 'none';
                    });
                    document.querySelectorAll('.detail-tab').forEach(function(t) {
                        t.style.color = 'var(--muted)';
                        t.style.borderBottomColor = 'transparent';
                    });
                    document.getElementById('tab-' + tabId).style.display = '';
                    btn.style.color = 'var(--accent)';
                    btn.style.borderBottomColor = 'var(--accent)';
                }
                "#
            }
        }
    }
}
