use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};

use abt_core::wms::arrival_notice::model::{
    ArrivalNotice, InspectArrivalNoticeReq, InspectItemReq, ReceiveArrivalNoticeReq, ReceiveItemReq,
};
use abt_core::wms::arrival_notice::repo::ArrivalNoticeRepo;
use abt_core::wms::arrival_notice::ArrivalNoticeService;
use abt_core::wms::enums::ArrivalStatus;
use abt_core::master_data::product::ProductService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::shared::identity::UserService;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::wms_arrival::*;
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form Data ──

#[derive(Debug, serde::Deserialize)]
pub struct ArrivalActionForm {
    pub action: String,
}

// ── Status Label ──

fn status_label(s: ArrivalStatus) -> (&'static str, &'static str) {
    match s {
        ArrivalStatus::Draft => ("草稿", "status-draft"),
        ArrivalStatus::Received => ("已收货", "status-received"),
        ArrivalStatus::Inspecting => ("检验中", "status-inspecting"),
        ArrivalStatus::Accepted => ("已接收", "status-completed"),
        ArrivalStatus::PartiallyAccepted => ("部分接收", "status-partial"),
        ArrivalStatus::Rejected => ("已拒收", "status-danger"),
        ArrivalStatus::Cancelled => ("已取消", "status-cancelled"),
    }
}

// ── Workflow Steps ──

fn workflow_steps(status: ArrivalStatus) -> Markup {
    let steps: &[(&str, bool)] = &[
        ("草稿", true),
        ("已收货", false),
        ("检验中", false),
        ("全部接收", false),
    ];

    let completed: Vec<bool> = match status {
        ArrivalStatus::Draft => vec![false, false, false, false],
        ArrivalStatus::Received => vec![true, true, false, false],
        ArrivalStatus::Inspecting => vec![true, true, true, false],
        ArrivalStatus::Accepted | ArrivalStatus::PartiallyAccepted | ArrivalStatus::Rejected => {
            vec![true, true, true, true]
        }
        ArrivalStatus::Cancelled => vec![true, false, false, false],
    };

    let current_idx = match status {
        ArrivalStatus::Draft => Some(0),
        ArrivalStatus::Received => Some(1),
        ArrivalStatus::Inspecting => Some(2),
        ArrivalStatus::Accepted | ArrivalStatus::PartiallyAccepted | ArrivalStatus::Rejected => Some(3),
        ArrivalStatus::Cancelled => None,
    };

    html! {
        div class="flex items-center" {
            @for (i, (label, _)) in steps.iter().enumerate() {
                @if i > 0 {
                    @let line_class = if completed[i] { "wf-line completed" } else { "wf-line" };
                    div class=(line_class) {}
                }
                @let step_class = match current_idx {
                    Some(ci) if ci == i => "wf-step current",
                    _ if completed[i] => "wf-step completed",
                    _ => "wf-step",
                };
                div class=(step_class) {
                    span class="w-[10px] h-[10px] rounded-full bg-border" {}
                    (label)
                }
            }
        }
        div style="display:flex;align-items:center;gap:var(--space-4);margin-top:var(--space-3);flex-wrap:wrap" {
            span style="font-size:12px;color:var(--muted)" { "检验结果分支：" }
            span style="display:inline-flex;align-items:center;gap:4px;font-size:12px;color:var(--success)" { "● 全部接收 (Accepted)" }
            span style="display:inline-flex;align-items:center;gap:4px;font-size:12px;color:var(--warn)" { "● 部分接收 (Partially Accepted)" }
            span style="display:inline-flex;align-items:center;gap:4px;font-size:12px;color:var(--danger)" { "● 拒收 (Rejected)" }
            span style="color:var(--border-soft)" { "|" }
            span style="display:inline-flex;align-items:center;gap:4px;font-size:12px;color:var(--muted)" { "仅草稿状态可取消 (Cancelled)" }
        }
    }
}

// ── Handlers ──

