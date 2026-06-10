use chrono::Datelike;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;
use std::collections::HashMap;

use abt_core::mes::work_order::WorkOrderService;
use abt_core::mes::work_report::WorkReportService;
use abt_core::shared::identity::UserService;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::mes_report::WageListPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[derive(Debug, Deserialize)]
pub struct WageListQuery {
    pub date_from: Option<String>,
    pub date_to: Option<String>,
}

#[require_permission("MES", "read")]
pub async fn get_wage_list(
    _path: WageListPath, ctx: RequestContext,
    axum::extract::Query(query): axum::extract::Query<WageListQuery>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    let report_svc = state.work_report_service();
    let wo_svc = state.work_order_service();
    let user_svc = state.user_service();

    // Default date range: current month
    let now = chrono::Local::now();
    let today = chrono::NaiveDate::parse_from_str(&now.format("%Y-%m-%d").to_string(), "%Y-%m-%d").unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
    let first_of_month = chrono::NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);
    let date_from = query.date_from.and_then(|d| d.parse().ok()).unwrap_or(first_of_month);
    let date_to = query.date_to.and_then(|d| d.parse().ok()).unwrap_or(today);

    let date_range = abt_core::mes::work_report::DateRange {
        from: date_from,
        to: date_to,
    };

    // Load all wage summaries
    let summaries = report_svc.list_all_wage_summaries(&service_ctx, &mut conn, date_range).await?;

    // Load worker names
    let users_result = user_svc.list_users(&service_ctx, &mut conn, 1, 200).await?;
    let user_map: HashMap<i64, String> = users_result.items.iter()
        .map(|u| (u.user_id, u.display_name.clone().unwrap_or_else(|| u.username.clone())))
        .collect();

    // Load work order doc numbers
    let mut wo_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for s in &summaries {
        for d in &s.details {
            wo_ids.insert(d.work_order_id);
        }
    }
    let mut wo_doc_map: HashMap<i64, String> = HashMap::new();
    for wo_id in wo_ids {
        if let Ok(wo) = wo_svc.find_by_id(&service_ctx, &mut conn, wo_id).await {
            wo_doc_map.insert(wo_id, wo.doc_number);
        }
    }

    // Compute aggregate stats
    let total_wage: rust_decimal::Decimal = summaries.iter().map(|s| s.total_amount).sum();
    let worker_count = summaries.len();
    let total_completed: rust_decimal::Decimal = summaries.iter()
        .flat_map(|s| s.details.iter())
        .map(|d| d.completed_qty)
        .sum();
    let total_defect: rust_decimal::Decimal = summaries.iter()
        .flat_map(|s| s.details.iter())
        .map(|d| d.defect_qty)
        .sum();
    let total_operator_defect: rust_decimal::Decimal = summaries.iter()
        .flat_map(|s| s.details.iter())
        .filter(|d| matches!(d.defect_reason, Some(abt_core::mes::enums::DefectReason::OperatorError)))
        .map(|d| d.defect_qty)
        .sum();

    let content = wage_list_page(
        &summaries, &user_map, &wo_doc_map,
        date_from, date_to,
        total_wage, worker_count, total_completed, total_defect, total_operator_defect,
    );
    Ok(Html(admin_page(is_htmx, "计件工资汇总", &claims, "production", WageListPath::PATH, "生产管理", None, content, &nav_filter).into_string()))
}

