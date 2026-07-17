use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::shared::types::pagination::{PageParams, PaginatedResult};
use abt_core::shared::types::{PgExecutor, ServiceContext};
use abt_core::wms::enums::{CycleCountStatus, PickingStatus};
use abt_core::wms::picking::model::StockPickingItem;
use abt_core::wms::picking::{IssueItemReq, IssueMaterialReq, PickingService};
use abt_core::wms::cycle_count::model::CycleCountItem;
use abt_core::wms::cycle_count::CycleCountService;
use abt_core::wms::low_stock_alert::service::LowStockAlertService;
use abt_core::wms::warehouse::model::{Warehouse, WarehouseFilter};
use abt_core::wms::warehouse::WarehouseService;
use abt_core::wms::work_center::model::{
    PendingTask, PendingTaskFilter, TaskSourceKind, Urgency, WorkCenterDomain,
    WorkCenterSummary,
};
use abt_core::wms::work_center::WorkCenterService;
use abt_core::shared::document_sequence::DocumentSequenceService;
use abt_core::purchase::order::model::PurchaseOrderItem;
use abt_core::purchase::order::PurchaseOrderService;
use abt_core::shared::enums::DocumentType;
use abt_core::wms::enums::TransactionType;
use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::ProductService;
use abt_core::master_data::supplier::SupplierService;
use abt_core::om::outsourcing_order::{ConfirmSentReq, OutsourcingOrderService};
use abt_core::master_data::work_center::WorkCenterService as MdWorkCenterService;
use rust_decimal::Decimal;
use std::collections::HashMap;

use crate::components::icon;
use crate::components::overlay::drawer_shell;
use crate::components::pagination::pagination;
use abt_core::wms::picking::model::{PoReceiveRow, ReceivePurchaseReq, ShipRowReq, ShippingHubSummary};
use abt_core::wms::inventory_transaction::{model::RecordTransactionReq, InventoryTransactionService};
use abt_core::wms::inventory::InventoryService;
use abt_core::mes::work_order::WorkOrderService;
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::shared::identity::UserService;
use crate::errors::Result;
use abt_core::shared::types::error::DomainError;
use crate::layout::page::admin_page;
use abt_core::master_data::print_template::{PrintTemplate, PrintTemplateService};
use crate::components::print_dropdown::print_dropdown;
use crate::routes::print_template::PrintTemplateListPath;
use crate::routes::shipping::ShippingPrintPath;
use crate::routes::wms_work_center::WmsWorkCenterPath;
use crate::utils::fmt_qty;
use crate::utils::{empty_as_none, RequestContext};
use crate::state::AppState;
use abt_macros::require_permission;

/// 单端点查询参数。`drawer`+`id` → 加载 drawer body；否则按 `domain`/过滤/分页 渲染 tab 主体
/// （非 htmx 整页，htmx 片段由客户端 `hx-select="#wc-domain-card"` 选取）。
#[derive(Debug, Deserialize, Default, Clone)]
pub struct WorkCenterQuery {
    pub drawer: Option<String>,
    pub id: Option<i64>,
    /// 当前 tab 环节 slug（默认 arrival）
    pub domain: Option<String>,
    pub keyword: Option<String>,
    /// overdue / soon / normal
    pub urgency: Option<String>,
    /// po / wo（仅待收货环节）
    pub source: Option<String>,
    /// 视图模式：pending（待办队列，默认）/ all（全量单据表格，阶段 3.1 领料单试点）
    pub view: Option<String>,
    pub page: Option<u32>,
}

/// tab 主体每页条数
const DOMAIN_PAGE_SIZE: u32 = 20;

/// 就地操作提交：action 决定分发，id 目标单据，items_json（收货/发料行级明细，JSON 字符串）。
/// idempotency_key 仅收货入库用（防双击重复入库），其他 action 不传 = None。
#[derive(Debug, Deserialize)]
pub struct WorkCenterActionForm {
    pub action: String,
    pub id: i64,
    pub items_json: Option<String>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    /// 批量操作目标 id 列表（逗号分隔，仅 batch_* action 用，如 batch_ship）
    #[serde(default)]
    pub ids: Option<String>,
    /// 发货仓库（direct_ship / batch_ship 用，选仓 drawer / 批量栏传入）
    #[serde(default)]
    pub warehouse_id: Option<String>,
    /// 送货单号（receive_po 采购到货确认用，透传到 ReceivePurchaseReq.delivery_note）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub delivery_note: Option<String>,
    /// 备注（receive_po 采购到货确认用，透传到 ReceivePurchaseReq.remark）
    #[serde(default, deserialize_with = "empty_as_none")]
    pub remark: Option<String>,
    /// 视图（仅 Requisition：pending/all，决定 POST 后重渲染哪个 card）
    #[serde(default)]
    pub view: Option<String>,
}

// ── domain ↔ slug / 动作 映射 ──

fn domain_from_str(s: &str) -> Option<WorkCenterDomain> {
    match s {
        "arrival" => Some(WorkCenterDomain::Arrival),
        "outbound" => Some(WorkCenterDomain::Outbound),
        "requisition" => Some(WorkCenterDomain::Requisition),
        "transfer" => Some(WorkCenterDomain::Transfer),
        "cycle-count" => Some(WorkCenterDomain::CycleCount),
        "outsource-issue" => Some(WorkCenterDomain::OutsourceIssue),
        "low-stock" => Some(WorkCenterDomain::LowStock),
        _ => None,
    }
}

fn domain_slug(d: WorkCenterDomain) -> &'static str {
    match d {
        WorkCenterDomain::Arrival => "arrival",
        WorkCenterDomain::Outbound => "outbound",
        WorkCenterDomain::Requisition => "requisition",
        WorkCenterDomain::Transfer => "transfer",
        WorkCenterDomain::CycleCount => "cycle-count",
        WorkCenterDomain::OutsourceIssue => "outsource-issue",
        WorkCenterDomain::LowStock => "low-stock",
    }
}

/// 从 query 解析当前 tab 环节（缺省/非法 → Arrival）
/// 解析当前 tab：query 指定则用指定；缺省/非法 → 待收货（仓库员主战场，固定入口最顺手）。
fn active_domain(q: &WorkCenterQuery) -> WorkCenterDomain {
    q.domain
        .as_deref()
        .and_then(domain_from_str)
        .unwrap_or(WorkCenterDomain::Arrival)
}

/// query → 后端过滤（urgency/source 字符串映射到枚举；keyword 去首尾空白，空串视为 None）
fn filter_from_query(q: &WorkCenterQuery) -> PendingTaskFilter {
    PendingTaskFilter {
        keyword: q
            .keyword
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from),
        urgency: q.urgency.as_deref().and_then(|s| match s {
            "overdue" => Some(Urgency::Overdue),
            "soon" => Some(Urgency::Soon),
            "normal" => Some(Urgency::Normal),
            _ => None,
        }),
        source_kind: q.source.as_deref().and_then(|s| match s {
            "po" => Some(TaskSourceKind::PurchaseOrder),
            "wo" => Some(TaskSourceKind::WorkOrder),
            _ => None,
        }),
    }
}

/// 某环节当前待办计数（从 summary 读对应 DomainStats.total）
fn domain_count(s: &WorkCenterSummary, d: WorkCenterDomain) -> u64 {
    s.of(d).total
}

/// 就地操作 action → 受影响环节（决定提交后刷新哪张卡片）
fn action_domain(action: &str) -> Result<WorkCenterDomain> {
    Ok(match action {
        "receive_po" | "receive_wo" => WorkCenterDomain::Arrival,
        // 发货操作（待出库 tab），操作后刷新 Outbound
        "batch_ship" | "direct_ship" => WorkCenterDomain::Outbound,
        "confirm" | "cancel" | "issue" => WorkCenterDomain::Requisition,
        "transfer_cancel" | "dispatch" | "complete" => WorkCenterDomain::Transfer,
        "cc_start" | "cc_complete" | "cc_cancel" | "cc_adjust" | "cc_approve" | "cc_reject" => {
            WorkCenterDomain::CycleCount
        }
        // 委外发料（仓库执行 dispatch+complete + om.confirm_sent）
        "osa_issue" => WorkCenterDomain::OutsourceIssue,
        "ack_low_stock" => WorkCenterDomain::LowStock,
        other => return Err(DomainError::validation(format!("未知作业动作: {other}")).into()),
    })
}

/// 各 domain tab 的收口入口：全部已 drawer 化。
fn domain_entries(active: WorkCenterDomain) -> Markup {
    const BTN_CLS: &str = "inline-flex items-center gap-1 px-3 py-1.5 rounded-sm bg-accent text-white text-xs font-semibold no-underline cursor-pointer border-none hover:opacity-90";
    match active {
        WorkCenterDomain::CycleCount => create_drawer_btn(
            BTN_CLS, "新建盘点单", "wc-cycle-count-create-overlay", "wc-cycle-count-create-drawer-body",
            crate::routes::wms_work_center::WcCycleCountCreateDrawerPath::PATH,
        ),
        WorkCenterDomain::Requisition => create_drawer_btn(
            BTN_CLS, "新建领料单", "wc-requisition-create-overlay", "wc-requisition-create-drawer-body",
            crate::routes::wms_work_center::WcRequisitionCreateDrawerPath::PATH,
        ),
        WorkCenterDomain::Transfer => create_drawer_btn(
            BTN_CLS, "新建调拨单", "wc-transfer-create-overlay", "wc-transfer-create-drawer-body",
            crate::routes::wms_work_center::WcTransferCreateDrawerPath::PATH,
        ),
        WorkCenterDomain::Arrival => create_drawer_btn(
            BTN_CLS, "新建入库单", "wc-stock-in-create-overlay", "wc-stock-in-create-drawer-body",
            crate::routes::wms_work_center::WcStockInCreateDrawerPath::PATH,
        ),
        WorkCenterDomain::Outbound => create_drawer_btn(
            BTN_CLS, "新建发货单", "wc-shipping-create-overlay", "wc-shipping-create-drawer-body",
            crate::routes::wms_work_center::WcShippingCreateDrawerPath::PATH,
        ),
        // LowStock 是异常提醒，无「新建」入口
        WorkCenterDomain::LowStock => html! {},
        // 委外发料由生产端建委外单时自动生成 picking，仓库无「新建」入口
        WorkCenterDomain::OutsourceIssue => html! {},
    }
}

/// drawer 新建按钮：hx-get 加载 drawer body，afterRequest 开 overlay。
fn create_drawer_btn(cls: &str, label: &str, overlay_id: &str, body_id: &str, src: &str) -> Markup {
    html! {
        button type="button" class=(cls)
            hx-get=(src)
            hx-target=(format!("#{body_id}")) hx-swap="innerHTML"
            _=(format!("on 'htmx:afterRequest'[detail.xhr.status<400] add .open to #{}", overlay_id)) {
            (icon::plus_icon("w-3 h-3")) (label)
        }
    }
}


/// 领料单状态 → (标签, 语义色 class)。作业中心全部视图用（对齐 list 页 status_label 语义）。
fn picking_status_label(s: PickingStatus) -> (&'static str, &'static str) {
    match s {
        PickingStatus::Draft => ("草稿", "bg-surface text-muted"),
        PickingStatus::Confirmed => ("已确认", "bg-accent-bg text-accent"),
        PickingStatus::Done => ("已完成", "bg-success-bg text-success"),
        PickingStatus::Cancelled => ("已取消", "bg-danger-bg text-danger"),
    }
}

/// 单据详情 drawer 触发器（button）：hx-get 加载 {drawer} body 到 #wc-drawer-body，
/// 成功后开 overlay。view 随请求带回（操作 form hidden view 据此回到当前视图）。
/// 领料单 drawer="req_detail"，调拨 drawer="transfer_detail"（阶段 3.2 收口）。
fn doc_detail_trigger(drawer: &str, id: i64, view: &str, label: Markup, cls: &str) -> Markup {
    let url = format!(
        "{}?drawer={drawer}&id={id}&view={view}",
        WmsWorkCenterPath::PATH
    );
    html! {
        button type="button" class=(cls)
            hx-get=(url) hx-target="#wc-drawer-body" hx-swap="innerHTML"
            _="on 'htmx:afterRequest'[detail.xhr.status<400] add .open to #wc-drawer-overlay" {
            (label)
        }
    }
}

/// 按 tab domain（+ Arrival 的 source_kind）返回该行「单据详情 drawer」名。
/// 单号列与对象列共用：点任一列都开同一张订单详情 drawer（对象列让用户从往来方也能钻取单据）。
/// LowStock 非单据（预警），无详情 drawer → None。
fn row_detail_drawer(domain: WorkCenterDomain, source_kind: TaskSourceKind) -> Option<&'static str> {
    match domain {
        WorkCenterDomain::Requisition => Some("req_detail"),
        WorkCenterDomain::Transfer => Some("transfer_detail"),
        WorkCenterDomain::CycleCount => Some("cc_detail"),
        WorkCenterDomain::OutsourceIssue => Some("osa_issue_detail"),
        WorkCenterDomain::Outbound => Some("ship_detail"),
        WorkCenterDomain::Arrival => {
            if matches!(source_kind, TaskSourceKind::WorkOrder) {
                Some("arrival_wo_detail")
            } else {
                Some("arrival_po_detail")
            }
        }
        WorkCenterDomain::LowStock => None,
    }
}

/// 领料单详情 drawer body（替代独立 detail 页，阶段 3.1 收口）：
/// 单据头（单号/状态/工单/仓库/日期）+ 行项目（产品/申请/实领）+ 就地操作（确认/取消/发料）。
/// 操作按钮各自 form 提交单端点，hidden view 携带当前视图，POST 后重渲染对应 card。
async fn req_detail_drawer_body(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
    view: Option<&str>,
) -> Result<Markup> {
    let req_svc = state.picking_service();
    let req = req_svc.get(ctx, db, id).await?;
    let items = req_svc.list_items(ctx, db, id).await.unwrap_or_default();
    let product_map: HashMap<i64, abt_core::master_data::product::model::Product> = state
        .product_service()
        .get_by_ids(ctx, db, items.iter().map(|i| i.product_id).collect())
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.product_id, p))
        .collect();
    let wh_name = state
        .warehouse_service()
        .get(ctx, db, req.from_warehouse_id.unwrap_or(0))
        .await
        .map(|w| w.name)
        .unwrap_or_else(|_| "—".into());

    let (status_text, status_cls) = picking_status_label(req.status);
    let view_val = view.unwrap_or("pending");

    let mut rows = html! {};
    for it in &items {
        let pname = product_map
            .get(&it.product_id)
            .map(|p| p.pdt_name.clone())
            .unwrap_or_else(|| format!("产品 #{}", it.product_id));
        let pcode = product_map
            .get(&it.product_id)
            .map(|p| p.product_code.clone())
            .unwrap_or_default();
        rows = html! {
            (rows)
            div class="flex items-center justify-between px-3 py-2 gap-2" {
                div class="min-w-0" {
                    div class="text-sm text-fg-2 truncate" { (pname) }
                    div class="text-xs text-muted truncate" { (pcode) }
                }
                div class="text-right shrink-0" {
                    div class="text-sm font-mono text-fg" { "申请 " (fmt_qty(it.qty_requested)) }
                    div class="text-xs font-mono text-muted" { "实领 " (fmt_qty(it.qty_done)) }
                }
            }
        };
    }

    let inner = html! {
        // 单据头
        div class="mb-4 pb-4 border-b border-border-soft" {
            div class="flex items-center gap-2 mb-3" {
                span class="text-base font-mono font-bold text-fg" { (req.doc_number) }
                span class=(format!("inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium {status_cls}")) {
                    (status_text)
                }
            }
            div class="grid grid-cols-2 gap-x-4 gap-y-2 text-xs" {
                div {
                    span class="text-muted" { "关联工单 " }
                    span class="font-mono text-fg-2" { "WO-" (req.work_order_id.unwrap_or(0)) }
                }
                div {
                    span class="text-muted" { "领料仓库 " }
                    span class="text-fg-2" { (wh_name) }
                }
                div {
                    span class="text-muted" { "领料日期 " }
                    span class="font-mono text-fg-2" {
                        (req.scheduled_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into()))
                    }
                }
            }
        }
        // 行项目
        div class="mb-2" {
            div class="text-xs font-semibold text-muted mb-2" { "明细（" (items.len()) " 项）" }
            div class="rounded-sm border border-border-soft divide-y divide-border-soft" {
                @if items.is_empty() {
                    div class="px-3 py-4 text-center text-sm text-muted" { "暂无明细" }
                } @else {
                    (rows)
                }
            }
        }
        // 操作
        (req_detail_actions(req.status, id, view_val))
    };

    Ok(req_detail_shell("领料单详情", inner))
}

/// 详情 drawer 壳：标题栏（含×）+ 内容区（非 form；操作按钮在 inner 内各自 form）。
fn req_detail_shell(title: &str, inner: Markup) -> Markup {
    html! {
        div class="flex items-center justify-between px-6 py-5 border-b border-border-soft" {
            div class="font-bold text-base text-fg" { (title) }
            button type="button"
                class="w-8 h-8 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg flex items-center justify-center"
                _="on click remove .open from #wc-drawer-overlay" {
                (icon::x_icon("w-4 h-4"))
            }
        }
        div class="px-6 py-5" { (inner) }
    }
}

/// 详情 drawer 内操作按钮：各自 form 提交单端点（action=confirm/cancel/issue）。
/// hidden view 携带当前视图，POST 后回到对应 card。class="contents" 让 form 不影响 flex 布局。
fn req_detail_actions(status: PickingStatus, id: i64, view: &str) -> Markup {
    let open_hs =
        "on 'htmx:afterRequest'[detail.xhr.status<400] remove .open from #wc-drawer-overlay";
    let cancel_btn = |label: &str, confirm: &str| -> Markup {
        html! {
            form hx-post=(WmsWorkCenterPath::PATH) hx-target="#wc-domain-card" hx-select="#wc-domain-card"
                hx-swap="outerHTML" hx-confirm=(confirm) _=(open_hs) class="contents" {
                input type="hidden" name="action" value="cancel";
                input type="hidden" name="id" value=(id);
                input type="hidden" name="view" value=(view);
                button type="submit"
                    class="inline-flex items-center gap-1.5 px-4 py-2 rounded-sm bg-white text-fg-2 border border-border text-sm font-medium cursor-pointer hover:bg-surface" {
                    (icon::x_icon("w-4 h-4")) (label)
                }
            }
        }
    };
    let primary_btn = |label: &str, action: &str, confirm: &str, ic: Markup| -> Markup {
        html! {
            form hx-post=(WmsWorkCenterPath::PATH) hx-target="#wc-domain-card" hx-select="#wc-domain-card"
                hx-swap="outerHTML" hx-confirm=(confirm) _=(open_hs) class="contents" {
                input type="hidden" name="action" value=(action);
                input type="hidden" name="id" value=(id);
                input type="hidden" name="view" value=(view);
                button type="submit"
                    class="inline-flex items-center gap-1.5 px-4 py-2 rounded-sm bg-accent text-white text-sm font-medium cursor-pointer border-none hover:opacity-90" {
                    (ic) (label)
                }
            }
        }
    };
    match status {
        PickingStatus::Draft => html! {
            div class="flex justify-end gap-3 mt-5 pt-4 border-t border-border-soft" {
                (cancel_btn("取消单据", "确定取消此领料单？"))
                (primary_btn("确认", "confirm", "确定确认此领料单？", icon::check_circle_icon("w-4 h-4")))
            }
        },
        PickingStatus::Confirmed => html! {
            div class="flex justify-end gap-3 mt-5 pt-4 border-t border-border-soft" {
                (cancel_btn("取消单据", "确定取消此领料单？"))
                (primary_btn("确认发料", "issue", "确认全量发料？将扣减库存并计入工单成本", icon::bolt_icon("w-4 h-4")))
            }
        },
        _ => html! {},
    }
}

// ── 发货详情 drawer（替代跳转发货详情页，Issue #188）──
// 只读展示，不提供操作按钮（"直接发货" 是行内操作，"新建发货单" 是创建入口）。