#[require_permission("INVENTORY", "read")]
pub async fn get_arrival_detail(
    path: ArrivalDetailPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.arrival_notice_service();

    let notice = svc.get(&service_ctx, &mut conn, path.id).await?;
    let items = ArrivalNoticeRepo::get_items(&mut conn, path.id)
        .await
        .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;

    let wh_name = state.warehouse_service()
        .get(&service_ctx, &mut conn, notice.warehouse_id)
        .await
        .map(|w| w.name)
        .unwrap_or_else(|_| "—".into());

    let supplier_name = state.supplier_service()
        .get(&service_ctx, &mut conn, notice.supplier_id)
        .await
        .map(|s| s.name)
        .unwrap_or_else(|_| "—".into());

    let operator_name = state.user_service()
        .get_user(&service_ctx, &mut conn, notice.operator_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());

    let product_svc = state.product_service();
    let mut product_names: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
    for item in &items {
        if !product_names.contains_key(&item.product_id)
            && let Ok(p) = product_svc.get(&service_ctx, &mut conn, item.product_id).await {
                product_names.insert(item.product_id, format!("{} ({})", p.pdt_name, p.product_code));
            }
    }

    let detail_path = ArrivalDetailPath { id: path.id }.to_string();
    let content = arrival_detail_page(&notice, &items, &detail_path, &wh_name, &supplier_name, &operator_name, &product_names);
    let page_html = admin_page(
        is_htmx,
        &format!("{} - 来料通知详情", notice.doc_number),
        &claims,
        "inventory",
        &detail_path,
        "库存管理",
        Some(&notice.doc_number),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("INVENTORY", "update")]
pub async fn post_arrival_action(
    path: ArrivalDetailPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ArrivalActionForm>,
) -> crate::errors::Result<axum::response::Response> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.arrival_notice_service();

    match form.action.as_str() {
        "cancel" => {
            svc.cancel(&service_ctx, &mut conn, path.id).await?;
        }
        "receive" => {
            // 快速收货：实收数量 = 申报数量
            let items = ArrivalNoticeRepo::get_items(&mut conn, path.id)
                .await
                .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
            let receive_items: Vec<ReceiveItemReq> = items.iter()
                .map(|item| ReceiveItemReq {
                    item_id: item.id,
                    received_qty: item.declared_qty,
                    batch_no: None,
                })
                .collect();
            svc.receive(&service_ctx, &mut conn, ReceiveArrivalNoticeReq {
                id: path.id,
                items: receive_items,
            }).await?;
        }
        "inspect" => {
            // 快速检验：合格数量 = 实收数量（全部接收）
            let items = ArrivalNoticeRepo::get_items(&mut conn, path.id)
                .await
                .map_err(|e| abt_core::shared::types::DomainError::Internal(e.into()))?;
            let inspect_items: Vec<InspectItemReq> = items.iter()
                .map(|item| InspectItemReq {
                    item_id: item.id,
                    accepted_qty: item.received_qty,
                })
                .collect();
            svc.inspect(&service_ctx, &mut conn, InspectArrivalNoticeReq {
                id: path.id,
                items: inspect_items,
            }).await?;
        }
        _ => {}
    }

    let redirect_url = ArrivalDetailPath { id: path.id }.to_string();
    let mut resp = axum::response::Response::default();
    resp.headers_mut().insert(
        axum::http::HeaderName::from_static("hx-redirect"),
        redirect_url.parse().unwrap(),
    );

    Ok(resp)
}

// ── Components ──