fn wage_list_page(
    summaries: &[abt_core::mes::work_report::WageSummary],
    user_map: &HashMap<i64, String>,
    wo_doc_map: &HashMap<i64, String>,
    date_from: chrono::NaiveDate,
    date_to: chrono::NaiveDate,
    total_wage: rust_decimal::Decimal,
    worker_count: usize,
    total_completed: rust_decimal::Decimal,
    total_defect: rust_decimal::Decimal,
    total_operator_defect: rust_decimal::Decimal,
) -> Markup {
    let date_from_str = date_from.format("%Y-%m-%d").to_string();
    let date_to_str = date_to.format("%Y-%m-%d").to_string();
    let total_completed_fmt = crate::utils::fmt_qty(total_completed);
    let total_defect_fmt = crate::utils::fmt_qty(total_defect);
    let defect_rate = if total_completed > rust_decimal::Decimal::ZERO {
        let rate = (total_defect / total_completed) * rust_decimal::Decimal::ONE_HUNDRED;
        format!("{:.1}%", rate)
    } else {
        "0%".to_string()
    };

    html! { div {
        div class="page-header" {
            h1 class="page-title" { "计件工资汇总" }
            div class="page-actions" {
                button class="btn btn-default" {
                    (maud::PreEscaped(r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4M7 10l5 5 5-5M12 15V3"/></svg>"#))
                    " 导出"
                }
            }
        }

        // 筛选栏
        div class="filter-bar" {
            div class="search-wrap" {
                (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><path d="M21 21l-4.35-4.35"/></svg>"#))
                input class="search-input" type="text" placeholder="搜索工人姓名、工号…";
            }
            input type="date" class="filter-select" value=(date_from_str) style="max-width:160px";
            span style="color:var(--muted);font-size:var(--text-sm);line-height:36px" { "至" }
            input type="date" class="filter-select" value=(date_to_str) style="max-width:160px";
        }

        // 汇总统计卡片
        div class="wage-summary" {
            div class="stat-card" {
                div class="stat-icon" style="background:linear-gradient(135deg,#e6f4ff,#d6e8ff);color:var(--accent)" {
                    (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 1v22M17 5H9.5a3.5 3.5 0 000 7h5a3.5 3.5 0 010 7H6"/></svg>"#))
                }
                div {
                    div class="stat-value" { "¥" (crate::utils::fmt_qty(total_wage)) }
                    div class="stat-label" { "本月工资总额" }
                }
            }
            div class="stat-card" {
                div class="stat-icon" style="background:linear-gradient(135deg,#f0fff0,#e0ffe0);color:var(--success)" {
                    (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0z"/></svg>"#))
                }
                div {
                    div class="stat-value" { (worker_count) }
                    div class="stat-label" { "计件工人数" }
                }
            }
            div class="stat-card" {
                div class="stat-icon" style="background:linear-gradient(135deg,#fff8eb,#fff0d6);color:var(--warn)" {
                    (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"/></svg>"#))
                }
                div {
                    div class="stat-value" { (total_completed_fmt) }
                    div class="stat-label" { "总完成数量" }
                    div style="font-size:var(--text-xs);color:var(--muted);margin-top:2px" { "不良品 " (total_defect_fmt) " (" (defect_rate) ")" }
                }
            }
            div class="stat-card" {
                div class="stat-icon" style="background:linear-gradient(135deg,#fff2f0,#ffe8e6);color:var(--danger)" {
                    (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 9v2m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>"#))
                }
                div {
                    div class="stat-value" { "—" }
                    div class="stat-label" { "扣减金额(操作失误)" }
                    div style="font-size:var(--text-xs);color:var(--muted);margin-top:2px" { "操作失误不良: " (crate::utils::fmt_qty(total_operator_defect)) "件" }
                }
            }
        }

        // 工资公式提示
        div class="formula-hint" {
            (maud::PreEscaped(r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4M12 8h.01"/></svg>"#))
            " 计算公式："
            code { "(完成数 + 非操作失误不良数) × 计件单价" }
            "，其中物料不良/设备故障/工艺问题照常计工资，操作失误不计工资"
        }

        // 工人工资明细卡片
        div class="wage-detail-card" {
            div class="wage-detail-header" {
                div class="wage-detail-title" {
                    (maud::PreEscaped(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0z"/></svg>"#))
                    " 工人工资明细"
                }
            }
            div class="wage-detail-body" {
                // Header row
                div class="worker-row worker-row-header" {
                    span { "工人" }
                    span { "完成数" }
                    span { "不良品数" }
                    span { "有效计件数" }
                    span { "应发工资" }
                    span {}
                }

                @if summaries.is_empty() {
                    div style="text-align:center;padding:var(--space-8);color:var(--muted)" { "暂无工资数据" }
                }

                @for (idx, summary) in summaries.iter().enumerate() {
                    @let worker_name = user_map.get(&summary.worker_id).cloned().unwrap_or_else(|| format!("工人#{}", summary.worker_id));
                    @let initial = worker_name.chars().next().unwrap_or('?');
                    @let wc = summary.details.iter().map(|d| d.completed_qty).sum::<rust_decimal::Decimal>();
                    @let wd = summary.details.iter().map(|d| d.defect_qty).sum::<rust_decimal::Decimal>();
                    @let we = summary.details.iter().map(|d| {
                        let non_op = match d.defect_reason {
                            Some(abt_core::mes::enums::DefectReason::OperatorError) => rust_decimal::Decimal::ZERO,
                            _ => d.defect_qty,
                        };
                        d.completed_qty + non_op
                    }).sum::<rust_decimal::Decimal>();
                    @let toggle_id = format!("w{}", idx);

                    // Worker summary row
                    div class="worker-row" style="cursor:pointer" {
                        div class="worker-name-cell" {
                            div class="worker-avatar" style="background:var(--accent)" { (initial) }
                            div class="worker-info" {
                                span class="worker-name" { (worker_name) }
                            }
                        }
                        span class="wage-mono" { (crate::utils::fmt_qty(wc)) }
                        span class="wage-mono text-danger" { (crate::utils::fmt_qty(wd)) }
                        span class="wage-mono" { (crate::utils::fmt_qty(we)) }
                        span class="wage-mono text-success" style="font-weight:700" { "¥" (crate::utils::fmt_qty(summary.total_amount)) }
                        span {
                            (maud::PreEscaped(r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M19 9l-7 7-7-7"/></svg>"#))
                        }
                        script { (maud::PreEscaped(format!("me().on('click',e=>{{var d=me('#{}');d.styles({{display:d.style.display==='none'?'':'none'}})}})", toggle_id))) }
                    }

                    // Expandable detail table
                    div class="wage-expand" id=(toggle_id) style="display:none" {
                        table class="wage-expand-table" {
                            thead { tr {
                                th { "工单" } th { "工序" } th { "完成" }
                                th { "不良(原因)" } th { "有效数" } th { "单价" }
                                th { "工资" }
                            }}
                            tbody {
                                @for detail in &summary.details {
                                    @let wo_doc = wo_doc_map.get(&detail.work_order_id).cloned().unwrap_or_else(|| "—".to_string());
                                    @let defect_label = match detail.defect_reason {
                                        Some(abt_core::mes::enums::DefectReason::MaterialDefect) => format!("{} (物料不良)", crate::utils::fmt_qty(detail.defect_qty)),
                                        Some(abt_core::mes::enums::DefectReason::EquipmentFault) => format!("{} (设备故障)", crate::utils::fmt_qty(detail.defect_qty)),
                                        Some(abt_core::mes::enums::DefectReason::OperatorError) => format!("{} (操作失误)", crate::utils::fmt_qty(detail.defect_qty)),
                                        Some(abt_core::mes::enums::DefectReason::ProcessIssue) => format!("{} (工艺问题)", crate::utils::fmt_qty(detail.defect_qty)),
                                        None if detail.defect_qty > rust_decimal::Decimal::ZERO => crate::utils::fmt_qty(detail.defect_qty),
                                        _ => "—".to_string(),
                                    };
                                    @let non_op_defect = match detail.defect_reason {
                                        Some(abt_core::mes::enums::DefectReason::OperatorError) => rust_decimal::Decimal::ZERO,
                                        _ => detail.defect_qty,
                                    };
                                    @let effective = detail.completed_qty + non_op_defect;
                                    tr {
                                        td class="mono" { (wo_doc) }
                                        td { (detail.process_name) }
                                        td class="mono" { (crate::utils::fmt_qty(detail.completed_qty)) }
                                        td class="mono text-danger" { (defect_label) }
                                        td class="mono" { (crate::utils::fmt_qty(effective)) }
                                        td class="mono" { "¥" (detail.unit_price) }
                                        td class="mono text-success" { "¥" (crate::utils::fmt_qty(detail.wage_amount)) }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 分页
        div class="pagination" {
            span { "共 " (worker_count) " 名工人" }
        }
    }}
}
