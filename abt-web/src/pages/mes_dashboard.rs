use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_dashboard::MesDashboardPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;
use abt_core::mes::dashboard::MesDashboardService;
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/mes")]
pub struct _MesPath;
#[require_permission("WORK_ORDER", "read")]
pub async fn get_mes_dashboard(
 _path: MesDashboardPath,
 ctx: RequestContext,
) -> Result<Html<String>> {
 let is_htmx = ctx.is_htmx();
 let nav_filter = ctx.nav_filter().await;
 let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
 let svc = state.mes_dashboard_service();
 let stats = svc.get_stats(&service_ctx, &mut conn).await?;
 let qs = svc.get_quick_entry_stats(&service_ctx, &mut conn).await?;
 let dq = svc.get_data_quality_stats(&service_ctx, &mut conn).await.unwrap_or_default();
 let recent = svc.get_recent_ops(&service_ctx, &mut conn, 5).await.unwrap_or_default();
 let content = mes_dashboard_page(&stats, &dq, &qs, &recent);
 let page_html = admin_page(
 is_htmx, "生产管理", &claims, "production",
 MesDashboardPath::PATH, "生产管理", None, content, &nav_filter,
 );
 Ok(Html(page_html.into_string()))
}

fn mes_dashboard_page(stats: &abt_core::mes::dashboard::model::DashboardStats, dq: &abt_core::mes::dashboard::model::DataQualityStats, qs: &abt_core::mes::dashboard::model::QuickEntryStats, recent: &[abt_core::mes::dashboard::model::RecentOp]) -> Markup {
 html! {
    div {
        // ── Page Header ──
        div class="flex items-center justify-between mb-6" {
            h1 class="text-xl font-bold text-fg tracking-tight" { "生产管理总览" }
            div class="flex gap-3" {
                button
                    class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                { (icon::download_icon("w-4 h-4")) " 导出报表" }
            }
        }
        // ── Stat Cards ──
        div class="grid grid-cols-5 gap-5 mb-6" {
            div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md" {
                div class="w-11 h-11 rounded-md grid place-items-center shrink-0 bg-[linear-gradient(135deg,#e6f4ff,#d6e8ff)] text-accent"
                { (icon::file_text_icon("w-5 h-5")) }
                div {
                    div class="text-2xl font-bold font-mono tabular-nums text-fg" {
                        (stats.plan_count)
                    }
                    div class="text-sm text-muted mt-1" { "本月生产计划" }
                }
            }
            div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md" {
                div class="w-11 h-11 rounded-md grid place-items-center shrink-0 bg-[linear-gradient(135deg,#f0fff0,#e0ffe0)] text-success"
                { (icon::tool_icon("w-5 h-5")) }
                div {
                    div class="text-2xl font-bold font-mono tabular-nums text-fg" {
                        (stats.active_order_count)
                    }
                    div class="text-sm text-muted mt-1" { "进行中工单" }
                }
            }
            div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md" {
                div class="w-11 h-11 rounded-md grid place-items-center shrink-0 bg-[linear-gradient(135deg,#fff8eb,#fff0d6)] text-warn"
                { (icon::briefcase_icon("w-5 h-5")) }
                div {
                    div class="text-2xl font-bold font-mono tabular-nums text-fg" {
                        (stats.active_batch_count)
                    }
                    div class="text-sm text-muted mt-1" { "活跃批次" }
                }
            }
            div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md" {
                div class="w-11 h-11 rounded-md grid place-items-center shrink-0 bg-[linear-gradient(135deg,#e6f4ff,#d6e8ff)] text-accent"
                { (icon::download_icon("w-5 h-5")) }
                div {
                    div class="text-2xl font-bold font-mono tabular-nums text-fg" {
                        (stats.pending_receipt_count)
                    }
                    div class="text-sm text-muted mt-1" { "待入库批次" }
                }
            }
            div class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md" {
                div class="w-11 h-11 rounded-md grid place-items-center shrink-0 bg-[linear-gradient(135deg,#f0fff0,#e0ffe0)] text-success"
                { (icon::check_circle_icon("w-5 h-5")) }
                div {
                    div class="text-2xl font-bold font-mono tabular-nums text-fg" {
                        (crate::utils::fmt_qty(stats.completed_qty))
                    }
                    div class="text-sm text-muted mt-1" { "本月完工数量" }
                }
            }
        }
        // ── 数据质量 ──
        div class="mb-8" {
            h2 class="text-lg font-semibold text-fg mb-4" { "数据质量" }
            div class="grid grid-cols-3 gap-5" {
                a   class="flex items-center gap-4 p-5 px-6 rounded-lg bg-bg border border-border-soft no-underline hover:shadow-md transition-shadow"
                    href="/admin/md/products"
                {
                    div class="text-2xl font-bold font-mono tabular-nums text-warn" {
                        (dq.no_routing_count)
                    }
                    div class="text-sm text-muted" { "个产品无 Routing" }
                }
                a   class="flex items-center gap-4 p-5 px-6 rounded-lg bg-bg border border-border-soft no-underline hover:shadow-md transition-shadow"
                    href="/admin/md/boms"
                {
                    div class="text-2xl font-bold font-mono tabular-nums text-warn" {
                        (dq.no_bom_count)
                    }
                    div class="text-sm text-muted" { "个产品无已发布 BOM" }
                }
                a   class="flex items-center gap-4 p-5 px-6 rounded-lg bg-bg border border-border-soft no-underline hover:shadow-md transition-shadow"
                    href="/admin/md/products"
                {
                    div class="text-2xl font-bold font-mono tabular-nums text-success" {
                        (dq.complete_count)
                    }
                    div class="text-sm text-muted" { "个产品数据完整" }
                }
            }
        }
        // ── Quick Entry Grid ──
        div class="mb-8" {
            h2 class="text-lg font-semibold text-fg mb-4" { "快捷入口" }
            div class="grid grid-cols-4 gap-4" {
                ({
                    quick_entry_card(
                        "/admin/mes/demand-pool",
                        "生产需求池",
                        "销售订单驱动的生产需求",
                        "purple",
                        0,
                        "条待处理",
                    )
                })
                ({
                    quick_entry_card(
                        "/admin/mes/plans",
                        "生产计划",
                        "MTO/MTS 双轨排产",
                        "blue",
                        qs.plan_total,
                        "条计划",
                    )
                })
                ({
                    quick_entry_card(
                        "/admin/mes/orders",
                        "工单管理",
                        "BOM展开与工序排程",
                        "green",
                        qs.order_active,
                        "进行中",
                    )
                })
                ({
                    quick_entry_card(
                        "/admin/mes/batches",
                        "生产批次",
                        "流转卡与工序进度",
                        "orange",
                        qs.batch_active,
                        "活跃批次",
                    )
                })
                ({
                    quick_entry_card(
                        "/admin/mes/reports",
                        "报工记录",
                        "工序报工与计件工资",
                        "blue",
                        qs.report_month,
                        "条本月",
                    )
                })
                ({
                    quick_entry_card(
                        "/admin/mes/inspections",
                        "生产报检",
                        "首检/巡检/完工检",
                        "red",
                        qs.insp_pending,
                        "待处理",
                    )
                })
                ({
                    quick_entry_card(
                        "/admin/mes/receipts",
                        "完工入库",
                        "入库确认与倒冲扣料",
                        "blue",
                        qs.receipt_pending,
                        "待入库",
                    )
                })
                ({
                    quick_entry_card(
                        "/admin/outsourcing/orders",
                        "委外管理",
                        "工序委外与收货跟进",
                        "amber",
                        0,
                        "笔在制",
                    )
                })
                ({
                    quick_entry_card(
                        "/admin/mes/cards",
                        "流转卡查询",
                        "扫码追踪工序进度",
                        "orange",
                        qs.batch_total,
                        "张流转卡",
                    )
                })
                ({
                    quick_entry_card(
                        "/admin/mes/schedule",
                        "排程看板",
                        "看板/甘特图视图",
                        "purple",
                        0,
                        "延期预警",
                    )
                })
                ({
                    quick_entry_card(
                        "/admin/mes/wages",
                        "计件工资",
                        "工人工资汇总明细",
                        "blue",
                        0,
                        "本月",
                    )
                })
                ({
                    quick_entry_card(
                        "/admin/mes/exceptions",
                        "生产异常",
                        "暂停/报废/不良追踪",
                        "red",
                        qs.insp_total,
                        "待处理",
                    )
                })
            }
        }
        // ── Recent Operations Table ──
        div class="mb-8" {
            h2 class="flex items-center gap-2 text-lg font-semibold text-fg mb-4" {
                (icon::clock_icon("w-4 h-4"))
                " 最近操作"
            }
            div class="data-card" {
                div class="overflow-x-auto" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "时间" }
                                th { "操作类型" }
                                th { "单号" }
                                th { "产品" }
                                th { "操作人" }
                            }
                        }
                        tbody {
                            @if recent.is_empty() {
                                tr {
                                    td class="text-muted text-[13px]" { "—" }
                                    td { "—" }
                                    td { "—" }
                                    td { "—" }
                                    td { "—" }
                                }
                            } @else {
                                @for op in recent {
                                    tr {
                                        td class="text-muted text-[13px]" {
                                            (op.created_at.format("%Y-%m-%d %H:%M"))
                                        }
                                        td { (op.op_type) }
                                        td { (op.doc_number) }
                                        td { (op.product_name.as_deref().unwrap_or("—")) }
                                        td { (op.operator_name.as_deref().unwrap_or("—")) }
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
fn quick_entry_card(href: &str, title: &str, desc: &str, color: &str, count: i64, stat_suffix: &str) -> Markup {
 let (bg, fg) = match color {
 "blue" => ("linear-gradient(135deg,#e6f4ff,#d6e8ff)", "var(--accent)"),
 "green" => ("linear-gradient(135deg,#f0fff0,#e0ffe0)", "var(--success)"),
 "orange" => ("linear-gradient(135deg,#fff8eb,#fff0d6)", "#fa8c16"),
 "purple" => ("linear-gradient(135deg,#f3e8ff,#e9d5ff)", "#7c3aed"),
 "cyan" => ("linear-gradient(135deg,#e6fffb,#b5f5ec)", "#13c2c2"),
 "teal" => ("linear-gradient(135deg,#e6f7ff,#bae7ff)", "var(--accent)"),
 "amber" => ("linear-gradient(135deg,#fffbe6,#fff1b8)", "#d4a017"),
 "red" => ("linear-gradient(135deg,#fff2f0,#ffe8e6)", "var(--danger)"),
 _ => ("rgba(0,0,0,0.04)", "var(--muted)"),
 };
 let icon_svg = match title {
 "生产需求池" => icon::grid_icon("w-full h-full"),
 "生产计划" => icon::file_text_icon("w-full h-full"),
 "工单管理" => icon::tool_icon("w-full h-full"),
 "生产批次" => icon::briefcase_icon("w-full h-full"),
 "报工记录" => icon::edit_icon("w-full h-full"),
 "生产报检" => icon::check_circle_icon("w-full h-full"),
 "完工入库" => icon::download_icon("w-full h-full"),
 "流转卡查询" => icon::search_icon("w-full h-full"),
 "排程看板" => icon::calendar_icon("w-full h-full"),
 "计件工资" => icon::dollar_icon("w-full h-full"),
 "物料消耗" => icon::box_icon("w-full h-full"),
 "生产异常" => icon::alert_triangle_icon("w-full h-full"),
 _ => icon::grid_icon("w-full h-full"),
 };
 html! {
    a   href=(href)
        class="block p-5 rounded-lg bg-bg border border-border-soft no-underline relative overflow-hidden hover:shadow-md transition-shadow"
    {
        div class="w-[22px] h-[22px] mb-3" style=(format!("color:{}", fg)) { (icon_svg) }
        div class="text-sm font-semibold text-fg mb-1" { (title) }
        div class="text-xs text-muted mb-3" { (desc) }
        div class="flex items-baseline gap-1" {
            span class="text-lg font-bold font-mono tabular-nums" style=(format!("color:{}", fg)) {
                (count)
            }
            span class="text-xs text-muted" { (stat_suffix) }
        }
    }
}
}