fn arrival_detail_page(
    notice: &ArrivalNotice,
    items: &[abt_core::wms::arrival_notice::model::ArrivalNoticeItem],
    detail_path: &str,
    wh_name: &str,
    supplier_name: &str,
    operator_name: &str,
    product_names: &std::collections::HashMap<i64, String>,
) -> Markup {
    let (status_text, status_class) = status_label(notice.status);
    let is_inspecting = notice.status == ArrivalStatus::Inspecting;

    html! {
        div {
            a href=(format!("{}?restore=true", ArrivalListPath::PATH)) class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" {
                (icon::chevron_left_icon("w-4 h-4"))
                "返回来料通知列表"
            }

            div class="block bg-bg border border-border-soft rounded-lg p-6" {
                div {
                    div class="flex items-center justify-between" {
                        h1 class="text-2xl font-extrabold font-font-mono tabular-nums" { (notice.doc_number) }
                        span class=(format!("status-pill {status_class}")) { (status_text) }
                    }
                }
                div class="flex gap-3" {
                    (arrival_action_buttons(notice.status, detail_path))
                }
            }

            (workflow_steps(notice.status))

            // ── 基本信息 ──
            div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]-title" { "基本信息" }
                div class="grid gap-4" {
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "单据编号" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" { (notice.doc_number) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "来源采购单" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" {
                            (notice.purchase_order_id.map(|id| id.to_string()).unwrap_or_else(|| "—".into()))
                        }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "供应商" }
                        span class="text-sm text-fg font-medium" { (supplier_name) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "到货仓库" }
                        span class="text-sm text-fg font-medium" { (wh_name) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "到货库区" }
                        span class="text-sm text-fg font-medium" {
                            (notice.zone_id.map(|id| id.to_string()).unwrap_or_else(|| "—".into()))
                        }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "到货日期" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" { (notice.arrival_date.format("%Y-%m-%d")) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "送货单号" }
                        span class="text-sm text-fg font-medium font-mono tabular-nums" { (notice.delivery_note.as_deref().unwrap_or("—")) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "备注" }
                        span class="text-sm text-fg font-medium" { (if notice.remark.is_empty() { "—" } else { &notice.remark }) }
                    }
                    div class="flex flex-col gap-1" {
                        span class="text-xs text-muted font-medium" { "操作员" }
                        span class="text-sm text-fg font-medium" { (operator_name) }
                    }
                }
            }

            // ── 行项明细 ──
            div class="data-card" {
                div class="overflow-x-auto" {
                    table class="data-table" {
                        thead {
                            tr {
                                th { "行号" }
                                th { "产品" }
                                th class="text-right text-[13px]" { "申报数量" }
                                th class="text-right text-[13px]" { "实收数量" }
                                th class="text-right text-[13px]" { "合格数量" }
                                th { "批次号" }
                            }
                        }
                        tbody {
                            @for (i, item) in items.iter().enumerate() {
                                tr {
                                    td class="font-mono tabular-nums" { (i + 1) }
                                    td { (product_names.get(&item.product_id).map(|n| n.as_str()).unwrap_or("—")) }
                                    td class="text-right text-[13px]" { (format!("{:.2}", item.declared_qty)) }
                                    td class="text-right text-[13px]" { (format!("{:.2}", item.received_qty)) }
                                    td class="text-right text-[13px]" { (format!("{:.2}", item.accepted_qty)) }
                                    td class="font-mono tabular-nums" { (item.batch_no.as_deref().unwrap_or("—")) }
                                }
                            }
                            @if items.is_empty() {
                                tr {
                                    td colspan="6" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                        "暂无物料明细"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── IQC 质检结果区 ──
            @if is_inspecting || notice.status == ArrivalStatus::Accepted || notice.status == ArrivalStatus::PartiallyAccepted || notice.status == ArrivalStatus::Rejected {
                div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]" style="border-left:3px solid var(--warn)" {
                    div class="bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-sm)]-title" style="display:flex;align-items:center;gap:var(--space-2)" {
                        (icon::clipboard_list_icon("w-4 h-4"))
                        "IQC质检结果"
                        span class="inline-flex items-center gap-[5px] rounded-full text-[12px] font-medium whitespace-nowrap bg-[#fff8eb] text-[#d46b08]" style="margin-left:var(--space-2)" { "检验中" }
                    }
                    div class="grid gap-4" style="margin-bottom:var(--space-4)" {
                        div class="flex flex-col gap-1" {
                            span class="text-xs text-muted font-medium" { "检验标准" }
                            span class="text-sm text-fg font-medium" { "GB/T 2828.1 抽样检验" }
                        }
                        div class="flex flex-col gap-1" {
                            span class="text-xs text-muted font-medium" { "AQL等级" }
                            span class="text-sm text-fg font-medium font-mono tabular-nums" { "0.65" }
                        }
                        div class="flex flex-col gap-1" {
                            span class="text-xs text-muted font-medium" { "检验员" }
                            span class="text-sm text-fg font-medium" { "—" }
                        }
                        div class="flex flex-col gap-1" {
                            span class="text-xs text-muted font-medium" { "计划完成日期" }
                            span class="text-sm text-fg font-medium font-mono tabular-nums" { "—" }
                        }
                    }
                    div style="background:var(--surface-warm);border:1px solid var(--border);border-radius:var(--radius-sm);padding:var(--space-3) var(--space-4);font-size:var(--text-sm);color:var(--fg-2)" {
                        strong style="color:var(--warn)" { "⚠ IQC硬门规则：" }
                        "质检不合格的物料将阻断入库流程。不合格批次将触发MRB（物料评审委员会）处理流程，需由质量部判定：退货 / 让步接收 / 挑选使用。"
                    }
                }
            }
        }
    }
}

fn arrival_action_buttons(status: ArrivalStatus, detail_path: &str) -> Markup {
    match status {
        ArrivalStatus::Draft => {
            html! {
                button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"cancel"}"#
                    hx-confirm="确定要取消此来料通知吗？"
                    hx-redirect=(detail_path) {
                    (icon::x_icon("w-4 h-4"))
                    "取消"
                }
                button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"receive"}"#
                    hx-confirm="确定要确认收货吗？实收数量将自动按申报数量填写。"
                    hx-redirect=(detail_path) {
                    (icon::check_circle_icon("w-4 h-4"))
                    "确认收货"
                }
            }
        }
        ArrivalStatus::Received => {
            html! {
                button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"cancel"}"#
                    hx-confirm="确定要取消此来料通知吗？"
                    hx-redirect=(detail_path) {
                    (icon::x_icon("w-4 h-4"))
                    "取消"
                }
                button class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                    hx-post=(detail_path)
                    hx-vals=r#"{"action":"inspect"}"#
                    hx-confirm="确定要开始检验并确认接收吗？合格数量将按实收数量自动填写。"
                    hx-redirect=(detail_path) {
                    (icon::clipboard_list_icon("w-4 h-4"))
                    "检验接收"
                }
            }
        }
        _ => html! {},
    }
}