/// 发货详情 drawer body（替代独立 shipping detail 页）：
/// 单据头（单号/状态/客户/日期）+ 行项目（产品/申请数/实发数）。
async fn ship_detail_drawer_body(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
) -> Result<Markup> {
    let picking_svc = state.picking_service();
    let s = picking_svc.find_by_id(ctx, db, id).await?;
    let items = picking_svc.list_items(ctx, db, id).await.unwrap_or_default();
    let hub = picking_svc
        .hub_summary(ctx, db, id)
        .await
        .unwrap_or(ShippingHubSummary {
            pending_ship_qty: Decimal::ZERO,
            shipped_qty: Decimal::ZERO,
            shortage: None,
        });
    let product_map: HashMap<i64, abt_core::master_data::product::model::Product> = state
        .product_service()
        .get_by_ids(ctx, db, items.iter().map(|i| i.product_id).collect())
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.product_id, p))
        .collect();

    // ── Odoo-aligned: Delivery Info (客户 + 收货地址/联系人) ──
    let (customer_name, address_text, contact_info) = if let Some(pid) = s.partner_id {
        let cust_svc = state.customer_service();
        let name = cust_svc
            .get(ctx, db, pid)
            .await
            .map(|c| c.name)
            .unwrap_or_else(|_| "—".into());
        let addresses = cust_svc
            .list_addresses(ctx, db, pid)
            .await
            .unwrap_or_default();
        let default_addr = addresses.iter().find(|a| a.is_default).or(addresses.first());
        let addr = default_addr
            .map(|a| {
                format!(
                    "{}{}{} {}",
                    a.province,
                    a.city,
                    a.district.as_deref().unwrap_or(""),
                    a.detail
                )
            })
            .unwrap_or_else(|| "—".into());
        let contact = default_addr
            .and_then(|a| {
                if a.contact_name.is_some() || a.contact_phone.is_some() {
                    Some(format!(
                        "{}{}",
                        a.contact_name.as_deref().unwrap_or(""),
                        a.contact_phone
                            .as_deref()
                            .map(|p| format!("  {}", p))
                            .unwrap_or_default(),
                    ))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "—".into());
        (name, addr, contact)
    } else {
        ("—".into(), "—".into(), "—".into())
    };

    // ── Odoo-aligned: Source Document & Warehouse ──
    let order_number = match s.source_id {
        Some(oid) => state
            .sales_order_service()
            .find_by_id(ctx, db, oid)
            .await
            .map(|o| o.doc_number)
            .unwrap_or_else(|_| "—".into()),
        None => "—".into(),
    };
    let warehouse_name = match s.from_warehouse_id {
        Some(wid) => state
            .warehouse_service()
            .get(ctx, db, wid)
            .await
            .map(|w| w.name)
            .unwrap_or_else(|_| "—".into()),
        None => "—".into(),
    };
    // ── Odoo-aligned: Responsible ──
    let operator_name = state
        .user_service()
        .get_user(ctx, db, s.operator_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());

    let print_templates = state
        .print_template_service()
        .list_by_document_type(db, "delivery_note")
        .await
        .unwrap_or_default();

    let (status_text, status_cls) = picking_status_label(s.status);

    // ── Items table (Odoo stock.move Operations tab) ──
    let mut rows = html! {};
    for (idx, it) in items.iter().enumerate() {
        let p = product_map.get(&it.product_id);
        let pcode = p.map(|p| p.product_code.as_str()).unwrap_or("—");
        let pname = p.map(|p| p.pdt_name.as_str()).unwrap_or("—");
        let spec = p.map(|p| p.meta.specification.as_str()).unwrap_or("—");
        let unit = p.map(|p| p.unit.as_str()).unwrap_or("—");
        rows = html! {
            (rows)
            tr class="border-b border-border-soft last:border-b-0 hover:bg-surface/50 transition-colors" {
                td class="py-2.5 px-3 text-muted font-mono" { (idx + 1) }
                td class="py-2.5 px-3 font-mono text-fg font-medium" { (pcode) }
                td class="py-2.5 px-3 text-fg-2" { (pname) }
                td class="py-2.5 px-3 text-muted max-w-[120px] truncate" { (spec) }
                td class="py-2.5 px-3 text-muted" { (unit) }
                td class="py-2.5 px-3 font-mono text-fg text-right tabular-nums" { (fmt_qty(it.qty_requested)) }
                td class="py-2.5 px-3 font-mono text-fg-2 text-right tabular-nums" { (fmt_qty(it.qty_done)) }
            }
        };
    }

    let inner = html! {
        // ── 统计带 ──
        div class="flex rounded-lg bg-surface border border-border-soft mb-5 overflow-hidden" {
            // 待发
            div class="flex-1 px-4 py-3 flex flex-col items-center gap-0.5 border-r border-border-soft" {
                span class="text-lg font-bold text-fg tabular-nums leading-none" { (fmt_qty(hub.pending_ship_qty)) }
                span class="text-[11px] text-muted font-medium" { "待发" }
            }
            // 已发
            div class="flex-1 px-4 py-3 flex flex-col items-center gap-0.5 border-r border-border-soft" {
                span class="text-lg font-bold text-success tabular-nums leading-none" { (fmt_qty(hub.shipped_qty)) }
                span class="text-[11px] text-muted font-medium" { "已发" }
            }
            // 库存
            div class="flex-1 px-4 py-3 flex flex-col items-center gap-0.5" {
                @if hub.shortage.is_some() {
                    span class="text-lg font-bold text-danger tabular-nums leading-none" { "缺货" }
                } @else {
                    span class="text-lg font-bold text-success tabular-nums leading-none" { "充足" }
                }
                span class="text-[11px] text-muted font-medium" { "库存" }
            }
        }
        // ── 来源链 ──
        @if s.source_id.is_some() {
            div class="flex items-center gap-2 text-xs mb-4 px-1" {
                span class="text-muted" { "来源" }
                span class="font-mono text-accent-600 font-medium" { (order_number) }
                span class="text-muted/40" { "→" }
                span class="font-mono text-fg font-semibold" { (s.doc_number) }
            }
        }
        // ── 单据头 ──
        div class="flex items-center justify-between mb-5" {
            div class="flex items-center gap-2.5" {
                span class="text-lg font-bold font-mono text-fg tracking-tight" { (s.doc_number) }
                span class=(format!("inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-semibold {status_cls}")) {
                    (status_text)
                }
            }
            (print_dropdown(
                "wc-print-frame",
                &ShippingPrintPath { id: s.id }.to_string(),
                &print_templates,
                &format!("{}?document_type=delivery_note", PrintTemplateListPath::PATH),
                true,
            ))
        }
        // ── 发货信息 ──
        div class="mb-5" {
            div class="flex items-center gap-2 mb-3" {
                span class="w-1 h-3.5 rounded-full bg-accent shrink-0" {}
                span class="text-xs font-semibold text-fg" { "发货信息" }
            }
            div class="grid grid-cols-2 gap-x-6 gap-y-3 pl-3" {
                div {
                    div class="text-[11px] text-muted mb-0.5" { "客户" }
                    div class="text-sm text-fg font-medium" { (customer_name) }
                }
                div {
                    div class="text-[11px] text-muted mb-0.5" { "发货仓库" }
                    div class="text-sm text-fg-2" { (warehouse_name) }
                }
                div {
                    div class="text-[11px] text-muted mb-0.5" { "收货地址" }
                    div class="text-sm text-fg-2" { (address_text) }
                }
                div {
                    div class="text-[11px] text-muted mb-0.5" { "联系人" }
                    div class="text-sm text-fg-2" { (contact_info) }
                }
            }
        }
        // ── 计划 ──
        div class="mb-5" {
            div class="flex items-center gap-2 mb-3" {
                span class="w-1 h-3.5 rounded-full bg-purple shrink-0" {}
                span class="text-xs font-semibold text-fg" { "计划" }
            }
            div class="grid grid-cols-2 gap-x-6 gap-y-3 pl-3" {
                div {
                    div class="text-[11px] text-muted mb-0.5" { "预计发货" }
                    div class="text-sm font-mono text-fg-2" {
                        (s.scheduled_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into()))
                    }
                }
                div {
                    div class="text-[11px] text-muted mb-0.5" { "操作员" }
                    div class="text-sm text-fg-2" { (operator_name) }
                }
            }
        }
        // ── 产品明细 ──
        div {
            div class="flex items-center gap-2 mb-3" {
                span class="w-1 h-3.5 rounded-full bg-success shrink-0" {}
                span class="text-xs font-semibold text-fg" { "产品明细（" (items.len()) " 项）" }
            }
            @if items.is_empty() {
                div class="rounded-lg border border-border-soft px-4 py-5 text-center text-sm text-muted" { "暂无明细" }
            } @else {
                div class="rounded-lg border border-border-soft overflow-hidden" {
                    table class="w-full text-xs" {
                        thead {
                            tr class="bg-surface/60 border-b border-border-soft" {
                                th class="py-2.5 px-3 text-left text-[11px] text-muted font-semibold w-7" { "#" }
                                th class="py-2.5 px-3 text-left text-[11px] text-muted font-semibold" { "产品编码" }
                                th class="py-2.5 px-3 text-left text-[11px] text-muted font-semibold" { "产品名称" }
                                th class="py-2.5 px-3 text-left text-[11px] text-muted font-semibold" { "规格" }
                                th class="py-2.5 px-3 text-left text-[11px] text-muted font-semibold w-12" { "单位" }
                                th class="py-2.5 px-3 text-right text-[11px] text-muted font-semibold" { "需求数量" }
                                th class="py-2.5 px-3 text-right text-[11px] text-muted font-semibold" { "已发货" }
                            }
                        }
                        tbody { (rows) }
                    }
                }
            }
        }
        // ── 备注 ──
        @if !s.remark.is_empty() {
            div class="mt-4 p-3.5 rounded-lg bg-surface border border-border-soft" {
                div class="flex items-start gap-2" {
                    span class="text-[11px] text-muted font-medium shrink-0 mt-px" { "备注" }
                    span class="text-xs text-fg-2 leading-relaxed" { (s.remark) }
                }
            }
        }
        // 隐藏 iframe：打印按钮 set src 后，print_shipping 响应自带 window.print()
        iframe id="wc-print-frame" class="hidden" {}
    };

    Ok(req_detail_shell("发货详情", inner))
}

// ── 到货详情 drawer（Issue #189：Arrival 单号可点击，按 source_kind 分 PO/WO）──

/// 采购到货详情 drawer body（只读）：PO 头 + 行项目（订购/已收）。
async fn arrival_po_detail_drawer_body(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
) -> Result<Markup> {
    let po_svc = state.purchase_order_service();
    let po = po_svc.get(ctx, db, id).await?;
    let items = po_svc.list_items(ctx, db, id).await.unwrap_or_default();
    let product_map: HashMap<i64, abt_core::master_data::product::model::Product> = state
        .product_service()
        .get_by_ids(ctx, db, items.iter().map(|i| i.product_id).collect())
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.product_id, p))
        .collect();
    let supplier_name = state
        .supplier_service()
        .get(ctx, db, po.supplier_id)
        .await
        .map(|s| s.name)
        .unwrap_or_else(|_| format!("供应商 #{}", po.supplier_id));
    let (status_label, status_cls) = match po.status {
        abt_core::purchase::enums::PurchaseOrderStatus::PartiallyReceived => ("部分到货", "text-warn bg-warn-bg"),
        abt_core::purchase::enums::PurchaseOrderStatus::Confirmed => ("待收货", "text-accent bg-accent-bg"),
        abt_core::purchase::enums::PurchaseOrderStatus::Received => ("已收货", "text-success bg-success-bg"),
        abt_core::purchase::enums::PurchaseOrderStatus::Draft => ("草稿", "text-muted bg-surface"),
        abt_core::purchase::enums::PurchaseOrderStatus::Closed => ("已关闭", "text-muted bg-surface"),
        abt_core::purchase::enums::PurchaseOrderStatus::Cancelled => ("已取消", "text-danger bg-danger-bg"),
        abt_core::purchase::enums::PurchaseOrderStatus::PendingApproval => ("待审批", "text-warn bg-warn-bg"),
    };

    // Items table rows (PO)
    let mut rows = html! {};
    for (idx, it) in items.iter().enumerate() {
        let p = product_map.get(&it.product_id);
        let pcode = p.map(|p| p.product_code.as_str()).unwrap_or("—");
        let pname = p.map(|p| p.pdt_name.as_str()).unwrap_or("—");
        let spec = p.map(|p| p.meta.specification.as_str()).unwrap_or("—");
        let unit = p.map(|p| p.unit.as_str()).unwrap_or("—");
        rows = html! {
            (rows)
            tr class="border-b border-border-soft last:border-b-0 hover:bg-surface/50 transition-colors" {
                td class="py-2.5 px-3 text-muted font-mono" { (idx + 1) }
                td class="py-2.5 px-3 font-mono text-fg font-medium" { (pcode) }
                td class="py-2.5 px-3 text-fg-2" { (pname) }
                td class="py-2.5 px-3 text-muted max-w-[120px] truncate" { (spec) }
                td class="py-2.5 px-3 text-muted" { (unit) }
                td class="py-2.5 px-3 font-mono text-fg text-right tabular-nums" { (fmt_qty(it.quantity)) }
                td class="py-2.5 px-3 font-mono text-fg-2 text-right tabular-nums" { (fmt_qty(it.received_qty)) }
            }
        };
    }

    let inner = html! {
        // ── 单据头 ──
        div class="flex items-center gap-2.5 mb-5" {
            span class="text-lg font-bold font-mono text-fg tracking-tight" { (po.doc_number) }
            span class=(format!("inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-semibold {status_cls}")) {
                (status_label)
            }
        }
        // ── 采购信息 ──
        div class="mb-5" {
            div class="flex items-center gap-2 mb-3" {
                span class="w-1 h-3.5 rounded-full bg-accent shrink-0" {}
                span class="text-xs font-semibold text-fg" { "采购信息" }
            }
            div class="grid grid-cols-2 gap-x-6 gap-y-3 pl-3" {
                div {
                    div class="text-[11px] text-muted mb-0.5" { "供应商" }
                    div class="text-sm text-fg font-medium" { (supplier_name) }
                }
                div {
                    div class="text-[11px] text-muted mb-0.5" { "下单日期" }
                    div class="text-sm font-mono text-fg-2" { (po.created_at.format("%Y-%m-%d").to_string()) }
                }
            }
        }
        // ── 产品明细 ──
        div {
            div class="flex items-center gap-2 mb-3" {
                span class="w-1 h-3.5 rounded-full bg-success shrink-0" {}
                span class="text-xs font-semibold text-fg" { "产品明细（" (items.len()) " 项）" }
            }
            @if items.is_empty() {
                div class="rounded-lg border border-border-soft px-4 py-5 text-center text-sm text-muted" { "暂无明细" }
            } @else {
                div class="rounded-lg border border-border-soft overflow-hidden" {
                    table class="w-full text-xs" {
                        thead {
                            tr class="bg-surface/60 border-b border-border-soft" {
                                th class="py-2.5 px-3 text-left text-[11px] text-muted font-semibold w-7" { "#" }
                                th class="py-2.5 px-3 text-left text-[11px] text-muted font-semibold" { "产品编码" }
                                th class="py-2.5 px-3 text-left text-[11px] text-muted font-semibold" { "产品名称" }
                                th class="py-2.5 px-3 text-left text-[11px] text-muted font-semibold" { "规格" }
                                th class="py-2.5 px-3 text-left text-[11px] text-muted font-semibold w-12" { "单位" }
                                th class="py-2.5 px-3 text-right text-[11px] text-muted font-semibold" { "订购数量" }
                                th class="py-2.5 px-3 text-right text-[11px] text-muted font-semibold" { "已收货" }
                            }
                        }
                        tbody { (rows) }
                    }
                }
            }
        }
    };

    Ok(req_detail_shell("采购到货详情", inner))
}

/// 生产入库详情 drawer body（只读）：工单头 + 产品（完工/已入库）。
async fn arrival_wo_detail_drawer_body(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
) -> Result<Markup> {
    let wo = state.work_order_service().find_by_id(ctx, db, id).await?;
    let product = state.product_service().get(ctx, db, wo.product_id).await?;
    // 已入库量：汇总 inventory_transactions 中 source_type=work_order / source_id=wo.id
    let received: Decimal = state
        .inventory_transaction_service()
        .find_by_source(ctx, db, "work_order", id)
        .await
        .unwrap_or_default()
        .iter()
        .map(|t| t.quantity)
        .sum();
    let pending = wo.completed_qty - received;
    // 来源 SO
    let so_doc = wo.source_so_doc.clone().unwrap_or_else(|| "—".into());
    // 工作中心
    let wc_name = match wo.work_center_id {
        Some(wcid) => state
            .work_center_service()
            .get(ctx, db, wcid)
            .await
            .map(|w| w.name)
            .unwrap_or_else(|_| "—".into()),
        None => "—".into(),
    };
    // 操作员
    let operator_name = state
        .user_service()
        .get_user(ctx, db, wo.operator_id)
        .await
        .map(|u| u.display_name.unwrap_or(u.username))
        .unwrap_or_else(|_| "—".into());

    let (status_text, status_cls) = match wo.status {
        abt_core::mes::enums::WorkOrderStatus::Draft => ("草稿", "bg-surface text-muted"),
        abt_core::mes::enums::WorkOrderStatus::Planned => ("已计划", "bg-accent-bg text-accent"),
        abt_core::mes::enums::WorkOrderStatus::Released => ("已下达", "bg-purple-bg text-purple"),
        abt_core::mes::enums::WorkOrderStatus::InProduction => ("生产中", "bg-warn-bg text-warn"),
        abt_core::mes::enums::WorkOrderStatus::Closed => ("已关闭", "bg-success-bg text-success"),
        abt_core::mes::enums::WorkOrderStatus::Cancelled => ("已取消", "bg-danger-bg text-danger"),
    };

    let inner = html! {
        // ── 统计带 ──
        div class="flex rounded-lg bg-surface border border-border-soft mb-5 overflow-hidden" {
            // 完工
            div class="flex-1 px-4 py-3 flex flex-col items-center gap-0.5 border-r border-border-soft" {
                span class="text-lg font-bold text-fg tabular-nums leading-none" { (fmt_qty(wo.completed_qty)) }
                span class="text-[11px] text-muted font-medium" { "完工" }
            }
            // 已入库
            div class="flex-1 px-4 py-3 flex flex-col items-center gap-0.5 border-r border-border-soft" {
                span class="text-lg font-bold text-success tabular-nums leading-none" { (fmt_qty(received)) }
                span class="text-[11px] text-muted font-medium" { "已入库" }
            }
            // 待入库
            div class="flex-1 px-4 py-3 flex flex-col items-center gap-0.5 border-r border-border-soft" {
                @if pending > Decimal::ZERO {
                    span class="text-lg font-bold text-warn tabular-nums leading-none" { (fmt_qty(pending)) }
                } @else {
                    span class="text-lg font-bold text-muted tabular-nums leading-none" { "—" }
                }
                span class="text-[11px] text-muted font-medium" { "待入库" }
            }
            // 报废
            div class="flex-1 px-4 py-3 flex flex-col items-center gap-0.5" {
                span class="text-lg font-bold text-fg tabular-nums leading-none" { (fmt_qty(wo.scrap_qty)) }
                span class="text-[11px] text-muted font-medium" { "报废" }
            }
        }
        // ── 来源链 ──
        @if so_doc != "—" {
            div class="flex items-center gap-2 text-xs mb-4 px-1" {
                span class="text-muted" { "来源" }
                span class="font-mono text-accent-600 font-medium" { (so_doc) }
                span class="text-muted/40" { "→" }
                span class="font-mono text-fg font-semibold" { (wo.doc_number) }
            }
        }
        // ── 单据头 ──
        div class="flex items-center gap-2.5 mb-5" {
            span class="text-lg font-bold font-mono text-fg tracking-tight" { (wo.doc_number) }
            span class=(format!("inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-semibold {status_cls}")) {
                (status_text)
            }
        }
        // ── 生产信息 ──
        div class="mb-5" {
            div class="flex items-center gap-2 mb-3" {
                span class="w-1 h-3.5 rounded-full bg-accent shrink-0" {}
                span class="text-xs font-semibold text-fg" { "生产信息" }
            }
            div class="grid grid-cols-2 gap-x-6 gap-y-3 pl-3" {
                div {
                    div class="text-[11px] text-muted mb-0.5" { "产品" }
                    div class="text-sm text-fg font-medium" { (product.pdt_name) }
                }
                div {
                    div class="text-[11px] text-muted mb-0.5" { "工作中心" }
                    div class="text-sm text-fg-2" { (wc_name) }
                }
                div {
                    div class="text-[11px] text-muted mb-0.5" { "计划数" }
                    div class="text-sm font-mono text-fg" { (fmt_qty(wo.planned_qty)) " " (product.unit) }
                }
                div {
                    div class="text-[11px] text-muted mb-0.5" { "完工数" }
                    div class="text-sm font-mono text-fg" { (fmt_qty(wo.completed_qty)) " " (product.unit) }
                }
            }
        }
        // ── 计划 ──
        div class="mb-5" {
            div class="flex items-center gap-2 mb-3" {
                span class="w-1 h-3.5 rounded-full bg-purple shrink-0" {}
                span class="text-xs font-semibold text-fg" { "计划" }
            }
            div class="grid grid-cols-2 gap-x-6 gap-y-3 pl-3" {
                div {
                    div class="text-[11px] text-muted mb-0.5" { "开始日期" }
                    div class="text-sm font-mono text-fg-2" { (wo.scheduled_start.format("%Y-%m-%d").to_string()) }
                }
                div {
                    div class="text-[11px] text-muted mb-0.5" { "结束日期" }
                    div class="text-sm font-mono text-fg-2" { (wo.scheduled_end.format("%Y-%m-%d").to_string()) }
                }
                div {
                    div class="text-[11px] text-muted mb-0.5" { "操作员" }
                    div class="text-sm text-fg-2" { (operator_name) }
                }
            }
        }
        // ── 产品明细 ──
        div {
            div class="flex items-center gap-2 mb-3" {
                span class="w-1 h-3.5 rounded-full bg-success shrink-0" {}
                span class="text-xs font-semibold text-fg" { "产品明细" }
            }
            div class="rounded-lg border border-border-soft overflow-hidden" {
                table class="w-full text-xs" {
                    thead {
                        tr class="bg-surface/60 border-b border-border-soft" {
                            th class="py-2.5 px-3 text-left text-[11px] text-muted font-semibold w-7" { "#" }
                            th class="py-2.5 px-3 text-left text-[11px] text-muted font-semibold" { "产品编码" }
                            th class="py-2.5 px-3 text-left text-[11px] text-muted font-semibold" { "产品名称" }
                            th class="py-2.5 px-3 text-left text-[11px] text-muted font-semibold" { "规格" }
                            th class="py-2.5 px-3 text-left text-[11px] text-muted font-semibold w-12" { "单位" }
                            th class="py-2.5 px-3 text-right text-[11px] text-muted font-semibold" { "计划数" }
                            th class="py-2.5 px-3 text-right text-[11px] text-muted font-semibold" { "完工数" }
                            th class="py-2.5 px-3 text-right text-[11px] text-muted font-semibold" { "已入库" }
                        }
                    }
                    tbody {
                        tr class="border-b border-border-soft last:border-b-0 hover:bg-surface/50 transition-colors" {
                            td class="py-2.5 px-3 text-muted font-mono" { "1" }
                            td class="py-2.5 px-3 font-mono text-fg font-medium" { (product.product_code) }
                            td class="py-2.5 px-3 text-fg-2" { (product.pdt_name) }
                            td class="py-2.5 px-3 text-muted max-w-[120px] truncate" { (product.meta.specification) }
                            td class="py-2.5 px-3 text-muted" { (product.unit) }
                            td class="py-2.5 px-3 font-mono text-fg text-right tabular-nums" { (fmt_qty(wo.planned_qty)) }
                            td class="py-2.5 px-3 font-mono text-fg text-right tabular-nums" { (fmt_qty(wo.completed_qty)) }
                            td class="py-2.5 px-3 font-mono text-fg-2 text-right tabular-nums" { (fmt_qty(received)) }
                        }
                    }
                }
            }
        }
        // ── 备注 ──
        @if !wo.remark.is_empty() {
            div class="mt-4 p-3.5 rounded-lg bg-surface border border-border-soft" {
                div class="flex items-start gap-2" {
                    span class="text-[11px] text-muted font-medium shrink-0 mt-px" { "备注" }
                    span class="text-xs text-fg-2 leading-relaxed" { (wo.remark) }
                }
            }
        }
    };

    Ok(req_detail_shell("生产入库详情", inner))
}

// ── 调拨全部视图 + 详情 drawer（阶段 3.2 收口，模式同领料单）──

/// 调拨详情 drawer body（替代独立 detail 页，阶段 3.2 收口）：
/// 单据头（单号/状态/来源仓→目标仓/日期）+ 行项目（产品/数量）+ 就地操作（取消/调出/完成）。
async fn transfer_detail_drawer_body(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
    view: Option<&str>,
) -> Result<Markup> {
    let trf_svc = state.picking_service();
    let trf = trf_svc.get(ctx, db, id).await?;
    let items = trf_svc.list_items(ctx, db, id).await.unwrap_or_default();
    let product_map: HashMap<i64, abt_core::master_data::product::model::Product> = state
        .product_service()
        .get_by_ids(ctx, db, items.iter().map(|i| i.product_id).collect())
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.product_id, p))
        .collect();
    let from_wh = state
        .warehouse_service()
        .get(ctx, db, trf.from_warehouse_id.unwrap_or(0))
        .await
        .map(|w| w.name)
        .unwrap_or_else(|_| "—".into());
    let to_wh = state
        .warehouse_service()
        .get(ctx, db, trf.to_warehouse_id.unwrap_or(0))
        .await
        .map(|w| w.name)
        .unwrap_or_else(|_| "—".into());

    let (status_text, status_cls) = picking_status_label(trf.status);
    let view_val = view.unwrap_or("pending");

    let mut rows = html! {};
    for it in &items {
        let pname = product_map
            .get(&it.product_id)
            .map(|p| p.pdt_name.clone())
            .unwrap_or_else(|| format!("产品 #{}", it.product_id));
        let pcode = product_map
            .get(&it.product_id)
            .map(|p| p.product_code.clone())
            .unwrap_or_default();
        rows = html! {
            (rows)
            div class="flex items-center justify-between px-3 py-2 gap-2" {
                div class="min-w-0" {
                    div class="text-sm text-fg-2 truncate" { (pname) }
                    div class="text-xs text-muted truncate" { (pcode) }
                }
                span class="text-sm font-mono text-fg shrink-0" { (fmt_qty(it.qty_requested)) }
            }
        };
    }

    let inner = html! {
        div class="mb-4 pb-4 border-b border-border-soft" {
            div class="flex items-center gap-2 mb-3" {
                span class="text-base font-mono font-bold text-fg" { (trf.doc_number) }
                span class=(format!("inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium {status_cls}")) {
                    (status_text)
                }
            }
            div class="grid grid-cols-2 gap-x-4 gap-y-2 text-xs" {
                div {
                    span class="text-muted" { "来源仓 " }
                    span class="text-fg-2" { (from_wh) }
                }
                div {
                    span class="text-muted" { "目标仓 " }
                    span class="text-fg-2" { (to_wh) }
                }
                div {
                    span class="text-muted" { "调拨日期 " }
                    span class="font-mono text-fg-2" {
                        (trf.scheduled_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "—".into()))
                    }
                }
            }
        }
        div class="mb-2" {
            div class="text-xs font-semibold text-muted mb-2" { "明细（" (items.len()) " 项）" }
            div class="rounded-sm border border-border-soft divide-y divide-border-soft" {
                @if items.is_empty() {
                    div class="px-3 py-4 text-center text-sm text-muted" { "暂无明细" }
                } @else {
                    (rows)
                }
            }
        }
        (transfer_detail_actions(trf.status, id, view_val))
    };

    Ok(req_detail_shell("调拨详情", inner))
}

/// 调拨详情 drawer 操作：Draft→取消/调出，InTransit→完成。各自 form 提交单端点。
fn transfer_detail_actions(status: PickingStatus, id: i64, view: &str) -> Markup {
    let open_hs =
        "on 'htmx:afterRequest'[detail.xhr.status<400] remove .open from #wc-drawer-overlay";
    let cancel_btn = |confirm: &str| -> Markup {
        html! {
            form hx-post=(WmsWorkCenterPath::PATH) hx-target="#wc-domain-card" hx-select="#wc-domain-card"
                hx-swap="outerHTML" hx-confirm=(confirm) _=(open_hs) class="contents" {
                input type="hidden" name="action" value="transfer_cancel";
                input type="hidden" name="id" value=(id);
                input type="hidden" name="view" value=(view);
                button type="submit"
                    class="inline-flex items-center gap-1.5 px-4 py-2 rounded-sm bg-white text-fg-2 border border-border text-sm font-medium cursor-pointer hover:bg-surface" {
                    (icon::x_icon("w-4 h-4")) "取消单据"
                }
            }
        }
    };
    let primary_btn = |label: &str, action: &str, confirm: &str, ic: Markup| -> Markup {
        html! {
            form hx-post=(WmsWorkCenterPath::PATH) hx-target="#wc-domain-card" hx-select="#wc-domain-card"
                hx-swap="outerHTML" hx-confirm=(confirm) _=(open_hs) class="contents" {
                input type="hidden" name="action" value=(action);
                input type="hidden" name="id" value=(id);
                input type="hidden" name="view" value=(view);
                button type="submit"
                    class="inline-flex items-center gap-1.5 px-4 py-2 rounded-sm bg-accent text-white text-sm font-medium cursor-pointer border-none hover:opacity-90" {
                    (ic) (label)
                }
            }
        }
    };
    match status {
        PickingStatus::Draft => html! {
            div class="flex justify-end gap-3 mt-5 pt-4 border-t border-border-soft" {
                (cancel_btn("确定取消此调拨单？"))
                (primary_btn("调出", "dispatch", "确定调出？将从来源仓扣减库存", icon::upload_icon("w-4 h-4")))
            }
        },
        PickingStatus::Confirmed => html! {
            div class="flex justify-end gap-3 mt-5 pt-4 border-t border-border-soft" {
                (primary_btn("完成调拨", "complete", "确定完成调拨？将入库到目标仓", icon::check_circle_icon("w-4 h-4")))
            }
        },
        _ => html! {},
    }
}

// ── 盘点全部视图 + 详情 drawer（阶段 3.2b 收口；count 录入 UI 原未实现，drawer 不含录入）──

/// 盘点状态 → (标签, 语义色 class)。
fn cc_status_label(s: CycleCountStatus) -> (&'static str, &'static str) {
    match s {
        CycleCountStatus::Draft => ("草稿", "bg-surface text-muted"),
        CycleCountStatus::Counting => ("盘点中", "bg-warn-bg text-warn"),
        CycleCountStatus::Completed => ("已完成", "bg-accent-bg text-accent"),
        CycleCountStatus::Adjusted => ("已调整", "bg-accent-bg text-accent"),
        CycleCountStatus::Cancelled => ("已取消", "bg-danger-bg text-danger"),
        CycleCountStatus::PendingReview => ("待审批", "bg-warn-bg text-warn"),
    }
}

/// 盘点详情 drawer body（替代独立 detail 页，阶段 3.2b 收口）：
/// 单据头（单号/状态/仓库/日期/盲盘）+ 行项目（系/盘/差三量）+ 就地操作（start/complete/cancel/adjust/approve/reject）。
/// 注：count（录入实盘量）UI 原详情页未实现，drawer 沿用——明细只读展示。
async fn cc_detail_drawer_body(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
    view: Option<&str>,
) -> Result<Markup> {
    let cc_svc = state.cycle_count_service();
    let cc = cc_svc.get(ctx, db, id).await?;
    let items: Vec<CycleCountItem> = cc_svc.get_items(ctx, db, id).await.unwrap_or_default();
    let product_map: HashMap<i64, abt_core::master_data::product::model::Product> = state
        .product_service()
        .get_by_ids(ctx, db, items.iter().map(|i| i.product_id).collect())
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.product_id, p))
        .collect();
    let wh_name = state
        .warehouse_service()
        .get(ctx, db, cc.warehouse_id)
        .await
        .map(|w| w.name)
        .unwrap_or_else(|_| "—".into());

    let (status_text, status_cls) = cc_status_label(cc.status);
    let view_val = view.unwrap_or("pending");

    let mut rows = html! {};
    for it in &items {
        let pname = product_map
            .get(&it.product_id)
            .map(|p| p.pdt_name.clone())
            .unwrap_or_else(|| format!("产品 #{}", it.product_id));
        let pcode = product_map
            .get(&it.product_id)
            .map(|p| p.product_code.clone())
            .unwrap_or_default();
        let variance_cls =
            if it.variance_qty == rust_decimal::Decimal::ZERO { "text-muted" } else { "text-warn" };
        rows = html! {
            (rows)
            div class="flex items-center justify-between px-3 py-2 gap-2" {
                div class="min-w-0" {
                    div class="text-sm text-fg-2 truncate" { (pname) }
                    div class="text-xs text-muted truncate" { (pcode) }
                }
                div class="text-right shrink-0 text-xs font-mono" {
                    div class="text-fg" { "系 " (fmt_qty(it.system_qty)) " · 盘 " (fmt_qty(it.counted_qty)) }
                    div class=(variance_cls) { "差 " (fmt_qty(it.variance_qty)) }
                }
            }
        };
    }

    let inner = html! {
        div class="mb-4 pb-4 border-b border-border-soft" {
            div class="flex items-center gap-2 mb-3" {
                span class="text-base font-mono font-bold text-fg" { (cc.doc_number) }
                span class=(format!("inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium {status_cls}")) {
                    (status_text)
                }
            }
            div class="grid grid-cols-2 gap-x-4 gap-y-2 text-xs" {
                div {
                    span class="text-muted" { "盘点仓库 " }
                    span class="text-fg-2" { (wh_name) }
                }
                div {
                    span class="text-muted" { "盘点日期 " }
                    span class="font-mono text-fg-2" { (cc.count_date.format("%Y-%m-%d")) }
                }
                @if cc.is_blind {
                    div {
                        span class="text-muted" { "模式 " }
                        span class="text-fg-2" { "盲盘" }
                    }
                }
            }
        }
        div class="mb-2" {
            div class="text-xs font-semibold text-muted mb-2" { "明细（" (items.len()) " 项）" }
            div class="rounded-sm border border-border-soft divide-y divide-border-soft" {
                @if items.is_empty() {
                    div class="px-3 py-4 text-center text-sm text-muted" { "暂无明细" }
                } @else {
                    (rows)
                }
            }
        }
        (cc_detail_actions(cc.status, id, view_val))
    };

    Ok(req_detail_shell("盘点详情", inner))
}

/// 盘点详情 drawer 操作：Draft→开始/取消，Counting→完成，Completed→调整/取消，
/// PendingReview→批准/驳回。各自 form 提交单端点（cc_ 前缀 action）。
fn cc_detail_actions(status: CycleCountStatus, id: i64, view: &str) -> Markup {
    let open_hs =
        "on 'htmx:afterRequest'[detail.xhr.status<400] remove .open from #wc-drawer-overlay";
    let cancel_btn = |confirm: &str| -> Markup {
        html! {
            form hx-post=(WmsWorkCenterPath::PATH) hx-target="#wc-domain-card" hx-select="#wc-domain-card"
                hx-swap="outerHTML" hx-confirm=(confirm) _=(open_hs) class="contents" {
                input type="hidden" name="action" value="cc_cancel";
                input type="hidden" name="id" value=(id);
                input type="hidden" name="view" value=(view);
                button type="submit"
                    class="inline-flex items-center gap-1.5 px-4 py-2 rounded-sm bg-white text-fg-2 border border-border text-sm font-medium cursor-pointer hover:bg-surface" {
                    (icon::x_icon("w-4 h-4")) "取消单据"
                }
            }
        }
    };
    let primary_btn = |label: &str, action: &str, confirm: &str, ic: Markup| -> Markup {
        html! {
            form hx-post=(WmsWorkCenterPath::PATH) hx-target="#wc-domain-card" hx-select="#wc-domain-card"
                hx-swap="outerHTML" hx-confirm=(confirm) _=(open_hs) class="contents" {
                input type="hidden" name="action" value=(action);
                input type="hidden" name="id" value=(id);
                input type="hidden" name="view" value=(view);
                button type="submit"
                    class="inline-flex items-center gap-1.5 px-4 py-2 rounded-sm bg-accent text-white text-sm font-medium cursor-pointer border-none hover:opacity-90" {
                    (ic) (label)
                }
            }
        }
    };
    match status {
        CycleCountStatus::Draft => html! {
            div class="flex justify-end gap-3 mt-5 pt-4 border-t border-border-soft" {
                (cancel_btn("确定取消此盘点单？"))
                (primary_btn("开始盘点", "cc_start", "确定开始盘点？", icon::plus_icon("w-4 h-4")))
            }
        },
        CycleCountStatus::Counting => html! {
            div class="flex justify-end gap-3 mt-5 pt-4 border-t border-border-soft" {
                (primary_btn("完成盘点", "cc_complete", "确定完成盘点？", icon::check_circle_icon("w-4 h-4")))
            }
        },
        CycleCountStatus::Completed => html! {
            div class="flex justify-end gap-3 mt-5 pt-4 border-t border-border-soft" {
                (cancel_btn("确定取消此盘点单？"))
                (primary_btn("调整库存", "cc_adjust", "确定按差异调整库存？", icon::bolt_icon("w-4 h-4")))
            }
        },
        CycleCountStatus::PendingReview => html! {
            div class="flex justify-end gap-3 mt-5 pt-4 border-t border-border-soft" {
                (primary_btn("驳回", "cc_reject", "确定驳回此盘点单？", icon::x_icon("w-4 h-4")))
                (primary_btn("批准", "cc_approve", "确定批准此盘点单？", icon::check_circle_icon("w-4 h-4")))
            }
        },
        _ => html! {},
    }
}

// ── Handlers（单端点）──

/// 作业中心唯一 GET：按 query 分支——drawer body / 卡片 body（懒加载）/ 整页
#[require_permission("INVENTORY", "read")]
pub async fn get_wms_work_center(
    _path: WmsWorkCenterPath,
    axum::extract::Query(q): axum::extract::Query<WorkCenterQuery>,
    ctx: RequestContext,
) -> Result<Html<String>> {
    // drawer body：加载某就地操作表单（点行内按钮 hx-get 填入 #wc-drawer-body）
    if let (Some(drawer), Some(id)) = (q.drawer.as_deref(), q.id) {
        return render_drawer_body(drawer, id, q.view.as_deref(), ctx).await;
    }
    // 否则：tab 主体（非 htmx 整页 / htmx 片段，按 domain + filter + page 渲染）
    render_work_center_page(ctx, &q).await
}

/// Issue #219：direct_ship 成功后的「出库成功」对话框（hx-swap-oob，根 id=wc-ship-success-modal）。
/// 含 print_dropdown 打印按钮（set #wc-ship-print-frame src → print_shipping 自带 window.print()）
/// +「完成」按钮。背景点击 / 完成 关闭。开：app.js 监听 openShipSuccess（= HX-Trigger-After-Settle
/// 广播）给本节点加 .is-open；用 JS 监听而非 hyperscript from:body（后者在本项目不可靠，对齐
/// wms_stock_in_create 的 body 事件走 document.addEventListener 范式）。
fn ship_success_modal(doc_number: &str, picking_id: i64, print_templates: &[PrintTemplate], manage_url: &str) -> Markup {
    let print_url = ShippingPrintPath { id: picking_id }.to_string();
    html! {
        div id="wc-ship-success-modal" hx-swap-oob="true"
            class="fixed inset-0 z-[1100] grid place-items-center bg-[rgba(15,23,42,0.45)] \
                   backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200 \
                   [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto"
            _="on click[me is event.target] remove .is-open from me" {
            div class="modal bg-bg rounded-xl w-[440px] max-w-[92vw] flex flex-col overflow-hidden shadow-xl"
                _="on click halt the event" {
                div class="px-6 pt-6 pb-3 flex flex-col items-center text-center" {
                    (icon::check_circle_icon("w-11 h-11 text-success"))
                    div class="mt-2 text-lg font-bold text-fg" { "出库成功" }
                    div class="mt-1 text-sm text-muted" {
                        "发货单 " span class="font-mono text-fg-2" { (doc_number) } " 已确认发出，可打印送货单"
                    }
                }
                div class="px-6 py-4 bg-surface border-t border-border-soft flex items-center justify-end gap-3" {
                    button type="button"
                        class="px-4 py-2 rounded-sm bg-white text-fg-2 border border-border text-sm font-medium cursor-pointer hover:bg-surface"
                        _="on click remove .is-open from #wc-ship-success-modal" { "完成" }
                    (print_dropdown("wc-ship-print-frame", &print_url, print_templates, manage_url, true))
                }
            }
        }
    }
}

/// 作业中心唯一 POST：执行就地操作，返回「当前 tab 主体 + 总数 badge oob」片段。
/// 客户端 hx-target=#wc-domain-card 替换 tab 主体、响应内 #wc-total-badge(hx-swap-oob) 更新顶栏总数、hyperscript 关 drawer。
#[require_permission("INVENTORY", "update")]
pub async fn post_work_center_action(
    _path: WmsWorkCenterPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<WorkCenterActionForm>,
) -> Result<impl IntoResponse> {
    let domain = action_domain(&form.action)?;
    let RequestContext { state, service_ctx, mut conn, .. } = ctx;
    let svc = state.wms_work_center_service();

    // 多步写事务包裹（范本 shipping_detail::ship_shipping）
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    dispatch_action(&state, &service_ctx, &mut tx, &form).await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    // 成功 toast（HX-Trigger: showToast 触发客户端到 /api/toast 拉取渲染）
    crate::toast::add_toast(
        service_ctx.operator_id,
        action_success_msg(&form.action),
        crate::toast::ToastType::Success,
    );

    // 重渲染当前 tab 主体（受影响 domain）+ 顶栏总数 badge（oob）。
    // Requisition 全部视图下的操作：重渲染全部 card（保持视图不跳回待办）；其余渲染待办队列。
    let summary = svc.summary(&service_ctx, &mut conn).await.unwrap_or_default();
    let warehouses = state
        .warehouse_service()
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
        .await
        .map(|r| r.items)
        .unwrap_or_default();

    let result = svc
        .list_pending(
            &service_ctx,
            &mut conn,
            domain,
            PendingTaskFilter::default(),
            PageParams::new(1, DOMAIN_PAGE_SIZE),
        )
        .await
        .unwrap_or_else(|_| PaginatedResult::empty(1, DOMAIN_PAGE_SIZE));
    // 主从表物料（按 domain）：picking 类（编码/名称/数量）+ cycle（盘点列）
    let picking_materials: Option<HashMap<i64, Vec<MaterialCell>>> = match domain {
        WorkCenterDomain::Arrival => {
            Some(build_arrival_materials(&state, &service_ctx, &mut conn, &result.items).await)
        }
        WorkCenterDomain::Outbound | WorkCenterDomain::Requisition | WorkCenterDomain::Transfer => {
            Some(build_picking_materials(&state, &service_ctx, &mut conn, &result.items).await)
        }
        _ => None,
    };
    let cycle_materials: Option<HashMap<i64, Vec<CycleCell>>> =
        if domain == WorkCenterDomain::CycleCount {
            Some(build_cycle_materials(&state, &service_ctx, &mut conn, &result.items).await)
        } else {
            None
        };
    let fragment: Markup = html! {
        (render_domain_card(
            domain,
            &summary,
            &result,
            &WorkCenterQuery::default(),
            &warehouses,
            picking_materials.as_ref(),
            cycle_materials.as_ref(),
        ))
        // 顶栏总数 badge：hx-swap-oob 自动替换页面 #wc-total-badge
        (total_badge(summary.total(), true))
    };
    // Issue #219：direct_ship 成功后弹「出库成功」对话框（含打印按钮），不再直接弹打印预览。
    // 对话框经 hx-swap-oob 替换页面 #wc-ship-success-modal，并用 HX-Trigger-After-Settle
    // 触发 openShipSuccess 事件（等 OOB swap 完对话框监听器就位再弹，§4.4 时序）。
    if form.action == "direct_ship" {
        let picking = state
            .picking_service()
            .find_by_id(&service_ctx, &mut conn, form.id)
            .await?;
        let print_templates = state
            .print_template_service()
            .list_by_document_type(&mut conn, "delivery_note")
            .await
            .unwrap_or_default();
        let manage_url = format!("{}?document_type=delivery_note", PrintTemplateListPath::PATH);
        let modal = ship_success_modal(&picking.doc_number, form.id, &print_templates, &manage_url);
        let body = format!("{}{}", fragment.into_string(), modal.into_string());
        return Ok((
            [("HX-Trigger-After-Settle", r#"{"openShipSuccess":""}"#)],
            Html(body),
        ));
    }
    // Issue #270：osa_issue 改变委外单状态，追加 outsourcingChanged 让采购作业中心「委外订单」tab 同步刷新。
    let trigger = if form.action == "osa_issue" {
        "showToast, outsourcingChanged"
    } else {
        "showToast"
    };
    Ok(([("HX-Trigger", trigger)], Html(fragment.into_string())))
}

/// 按 action 分发到各域 service（均在传入事务内执行）
async fn dispatch_action(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    form: &WorkCenterActionForm,
) -> Result<()> {
    match form.action.as_str() {
        "receive_po" => {
            // 采购 PO 直收入库闭环（取消来料通知后）：receive_purchase 事务内
            // record 库存 + 回写 PO received_qty/状态 + 立应付 + 成本。幂等由 service 内 try_claim。
            let rows: Vec<ReceiveRowJson> = parse_items_json(form)?;
            let po_rows: Vec<PoReceiveRow> = rows
                .into_iter()
                .map(|r| -> Result<PoReceiveRow> {
                    Ok(PoReceiveRow {
                        order_item_id: r
                            .order_item_id
                            .as_deref()
                            .filter(|s| !s.is_empty())
                            .ok_or_else(|| DomainError::validation("缺少订单明细行 order_item_id"))?
                            .parse::<i64>()
                            .map_err(|e| DomainError::validation(format!("order_item_id 解析失败: {e}")))?,
                        product_id: r
                            .product_id
                            .parse::<i64>()
                            .map_err(|e| DomainError::validation(format!("product_id 解析失败: {e}")))?,
                        received_qty: r
                            .received_qty
                            .parse::<Decimal>()
                            .map_err(|e| DomainError::validation(format!("收货数量解析失败: {e}")))?,
                        batch_no: r.batch_no.filter(|s| !s.is_empty()),
                        warehouse_id: r
                            .warehouse_id
                            .as_deref()
                            .filter(|s| !s.is_empty())
                            .ok_or_else(|| DomainError::validation("每行必须选择目标仓库"))?
                            .parse::<i64>()
                            .map_err(|e| DomainError::validation(format!("仓库解析失败: {e}")))?,
                        bin_id: parse_opt_i64(&r.bin_id, "目标库位")?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            state
                .picking_service()
                .receive_purchase(
                    ctx,
                    db,
                    ReceivePurchaseReq {
                        po_id: form.id,
                        rows: po_rows,
                        delivery_note: form.delivery_note.clone(),
                        remark: form.remark.clone(),
                        idempotency_key: form.idempotency_key.clone(),
                    },
                )
                .await?;
        }
        "receive_wo" => {
            // 生产工单入库：仅 record 库存（source=work_order），不立应付、不回写 completed_qty（报工已累加）
            let rows: Vec<ReceiveRowJson> = parse_items_json(form)?;
            let inv_svc = state.inventory_transaction_service();
            let wh_svc = state.warehouse_service();
            let wo = state.work_order_service().find_by_id(ctx, db, form.id).await?;
            let doc_number = state
                .document_sequence_service()
                .next_number(ctx, db, DocumentType::StockReceipt)
                .await?;
            for r in rows {
                let product_id = r
                    .product_id
                    .parse::<i64>()
                    .map_err(|e| DomainError::validation(format!("product_id 解析失败: {e}")))?;
                let qty = r
                    .received_qty
                    .parse::<Decimal>()
                    .map_err(|e| DomainError::validation(format!("收货数量解析失败: {e}")))?;
                let warehouse_id = r
                    .warehouse_id
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| DomainError::validation("必须选择目标仓库"))?
                    .parse::<i64>()
                    .map_err(|e| DomainError::validation(format!("仓库解析失败: {e}")))?;
                let bin_id = parse_opt_i64(&r.bin_id, "目标库位")?;
                let zone_id = wh_svc
                    .get_or_create_default_zone(ctx, db, warehouse_id)
                    .await
                    .ok()
                    .map(|z| z.id);
                let default_bin = if let Some(zid) = zone_id {
                    wh_svc
                        .list_bins(ctx, db, zid, None, 1, 1)
                        .await
                        .ok()
                        .and_then(|x| x.items.first().map(|b| b.id))
                } else {
                    None
                };
                inv_svc
                    .record(
                        ctx,
                        db,
                        RecordTransactionReq {
                            doc_number: Some(doc_number.clone()),
                            delivery_no: None,
                            source_doc_number: Some(wo.doc_number.clone()),
                            transaction_type: TransactionType::ProductionReceipt,
                            product_id,
                            warehouse_id,
                            zone_id,
                            bin_id: bin_id.or(default_bin),
                            batch_no: r.batch_no.filter(|s| !s.is_empty()),
                            quantity: qty,
                            unit_cost: None,
                            source_type: "work_order".to_string(),
                            source_id: form.id,
                            remark: None,
                        },
                    )
                    .await?;
            }
        }
        "direct_ship" => {
            // 直接发（Confirmed 待发货单）：仓库由选仓 drawer 传入
            let warehouse_id = parse_warehouse(form)?;
            // 行级库位/批次/数量：drawer items_json → ShipRowReq；batch_ship 走旧 direct_ship
            let ship_rows: Vec<ShipRowJson> = parse_items_json(form)?;
            let rows: Vec<ShipRowReq> = ship_rows.into_iter().map(|r| ShipRowReq {
                picking_item_id: r.picking_item_id.parse().unwrap_or(0),
                warehouse_id: r.warehouse_id.parse().unwrap_or(0),
                qty: r.qty.parse().unwrap_or(Decimal::ZERO),
                bin_id: r.bin_id.and_then(|s| s.parse().ok()),
                batch_no: r.batch_no.filter(|s| !s.is_empty()),
            }).collect();
            state.picking_service().direct_ship_rows(ctx, db, form.id, warehouse_id, rows).await?;
        }
        "batch_ship" => {
            // 批量直接发（待发货单）：循环 direct_ship，外层 tx 任一失败 → 整体回滚
            let ids = parse_ids(form)?;
            if ids.is_empty() {
                return Err(DomainError::validation("未选择待发货单").into());
            }
            let warehouse_id = parse_warehouse(form)?;
            for id in ids {
                state.picking_service().direct_ship(ctx, db, id, warehouse_id, None).await?;
            }
        }
        "confirm" => {
            state.picking_service().confirm(ctx, db, form.id).await?;
        }
        "cancel" => {
            state.picking_service().cancel(ctx, db, form.id).await?;
        }
        "issue" => {
            // 发料：drawer 选定出货仓库 + 行级库位/实发（items_json）；issue 按选仓 update_from_warehouse + 行 bin 扣
            let req_svc = state.picking_service();
            let rows: Vec<ShipRowJson> = parse_items_json(form)?;
            if rows.is_empty() {
                return Err(DomainError::validation("发料明细不能为空").into());
            }
            let warehouse_id = rows[0].warehouse_id.parse::<i64>().unwrap_or(0);
            if warehouse_id <= 0 {
                return Err(DomainError::validation("请选择出货仓库").into());
            }
            let issue_items = rows
                .into_iter()
                .map(|r| IssueItemReq {
                    item_id: r.picking_item_id.parse().unwrap_or(0),
                    issued_qty: r.qty.parse().unwrap_or(Decimal::ZERO),
                    bin_id: r.bin_id.and_then(|s| s.parse().ok()),
                })
                .collect::<Vec<_>>();
            req_svc
                .issue(ctx, db, IssueMaterialReq { id: form.id, warehouse_id, items: issue_items })
                .await?;
        }
        "transfer_cancel" => {
            state.picking_service().cancel(ctx, db, form.id).await?;
        }
        "dispatch" => {
            state.picking_service().dispatch(ctx, db, form.id).await?;
        }
        "complete" => {
            state.picking_service().complete(ctx, db, form.id).await?;
        }
        "cc_start" => {
            state.cycle_count_service().start_count(ctx, db, form.id).await?;
        }
        "cc_complete" => {
            state.cycle_count_service().complete(ctx, db, form.id).await?;
        }
        "cc_cancel" => {
            state.cycle_count_service().cancel(ctx, db, form.id).await?;
        }
        "cc_adjust" => {
            state.cycle_count_service().adjust(ctx, db, form.id).await?;
        }
        "cc_approve" => {
            state.cycle_count_service().approve(ctx, db, form.id).await?;
        }
        "cc_reject" => {
            state.cycle_count_service().reject(ctx, db, form.id).await?;
        }
        "ack_low_stock" => {
            state.low_stock_alert_service().ack(ctx, db, form.id).await?;
        }
        "osa_issue" => {
            // Issue #270 委外发料（仓库执行）：form.id = OutsourceIssue picking id。
            // 一张发料单可能含多个源仓的原材料 → execute_outsource_issue 逐行按实际仓扣源仓 +
            // 统一入虚拟仓 + 置 Done；再 om.confirm_sent 回写 OSA Draft→Sent。事务内原子提交。
            let pk_svc = state.picking_service();
            let picking = pk_svc.get(ctx, db, form.id).await?;
            let osa_id = picking.source_id.ok_or_else(|| {
                DomainError::validation("委外发料单未关联委外单（source_id 缺失）")
            })?;
            pk_svc.execute_outsource_issue(ctx, db, form.id).await?;
            let osa = state
                .outsourcing_order_service()
                .find_by_id(ctx, db, osa_id)
                .await?;
            state
                .outsourcing_order_service()
                .confirm_sent(
                    ctx,
                    db,
                    ConfirmSentReq {
                        id: osa_id,
                        expected_version: osa.version,
                        remark: Some("仓库发料完成".into()),
                    },
                )
                .await?;
        }
        other => return Err(DomainError::validation(format!("未知作业动作: {other}")).into()),
    }
    Ok(())
}

/// 就地操作成功 toast 文案（按 action 语义给反馈）
fn action_success_msg(action: &str) -> &'static str {
    match action {
        "receive_po" => "收货入库完成",
        "receive_wo" => "工单入库完成",
        "direct_ship" | "batch_ship" => "发货完成",
        "issue" => "发料完成",
        "confirm" => "已确认",
        "cancel" | "transfer_cancel" => "已取消",
        "dispatch" => "已发车",
        "complete" => "已完成",
        "cc_start" => "盘点已开始",
        "cc_complete" => "盘点已完成",
        "cc_cancel" => "盘点已取消",
        "cc_adjust" => "盘点已调整",
        "cc_approve" => "盘点已审批",
        "cc_reject" => "盘点已驳回",
        "ack_low_stock" => "已确认",
        "osa_issue" => "委外发料完成",
        _ => "操作完成",
    }
}

/// 解析批量 action 的 ids（逗号分隔 → Vec<i64>），仅 batch_* action 用。
fn parse_ids(form: &WorkCenterActionForm) -> Result<Vec<i64>> {
    form.ids
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<i64>().map_err(|e| DomainError::validation(format!("id 解析失败: {e}")).into()))
        .collect()
}

/// 解析发货仓库（direct_ship / batch_ship 用，选仓 drawer / 批量栏传入）。
fn parse_warehouse(form: &WorkCenterActionForm) -> Result<i64> {
    form.warehouse_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| DomainError::validation("请选择发货仓库").into())
        .and_then(|s| s.parse::<i64>().map_err(|e| DomainError::validation(format!("仓库解析失败: {e}")).into()))
}

fn parse_items_json<T: serde::de::DeserializeOwned>(form: &WorkCenterActionForm) -> Result<Vec<T>> {
    Ok(serde_json::from_str::<Vec<T>>(form.items_json.as_deref().unwrap_or("[]"))
        .map_err(|e| DomainError::validation(format!("明细解析失败: {e}")))?)
}

/// 可选整型解析：None / 空串 → None；否则 parse。用于发料仓库/库位（wcCollectItems 收的是字符串）。
fn parse_opt_i64(s: &Option<String>, label: &str) -> Result<Option<i64>> {
    match s {
        None => Ok(None),
        Some(v) if v.trim().is_empty() => Ok(None),
        Some(v) => v
            .parse::<i64>()
            .map(Some)
            .map_err(|e| DomainError::validation(format!("{label}解析失败: {e}")).into()),
    }
}

// 行级明细走 hidden items_json（JSON 字符串），字段统一用 String（i.value 为字符串），服务端再 parse
// 对齐 quotation/sales_order 的 ItemWeb 范式（见 static/app.js lineItemCalc.collectItems）
/// 收货 drawer 行级明细（直入库：每行带目标仓库/库位，提交走 stock_in_from_notice）
#[derive(Debug, Deserialize)]
/// 行级发货明细（direct_ship drawer items_json）
struct ShipRowJson {
    picking_item_id: String,
    warehouse_id: String,
    qty: String,
    bin_id: Option<String>,
    batch_no: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReceiveRowJson {
    /// 采购明细行 id（receive_po 必填；receive_wo 工单入库不用 = None）
    order_item_id: Option<String>,
    product_id: String,
    received_qty: String,
    batch_no: Option<String>,
    warehouse_id: Option<String>,
    bin_id: Option<String>,
}

// ── 页面 / 片段渲染 ──

/// 渲染作业中心：非 htmx → 整页（标题 + 总数 badge + tab 主体 + drawer/picker 壳）；
/// 待收货物料单元格（编码/名称/数量）——Arrival 主从表 rowspan 明细列。
#[derive(Clone)]
struct MaterialCell {
    code: String,
    name: String,
    qty: Decimal,
}

/// 待收货(Arrival)：装配每个单据的物料 cells（po 多物料 / wo 单产品），供主从表 rowspan 常驻显示。
async fn build_arrival_materials(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    tasks: &[PendingTask],
) -> HashMap<i64, Vec<MaterialCell>> {
    let po_ids: Vec<i64> = tasks
        .iter()
        .filter(|t| t.source_kind == TaskSourceKind::PurchaseOrder)
        .map(|t| t.doc_id)
        .collect();
    let wo_ids: Vec<i64> = tasks
        .iter()
        .filter(|t| t.source_kind == TaskSourceKind::WorkOrder)
        .map(|t| t.doc_id)
        .collect();
    // PO 物料（批量，复用 PR #255 的 list_items_by_order_ids）
    let mut po_items: HashMap<i64, Vec<PurchaseOrderItem>> = HashMap::new();
    let mut product_ids: Vec<i64> = Vec::new();
    for it in state
        .purchase_order_service()
        .list_items_by_order_ids(ctx, db, &po_ids)
        .await
        .unwrap_or_default()
    {
        product_ids.push(it.product_id);
        po_items.entry(it.order_id).or_default().push(it);
    }
    // 工单产品（批量 brief：id/product_id/planned_qty）
    let wo_briefs = state
        .work_order_service()
        .list_product_brief_by_ids(ctx, db, &wo_ids)
        .await
        .unwrap_or_default();
    for w in &wo_briefs {
        product_ids.push(w.product_id);
    }
    // 产品编码/名称
    let (codes, names): (HashMap<i64, String>, HashMap<i64, String>) = if product_ids.is_empty() {
        (HashMap::new(), HashMap::new())
    } else {
        let products = state
            .product_service()
            .get_by_ids(ctx, db, product_ids)
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
        )
    };
    // 装配每个 task 的物料 cells（po 多物料 / wo 单产品）
    let mut map: HashMap<i64, Vec<MaterialCell>> = HashMap::new();
    for t in tasks {
        let cells = match t.source_kind {
            TaskSourceKind::PurchaseOrder => po_items
                .get(&t.doc_id)
                .map(|items| {
                    items
                        .iter()
                        .map(|it| MaterialCell {
                            code: codes
                                .get(&it.product_id)
                                .cloned()
                                .unwrap_or_else(|| "—".into()),
                            name: names
                                .get(&it.product_id)
                                .cloned()
                                .unwrap_or_else(|| "—".into()),
                            qty: it.quantity,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            TaskSourceKind::WorkOrder => wo_briefs
                .iter()
                .find(|w| w.id == t.doc_id)
                .map(|w| {
                    vec![MaterialCell {
                        code: codes
                            .get(&w.product_id)
                            .cloned()
                            .unwrap_or_else(|| "—".into()),
                        name: names
                            .get(&w.product_id)
                            .cloned()
                            .unwrap_or_else(|| "—".into()),
                        qty: w.planned_qty,
                    }]
                })
                .unwrap_or_default(),
        };
        map.insert(t.doc_id, cells);
    }
    map
}

/// 批量取产品编码/名称 map（各 domain 物料装配共用）。
async fn product_codes_names(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    product_ids: Vec<i64>,
) -> (HashMap<i64, String>, HashMap<i64, String>) {
    if product_ids.is_empty() {
        return (HashMap::new(), HashMap::new());
    }
    let products = state
        .product_service()
        .get_by_ids(ctx, db, product_ids)
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
    )
}

/// 待出库/待领料/待调拨（picking）：装配每个作业单据的物料 cells（编码/名称/请求数），主从表 rowspan。
async fn build_picking_materials(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    tasks: &[PendingTask],
) -> HashMap<i64, Vec<MaterialCell>> {
    let picking_ids: Vec<i64> = tasks.iter().map(|t| t.doc_id).collect();
    let mut items_by_picking: HashMap<i64, Vec<StockPickingItem>> = HashMap::new();
    let mut product_ids: Vec<i64> = Vec::new();
    for it in state
        .picking_service()
        .list_items_by_picking_ids(ctx, db, &picking_ids)
        .await
        .unwrap_or_default()
    {
        product_ids.push(it.product_id);
        items_by_picking.entry(it.picking_id).or_default().push(it);
    }
    let (codes, names) = product_codes_names(state, ctx, db, product_ids).await;
    let mut map: HashMap<i64, Vec<MaterialCell>> = HashMap::new();
    for t in tasks {
        let cells = items_by_picking
            .get(&t.doc_id)
            .map(|items| {
                items
                    .iter()
                    .map(|it| MaterialCell {
                        code: codes
                            .get(&it.product_id)
                            .cloned()
                            .unwrap_or_else(|| "—".into()),
                        name: names
                            .get(&it.product_id)
                            .cloned()
                            .unwrap_or_else(|| "—".into()),
                        qty: it.qty_requested,
                    })
                    .collect()
            })
            .unwrap_or_default();
        map.insert(t.doc_id, cells);
    }
    map
}

/// 待盘点物料单元格（编码/名称/系统量/盘点量/差异）——CycleCount 主从表 rowspan 明细列。
#[derive(Clone)]
struct CycleCell {
    code: String,
    name: String,
    system_qty: Decimal,
    counted_qty: Decimal,
    variance_qty: Decimal,
}

/// 待盘点（CycleCount）：装配每个盘点单的物料 cells（编码/名称/系统量/盘点量/差异）。
async fn build_cycle_materials(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    tasks: &[PendingTask],
) -> HashMap<i64, Vec<CycleCell>> {
    let count_ids: Vec<i64> = tasks.iter().map(|t| t.doc_id).collect();
    let mut items_by_count: HashMap<i64, Vec<CycleCountItem>> = HashMap::new();
    let mut product_ids: Vec<i64> = Vec::new();
    for it in state
        .cycle_count_service()
        .list_items_by_count_ids(ctx, db, &count_ids)
        .await
        .unwrap_or_default()
    {
        product_ids.push(it.product_id);
        items_by_count.entry(it.count_id).or_default().push(it);
    }
    let (codes, names) = product_codes_names(state, ctx, db, product_ids).await;
    let mut map: HashMap<i64, Vec<CycleCell>> = HashMap::new();
    for t in tasks {
        let cells = items_by_count
            .get(&t.doc_id)
            .map(|items| {
                items
                    .iter()
                    .map(|it| CycleCell {
                        code: codes
                            .get(&it.product_id)
                            .cloned()
                            .unwrap_or_else(|| "—".into()),
                        name: names
                            .get(&it.product_id)
                            .cloned()
                            .unwrap_or_else(|| "—".into()),
                        system_qty: it.system_qty,
                        counted_qty: it.counted_qty,
                        variance_qty: it.variance_qty,
                    })
                    .collect()
            })
            .unwrap_or_default();
        map.insert(t.doc_id, cells);
    }
    map
}

/// htmx → admin_page(true) 返回 tab 主体片段，客户端 `hx-select="#wc-domain-card"` 选取。
async fn render_work_center_page(
    ctx: RequestContext,
    q: &WorkCenterQuery,
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

    let svc = state.wms_work_center_service();
    let summary = svc.summary(&service_ctx, &mut conn).await.unwrap_or_default();
    let domain = active_domain(q);
    let page = q.page.unwrap_or(1).max(1);
    let warehouses = state
        .warehouse_service()
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
        .await
        .map(|r| r.items)
        .unwrap_or_default();

    // tab 主体内容：待办队列（list_pending；全量查询走「单据台账」页 /admin/wms/ledger）
    let mut filter = filter_from_query(q);
    // source 仅对 Arrival 有意义：切到其他 tab 时旧 filter-form 可能仍携带 source，忽略之
    if domain != WorkCenterDomain::Arrival {
        filter.source_kind = None;
    }
    let result = svc
        .list_pending(
            &service_ctx,
            &mut conn,
            domain,
            filter,
            PageParams::new(page, DOMAIN_PAGE_SIZE),
        )
        .await
        .unwrap_or_else(|_| PaginatedResult::empty(page, DOMAIN_PAGE_SIZE));
    // 主从表物料：Arrival/Outbound/Requisition/Transfer → MaterialCell（编码/名称/数量）；
    // CycleCount → CycleCell（盘点列）；LowStock 无明细。
    let picking_materials: Option<HashMap<i64, Vec<MaterialCell>>> = match domain {
        WorkCenterDomain::Arrival => {
            Some(build_arrival_materials(&state, &service_ctx, &mut conn, &result.items).await)
        }
        WorkCenterDomain::Outbound | WorkCenterDomain::Requisition | WorkCenterDomain::Transfer => {
            Some(build_picking_materials(&state, &service_ctx, &mut conn, &result.items).await)
        }
        _ => None,
    };
    let cycle_materials: Option<HashMap<i64, Vec<CycleCell>>> =
        if domain == WorkCenterDomain::CycleCount {
            Some(build_cycle_materials(&state, &service_ctx, &mut conn, &result.items).await)
        } else {
            None
        };
    let domain_markup: Markup = render_domain_card(
        domain,
        &summary,
        &result,
        q,
        &warehouses,
        picking_materials.as_ref(),
        cycle_materials.as_ref(),
    );

    let content = if is_htmx {
        // htmx 片段：tab 主体 + 顶栏总数 badge oob（wcChanged 触发 card 自刷新时一并更新顶栏待办数）
        html! {
            (domain_markup)
            (total_badge(summary.total(), true))
        }
    } else {
        // 整页：标题 + 总数 badge + tab 主体（裸标题，对齐 MES 作业中心范式）
        html! {
            div class="flex items-center justify-between mb-4 flex-wrap gap-4" {
                div {
                    div class="flex items-center gap-2.5" {
                        h1 class="text-xl font-bold text-fg tracking-tight" { "仓库作业中心" }
                        (total_badge(summary.total(), false))
                    }
                    p class="text-sm text-muted mt-1" { "待收货 · 待出库 · 待领料 · 待调拨 · 待盘点 一屏处理，就地收发与盘点" }
                }
            }
            (domain_markup)
            // 共享 drawer overlay（各域 GET ?drawer=&id= 把 body 填入 #wc-drawer-body）
            (wc_drawer_shell())
            // Issue #219：确认发货（direct_ship）成功后弹「出库成功」对话框 + 打印按钮。
            // 隐藏打印 iframe（点打印按钮 set 其 src → print_shipping 响应自带 window.print()）。
            iframe id="wc-ship-print-frame" class="hidden" {}
            // 「出库成功」对话框占位：direct_ship 成功时 POST 响应以 hx-swap-oob 替换它，
            // 并经 HX-Trigger-After-Settle: openShipSuccess 加 .is-open 弹出（§4.4 时序）。
            div id="wc-ship-success-modal" class="hidden" {}
            // 各 domain 创建 drawer（新建按钮 hx-get 填 body；submit 保 tab）
            (render_drawer_overlay("wc-cycle-count-create-overlay", "wc-cycle-count-create-drawer-body", "新建盘点单", "w-[900px] max-w-[94vw]"))
            (render_drawer_overlay("wc-requisition-create-overlay", "wc-requisition-create-drawer-body", "新建领料单", "w-[1000px] max-w-[94vw]"))
            (render_drawer_overlay("wc-transfer-create-overlay", "wc-transfer-create-drawer-body", "新建调拨单", "w-[750px] max-w-[94vw]"))
            (render_drawer_overlay("wc-shipping-create-overlay", "wc-shipping-create-drawer-body", "新建发货单", "w-[1000px] max-w-[94vw]"))
            (render_drawer_overlay("wc-stock-in-create-overlay", "wc-stock-in-create-drawer-body", "新建入库单", "w-[1000px] max-w-[94vw]"))
            // 库位选择弹窗（左仓库 + 右库位；3 drawer 的 warehouse_bin_cell 共用此页面级 shell）
            (crate::components::bin_search::bin_picker_modal("bin-picker-modal", &warehouses))
            // drawer 交互脚本（drawer body 经 innerHTML swap 不执行 script[src]，由宿主页预载）
            script src=(crate::layout::page::cache_url("/shipping-create.js")) {}
            script src=(crate::layout::page::cache_url("/wms-stock-in-create.js")) {}
            script src=(crate::layout::page::cache_url("/requisition-create.js")) {}
        }
    };

    let page_html = admin_page(
        is_htmx,
        "仓库作业中心",
        &claims,
        "inventory",
        WmsWorkCenterPath::PATH,
        "库存管理",
        Some("仓库作业中心"),
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

/// 顶栏待办总数 badge（h1 标题后）。`oob=true` 时带 hx-swap-oob，就地操作后由 POST 响应局部刷新。
fn total_badge(total: u64, oob: bool) -> Markup {
    // 对齐 MES 作业中心 badge 范式：无边框 + 内层数字加粗 span + "待办"
    let cls = "inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-accent-bg text-accent text-xs font-semibold";
    let inner = html! {
        span class="font-mono tabular-nums font-bold" { (total) }
        "待办"
    };
    if oob {
        html! {
            span id="wc-total-badge" class=(cls) hx-swap-oob="true" { (inner) }
        }
    } else {
        html! {
            span id="wc-total-badge" class=(cls) { (inner) }
        }
    }
}

/// domain tab 图标（药丸按钮内，w-4 h-4；与 domain_card_head 同图标，仅尺寸不同）。
fn domain_tab_icon(d: WorkCenterDomain) -> Markup {
    match d {
        WorkCenterDomain::Arrival => icon::truck_icon("w-4 h-4"),
        WorkCenterDomain::Outbound => icon::upload_icon("w-4 h-4"),
        WorkCenterDomain::Requisition => icon::clipboard_list_icon("w-4 h-4"),
        WorkCenterDomain::Transfer => icon::arrow_left_right_icon("w-4 h-4"),
        WorkCenterDomain::CycleCount => icon::clipboard_document_icon("w-4 h-4"),
        WorkCenterDomain::OutsourceIssue => icon::package_icon("w-4 h-4"),
        WorkCenterDomain::LowStock => icon::info_icon("w-4 h-4"),
    }
}

/// domain 显示名（tab 标签；与 card 头标题同语义）。
fn domain_label(d: WorkCenterDomain) -> &'static str {
    match d {
        WorkCenterDomain::Arrival => "待收货",
        WorkCenterDomain::Outbound => "待出库",
        WorkCenterDomain::Requisition => "待领料",
        WorkCenterDomain::Transfer => "待调拨",
        WorkCenterDomain::CycleCount => "待盘点",
        WorkCenterDomain::OutsourceIssue => "委外发料",
        WorkCenterDomain::LowStock => "低库存",
    }
}

/// domain 药丸 tab 活跃/非活跃样式（对齐 MES toggle_cls，mes_work_center.rs:540）。
fn domain_toggle_cls(active: bool) -> &'static str {
    if active {
        "inline-flex items-center gap-1 px-3.5 py-1.5 text-sm text-accent font-semibold cursor-pointer bg-accent-bg rounded-sm border-none transition-colors"
    } else {
        "inline-flex items-center gap-1 px-3.5 py-1.5 text-sm text-muted font-medium cursor-pointer bg-transparent border-none rounded-sm hover:text-fg hover:bg-surface transition-colors"
    }
}

/// domain tab 计数徽章（对齐 MES tab_badge，mes_work_center.rs:379）。
fn domain_tab_badge(n: u64) -> Markup {
    if n > 0 {
        html! {
            span class="ml-1 inline-flex items-center justify-center min-w-[20px] h-5 px-1.5 rounded-full bg-accent text-accent-on text-[11px] font-bold font-mono tabular-nums leading-none" {
                (n)
            }
        }
    } else {
        html! {}
    }
}

/// 7 个环节药丸 tab 栏（对齐 MES demand_filter_bar 第一行范式，mes_work_center.rs:416）。
/// 切 tab 强制 page=1、携带 #wc-domain-filter；整体在 #wc-domain-card 内，
/// outerHTML 替换时 tab 栏随之刷新，无需 #status-tabs oob。
fn render_domain_tabs(active: WorkCenterDomain, summary: &WorkCenterSummary) -> Markup {
    const DOMAINS: [WorkCenterDomain; 7] = [
        WorkCenterDomain::Arrival,
        WorkCenterDomain::Outbound,
        WorkCenterDomain::Requisition,
        WorkCenterDomain::Transfer,
        WorkCenterDomain::CycleCount,
        WorkCenterDomain::OutsourceIssue,
        WorkCenterDomain::LowStock,
    ];
    html! {
        div class="flex items-center gap-1 flex-wrap px-5 pt-3 border-b border-border-soft" {
            @for d in DOMAINS {
                button class=(domain_toggle_cls(d == active)) type="button"
                    hx-get=(WmsWorkCenterPath::PATH)
                    hx-vals=(serde_json::json!({ "domain": domain_slug(d), "page": "1" }).to_string())
                    hx-target="#wc-domain-card" hx-select="#wc-domain-card" hx-swap="outerHTML"
                    hx-include="#wc-domain-filter" {
                    (domain_tab_icon(d))
                    (domain_label(d))
                    (domain_tab_badge(domain_count(summary, d)))
                }
            }
        }
    }
}

/// card 头：图标（带紧急度角标）+ domain 标题 + meta（待办数 + 描述 + 紧急度），对齐
/// MES `render_card_shell` 的 card 头范式。随 active domain 切换图标/标题/描述。
fn domain_card_head(active: WorkCenterDomain, summary: &WorkCenterSummary) -> Markup {
    let (title, icon_mkp, desc): (&str, Markup, &str) = match active {
        WorkCenterDomain::Arrival => ("待收货", icon::truck_icon("w-[15px] h-[15px]"), "采购 PO / 生产工单 收货入库"),
        WorkCenterDomain::Outbound => ("待出库", icon::upload_icon("w-[15px] h-[15px]"), "销售订单 发出立应收"),
        WorkCenterDomain::Requisition => ("待领料", icon::clipboard_list_icon("w-[15px] h-[15px]"), "生产工单 领料发料"),
        WorkCenterDomain::Transfer => ("待调拨", icon::arrow_left_right_icon("w-[15px] h-[15px]"), "仓间调拨 出入库"),
        WorkCenterDomain::CycleCount => ("待盘点", icon::clipboard_document_icon("w-[15px] h-[15px]"), "库存盘点 审批调整"),
        WorkCenterDomain::OutsourceIssue => ("委外发料", icon::package_icon("w-[15px] h-[15px]"), "委外单 发料给供应商"),
        WorkCenterDomain::LowStock => ("低库存", icon::info_icon("w-[15px] h-[15px]"), "安全库存预警 确认处理"),
    };
    let s = summary.of(active);
    let total = s.total;
    let overdue = s.overdue;
    let soon = s.soon;
    let mut meta = format!("{total} 张待办 · {desc}");
    let dot = if overdue > 0 {
        meta.push_str(&format!(" · {overdue} 逾期"));
        Some("danger")
    } else if soon > 0 {
        meta.push_str(&format!(" · {soon} 临期"));
        Some("warn")
    } else {
        None
    };
    html! {
        div class="flex items-center gap-3 px-5 py-3 border-b border-border-soft" {
            div class="relative w-7 h-7 rounded-md grid place-items-center bg-surface text-fg-2 shrink-0" {
                (icon_mkp)
                @if let Some(token) = dot {
                    span class=(format!("absolute -top-0.5 -right-0.5 w-2 h-2 rounded-full bg-{token} ring-2 ring-bg")) {}
                }
            }
            span class="font-semibold text-fg shrink-0" { (title) }
            span class="text-xs text-muted font-mono flex-1 min-w-0 truncate" { (meta) }
        }
    }
}

/// tab 主体卡片：`#wc-domain-card`（status-tabs + filter-form + 队列表格 + 分页）。
/// 整体可被 hx-target/hx-select outerHTML 替换（切 tab / 搜索 / 分页 / 就地操作后回填）。
fn render_domain_card(
    active: WorkCenterDomain,
    summary: &WorkCenterSummary,
    result: &PaginatedResult<PendingTask>,
    q: &WorkCenterQuery,
    warehouses: &[Warehouse],
    picking_materials: Option<&HashMap<i64, Vec<MaterialCell>>>,
    cycle_materials: Option<&HashMap<i64, Vec<CycleCell>>>,
) -> Markup {
    let (overdue, soon) = {
        let s = summary.of(active);
        (s.overdue, s.soon)
    };
    html! {
       div id="wc-domain-card"
           hx-get=(WmsWorkCenterPath::PATH)
           hx-trigger="wcChanged from:body"
           hx-include="#wc-domain-filter"
            hx-target="this" hx-select="#wc-domain-card" hx-swap="outerHTML"
            class="bg-bg border border-border-soft rounded-lg mb-4 shadow-card overflow-hidden" {
            // card 头（图标 + 紧急度角标 + domain 标题 + meta），对齐 MES render_card_shell 范式
            (domain_card_head(active, summary))
            // tab 栏（6 环节药丸 button + 图标 + 实数 badge；切 tab 强制 page=1、携带 filter）。
            // 对齐 MES 作业中心 demand_filter_bar 第一行范式（mes_work_center.rs:416）。
            // #wc-domain-card 整体 outerHTML 替换时 tab 栏随之刷新，无需 #status-tabs oob。
            (render_domain_tabs(active, summary))
            // 过滤表单（紧急度快捷 pill 随表单一并渲染，不再单列）
            (render_domain_filter(active, q, overdue, soon))
            // 队列表格 + 分页
            div class="p-4" {
                @if active == WorkCenterDomain::LowStock {
                    (render_low_stock_list(&result.items))
                } @else {
                    (render_task_table(
                        &result.items,
                        active,
                        picking_materials,
                        cycle_materials,
                    ))
                }
                @if result.total_pages > 1 {
                    div class="mt-3" {
                        (pagination(
                            WmsWorkCenterPath::PATH,
                            "#wc-domain-card",
                            "#wc-domain-filter",
                            result.total,
                            result.page,
                            result.total_pages,
                        ))
                    }
                }
            }
            // 待出库批量发货栏（固定底部，JS 在 .wc-ship-cb:checked > 0 时显隐）
            @if active == WorkCenterDomain::Outbound {
                (wc_batch_bar(warehouses))
            }
        }
    }
}

/// 过滤表单：keyword 搜索（防抖）+ 紧急度筛选 + 来源筛选（仅待收货）。
/// hidden domain 携带当前 tab（搜索/分页不切 tab）；切 tab 由 status-tabs 的 hx-vals 覆盖。
fn render_domain_filter(active: WorkCenterDomain, q: &WorkCenterQuery, overdue: u64, soon: u64) -> Markup {
    let kw = q.keyword.as_deref().unwrap_or("");
    let urg = q.urgency.as_deref().unwrap_or("");
    let src = q.source.as_deref().unwrap_or("");
    html! {
        form id="wc-domain-filter"
            class="flex items-center gap-3 flex-wrap px-4 py-3 border-b border-border-soft"
            hx-get=(WmsWorkCenterPath::PATH)
            hx-trigger="change, keyup changed delay:300ms from:.wc-search-input"
            hx-target="#wc-domain-card"
            hx-select="#wc-domain-card"
            hx-swap="outerHTML"
            hx-include="#wc-domain-filter" {
            input type="hidden" name="domain" value=(domain_slug(active));
            // 关键词
            div class="relative" {
                (icon::search_icon("w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"));
                input class="wc-search-input w-[200px] pl-8 pr-3 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                    type="text" name="keyword" placeholder="搜索单号 / 对象"
                    value=(kw);
            }
            // 紧急度（低库存都是 Overdue，不筛）
            @if active != WorkCenterDomain::LowStock {
                select id="wc-urgency-select" class="px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                    name="urgency" {
                    option value="" selected[urg.is_empty()] { "全部紧急度" }
                    option value="overdue" selected[urg == "overdue"] { "逾期" }
                    option value="soon" selected[urg == "soon"] { "临期" }
                    option value="normal" selected[urg == "normal"] { "正常" }
                }
            }
            // 来源（仅待收货：PO / 工单）
            @if active == WorkCenterDomain::Arrival {
                select class="px-2.5 py-1.5 border border-border rounded-sm text-sm bg-white text-fg outline-none focus:border-accent"
                    name="source" {
                    option value="" selected[src.is_empty()] { "全部来源" }
                    option value="po" selected[src == "po"] { "采购 PO" }
                    option value="wo" selected[src == "wo"] { "生产工单" }
                }
            }
            // 紧急度快捷 pill：点击 = 设紧急度下拉值并触发 change，复用 filter form 的 hx-trigger="change"
            div class="ml-auto flex items-center gap-2" {
                // 各 domain 收口入口：新建 / 查看全部（侧边栏菜单已废弃，跳转保留的业务路由）
                (domain_entries(active))
                // 紧急度快捷 pill（低库存都是 Overdue，不显示 urgency pill）
                @if active != WorkCenterDomain::LowStock {
                    @if overdue > 0 {
                        button type="button"
                            class="inline-flex items-center gap-1 px-2 py-1 rounded-full text-xs font-medium bg-danger-bg text-danger border border-danger/30 cursor-pointer hover:bg-danger/15 transition-colors"
                            _="on click set #wc-urgency-select's value to 'overdue' then trigger change on #wc-urgency-select" {
                            span class="w-1.5 h-1.5 rounded-full bg-danger" {}
                            (overdue) " 逾期"
                        }
                    }
                    @if soon > 0 {
                        button type="button"
                            class="inline-flex items-center gap-1 px-2 py-1 rounded-full text-xs font-medium bg-warn-bg text-warn border border-warn/30 cursor-pointer hover:bg-warn/15 transition-colors"
                            _="on click set #wc-urgency-select's value to 'soon' then trigger change on #wc-urgency-select" {
                            span class="w-1.5 h-1.5 rounded-full bg-warn" {}
                            (soon) " 临期"
                        }
                    }
                }
            }
        }
    }
}

/// 低库存预警紧凑列表（满宽，一行式：产品+仓名 / 摘要 / 紧急度 badge）。
/// 替代通用 render_task_table（多列稀疏）——低库存字段少，紧凑列表更清晰。
fn render_low_stock_list(tasks: &[PendingTask]) -> Markup {
    if tasks.is_empty() {
        return html! {
            div class="mt-2 p-4 text-center text-sm text-muted bg-surface rounded-md" { "暂无预警" }
        };
    }
    html! {
        div class="mt-2 divide-y divide-border-soft border-y border-border-soft" {
            @for t in tasks {
                div class="flex items-center gap-4 py-3" {
                    // 产品 + 仓库
                    div class="flex-1 min-w-0" {
                        div class="text-sm font-medium text-fg truncate" { (t.doc_number) }
                        div class="text-xs text-muted truncate" { (t.counterparty) }
                    }
                    // 摘要（当前 · 安全 · 缺）
                    div class="text-xs font-mono text-fg-2 whitespace-nowrap shrink-0" {
                        (t.summary)
                    }
                    // 紧急度 badge（低库存都是逾期）
                    span class="inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium bg-danger-bg text-danger whitespace-nowrap shrink-0" {
                        "逾期"
                    }
                }
            }
        }
    }
}

fn render_task_table(
    tasks: &[PendingTask],
    domain: WorkCenterDomain,
    picking_materials: Option<&HashMap<i64, Vec<MaterialCell>>>,
    cycle_materials: Option<&HashMap<i64, Vec<CycleCell>>>,
) -> Markup {
    if tasks.is_empty() {
        return html! {
            div class="mt-2 p-4 text-center text-sm text-muted bg-surface rounded-md" { "暂无待办" }
        };
    }
    // CycleCount → 盘点列主从表（编码/名称/系统量/盘点量/差异）
    if let Some(mats) = cycle_materials {
        return render_cycle_table(tasks, domain, mats);
    }
    // Arrival/Outbound/Requisition/Transfer → 编码/名称/数量主从表
    if let Some(mats) = picking_materials {
        return render_master_table(tasks, domain, mats);
    }
    html! {
        table class="w-full border-collapse mt-2" {
            thead {
                tr {
                    @if domain == WorkCenterDomain::Outbound {
                        th class="text-left text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft w-10" {
                            input type="checkbox" class="wc-select-all"
                                title="全选待发货"
                                _="on click call wcToggleAll(me)" {}
                        }
                    }
                    th class="text-left text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft" { "单号" }
                    th class="text-left text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft" { "对象" }
                    th class="text-left text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft" { "摘要" }
                    th class="text-left text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft" { "收到" }
                    th class="text-left text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft" { "到期" }
                    th class="text-left text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft" { "紧急度" }
                    @if domain != WorkCenterDomain::LowStock {
                        th class="text-right text-xs font-semibold text-muted py-2 px-3 border-b border-border-soft" { "操作" }
                    }
                }
            }
            tbody {
                @for t in tasks {
                    (render_task_row(t, domain))
                }
            }
        }
    }
}

/// 主从表（Arrival/Outbound/Requisition/Transfer 共用）：每单一个 tbody.pc-row-group，
/// 订单级列 rowspan + 物料列（编码/名称/数量）常驻 + 斑马纹 + pc-grid 边框 + 垂直居中。
fn render_master_table(
    tasks: &[PendingTask],
    domain: WorkCenterDomain,
    materials: &HashMap<i64, Vec<MaterialCell>>,
) -> Markup {
    html! {
        div class="overflow-x-auto mt-2" {
            table class="w-full text-sm border-collapse pc-grid" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "单号" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "对象" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "产品编码" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "产品名称" }
                        th class="text-right font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "数量" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "摘要" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "收到" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "到期" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "紧急度" }
                        th class="text-right font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "操作" }
                    }
                }
                @for (i, t) in tasks.iter().enumerate() {
                    @let cells = materials.get(&t.doc_id).cloned().unwrap_or_default();
                    @let n = if cells.is_empty() { 1usize } else { cells.len() };
                    @let (urgency_label, urgency_cls) = match t.urgency {
                        Urgency::Overdue => ("逾期", "bg-danger-bg text-danger"),
                        Urgency::Soon => ("临期", "bg-warn-bg text-warn"),
                        Urgency::Normal => ("正常", "bg-surface text-muted"),
                    };
                    // 斑马纹：奇数单浅灰背景，区分每单边界；紧急度改由紧急度列 pill 提示（不再整行染色）
                    @let row_bg = if i % 2 == 1 {
                        "bg-surface-raised"
                    } else {
                        ""
                    };
                    @let received = t.received_at.map(|d| d.format("%m-%d %H:%M").to_string()).unwrap_or_else(|| "—".into());
                    @let expected = t.expected_at.map(|d| d.format("%m-%d").to_string()).unwrap_or_else(|| "—".into());
                    tbody class="pc-row-group" {
                        tr class=(row_bg) {
                            td rowspan=(n.to_string()) class="py-2.5 px-3 text-sm font-mono text-accent font-semibold align-middle whitespace-nowrap border border-border-soft" {
                                @if let Some(drw) = row_detail_drawer(domain, t.source_kind) {
                                    (doc_detail_trigger(drw, t.doc_id, "pending", html! { (t.doc_number) },
                                        "font-mono text-accent font-semibold text-sm bg-transparent border-none p-0 cursor-pointer hover:underline"))
                                } @else {
                                    (t.doc_number)
                                }
                            }
                            td rowspan=(n.to_string()) class="py-2.5 px-3 text-sm align-middle whitespace-nowrap border border-border-soft" {
                                @let cp = t.counterparty.as_str();
                                @if let Some(drw) = row_detail_drawer(domain, t.source_kind) {
                                    (doc_detail_trigger(drw, t.doc_id, "pending",
                                        html! { span class="block truncate" title=(cp) { (cp) } },
                                        "inline-block max-w-[160px] align-middle truncate text-fg-2 text-sm bg-transparent border-none p-0 cursor-pointer hover:text-accent hover:underline text-left"))
                                } @else {
                                    span class="inline-block max-w-[160px] align-middle truncate text-fg-2" title=(cp) { (cp) }
                                }
                            }
                            @if let Some(first) = cells.first() {
                                (material_cell_tds(first))
                            } @else {
                                td colspan="3" class="py-2.5 px-3 text-sm text-muted text-center border border-border-soft" { "—" }
                            }
                            td rowspan=(n.to_string()) class="py-2.5 px-3 text-sm text-muted align-middle whitespace-nowrap border border-border-soft" { (t.summary) }
                            td rowspan=(n.to_string()) class="py-2.5 px-3 text-sm font-mono text-muted align-middle whitespace-nowrap border border-border-soft" { (received) }
                            td rowspan=(n.to_string()) class="py-2.5 px-3 text-sm font-mono text-fg-2 align-middle whitespace-nowrap border border-border-soft" { (expected) }
                            td rowspan=(n.to_string()) class="py-2.5 px-3 align-middle whitespace-nowrap border border-border-soft" {
                                span class=(format!("inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium {urgency_cls}")) {
                                    (urgency_label)
                                }
                            }
                            td rowspan=(n.to_string()) class="py-2.5 px-3 text-right align-middle whitespace-nowrap border border-border-soft" {
                                (render_row_action(t))
                            }
                        }
                        @for c in cells.iter().skip(1) {
                            tr class=(row_bg) {
                                (material_cell_tds(c))
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 待收货物料单元格（编码/名称/数量）——主从表明细列，首行与后续行共用。
fn material_cell_tds(c: &MaterialCell) -> Markup {
    html! {
        td class="py-2.5 px-3 text-sm font-mono text-accent whitespace-nowrap border border-border-soft" { (c.code) }
        td class="py-2.5 px-3 text-sm text-fg whitespace-nowrap border border-border-soft" {
            span class="inline-block max-w-[240px] align-middle truncate" title=(c.name.as_str()) { (c.name.as_str()) }
        }
        td class="py-2.5 px-3 text-sm text-right font-mono whitespace-nowrap border border-border-soft" { (fmt_qty(c.qty)) }
    }
}

/// 待盘点(CycleCount)主从表：每单一个 tbody.pc-row-group，订单级列 rowspan +
/// 盘点项列（编码/名称/系统量/盘点量/差异）常驻 + 斑马纹 + pc-grid。
fn render_cycle_table(
    tasks: &[PendingTask],
    domain: WorkCenterDomain,
    materials: &HashMap<i64, Vec<CycleCell>>,
) -> Markup {
    html! {
        div class="overflow-x-auto mt-2" {
            table class="w-full text-sm border-collapse pc-grid" {
                thead {
                    tr class="bg-surface-raised text-xs text-muted" {
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "单号" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "对象" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "产品编码" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "产品名称" }
                        th class="text-right font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "系统量" }
                        th class="text-right font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "盘点量" }
                        th class="text-right font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "差异" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "摘要" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "收到" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "到期" }
                        th class="text-left font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "紧急度" }
                        th class="text-right font-semibold py-2 px-3 uppercase tracking-wide whitespace-nowrap border border-border-soft" { "操作" }
                    }
                }
                @for (i, t) in tasks.iter().enumerate() {
                    @let cells = materials.get(&t.doc_id).cloned().unwrap_or_default();
                    @let n = if cells.is_empty() { 1usize } else { cells.len() };
                    @let (urgency_label, urgency_cls) = match t.urgency {
                        Urgency::Overdue => ("逾期", "bg-danger-bg text-danger"),
                        Urgency::Soon => ("临期", "bg-warn-bg text-warn"),
                        Urgency::Normal => ("正常", "bg-surface text-muted"),
                    };
                    @let row_bg = if i % 2 == 1 { "bg-surface-raised" } else { "" };
                    @let received = t.received_at.map(|d| d.format("%m-%d %H:%M").to_string()).unwrap_or_else(|| "—".into());
                    @let expected = t.expected_at.map(|d| d.format("%m-%d").to_string()).unwrap_or_else(|| "—".into());
                    tbody class="pc-row-group" {
                        tr class=(row_bg) {
                            td rowspan=(n.to_string()) class="py-2.5 px-3 text-sm font-mono text-accent font-semibold align-middle whitespace-nowrap border border-border-soft" {
                                @if let Some(drw) = row_detail_drawer(domain, t.source_kind) {
                                    (doc_detail_trigger(drw, t.doc_id, "pending", html! { (t.doc_number) },
                                        "font-mono text-accent font-semibold text-sm bg-transparent border-none p-0 cursor-pointer hover:underline"))
                                } @else {
                                    (t.doc_number)
                                }
                            }
                            td rowspan=(n.to_string()) class="py-2.5 px-3 text-sm align-middle whitespace-nowrap border border-border-soft" {
                                @let cp = t.counterparty.as_str();
                                @if let Some(drw) = row_detail_drawer(domain, t.source_kind) {
                                    (doc_detail_trigger(drw, t.doc_id, "pending",
                                        html! { span class="block truncate" title=(cp) { (cp) } },
                                        "inline-block max-w-[160px] align-middle truncate text-fg-2 text-sm bg-transparent border-none p-0 cursor-pointer hover:text-accent hover:underline text-left"))
                                } @else {
                                    span class="inline-block max-w-[160px] align-middle truncate text-fg-2" title=(cp) { (cp) }
                                }
                            }
                            @if let Some(first) = cells.first() {
                                (cycle_cell_tds(first))
                            } @else {
                                td colspan="5" class="py-2.5 px-3 text-sm text-muted text-center border border-border-soft" { "—" }
                            }
                            td rowspan=(n.to_string()) class="py-2.5 px-3 text-sm text-muted align-middle whitespace-nowrap border border-border-soft" { (t.summary) }
                            td rowspan=(n.to_string()) class="py-2.5 px-3 text-sm font-mono text-muted align-middle whitespace-nowrap border border-border-soft" { (received) }
                            td rowspan=(n.to_string()) class="py-2.5 px-3 text-sm font-mono text-fg-2 align-middle whitespace-nowrap border border-border-soft" { (expected) }
                            td rowspan=(n.to_string()) class="py-2.5 px-3 align-middle whitespace-nowrap border border-border-soft" {
                                span class=(format!("inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium {urgency_cls}")) {
                                    (urgency_label)
                                }
                            }
                            td rowspan=(n.to_string()) class="py-2.5 px-3 text-right align-middle whitespace-nowrap border border-border-soft" {
                                (render_row_action(t))
                            }
                        }
                        @for c in cells.iter().skip(1) {
                            tr class=(row_bg) {
                                (cycle_cell_tds(c))
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 待盘点物料单元格（编码/名称/系统量/盘点量/差异）——CycleCount 主从表明细列。
fn cycle_cell_tds(c: &CycleCell) -> Markup {
    html! {
        td class="py-2.5 px-3 text-sm font-mono text-accent whitespace-nowrap border border-border-soft" { (c.code) }
        td class="py-2.5 px-3 text-sm text-fg whitespace-nowrap border border-border-soft" {
            span class="inline-block max-w-[240px] align-middle truncate" title=(c.name.as_str()) { (c.name.as_str()) }
        }
        td class="py-2.5 px-3 text-sm text-right font-mono whitespace-nowrap border border-border-soft" { (fmt_qty(c.system_qty)) }
        td class="py-2.5 px-3 text-sm text-right font-mono whitespace-nowrap border border-border-soft" { (fmt_qty(c.counted_qty)) }
        td class="py-2.5 px-3 text-sm text-right font-mono whitespace-nowrap border border-border-soft" { (fmt_qty(c.variance_qty)) }
    }
}

fn render_task_row(t: &PendingTask, domain: WorkCenterDomain) -> Markup {
    let (urgency_label, urgency_cls) = match t.urgency {
        Urgency::Overdue => ("逾期", "bg-danger-bg text-danger"),
        Urgency::Soon => ("临期", "bg-warn-bg text-warn"),
        Urgency::Normal => ("正常", "bg-surface text-muted"),
    };
    // 整行背景按紧急度染色（对齐原型 tr.overdue / tr.soon）
    let row_bg = match t.urgency {
        Urgency::Overdue => "bg-danger-bg",
        Urgency::Soon => "bg-warn-bg",
        Urgency::Normal => "",
    };
    let expected = t
        .expected_at
        .map(|d| d.format("%m-%d").to_string())
        .unwrap_or_else(|| "—".into());
    let received = t
        .received_at
        .map(|d| d.format("%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "—".into());
    html! {
        tr class=(format!("border-b border-border-soft last:border-b-0 {row_bg}")) {
            @if domain == WorkCenterDomain::Outbound {
                td class="py-3 px-3" {
                    input type="checkbox" class="wc-ship-cb" value=(t.doc_id) {};
                }
            }
            td class="py-3 px-3 text-sm font-mono text-accent font-semibold" {
                @if let Some(drw) = row_detail_drawer(domain, t.source_kind) {
                    (doc_detail_trigger(drw, t.doc_id, "pending", html! { (t.doc_number) },
                        "font-mono text-accent font-semibold text-sm bg-transparent border-none p-0 cursor-pointer hover:underline"))
                } @else {
                    (t.doc_number)
                }
            }
            td class="py-3 px-3 text-sm" {
                @if let Some(drw) = row_detail_drawer(domain, t.source_kind) {
                    (doc_detail_trigger(drw, t.doc_id, "pending", html! { (t.counterparty) },
                        "text-fg-2 text-sm bg-transparent border-none p-0 cursor-pointer hover:text-accent hover:underline text-left"))
                } @else {
                    span class="text-fg-2" { (t.counterparty) }
                }
            }
            td class="py-3 px-3 text-sm text-muted" {
                (t.summary)
            }
            td class="py-3 px-3 text-sm font-mono text-muted" { (received) }
            td class="py-3 px-3 text-sm font-mono text-fg-2" { (expected) }
            td class="py-3 px-3" {
                span class=(format!("inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium {urgency_cls}")) {
                    (urgency_label)
                }
            }
            @if domain != WorkCenterDomain::LowStock {
                td class="py-3 px-3 text-right" {
                    (render_row_action(t))
                }
            }
        }
    }
}

/// 待出库批量发货栏（固定底部，复用 MES detail_batch_bar 范式）。
/// 默认 hidden，JS（app.js `wcUpdateBatchBar`）在 `.wc-ship-cb:checked > 0` 时加 `.show`。
/// 提交走单端点 POST action=batch_ship，响应仍是 `#wc-domain-card` + `#wc-total-badge`(oob)。
fn wc_batch_bar(warehouses: &[Warehouse]) -> Markup {
    html! {
        form id="wc-batch-bar"
            class="wc-batch-bar hidden show:flex fixed bottom-4 left-1/2 -translate-x-1/2 z-50 items-center gap-4 px-5 py-3 rounded-md bg-fg text-white text-sm shadow-lg"
            hx-post=(WmsWorkCenterPath::PATH)
            hx-target="#wc-domain-card"
            hx-select="#wc-domain-card"
            hx-swap="outerHTML"
            hx-confirm="确认批量发出选中的待出库单？将从所选仓库逐张扣库存并立应收（任一失败整体回滚）" {
            input type="hidden" name="action" value="batch_ship";
            input type="hidden" name="ids" value="";
            // 批量发货仓库：所有选中单统一从此仓发出
            select name="warehouse_id" required
                class="px-2 py-1.5 rounded-sm bg-white text-fg text-xs font-medium border-none outline-none cursor-pointer max-w-[140px]" {
                option value="" disabled selected { "选发货仓" }
                @for w in warehouses {
                    option value=(w.id) { (w.name) }
                }
            }
            span {
                "已选 "
                span class="wc-batch-count font-mono font-bold" { "0" }
                " 条"
            }
            button type="submit"
                class="ml-auto inline-flex items-center gap-1 px-3 py-1.5 rounded-sm bg-accent text-white text-xs font-semibold border-none cursor-pointer hover:opacity-90" {
                (icon::upload_icon("w-3.5 h-3.5"))
                "批量发货"
            }
            button type="button"
                class="inline-flex items-center gap-1 px-3 py-1.5 rounded-sm border border-white/15 text-white/70 text-xs font-medium bg-transparent cursor-pointer hover:text-white hover:bg-white/10"
                _="on click call wcClearBatch()" {
                "清除"
            }
        }
    }
}

/// 行内操作入口：收货/发货/发料/调拨 → hx-get 加载 drawer body；盘点 → 跳详情页。
/// Outbound（待发货）：选仓 drawer 直接发货（拣货已移除，Confirmed→Shipped）。
fn render_row_action(t: &PendingTask) -> Markup {
    let open_hs =
        "on 'htmx:afterRequest'[detail.xhr.status<400] add .open to #wc-drawer-overlay";
    match t.domain {
        WorkCenterDomain::Arrival => match t.source_kind {
            TaskSourceKind::PurchaseOrder => drawer_btn("收货", "receive_po", t.doc_id, icon::truck_icon("w-3 h-3"), open_hs),
            TaskSourceKind::WorkOrder => drawer_btn("入库", "receive_wo", t.doc_id, icon::package_icon("w-3 h-3"), open_hs),
        },
        WorkCenterDomain::Outbound => {
            // 待发货（Confirmed）：选仓 drawer 直接发货（拣货已移除）
            drawer_btn("直接发货", "direct_ship", t.doc_id, icon::upload_icon("w-3 h-3"), open_hs)
        }
        WorkCenterDomain::Requisition => {
            drawer_btn("发料", "issue", t.doc_id, icon::clipboard_list_icon("w-3 h-3"), open_hs)
        }
        WorkCenterDomain::Transfer => {
            drawer_btn("办理", "transfer", t.doc_id, icon::arrow_right_icon("w-3 h-3"), open_hs)
        }
        // 盘点：详情 + 操作（start/complete/approve/reject…）走 cc_detail drawer
        WorkCenterDomain::CycleCount => {
            doc_detail_trigger("cc_detail", t.doc_id, "pending", html! { "详情" (icon::clipboard_list_icon("w-3 h-3")) },
                "inline-flex items-center gap-1 px-3 py-1.5 rounded-sm bg-accent text-white text-xs font-semibold cursor-pointer border-none hover:opacity-90")
        }
        // 委外发料：详情 drawer（picking + 物料清单 + 确认发料表单，POST osa_issue）
        WorkCenterDomain::OutsourceIssue => {
            doc_detail_trigger("osa_issue_detail", t.doc_id, "pending", html! { "发料" (icon::package_icon("w-3 h-3")) },
                "inline-flex items-center gap-1 px-3 py-1.5 rounded-sm bg-accent text-white text-xs font-semibold cursor-pointer border-none hover:opacity-90")
        }
        // 低库存预警：行内 ack（POST action=ack_low_stock，刷新当前 card）
        WorkCenterDomain::LowStock => {
            html! {
                button type="button"
                    class="inline-flex items-center gap-1 px-3 py-1.5 rounded-sm bg-white text-fg-2 border border-border text-xs font-semibold cursor-pointer hover:bg-surface hover:text-accent"
                    hx-post=(WmsWorkCenterPath::PATH)
                    hx-vals=(serde_json::json!({"action": "ack_low_stock", "id": t.doc_id}).to_string())
                    hx-target="#wc-domain-card"
                    hx-select="#wc-domain-card"
                    hx-swap="outerHTML"
                    hx-confirm="确认此低库存预警已处理？"
                { (icon::check_circle_icon("w-3.5 h-3.5")) "确认" }
            }
        }
    }
}

/// 行内 drawer 触发按钮：hx-get 加载 drawer body 到 #wc-drawer-body，成功后打开 overlay。
fn drawer_btn(label: &str, action: &str, doc_id: i64, ic: Markup, open_hs: &str) -> Markup {
    let url = format!("{}?drawer={action}&id={doc_id}", WmsWorkCenterPath::PATH);
    html! {
        button type="button"
            class="inline-flex items-center gap-1 px-3 py-1.5 rounded-sm bg-accent text-white text-xs font-semibold cursor-pointer border-none hover:opacity-90"
            hx-get=(url)
            hx-target="#wc-drawer-body"
            hx-swap="innerHTML"
            _=(open_hs) {
            (ic)
            (label)
        }
    }
}

/// 共享 drawer overlay 壳：页面渲染一次，各域 GET ?drawer=&id= 填 #wc-drawer-body。
/// 显隐由 .drawer-overlay 的 .open class 控制（uno.config.ts preflight，经 drawer_shell 统一）；× / 背景点击关闭。
fn wc_drawer_shell() -> Markup {
    drawer_shell("wc-drawer-overlay", "w-[720px]", html! {
        div id="wc-drawer-body" class="flex-1 flex flex-col overflow-hidden" {}
    })
}

/// 创建 drawer overlay 壳：标题栏（含×）+ body 槽（按钮 hx-get 填入 #body_id）。
/// 开：按钮 afterRequest add .open / body afterSettle add .open；关：× / Esc（drawer_shell 自带）/ form afterRequest 守卫。
/// 仿 purchase_work_center::render_drawer_overlay。
fn render_drawer_overlay(overlay_id: &str, body_id: &str, title: &str, width_class: &str) -> Markup {
    drawer_shell(overlay_id, width_class, html! {
        div class="flex items-center justify-between px-6 py-5 border-b border-border-soft" {
            div class="font-bold text-base text-fg" { (title) }
            button type="button"
                class="w-8 h-8 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg flex items-center justify-center"
                _=(format!("on click remove .open from #{}", overlay_id)) {
                (icon::x_icon("w-4 h-4"))
            }
        }
        div id=(body_id) class="flex-1 overflow-y-auto"
            _=(format!("on 'htmx:afterSettle' add .open to #{}", overlay_id)) {}
    })
}

// ── 盘点创建 drawer（CycleCount tab「新建盘点单」按钮 hx-get 填 body）──

#[require_permission("INVENTORY", "read")]
pub async fn get_cycle_count_create_drawer(
    _path: crate::routes::wms_work_center::WcCycleCountCreateDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let warehouses = state
        .warehouse_service()
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
        .await
        .map(|r| r.items)
        .unwrap_or_default();
    // MES 守卫：提交成功（空 body 200）才关 drawer；子请求/校验失败重渲染（非空）不关
    let after_hs = "on 'htmx:afterRequest'[detail.xhr.responseText.length==0 and detail.xhr.status<400] remove .open from #wc-cycle-count-create-overlay";
    Ok(Html(
        crate::pages::wms_cycle_count_create::cycle_count_create_page(
            &warehouses,
            crate::routes::wms_work_center::WcCycleCountCreatePath::PATH,
            after_hs,
            false,
            false,
        )
        .into_string(),
    ))
}

#[require_permission("INVENTORY", "create")]
pub async fn post_cycle_count_create(
    _path: crate::routes::wms_work_center::WcCycleCountCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<crate::pages::wms_cycle_count_create::CreateCycleCountForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    crate::pages::wms_cycle_count_create::do_create_cycle_count(&state, &service_ctx, form).await?;
    // 成功 toast + wcChanged：form afterRequest 守卫关 drawer；#wc-domain-card 监听 wcChanged 自刷新（带 active domain 保 tab）
    crate::toast::add_toast(
        service_ctx.operator_id,
        "盘点单已创建",
        crate::toast::ToastType::Success,
    );
    Ok(([("HX-Trigger", "wcChanged, showToast")], Html(String::new())))
}

// ── 领料创建 drawer（Requisition tab「新建领料单」按钮）──

#[require_permission("INVENTORY", "read")]
pub async fn get_requisition_create_drawer(
    _path: crate::routes::wms_work_center::WcRequisitionCreateDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let warehouses = state
        .warehouse_service()
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
        .await
        .map(|r| r.items)
        .unwrap_or_default();
    let after_hs = "on 'htmx:afterRequest'[detail.xhr.responseText.length==0 and detail.xhr.status<400] remove .open from #wc-requisition-create-overlay";
    Ok(Html(
        crate::pages::wms_requisition_create::requisition_create_page(
            &warehouses,
            crate::routes::wms_work_center::WcRequisitionCreatePath::PATH,
            after_hs,
            false,
            false,
        )
        .into_string(),
    ))
}

#[require_permission("INVENTORY", "create")]
pub async fn post_requisition_create(
    _path: crate::routes::wms_work_center::WcRequisitionCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<crate::pages::wms_requisition_create::RequisitionCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    crate::pages::wms_requisition_create::do_create_requisition(&state, &service_ctx, form).await?;
    crate::toast::add_toast(
        service_ctx.operator_id,
        "领料单已创建",
        crate::toast::ToastType::Success,
    );
    Ok(([("HX-Trigger", "wcChanged, showToast")], Html(String::new())))
}

// ── 调拨创建 drawer（Transfer tab「新建调拨单」按钮）──

#[require_permission("INVENTORY", "read")]
pub async fn get_transfer_create_drawer(
    _path: crate::routes::wms_work_center::WcTransferCreateDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let warehouses = state
        .warehouse_service()
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
        .await
        .map(|r| r.items)
        .unwrap_or_default();
    let after_hs = "on 'htmx:afterRequest'[detail.xhr.responseText.length==0 and detail.xhr.status<400] remove .open from #wc-transfer-create-overlay";
    Ok(Html(
        crate::pages::wms_transfer_create::transfer_create_page(
            &warehouses,
            crate::routes::wms_work_center::WcTransferCreatePath::PATH,
            after_hs,
            false,
        )
        .into_string(),
    ))
}

#[require_permission("INVENTORY", "create")]
pub async fn post_transfer_create(
    _path: crate::routes::wms_work_center::WcTransferCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<crate::pages::wms_transfer_create::TransferCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    crate::pages::wms_transfer_create::do_create_transfer(&state, &service_ctx, form).await?;
    crate::toast::add_toast(
        service_ctx.operator_id,
        "调拨单已创建",
        crate::toast::ToastType::Success,
    );
    Ok(([("HX-Trigger", "wcChanged, showToast")], Html(String::new())))
}

// ── 发货创建 drawer（Outbound tab「新建发货单」按钮）──

#[require_permission("SHIPPING", "read")]
pub async fn get_shipping_create_drawer(
    _path: crate::routes::wms_work_center::WcShippingCreateDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let customers = state
        .customer_service()
        .list(
            &service_ctx, &mut conn,
            abt_core::master_data::customer::model::CustomerQuery {
                name: None, status: None, category: None, owner_id: None,
            },
            abt_core::shared::types::PageParams::new(1, 200),
        )
        .await
        .map(|r| r.items)
        .unwrap_or_default();
    let warehouses = state
        .warehouse_service()
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
        .await
        .map(|r| r.items)
        .unwrap_or_default();
    let prefill = crate::pages::shipping_create::ShippingPrefill::default();
    let after_hs = "on 'htmx:afterRequest'[detail.xhr.responseText.length==0 and detail.xhr.status<400] remove .open from #wc-shipping-create-overlay";
    Ok(Html(
        crate::pages::shipping_create::shipping_create_page(
            &customers, &warehouses, &prefill,
            crate::routes::wms_work_center::WcShippingCreatePath::PATH,
            after_hs, false,
        )
        .into_string(),
    ))
}

#[require_permission("SHIPPING", "create")]
pub async fn post_shipping_create(
    _path: crate::routes::wms_work_center::WcShippingCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<crate::pages::shipping_create::ShippingCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    crate::pages::shipping_create::do_create_shipping(&state, &service_ctx, form).await?;
    crate::toast::add_toast(
        service_ctx.operator_id,
        "发货单已创建",
        crate::toast::ToastType::Success,
    );
    Ok(([("HX-Trigger", "wcChanged, showToast")], Html(String::new())))
}

// ── 入库创建 drawer（Arrival tab「新建入库单」按钮）──

#[require_permission("INVENTORY", "read")]
pub async fn get_stock_in_create_drawer(
    _path: crate::routes::wms_work_center::WcStockInCreateDrawerPath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let after_hs = "on 'htmx:afterRequest'[detail.xhr.responseText.length==0 and detail.xhr.status<400] remove .open from #wc-stock-in-create-overlay";
    let warehouses = state
        .warehouse_service()
        .list(&service_ctx, &mut conn, WarehouseFilter::default(), 1, 200)
        .await
        .map(|r| r.items)
        .unwrap_or_default();
    Ok(Html(
        crate::pages::wms_stock_in_create::stock_in_create_content(
            crate::routes::wms_work_center::WcStockInCreatePath::PATH,
            after_hs,
            false,
            &warehouses,
            false,
        )
        .into_string(),
    ))
}

#[require_permission("INVENTORY", "create")]
pub async fn post_stock_in_create(
    _path: crate::routes::wms_work_center::WcStockInCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<crate::pages::wms_stock_in_create::StockInCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext { state, service_ctx, .. } = ctx;
    crate::pages::wms_stock_in_create::do_create_stock_in(&state, &service_ctx, form).await?;
    crate::toast::add_toast(
        service_ctx.operator_id,
        "入库单已创建",
        crate::toast::ToastType::Success,
    );
    Ok(([("HX-Trigger", "wcChanged, showToast")], Html(String::new())))
}

// ── drawer body（GET ?drawer=&id=）：按 action 渲染表单，提交走单端点 POST ──

async fn render_drawer_body(action: &str, id: i64, view: Option<&str>, ctx: RequestContext) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let body = match action {
        "receive_po" => po_receive_drawer_body(&state, &service_ctx, &mut conn, id).await?,
        "receive_wo" => wo_receive_drawer_body(&state, &service_ctx, &mut conn, id).await?,
        "direct_ship" => direct_ship_drawer_body(&state, &service_ctx, &mut conn, id).await?,
        "issue" => issue_drawer_body(&state, &service_ctx, &mut conn, id).await?,
        "req_detail" => req_detail_drawer_body(&state, &service_ctx, &mut conn, id, view).await?,
        "transfer_detail" => transfer_detail_drawer_body(&state, &service_ctx, &mut conn, id, view).await?,
        "cc_detail" => cc_detail_drawer_body(&state, &service_ctx, &mut conn, id, view).await?,
        "osa_issue_detail" => osa_issue_drawer_body(&state, &service_ctx, &mut conn, id).await?,
        "transfer" => transfer_drawer_body(&state, &service_ctx, &mut conn, id).await?,
        "ship_detail" => ship_detail_drawer_body(&state, &service_ctx, &mut conn, id).await?,
        "arrival_po_detail" => arrival_po_detail_drawer_body(&state, &service_ctx, &mut conn, id).await?,
        "arrival_wo_detail" => arrival_wo_detail_drawer_body(&state, &service_ctx, &mut conn, id).await?,
        other => return Err(DomainError::validation(format!("未知 drawer 动作: {other}")).into()),
    };
    Ok(Html(body.into_string()))
}

/// drawer 操作表单：标题栏（含×）+ form（hidden action/id，hx-post 单端点，target 受影响卡片，
/// 成功关 drawer）包裹 inner。顶栏总数由 POST 响应内 #wc-total-badge(hx-swap-oob) 自动更新。
fn drawer_form(
    title: &str,
    action: &str,
    id: i64,
    _domain: WorkCenterDomain,
    confirm: &str,
    onsubmit: &str,
    inner: Markup,
    footer_label: &str,
) -> Markup {
    let footer = drawer_footer(footer_label);
    html! {
       div class="flex items-center justify-between px-6 py-5 border-b border-border-soft" {
            div class="font-bold text-base text-fg" { (title) }
            button type="button"
                class="w-8 h-8 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg flex items-center justify-center"
                _="on click remove .open from #wc-drawer-overlay" {
                (icon::x_icon("w-4 h-4"))
            }
        }
      form id=(format!("wc-{action}-form"))
            hx-post=(WmsWorkCenterPath::PATH)
            hx-target="#wc-domain-card"
            hx-select="#wc-domain-card"
            hx-swap="outerHTML"
            hx-confirm=(confirm)
            onsubmit=(onsubmit)
            _="on 'htmx:afterRequest'[detail.xhr.status<400 and detail.elt is me] remove .open from #wc-drawer-overlay"
            class="flex-1 flex flex-col overflow-hidden" {
            input type="hidden" name="action" value=(action);
            input type="hidden" name="id" value=(id);
            div class="flex-1 overflow-y-auto px-6 py-5" {
                (inner)
            }
            (footer)
        }
    }
}

/// drawer 非操作态（部分发料等不可操作状态）：标题栏 + 警示 + 跳详情链接
fn drawer_message(
    title: &str,
    doc_label: &str,
    doc_number: &str,
    msg: &str,
    link_url: &str,
    link_label: &str,
) -> Markup {
    html! {
        div class="flex items-center justify-between px-6 py-5 border-b border-border-soft" {
            div class="font-bold text-base text-fg" { (title) }
            button type="button"
                class="w-8 h-8 border-none bg-transparent text-muted cursor-pointer rounded-sm hover:bg-surface hover:text-fg flex items-center justify-center"
                _="on click remove .open from #wc-drawer-overlay" {
                (icon::x_icon("w-4 h-4"))
            }
        }
        div class="px-6 py-5" {
            div class="mb-3" {
                span class="text-xs text-muted font-medium" { (doc_label) " " }
                span class="text-sm font-mono font-semibold text-fg" { (doc_number) }
            }
            p class="text-sm text-warn mb-5" { (msg) }
            div class="flex justify-end" {
                a class="inline-flex items-center gap-1 px-4 py-2 rounded-sm bg-accent text-white text-sm font-medium no-underline cursor-pointer border-none hover:opacity-90"
                    href=(link_url) {
                    (link_label) (icon::arrow_right_icon("w-3.5 h-3.5"))
                }
            }
        }
    }
}

/// drawer 底部取消/提交（提交按钮在 form 内，type=submit）
fn drawer_footer(submit_label: &str) -> Markup {
    html! {
        div class="shrink-0 flex justify-end gap-3 px-6 py-4 bg-bg border-t border-border-soft" {
            button type="button"
                class="px-4 py-2 rounded-sm bg-white text-fg-2 border border-border text-sm font-medium cursor-pointer hover:bg-surface"
                _="on click remove .open from #wc-drawer-overlay" { "取消" }
            button type="submit"
                class="px-4 py-2 rounded-sm bg-accent text-white text-sm font-medium cursor-pointer border-none hover:opacity-90"
                { (submit_label) }
        }
    }
}

async fn po_receive_drawer_body(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
) -> Result<Markup> {
    let po_svc = state.purchase_order_service();
    let po = po_svc.get(ctx, db, id).await?;
    let items = po_svc.list_items(ctx, db, id).await.unwrap_or_default();
    let warehouses = state
        .warehouse_service()
        .list(ctx, db, WarehouseFilter::default(), 1, 200)
        .await
        .map(|r| r.items)
        .unwrap_or_default();
    let product_map: HashMap<i64, abt_core::master_data::product::model::Product> = state
        .product_service()
        .get_by_ids(ctx, db, items.iter().map(|i| i.product_id).collect())
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.product_id, p))
        .collect();
    let supplier_name = state
        .supplier_service()
        .get(ctx, db, po.supplier_id)
        .await
        .map(|s| s.name)
        .unwrap_or_else(|_| format!("供应商 #{}", po.supplier_id));
    let (status_label, status_cls) = match po.status {
        abt_core::purchase::enums::PurchaseOrderStatus::PartiallyReceived => ("部分到货", "text-warn bg-warn-bg"),
        abt_core::purchase::enums::PurchaseOrderStatus::Confirmed => ("待收货", "text-accent bg-accent-bg"),
        abt_core::purchase::enums::PurchaseOrderStatus::Received => ("已收货", "text-success bg-success-bg"),
        abt_core::purchase::enums::PurchaseOrderStatus::Draft => ("草稿", "text-muted bg-surface"),
        abt_core::purchase::enums::PurchaseOrderStatus::Closed => ("已关闭", "text-muted bg-surface"),
        abt_core::purchase::enums::PurchaseOrderStatus::Cancelled => ("已取消", "text-danger bg-danger-bg"),
        abt_core::purchase::enums::PurchaseOrderStatus::PendingApproval => ("待审批", "text-warn bg-warn-bg"),
    };

    let mut rows = html! {};
    let mut pending_count: i32 = 0;
    let mut total_pending: Decimal = Decimal::ZERO;
    for it in items.iter() {
        let pending = it.quantity - it.received_qty;
        if pending <= Decimal::ZERO {
            continue; // 已收完的行跳过
        }
        pending_count += 1;
        total_pending += pending;
        let unit = product_map.get(&it.product_id).map(|p| p.unit.clone()).unwrap_or_default();
        let prod_name = product_map.get(&it.product_id).map(|p| p.pdt_name.clone()).unwrap_or_else(|| format!("产品 #{}", it.product_id));
        let prod_code = product_map.get(&it.product_id).map(|p| p.product_code.clone()).unwrap_or_default();
        let auto_wh = if warehouses.len() == 1 { warehouses[0].id.to_string() } else { String::new() };
        let bid = format!("rcv-bin-{}", it.id);
        rows = html! {
            (rows)
            tr class="hover:bg-surface transition-colors duration-100" data-row data-unit=(unit) {
                input type="hidden" data-k="order_item_id" value=(it.id);
                input type="hidden" data-k="product_id" value=(it.product_id);
                // 产品
                td class="py-2 px-2.5 min-w-0" {
                    div class="text-sm text-fg font-medium leading-tight truncate w-[180px]" title=(prod_name) { (prod_name) }
                    @if !prod_code.is_empty() {
                        div class="text-xs text-muted font-mono truncate w-[180px]" { (prod_code) }
                    }
                }
                // 待收
                td class="py-2 px-2 text-right whitespace-nowrap" {
                    span class="text-xs text-muted font-mono" { (fmt_qty(pending)) " " (unit) }
                }
                // 仓库 + 库位（弹窗式：左仓库 + 右库位，排除他物料占用 + 同物料推荐）
                td class="py-1.5 px-1.5 w-[140px]" {
                    (crate::components::bin_search::warehouse_bin_cell(&bid, it.product_id, &warehouses, &auto_wh, "inbound"))
                }
                // 批次
                td class="py-1.5 px-1.5" {
                    input type="text" data-k="batch_no"
                        placeholder="可选"
                        class="w-[70px] px-1.5 py-1.5 border border-border rounded-sm text-xs font-mono bg-white focus:border-accent focus:shadow-[var(--shadow-focus)] outline-none transition-all duration-150";
                }
                // 实收
                td class="py-1.5 px-1.5" {
                    input type="number" data-k="received_qty" value=(fmt_qty(pending)) min="0" step="any"
                        class="w-[64px] px-2 py-1.5 border border-border rounded-sm text-xs font-mono text-right bg-white focus:border-accent focus:shadow-[var(--shadow-focus)] outline-none transition-all duration-150";
                }
            }
        };
    }

    let inner = html! {
        // 幂等键：drawer body 加载时生成（防双击重复入库），顶层字段不进 items_json
        input type="hidden" name="idempotency_key"
            _="on load call wcGenIdempotencyKey(me)" {};
        input type="hidden" name="items_json" value="[]";
        // 单号信息条：采购订单号 + 状态 + 供应商 + 下单日期
        div class="flex items-center justify-between mb-4 pb-4 border-b border-border-soft gap-3" {
            div class="flex items-center gap-2 min-w-0" {
                (icon::truck_icon("w-4 h-4 text-muted shrink-0"))
                div class="min-w-0" {
                    div class="flex items-center gap-2 flex-wrap" {
                        span class="text-xs text-muted" { "采购订单" }
                        span class="text-sm font-mono font-semibold text-fg" { (po.doc_number) }
                        span class=(format!("text-xs px-2 py-0.5 rounded-full font-medium {}", status_cls)) { (status_label) }
                    }
                    div class="text-xs text-fg-2 mt-0.5 truncate" {
                        (supplier_name) " · 下单 " (po.order_date.format("%Y-%m-%d"))
                    }
                }
            }
            span class="text-xs text-muted shrink-0" { (pending_count) " 项 · 待收 " (fmt_qty(total_pending)) }
        }
        // 送货单号 + 备注（顶层字段，透传到 ReceivePurchaseReq）
        div class="grid grid-cols-2 gap-3 mb-4" {
            input type="text" name="delivery_note"
                class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                placeholder="送货单号（可选）" {};
            input type="text" name="remark"
                class="px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent focus:shadow-[var(--shadow-focus)]"
                placeholder="备注（差异说明等，可选）" {};
        }
        // 统一仓库
        div class="mb-3 flex items-center gap-2" {
            select
                _="on change call wcApplyWarehouseAll(me)"
                class="flex-1 px-2.5 py-2 border border-border rounded-sm text-sm bg-surface text-muted focus:border-accent outline-none transition-all duration-150" {
                option value="" selected { "统一仓库：应用到所有行…" }
                @for w in &warehouses {
                    option value=(w.id) { (w.name) }
                }
            }
        }
        // 产品明细表格
        div class="mb-4 overflow-visible" {
            table class="w-full text-sm border-collapse" {
                thead {
                    tr class="border-b border-border-soft text-xs text-muted" {
                        th class="py-2 px-2.5 text-left font-semibold" { "产品" }
                        th class="py-2 px-2 text-right font-semibold whitespace-nowrap" { "待收" }
                        th class="py-2 px-1.5 text-left font-semibold whitespace-nowrap" { "仓库 / 库位" }
                        th class="py-2 px-1.5 text-left font-semibold whitespace-nowrap" { "批次" }
                        th class="py-2 px-1.5 text-right font-semibold whitespace-nowrap" { "实收" }
                    }
                }
                tbody class="divide-y divide-border-soft" { (rows) }
            }
        }
        // 提示
        div class="mb-4 p-3 rounded-md bg-accent-bg border border-accent/20 flex items-start gap-2" {
            (icon::clock_icon("w-3.5 h-3.5 text-accent mt-0.5 shrink-0"))
            p class="text-xs text-accent leading-relaxed" {
                "确认后直接入库，并自动回写采购订单收货量、立应付账款"
            }
        }
    };
    Ok(drawer_form(
        "采购收货入库",
        "receive_po",
        id,
        WorkCenterDomain::Arrival,
        "确认收货入库？将直接入库并回写采购订单",
        "wcReceiveSubmit(this)",
        inner,
        "确认入库",
    ))
}

/// 生产工单入库 drawer：完工产品（completed_qty - 已入库量）上架，仅记库存（不立应付、不回写工单完工量）
async fn wo_receive_drawer_body(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
) -> Result<Markup> {
    let wo = state.work_order_service().find_by_id(ctx, db, id).await?;
    let product = state.product_service().get(ctx, db, wo.product_id).await?;
    let received: Decimal = state
        .inventory_transaction_service()
        .find_by_source(ctx, db, "work_order", id)
        .await
        .unwrap_or_default()
        .iter()
        .map(|t| t.quantity)
        .sum();
    let pending = wo.completed_qty - received;
    let warehouses = state
        .warehouse_service()
        .list(ctx, db, WarehouseFilter::default(), 1, 200)
        .await
        .map(|r| r.items)
        .unwrap_or_default();

    let auto_wh = if warehouses.len() == 1 { warehouses[0].id.to_string() } else { String::new() };
    let bid = format!("wo-bin-{}", wo.product_id);

    let inner = html! {
        input type="hidden" name="items_json" value="[]";
        // 单号信息条（对齐收货/发货 drawer）
        div class="flex items-center justify-between mb-4 pb-4 border-b border-border-soft" {
            div class="flex items-center gap-2" {
                (icon::package_icon("w-4 h-4 text-muted"))
                span class="text-xs text-muted" { "生产工单" }
                span class="text-sm font-mono font-semibold text-fg" { (wo.doc_number) }
            }
            span class="text-xs text-muted" {
                "完工 " (fmt_qty(wo.completed_qty)) " · 已入库 " (fmt_qty(received)) " · 待入库 "
                span class="text-fg font-semibold" { (fmt_qty(pending)) }
            }
        }
        @if pending <= Decimal::ZERO {
            div class="mb-4 p-3 rounded-md bg-warn-bg border border-warn/20 flex items-start gap-2" {
                (icon::clock_icon("w-3.5 h-3.5 text-warn mt-0.5 shrink-0"))
                p class="text-xs text-warn font-medium leading-relaxed" {
                    "该工单完工产品已全部入库，无需操作。"
                }
            }
        } @else {
            // 统一仓库
            div class="mb-3 flex items-center gap-2" {
                select
                    _="on change call wcApplyWarehouseAll(me)"
                    class="flex-1 px-2.5 py-2 border border-border rounded-sm text-sm bg-surface text-muted focus:border-accent outline-none transition-all duration-150" {
                    option value="" selected { "统一仓库：应用到所有行…" }
                    @for w in &warehouses {
                        option value=(w.id) { (w.name) }
                    }
                }
            }
            // 产品明细表格
            div class="mb-4 overflow-visible" {
                table class="w-full text-sm border-collapse" {
                    thead {
                        tr class="border-b border-border-soft text-xs text-muted" {
                            th class="py-2 px-2.5 text-left font-semibold" { "产品" }
                            th class="py-2 px-2 text-right font-semibold whitespace-nowrap" { "待入库" }
                            th class="py-2 px-1.5 text-left font-semibold whitespace-nowrap" { "仓库 / 库位" }
                            th class="py-2 px-1.5 text-left font-semibold whitespace-nowrap" { "批次" }
                            th class="py-2 px-1.5 text-right font-semibold whitespace-nowrap" { "入库量" }
                        }
                    }
                    tbody class="divide-y divide-border-soft" {
                        tr class="hover:bg-surface transition-colors duration-100" data-row data-unit=(product.unit) {
                            input type="hidden" data-k="product_id" value=(wo.product_id);
                            td class="py-2 px-2.5 min-w-0" {
                                div class="text-sm text-fg font-medium leading-tight truncate w-[180px]" title=(product.pdt_name) { (product.pdt_name) }
                                div class="text-xs text-muted font-mono truncate w-[180px]" { (product.product_code) }
                            }
                            td class="py-2 px-2 text-right whitespace-nowrap" {
                                span class="text-xs text-muted font-mono" { (fmt_qty(pending)) " " (product.unit) }
                            }
                            // 仓库 + 库位（弹窗式）
                            td class="py-1.5 px-1.5 w-[140px]" {
                                (crate::components::bin_search::warehouse_bin_cell(&bid, wo.product_id, &warehouses, &auto_wh, "inbound"))
                            }
                            td class="py-1.5 px-1.5" {
                                input type="text" data-k="batch_no"
                                    placeholder="可选"
                                    class="w-[70px] px-1.5 py-1.5 border border-border rounded-sm text-xs font-mono bg-white focus:border-accent focus:shadow-[var(--shadow-focus)] outline-none transition-all duration-150";
                            }
                            td class="py-1.5 px-1.5" {
                                input type="number" data-k="received_qty" value=(fmt_qty(pending)) min="0" step="any"
                                    class="w-[64px] px-2 py-1.5 border border-border rounded-sm text-xs font-mono text-right bg-white focus:border-accent focus:shadow-[var(--shadow-focus)] outline-none transition-all duration-150";
                            }
                        }
                    }
                }
            }
            // 提示
            div class="mb-4 p-3 rounded-md bg-accent-bg border border-accent/20 flex items-start gap-2" {
                (icon::clock_icon("w-3.5 h-3.5 text-accent mt-0.5 shrink-0"))
                p class="text-xs text-accent leading-relaxed" {
                    "生产入库仅登记库存（不计应付、不回写工单完工量——报工时已累加）"
                }
            }
        }
    };
    Ok(drawer_form(
        "生产入库",
        "receive_wo",
        id,
        WorkCenterDomain::Arrival,
        "确认生产入库？",
        "wcReceiveSubmit(this)",
        inner,
        "确认入库",
    ))
}

async fn direct_ship_drawer_body(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
) -> Result<Markup> {
    let s = state.picking_service().find_by_id(ctx, db, id).await?;
    // 仅 Confirmed（待发货）单可直发；其他状态展示只读详情 drawer（Issue #188 收口，不再跳发货详情页）
    if s.status != PickingStatus::Confirmed {
        let (status_text, _) = picking_status_label(s.status);
        let detail = ship_detail_drawer_body(state, ctx, db, id).await?;
        // 在详情上方插入状态提示条
        let result: Markup = html! {
            div class="px-6 py-3 bg-warn-bg border-b border-warn-200" {
                span class="text-sm text-warn font-medium" {
                    "该单当前状态为「" (status_text) "」，无法直接发货。"
                }
            }
            (detail)
        };
        return Ok(result);
    }
    let items = state.picking_service().list_items(ctx, db, id).await.unwrap_or_default();
    let product_map: HashMap<i64, abt_core::master_data::product::model::Product> = state
        .product_service()
        .get_by_ids(ctx, db, items.iter().map(|i| i.product_id).collect())
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.product_id, p))
        .collect();
    // 可用库存（全仓库 ATP），选仓库后通过 HTMX 端点动态刷新
    let inv_svc = state.inventory_transaction_service();
    let avail_map: HashMap<i64, Decimal> = inv_svc
        .query_available_batch(ctx, db, &items.iter().map(|i| i.product_id).collect::<Vec<_>>(), None)
        .await
        .unwrap_or_default();
    let warehouses = state
        .warehouse_service()
        .list(ctx, db, WarehouseFilter::default(), 1, 200)
        .await
        .map(|r| r.items)
        .unwrap_or_default();
    let total_qty: Decimal = items.iter().map(|i| i.qty_requested).sum();

    let mut rows = html! {};
    let mut shortage_count: i32 = 0;
    for it in &items {
        let prod_name = product_map
            .get(&it.product_id)
            .map(|p| p.pdt_name.clone())
            .unwrap_or_else(|| format!("产品 #{}", it.product_id));
        let avail = avail_map.get(&it.product_id).copied().unwrap_or(Decimal::ZERO);
        let is_shortage = avail < it.qty_requested;
        if is_shortage { shortage_count += 1; }
        let unit = product_map.get(&it.product_id).map(|p| p.unit.clone()).unwrap_or_default();
        let prod_code = product_map.get(&it.product_id).map(|p| p.product_code.clone()).unwrap_or_default();
        rows = html! {
            (rows)
            tr class="hover:bg-surface transition-colors duration-100" data-row data-pid=(it.product_id) data-need=(it.qty_requested) data-unit=(unit) {
                input type="hidden" data-k="picking_item_id" value=(it.id);
                input type="hidden" data-k="product_id" value=(it.product_id);
                // 产品
                td class="py-2 px-2.5 min-w-0" {
                    div class="text-sm text-fg font-medium leading-tight truncate w-[160px]" title=(prod_name) { (prod_name) }
                    @if !prod_code.is_empty() {
                        div class="text-xs text-muted font-mono truncate w-[160px]" { (prod_code) }
                    }
                }
                // 需求量
                td class="py-2 px-2 text-right whitespace-nowrap" {
                    span class="text-xs text-muted font-mono" { (fmt_qty(it.qty_requested)) }
                }
                // 可用库存
                td class="py-2 px-2 text-right whitespace-nowrap" {
                    span class="text-xs font-mono" data-avail {
                        @if is_shortage {
                            span class="text-danger" { (fmt_qty(avail)) " 缺" }
                        } @else {
                            span class="text-muted" { (fmt_qty(avail)) }
                        }
                    }
                }
                // 仓库 + 库位（弹窗式，出库：只列该物料有实物存量的库位 + 可用量）
                td class="py-1.5 px-1.5 w-[200px]" {
                    @let bid = format!("bin-{}", it.id);
                    (crate::components::bin_search::warehouse_bin_cell(&bid, it.product_id, &warehouses, "", "outbound"))
                }
                // 批次
                td class="py-1.5 px-1.5" {
                    input type="text" data-k="batch_no"
                        class="w-[70px] px-1.5 py-1.5 border border-border rounded-sm text-xs font-mono bg-white focus:border-accent focus:shadow-[var(--shadow-focus)] outline-none transition-all duration-150";
                }
                // 实发
                td class="py-1.5 px-1.5" {
                    input type="number" data-k="qty" value=(fmt_qty(it.qty_requested)) min="0" step="any"
                        class="w-[64px] px-2 py-1.5 border border-border rounded-sm text-xs font-mono text-right bg-white focus:border-accent focus:shadow-[var(--shadow-focus)] outline-none transition-all duration-150";
                }
            }
        };
    }

    let inner = html! {
        // 隐藏 items_json（wcShipCollectRows 填充）
        input type="hidden" name="items_json" value="[]" {};
        // 单号信息条
        div class="flex items-center justify-between mb-4 pb-4 border-b border-border-soft" {
            div class="flex items-center gap-2" {
                (icon::truck_icon("w-4 h-4 text-muted"))
                span class="text-xs text-muted" { "发货单" }
                span class="text-sm font-mono font-semibold text-fg" { (s.doc_number) }
            }
            div class="flex items-center gap-2" {
                @if shortage_count > 0 {
                    span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-semibold bg-danger-bg text-danger" {
                        (shortage_count) " 项缺货"
                    }
                }
                span class="text-xs text-muted" { (items.len()) " 项 · " (fmt_qty(total_qty)) }
            }
        }
        // 统一仓库（批量应用到所有行）
        div class="mb-3 flex items-center gap-2" {
            select id="ship-warehouse"
                _="on change call wcApplyWarehouseAll(me) then wcShipRefreshStock(me)"
                class="flex-1 px-2.5 py-2 border border-border rounded-sm text-sm bg-surface text-muted focus:border-accent outline-none transition-all duration-150" {
                option value="" selected { "统一仓库：应用到所有行…" }
                @for w in &warehouses {
                    option value=(w.id) { (w.name) }
                }
            }
        }
        // 产品明细表格
        div class="mb-4 overflow-visible" {
            table class="w-full text-sm border-collapse" {
                thead {
                    tr class="border-b border-border-soft text-xs text-muted" {
                        th class="py-2 px-2.5 text-left font-semibold" { "产品" }
                        th class="py-2 px-2 text-right font-semibold whitespace-nowrap" { "需求" }
                        th class="py-2 px-2 text-right font-semibold whitespace-nowrap" { "可用" }
                        th class="py-2 px-1.5 text-left font-semibold whitespace-nowrap" { "仓库 / 库位" }
                        th class="py-2 px-1.5 text-left font-semibold whitespace-nowrap" { "批次" }
                        th class="py-2 px-1.5 text-right font-semibold whitespace-nowrap" { "实发" }
                    }
                }
                tbody class="divide-y divide-border-soft" { (rows) }
            }
        }
        // 操作提示
        div class="mb-4 p-3 rounded-md bg-warn-bg border border-warn/20 flex items-start gap-2" {
            (icon::clock_icon("w-3.5 h-3.5 text-warn mt-0.5 shrink-0"))
            p class="text-xs text-warn leading-relaxed" {
                "确认发出将从所选仓库扣减库存、立应收账款并回写销售订单"
            }
        }
    };
    Ok(drawer_form(
        "直接发货",
        "direct_ship",
        id,
        WorkCenterDomain::Outbound,
        "确认直接发出？将从所选仓库扣库存并立应收",
        "return wcShipCollectRows(this)",
        inner,
        "确认发出",
    ))
}

/// 发货 drawer 选仓库后查询各产品可用库存 → JSON {pid: qty}。
#[require_permission("SHIPPING", "read")]
pub async fn get_ship_stock_avail(
    _path: crate::routes::wms_work_center::WcShipStockAvailPath,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    ctx: RequestContext,
) -> Result<axum::Json<serde_json::Value>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let warehouse_id: Option<i64> = params.get("warehouse_id")
        .and_then(|s| s.parse().ok())
        .filter(|&v: &i64| v > 0);
    let product_ids: Vec<i64> = params.get("product_ids")
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    let inv_svc = state.inventory_transaction_service();
    let avail_map = inv_svc
        .query_available_batch(&service_ctx, &mut conn, &product_ids, warehouse_id)
        .await
        .unwrap_or_default();
    let json_map: std::collections::HashMap<String, String> = avail_map
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Ok(axum::Json(serde_json::json!(json_map)))
}

/// 调拨 drawer 选源仓后查询各产品可用库存 → JSON {pid: qty}（INVENTORY 权限，复用 query_available_batch ATP 口径）。
#[require_permission("INVENTORY", "read")]
pub async fn get_transfer_stock_avail(
    _path: crate::routes::wms_work_center::WcTransferStockAvailPath,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    ctx: RequestContext,
) -> Result<axum::Json<serde_json::Value>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let warehouse_id: Option<i64> = params.get("warehouse_id")
        .and_then(|s| s.parse().ok())
        .filter(|&v: &i64| v > 0);
    let product_ids: Vec<i64> = params.get("product_ids")
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    let avail_map = state
        .inventory_transaction_service()
        .query_available_batch(&service_ctx, &mut conn, &product_ids, warehouse_id)
        .await
        .unwrap_or_default();
    let json_map: HashMap<String, String> = avail_map
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Ok(axum::Json(serde_json::json!(json_map)))
}

/// 盘点 drawer 选库位后查该物料系统账面数量（快照）→ JSON {system_qty}。
#[require_permission("INVENTORY", "read")]
pub async fn get_cycle_count_system_qty(
    _path: crate::routes::wms_work_center::WcCycleCountSystemQtyPath,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    ctx: RequestContext,
) -> Result<axum::Json<serde_json::Value>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let product_id: i64 = params.get("product_id").and_then(|s| s.parse().ok()).unwrap_or(0);
    let bin_id: i64 = params.get("bin_id").and_then(|s| s.parse().ok()).unwrap_or(0);
    let qty = if product_id <= 0 || bin_id <= 0 {
        Decimal::ZERO
    } else {
        state
            .inventory_service()
            .query(
                &service_ctx,
                &mut conn,
                abt_core::wms::inventory::model::InventoryQueryFilter {
                    product_id: Some(product_id),
                    keyword: None,
                    warehouse_id: None,
                    bin_id: Some(bin_id),
                },
                1,
                1,
            )
            .await
            .ok()
            .and_then(|r| r.items.into_iter().next())
            .map(|i| i.quantity)
            .unwrap_or(Decimal::ZERO)
    };
    Ok(axum::Json(serde_json::json!({ "system_qty": qty.to_string() })))
}

async fn issue_drawer_body(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
) -> Result<Markup> {
    let req_svc = state.picking_service();
    let req = req_svc.get(ctx, db, id).await?;
    if req.status == PickingStatus::Confirmed {
        let items = req_svc.list_items(ctx, db, id).await.unwrap_or_default();
        let product_ids: Vec<i64> = items.iter().map(|i| i.product_id).collect();
        let product_map: HashMap<i64, abt_core::master_data::product::model::Product> = state
            .product_service()
            .get_by_ids(ctx, db, product_ids.clone())
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|p| (p.product_id, p))
            .collect();
        // 可用库存（全仓库 ATP），选仓库后 wcShipRefreshStock 刷新
        let avail_map: HashMap<i64, Decimal> = state
            .inventory_transaction_service()
            .query_available_batch(ctx, db, &product_ids, None)
            .await
            .unwrap_or_default();
        let warehouses = state
            .warehouse_service()
            .list(ctx, db, WarehouseFilter::default(), 1, 200)
            .await
            .map(|r| r.items)
            .unwrap_or_default();
        let total_qty: Decimal = items.iter().map(|i| i.qty_requested).sum();
        let mut shortage_count: i32 = 0;

        let mut rows = html! {};
        for it in &items {
            let prod_name = product_map
                .get(&it.product_id)
                .map(|p| p.pdt_name.clone())
                .unwrap_or_else(|| format!("产品 #{}", it.product_id));
            let prod_code = product_map.get(&it.product_id).map(|p| p.product_code.clone()).unwrap_or_default();
            let avail = avail_map.get(&it.product_id).copied().unwrap_or(Decimal::ZERO);
            let is_shortage = avail < it.qty_requested;
            if is_shortage {
                shortage_count += 1;
            }
            rows = html! {
                (rows)
                tr class="hover:bg-surface transition-colors duration-100" data-row data-pid=(it.product_id) data-need=(it.qty_requested) {
                    input type="hidden" data-k="picking_item_id" value=(it.id);
                    input type="hidden" data-k="product_id" value=(it.product_id);
                    // 产品
                    td class="py-2 px-2.5 min-w-0" {
                        div class="text-sm text-fg font-medium leading-tight truncate w-[160px]" title=(prod_name) { (prod_name) }
                        @if !prod_code.is_empty() {
                            div class="text-xs text-muted font-mono truncate w-[160px]" { (prod_code) }
                        }
                    }
                    // 申请量
                    td class="py-2 px-2 text-right whitespace-nowrap" {
                        span class="text-xs text-muted font-mono" { (fmt_qty(it.qty_requested)) }
                    }
                    // 可用（选仓后 wcShipRefreshStock 刷新）
                    td class="py-2 px-2 text-right whitespace-nowrap" {
                        span class="text-xs font-mono" data-avail {
                            @if is_shortage {
                                span class="text-danger" { (fmt_qty(avail)) " 缺" }
                            } @else {
                                span class="text-muted" { (fmt_qty(avail)) }
                            }
                        }
                    }
                    // 仓库 + 库位（弹窗式 outbound：只列该物料有实物存量的库位）
                    td class="py-1.5 px-1.5 w-[200px]" {
                        @let bid = format!("bin-{}", it.id);
                        (crate::components::bin_search::warehouse_bin_cell(&bid, it.product_id, &warehouses, "", "outbound"))
                    }
                    // 实发
                    td class="py-1.5 px-1.5" {
                        input type="number" data-k="qty" value=(fmt_qty(it.qty_requested)) min="0" step="any"
                            class="w-[64px] px-2 py-1.5 border border-border rounded-sm text-xs font-mono text-right bg-white focus:border-accent focus:shadow-[var(--shadow-focus)] outline-none transition-all duration-150";
                    }
                }
            };
        }

        let inner = html! {
            // 隐藏 items_json（wcShipCollectRows 填充）
            input type="hidden" name="items_json" value="[]" {};
            // 单号信息条
            div class="flex items-center justify-between mb-4 pb-4 border-b border-border-soft" {
                div class="flex items-center gap-2" {
                    (icon::box_icon("w-4 h-4 text-muted"))
                    span class="text-xs text-muted" { "领料单" }
                    span class="text-sm font-mono font-semibold text-fg" { (req.doc_number) }
                }
                div class="flex items-center gap-2" {
                    @if shortage_count > 0 {
                        span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-semibold bg-danger-bg text-danger" {
                            (shortage_count) " 项缺料"
                        }
                    }
                    span class="text-xs text-muted" { (items.len()) " 项 · " (fmt_qty(total_qty)) }
                }
            }
            // 统一仓库（批量应用到所有行）
            div class="mb-3 flex items-center gap-2" {
                select id="issue-warehouse"
                    _="on change call wcApplyWarehouseAll(me) then wcShipRefreshStock(me)"
                    class="flex-1 px-2.5 py-2 border border-border rounded-sm text-sm bg-surface text-muted focus:border-accent outline-none transition-all duration-150" {
                    option value="" selected { "统一仓库：应用到所有行…" }
                    @for w in &warehouses {
                        option value=(w.id) { (w.name) }
                    }
                }
            }
            // 物料明细表格
            div class="mb-4 overflow-visible" {
                table class="w-full text-sm border-collapse" {
                    thead {
                        tr class="border-b border-border-soft text-xs text-muted" {
                            th class="py-2 px-2.5 text-left font-semibold" { "产品" }
                            th class="py-2 px-2 text-right font-semibold whitespace-nowrap" { "申请" }
                            th class="py-2 px-2 text-right font-semibold whitespace-nowrap" { "可用" }
                            th class="py-2 px-1.5 text-left font-semibold whitespace-nowrap" { "仓库 / 库位" }
                            th class="py-2 px-1.5 text-right font-semibold whitespace-nowrap" { "实发" }
                        }
                    }
                    tbody class="divide-y divide-border-soft" { (rows) }
                }
            }
            // 操作提示
            div class="mb-4 p-3 rounded-md bg-warn-bg border border-warn/20 flex items-start gap-2" {
                (icon::clock_icon("w-3.5 h-3.5 text-warn mt-0.5 shrink-0"))
                p class="text-xs text-warn leading-relaxed" {
                    "确认发料将从所选仓库/库位扣减库存并计入工单成本"
                }
            }
        };
        Ok(drawer_form(
            "发料",
            "issue",
            id,
            WorkCenterDomain::Requisition,
            "确认发料？将从所选仓库/库位扣减库存并计入工单成本",
            "return wcShipCollectRows(this)",
            inner,
            "确认发料",
        ))
    } else {
        // PartiallyIssued：issue 记绝对量，就地重复发料会重复扣库存。detail 页已收口删除，
        // 引导回作业中心全部视图（详情 drawer 可查看明细，续发暂未支持）
        let url = format!("{}?domain=requisition&view=all", WmsWorkCenterPath::PATH);
        Ok(drawer_message(
            "发料",
            "领料单",
            &req.doc_number,
            "该单已部分发料，就地续发会重复扣库存。可在全部视图查看明细。",
            &url,
            "返回作业中心",
        ))
    }
}

async fn transfer_drawer_body(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
) -> Result<Markup> {
    let trf = state.picking_service().get(ctx, db, id).await?;
    let items = state.picking_service().list_items(ctx, db, id).await.unwrap_or_default();
    let (title, action, hint, btn_label) = match trf.status {
        PickingStatus::Draft => ("调出", "dispatch", "确认调出将从源仓扣减库存、单据进入在途。", "确认调出"),
        PickingStatus::Confirmed => ("到货确认", "complete", "确认到货将把库存计入目标仓、完成调拨。", "确认到货"),
        _ => ("调拨", "complete", "该单当前状态不可就地操作。", "确认"),
    };
    let inner = html! {
        div class="mb-3" {
            span class="text-xs text-muted font-medium" { "调拨单 " }
            span class="text-sm font-mono font-semibold text-fg" { (trf.doc_number) }
        }
        p class="text-sm text-muted mb-2" { "仓 " (trf.from_warehouse_id.unwrap_or(0)) " → " (trf.to_warehouse_id.unwrap_or(0)) " · 共 " (items.len()) " 项" }
        p class="text-sm text-muted mb-5" { (hint) }
    };
    Ok(drawer_form(
        title,
        action,
        id,
        WorkCenterDomain::Transfer,
        "确认执行此调拨操作？",
        "",
        inner,
        btn_label,
    ))
}

/// 委外发料 drawer body（Issue #270）：仓库执行发料。id = OutsourceIssue picking id。
/// 显示委外单 + 供应商 + 发料清单（物料/数量），「确认发料」POST osa_issue → dispatch_action
/// 对该 OSA 全部 sibling picking dispatch+complete + om.confirm_sent（OSA Draft→Sent）。
async fn osa_issue_drawer_body(
    state: &AppState,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    id: i64,
) -> Result<Markup> {
    let pk_svc = state.picking_service();
    let picking = pk_svc.get(ctx, db, id).await?;
    let osa_id = picking.source_id.ok_or_else(|| {
        DomainError::validation("委外发料单未关联委外单（source_id 缺失）")
    })?;
    let osa = state
        .outsourcing_order_service()
        .find_by_id(ctx, db, osa_id)
        .await?;
    let items = pk_svc.list_items(ctx, db, id).await.unwrap_or_default();
    let product_map: HashMap<i64, abt_core::master_data::product::model::Product> = state
        .product_service()
        .get_by_ids(ctx, db, items.iter().map(|i| i.product_id).collect())
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.product_id, p))
        .collect();
    let supplier_name = state
        .supplier_service()
        .get(ctx, db, osa.supplier_id)
        .await
        .map(|s| s.name)
        .unwrap_or_else(|_| format!("供应商 #{}", osa.supplier_id));
    let from_wh = state
        .warehouse_service()
        .get(ctx, db, picking.from_warehouse_id.unwrap_or(0))
        .await
        .map(|w| w.name)
        .unwrap_or_else(|_| "—".into());

    let mut rows = html! {};
    for it in &items {
        let pname = product_map
            .get(&it.product_id)
            .map(|p| p.pdt_name.clone())
            .unwrap_or_else(|| format!("产品 #{}", it.product_id));
        let pcode = product_map
            .get(&it.product_id)
            .map(|p| p.product_code.clone())
            .unwrap_or_default();
        rows = html! {
            (rows)
            div class="flex items-center justify-between px-3 py-2 gap-2" {
                div class="min-w-0" {
                    div class="text-sm text-fg-2 truncate" { (pname) }
                    div class="text-xs text-muted truncate" { (pcode) }
                }
                div class="text-right shrink-0 text-sm font-mono text-fg" { "发料 " (fmt_qty(it.qty_requested)) }
            }
        };
    }

    let inner = html! {
        div class="mb-4 pb-4 border-b border-border-soft" {
            div class="flex items-center gap-2 mb-3" {
                span class="text-xs text-muted font-medium" { "委外单 " }
                span class="text-base font-mono font-bold text-fg" { (osa.doc_number) }
            }
            div class="grid grid-cols-2 gap-x-4 gap-y-2 text-xs" {
                div {
                    span class="text-muted" { "供应商 " }
                    span class="text-fg-2" { (supplier_name) }
                }
                div {
                    span class="text-muted" { "发料仓库 " }
                    span class="text-fg-2" { (from_wh) }
                }
            }
        }
        p class="text-sm text-muted mb-3" { "确认发料将从源仓扣减物料、计入委外虚拟仓，并回写委外单为「已发料」。" }
        div class="mb-2" {
            div class="text-xs font-semibold text-muted mb-2" { "发料清单（" (items.len()) " 项）" }
            div class="rounded-sm border border-border-soft divide-y divide-border-soft" {
                @if items.is_empty() {
                    div class="px-3 py-4 text-center text-sm text-muted" { "暂无明细" }
                } @else {
                    (rows)
                }
            }
        }
    };

    Ok(drawer_form(
        "委外发料",
        "osa_issue",
        id,
        WorkCenterDomain::OutsourceIssue,
        "确认执行委外发料？",
        "",
        inner,
        "确认发料",
    ))
}

