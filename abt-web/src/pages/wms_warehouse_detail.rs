use axum_extra::routing::TypedPath;
use axum::Form;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse};
use maud::{Markup, html};
use serde::Deserialize;

use abt_core::wms::warehouse::model::*;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::wms::enums::{BinStatus, WarehouseStatus, WarehouseType, ZoneType};

use crate::components::icon;
use crate::layout::page::admin_page;
use crate::routes::wms_warehouse::{
    WarehouseDeletePath, WarehouseDetailPath, WarehouseEditPath, WarehouseListPath,
    WarehouseZoneBinsPath, WarehouseZoneCreatePath, WarehouseZonePath,
};
use crate::utils::RequestContext;

use abt_macros::require_permission;

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub struct ZoneForm {
    pub code: String,
    pub name: String,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub zone_type: Option<i16>,
    #[serde(default, deserialize_with = "crate::utils::empty_as_none")]
    pub sort_order: Option<i32>,
    pub remark: Option<String>,
}


// ── Handlers ──

#[require_permission("WAREHOUSE", "read")]
pub async fn get_warehouse_detail(
    path: WarehouseDetailPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let can_delete = ctx.has_permission("WAREHOUSE", "delete").await;
    let can_edit = ctx.has_permission("WAREHOUSE", "update").await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.warehouse_service();

    let warehouse = svc.get(&service_ctx, &mut conn, path.id).await?;
    let zones = svc.list_zones(&service_ctx, &mut conn, path.id).await?;
    let stats = svc.get_warehouse_inventory_stats(&service_ctx, &mut conn, path.id).await.ok();

    let content = warehouse_detail_page(&warehouse, &zones, stats.as_ref(), can_delete, can_edit);
    let detail_path_str = WarehouseDetailPath { id: path.id }.to_string();
    let page_html = admin_page(
        is_htmx,
        &format!("{} - 仓库详情", warehouse.name),
        &claims,
        "inventory",
        &detail_path_str,
        "库存管理",
        Some(&warehouse.name),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("WAREHOUSE", "read")]
pub async fn get_warehouse_edit(
    path: WarehouseEditPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.warehouse_service();

    let warehouse = svc.get(&service_ctx, &mut conn, path.id).await?;

    let content = crate::pages::wms_warehouse_create::warehouse_create_page(Some(&warehouse));
    let edit_path_str = WarehouseEditPath { id: path.id }.to_string();
    let page_html = admin_page(
        is_htmx,
        &format!("编辑 - {}", warehouse.name),
        &claims,
        "inventory",
        &edit_path_str,
        "库存管理",
        Some(&format!("编辑 - {}", warehouse.name)),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("WAREHOUSE", "update")]
pub async fn update_warehouse(
    path: WarehouseEditPath,
    ctx: RequestContext,
    Form(form): Form<crate::pages::wms_warehouse_create::WarehouseCreateForm>,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.warehouse_service();

    let warehouse_type = WarehouseType::from_i16(form.warehouse_type)
        .ok_or_else(|| abt_core::shared::types::DomainError::validation("无效的仓库类型"))?;

    let is_virtual = form.is_virtual.is_some();

    if form.name.trim().is_empty() {
        return Err(abt_core::shared::types::DomainError::validation("仓库名称不能为空").into());
    }

    let req = UpdateWarehouseReq {
        name: Some(form.name),
        warehouse_type: Some(warehouse_type),
        address: if is_virtual { None } else { form.address.filter(|s| !s.is_empty()) },
        manager_id: form.manager_id,
        is_virtual: Some(is_virtual),
        remark: form.remark.filter(|s| !s.is_empty()),
        status: None,
    };

    svc.update(&service_ctx, &mut conn, path.id, req).await?;

    let redirect = WarehouseDetailPath { id: path.id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

#[require_permission("WAREHOUSE", "delete")]
pub async fn delete_warehouse(
    path: WarehouseDeletePath,
    ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.warehouse_service();

    svc.delete(&service_ctx, &mut conn, path.id).await?;

    Ok(([("HX-Redirect", WarehouseListPath::PATH)], Html(String::new())))
}

// ── Zone CRUD ──

#[require_permission("WAREHOUSE", "read")]
pub async fn get_zones(
    path: WarehouseZoneCreatePath,
    ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.warehouse_service();
    let zones = svc.list_zones(&service_ctx, &mut conn, path.id).await?;
    Ok(Html(zones_table_fragment(&zones, path.id).into_string()))
}

#[require_permission("WAREHOUSE", "create")]
pub async fn create_zone(
    path: WarehouseZoneCreatePath,
    ctx: RequestContext,
    Form(form): Form<ZoneForm>,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.warehouse_service();

    let zone_type = form.zone_type
        .and_then(ZoneType::from_i16)
        .ok_or_else(|| abt_core::shared::types::DomainError::validation("请选择库区类型"))?;

    let req = CreateZoneReq {
        code: form.code,
        name: form.name,
        zone_type,
        sort_order: form.sort_order,
        remark: form.remark.filter(|s| !s.is_empty()),
    };

    svc.create_zone(&service_ctx, &mut conn, path.id, req).await?;

    // Re-render zones table
    let zones = svc.list_zones(&service_ctx, &mut conn, path.id).await?;
    Ok((
        StatusCode::OK,
        [("HX-Trigger", "zoneChanged")],
        Html(zones_table_fragment(&zones, path.id).into_string()),
    ))
}

#[require_permission("WAREHOUSE", "read")]
pub async fn get_zone_edit_form(
    path: WarehouseZonePath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.warehouse_service();
    let zone = svc.get_zone(&service_ctx, &mut conn, path.zone_id).await?;
    Ok(Html(zone_edit_form_fragment(&zone).into_string()))
}

#[require_permission("WAREHOUSE", "update")]
pub async fn update_zone(
    path: WarehouseZonePath,
    ctx: RequestContext,
    Form(form): Form<ZoneForm>,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.warehouse_service();

    let zone_type = form.zone_type
        .and_then(ZoneType::from_i16)
        .ok_or_else(|| abt_core::shared::types::DomainError::validation("请选择库区类型"))?;

    let req = UpdateZoneReq {
        name: Some(form.name),
        zone_type: Some(zone_type),
        sort_order: form.sort_order,
        remark: form.remark.filter(|s| !s.is_empty()),
    };

    svc.update_zone(&service_ctx, &mut conn, path.zone_id, req).await?;

    Ok((StatusCode::OK, [("HX-Trigger", "zoneChanged")], Html(String::new())))
}

#[require_permission("WAREHOUSE", "delete")]
pub async fn delete_zone(
    path: WarehouseZonePath,
    ctx: RequestContext,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.warehouse_service();

    svc.delete_zone(&service_ctx, &mut conn, path.zone_id).await?;

    Ok((StatusCode::OK, [("HX-Trigger", "zoneChanged")], Html(String::new())))
}

#[require_permission("WAREHOUSE", "read")]
pub async fn get_zone_bins(
    path: WarehouseZoneBinsPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.warehouse_service();

    let bins = svc.list_bins(&service_ctx, &mut conn, path.zone_id, None, 1, 50).await?;

    Ok(Html(bins_table_fragment(&bins.items).into_string()))
}

// ── Helpers ──

fn warehouse_type_label(t: &WarehouseType) -> &'static str {
    match t {
        WarehouseType::RawMaterial => "原材料仓",
        WarehouseType::FinishedGoods => "成品仓",
        WarehouseType::SemiFinished => "半成品仓",
        WarehouseType::Consumable => "辅料仓",
        WarehouseType::VirtualOutsource => "虚拟仓",
    }
}

fn warehouse_status_label(s: &WarehouseStatus) -> &'static str {
    match s {
        WarehouseStatus::Active => "启用",
        WarehouseStatus::Inactive => "停用",
    }
}

fn warehouse_status_class(s: &WarehouseStatus) -> &'static str {
    match s {
        WarehouseStatus::Active => "status-accepted",
        WarehouseStatus::Inactive => "status-rejected",
    }
}

fn zone_type_label(t: &ZoneType) -> &'static str {
    match t {
        ZoneType::Receiving => "收货区",
        ZoneType::Storage => "存储区",
        ZoneType::Picking => "拣货区",
        ZoneType::Packing => "包装区",
        ZoneType::Inspection => "待检区",
        ZoneType::Returns => "退货区",
    }
}


fn bin_status_label(s: &BinStatus) -> &'static str {
    match s {
        BinStatus::Empty => "空闲",
        BinStatus::Occupied => "占用",
        BinStatus::Locked => "锁定",
        BinStatus::Disabled => "停用",
    }
}

fn bin_status_class(s: &BinStatus) -> &'static str {
    match s {
        BinStatus::Empty => "status-progress",
        BinStatus::Occupied => "status-accepted",
        BinStatus::Locked => "status-rejected",
        BinStatus::Disabled => "status-draft",
    }
}

// ── Components ──

fn warehouse_detail_page(
    warehouse: &Warehouse,
    zones: &[Zone],
    stats: Option<&WarehouseInventoryStats>,
    can_delete: bool,
    can_edit: bool,
) -> Markup {
    let detail_path = WarehouseDetailPath { id: warehouse.id };
    let edit_path = WarehouseEditPath { id: warehouse.id };
    let delete_path = WarehouseDeletePath { id: warehouse.id };

    let status_label = warehouse_status_label(&warehouse.status);
    let status_class = warehouse_status_class(&warehouse.status);
    let type_label = warehouse_type_label(&warehouse.warehouse_type);

    html! {
        div {
        // ── Detail Header ──
        div class="detail-header" style="display:flex;align-items:flex-start;justify-content:space-between;margin-bottom:var(--space-5)" {
            div {
                div style="display:flex;align-items:center;gap:var(--space-3)" {
                    h1 class="detail-no" style="font-size:var(--text-xl);font-weight:700;margin:0;font-family:var(--font-mono)" { (warehouse.code) }
                    span class=(format!("status-pill {status_class}")) { (status_label) }
                    @if warehouse.is_virtual {
                        span class="status-pill" style="background:rgba(114,46,209,0.08);color:#722ed1;font-size:11px;padding:2px 8px" { "虚拟仓" }
                    }
                }
                div style="margin-top:var(--space-2);font-size:13px;color:var(--muted)" { (warehouse.name) }
            }
            div class="flex gap-3" {
                a class="btn btn-default" href=(format!("{}?restore=true", WarehouseListPath::PATH)) {
                    (icon::arrow_left_icon("w-4 h-4"))
                    " 返回列表"
                }
                @if can_edit {
                    a class="btn btn-primary" href=(edit_path) {
                        (icon::edit_icon("w-4 h-4"))
                        " 编辑"
                    }
                }
                @if can_delete {
                    button type="button" class="btn btn-danger" style="margin-left:var(--space-2)"
                        hx-post=(delete_path)
                        hx-confirm=(format!("删除后无法恢复，确定要删除仓库 <strong>{}</strong> 吗？", warehouse.name))
                        hx-target="body"
                        hx-swap="none" {
                        (icon::trash_icon("w-4 h-4"))
                        " 删除"
                    }
                }
            }
        }

        // ── Info Card ──
        div class="info-card" {
            div class="info-card-title" { "仓库信息" }
            div class="info-grid" {
                div class="info-item" {
                    span class="info-label" { "仓库编码" }
                    span class="info-value mono" { (warehouse.code) }
                }
                div class="info-item" {
                    span class="info-label" { "仓库名称" }
                    span class="info-value" { (warehouse.name) }
                }
                div class="info-item" {
                    span class="info-label" { "仓库类型" }
                    span class="info-value" { (type_label) }
                }
                div class="info-item" {
                    span class="info-label" { "状态" }
                    span class="info-value" {
                        span class=(format!("status-pill {status_class}")) { (status_label) }
                    }
                }
                div class="info-item" {
                    span class="info-label" { "地址" }
                    span class="info-value" {
                        @if warehouse.is_virtual {
                            "—"
                        } @else if let Some(ref addr) = warehouse.address {
                            (addr)
                        } @else {
                            "—"
                        }
                    }
                }
                div class="info-item" {
                    span class="info-label" { "管理员" }
                    span class="info-value" { "—" }
                }
                div class="info-item" {
                    span class="info-label" { "创建时间" }
                    span class="info-value mono" { (warehouse.created_at.format("%Y-%m-%d")) }
                }
            }
        }

        // ── Zones Table ──
        div class="sub-section" style="background:var(--bg);border:1px solid var(--border-soft);border-radius:var(--radius-md);padding:var(--space-6);margin-bottom:var(--space-6)" {
            div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:var(--space-4);padding-bottom:var(--space-3);border-bottom:1px solid var(--border-soft)" {
                div style="font-size:var(--text-base);font-weight:600;color:var(--fg)" {
                    "库区列表 "
                    span style="font-weight:400;font-size:12px;color:var(--muted);margin-left:var(--space-2)" {
                        "共 " (zones.len()) " 个库区"
                    }
                }
                button type="button" class="btn btn-primary" style="font-size:12px;padding:4px 12px" _="on click add .is-open to #zone-create-modal" {
                    (icon::plus_icon("w-3.5 h-3.5"))
                    "新建库区"
                }
            }
            div id="zones-table-container" hx-trigger="zoneChanged from:body" hx-get=(format!("{}/zones", detail_path)) hx-target="#zones-table-container" hx-swap="innerHTML" {
                (zones_table_fragment(zones, warehouse.id))
            }
        }

        // ── Zone Bins Table (placeholder, populated on zone click) ──
        div id="bins-section" class="sub-section" style="background:var(--bg);border:1px solid var(--border-soft);border-radius:var(--radius-md);padding:var(--space-6);margin-bottom:var(--space-6)" {
            div class="sub-section-title" style="font-size:var(--text-base);font-weight:600;color:var(--fg);margin-bottom:var(--space-4);padding-bottom:var(--space-3);border-bottom:1px solid var(--border-soft)" {
                "储位明细 "
                span style="font-weight:400;font-size:12px;color:var(--muted);margin-left:var(--space-2)" {
                    "请点击库区查看储位"
                }
            }
            div id="bins-table-container" {
                div style="text-align:center;padding:var(--space-8);color:var(--muted)" { "选择库区后显示储位列表" }
            }
        }

        // ── Stats ──
        (stats_section(stats))

        // ── Zone Create Modal ──
        (crate::components::modal::modal(
            "zone-create-modal",
            "新建库区",
            "保存",
            "create-zone-form",
            &WarehouseZoneCreatePath { id: warehouse.id }.to_string(),
            html! {
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label { "库区编码 " span style="color:var(--danger)" { "*" } }
                        input type="text" name="code" required placeholder="如 A-07";
                    }
                    div class="form-field" {
                        label { "库区名称 " span style="color:var(--danger)" { "*" } }
                        input type="text" name="name" required placeholder="请输入库区名称";
                    }
                    div class="form-field" {
                        label { "库区类型 " span style="color:var(--danger)" { "*" } }
                        select name="zone_type" required {
                            option value="" disabled selected { "-- 请选择 --" }
                            option value="1" { "收货区" }
                            option value="2" { "存储区" }
                            option value="3" { "拣货区" }
                            option value="4" { "包装区" }
                            option value="5" { "待检区" }
                            option value="6" { "退货区" }
                        }
                    }
                    div class="form-field" {
                        label { "排序" }
                        input type="number" name="sort_order" placeholder="排序号";
                    }
                    div class="form-field field-full" {
                        label { "备注" }
                        textarea name="remark" placeholder="库区备注信息…"
                            style="width:100%;min-height:60px;resize:vertical" {}
                    }
                }
            },
        ))

        // ── Zone Edit Modal ──
        div id="zone-edit-modal" class="modal-overlay" { }
        (maud::PreEscaped(r#"<script>
var zem = document.querySelector('#zone-edit-modal');
zem.addEventListener('htmx:afterSettle', function(ev){ if(ev.detail.xhr.responseText.length > 0) zem.classList.add('is-open'); });
zem.addEventListener('click', function(ev){ if(ev.target === zem) zem.classList.remove('is-open'); });
document.body.addEventListener('zoneChanged', function(){ zem.classList.remove('is-open'); });
</script>"#))
        }
    }
}

fn zone_edit_form_fragment(zone: &Zone) -> Markup {
    let put_path = WarehouseZonePath { zone_id: zone.id };
    let type_val: i16 = match zone.zone_type {
        ZoneType::Receiving => 1,
        ZoneType::Storage => 2,
        ZoneType::Picking => 3,
        ZoneType::Packing => 4,
        ZoneType::Inspection => 5,
        ZoneType::Returns => 6,
    };

    html! {
        form class="modal" hx-put=(put_path) hx-swap="none" {
            div class="modal-head" {
                h2 { "编辑库区" }
                button type="button" style="background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px" _="on click remove .is-open from #zone-edit-modal" {
                    "×"
                }
            }
            div class="modal-body" {
                div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                    div class="form-field" {
                        label { "库区编码" }
                        input type="text" name="code" value=(zone.code) readonly
                            style="background:var(--surface);color:var(--muted)";
                    }
                    div class="form-field" {
                        label { "库区名称 " span style="color:var(--danger)" { "*" } }
                        input type="text" name="name" value=(zone.name) required;
                    }
                    div class="form-field" {
                        label { "库区类型 " span style="color:var(--danger)" { "*" } }
                        select name="zone_type" required {
                            option value="1" selected[type_val == 1] { "收货区" }
                            option value="2" selected[type_val == 2] { "存储区" }
                            option value="3" selected[type_val == 3] { "拣货区" }
                            option value="4" selected[type_val == 4] { "包装区" }
                            option value="5" selected[type_val == 5] { "待检区" }
                            option value="6" selected[type_val == 6] { "退货区" }
                        }
                    }
                    div class="form-field" {
                        label { "排序" }
                        input type="number" name="sort_order" value=(zone.sort_order);
                    }
                    div class="form-field field-full" {
                        label { "备注" }
                        textarea name="remark" style="width:100%;min-height:60px;resize:vertical" {
                            (zone.remark.as_deref().unwrap_or(""))
                        }
                    }
                }
            }
            div class="modal-foot" {
                button type="button" class="btn btn-default" _="on click remove .is-open from #zone-edit-modal" {
                    "取消"
                }
                button type="submit" class="btn btn-primary" { "保存" }
            }
        }
    }
}

fn zones_table_fragment(zones: &[Zone], warehouse_id: i64) -> Markup {
    html! {
        div class="data-card" style="margin-bottom:0" {
            div class="data-card-scroll" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "库区编码" }
                            th { "名称" }
                            th { "类型" }
                            th { "储位数" }
                            th { "排序" }
                            th { "备注" }
                            th { "操作" }
                        }
                    }
                    tbody {
                        @for z in zones {
                            (zone_row(z, warehouse_id))
                        }
                        @if zones.is_empty() {
                            tr {
                                td colspan="7" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无库区数据"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn zone_row(z: &Zone, _warehouse_id: i64) -> Markup {
    let bins_path = WarehouseZoneBinsPath { zone_id: z.id };
    let delete_path = WarehouseZonePath { zone_id: z.id };
    let type_label = zone_type_label(&z.zone_type);

    html! {
        tr {
            td class="mono" { (z.code) }
            td { (z.name) }
            td {
                span class="status-pill" style="background:rgba(22,119,255,0.06);color:#1677ff" { (type_label) }
            }
            td class="num-right" style="color:var(--muted)" { "—" }
            td class="mono" { (z.sort_order) }
            td style="color:var(--muted)" {
                @if let Some(ref r) = z.remark { (r) } @else { "—" }
            }
            td {
                div class="row-actions" {
                    button type="button" class="row-action-btn" title="查看储位"
                        hx-get=(bins_path)
                        hx-target="#bins-table-container"
                        hx-swap="innerHTML" {
                        (icon::eye_icon("w-4 h-4"))
                    }
                    button type="button" class="row-action-btn" title="编辑"
                        hx-get=(WarehouseZonePath { zone_id: z.id })
                        hx-target="#zone-edit-modal"
                        hx-swap="innerHTML" {
                        (icon::edit_icon("w-4 h-4"))
                    }
                    button type="button" class="row-action-btn" title="删除" style="color:var(--danger)"
                        hx-delete=(delete_path)
                        hx-confirm="确定要删除该库区吗？删除后不可恢复。"
                        hx-target="closest tr"
                        hx-swap="outerHTML swap:0.5s" {
                        (icon::trash_icon("w-4 h-4"))
                    }
                }
            }
        }
    }
}

fn bins_table_fragment(bins: &[Bin]) -> Markup {
    html! {
        div class="data-card" style="margin-bottom:0" {
            div class="data-card-scroll" {
                table class="data-table" {
                    thead {
                        tr {
                            th { "储位编码" }
                            th { "名称" }
                            th { "行/列/层" }
                            th { "容量上限" }
                            th { "状态" }
                            th { "温控要求" }
                        }
                    }
                    tbody {
                        @for b in bins {
                            (bin_row(b))
                        }
                        @if bins.is_empty() {
                            tr {
                                td colspan="6" style="text-align:center;padding:var(--space-8);color:var(--muted)" {
                                    "暂无储位数据"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn bin_row(b: &Bin) -> Markup {
    let status_label = bin_status_label(&b.status);
    let status_class = bin_status_class(&b.status);

    let row_col = format!(
        "{} / {} / {}",
        b.row_no.as_deref().unwrap_or("—"),
        b.column_no.as_deref().unwrap_or("—"),
        b.layer_no.as_deref().unwrap_or("—")
    );

    html! {
        tr {
            td class="mono" { (b.code) }
            td { (b.name) }
            td class="mono" { (row_col) }
            td class="num-right" {
                @if let Some(cap) = b.capacity_limit {
                    (format!("{:.2}", cap))
                } @else {
                    "—"
                }
            }
            td {
                span class=(format!("status-pill {status_class}")) { (status_label) }
            }
            td {
                @if let Some(ref req) = b.temperature_req {
                    (req)
                } @else {
                    span style="color:var(--muted)" { "—" }
                }
            }
        }
    }
}

fn stats_section(stats: Option<&WarehouseInventoryStats>) -> Markup {
    let (total_qty, product_count, low_stock, safety_warning) = match stats {
        Some(s) => (s.total_quantity.to_string(), s.product_count.to_string(), s.low_stock_count.to_string(), "0".to_string()),
        None => ("—".to_string(), "—".to_string(), "—".to_string(), "—".to_string()),
    };

    html! {
        div class="sub-section" style="background:var(--bg);border:1px solid var(--border-soft);border-radius:var(--radius-md);padding:var(--space-6);margin-bottom:var(--space-6)" {
            div class="sub-section-title" style="font-size:var(--text-base);font-weight:600;color:var(--fg);margin-bottom:var(--space-4);padding-bottom:var(--space-3);border-bottom:1px solid var(--border-soft)" {
                "库存统计"
            }
            div style="display:grid;grid-template-columns:repeat(4,1fr);gap:var(--space-5)" {
                div style="background:var(--surface-raised);border:1px solid var(--border-soft);border-radius:var(--radius-md);padding:var(--space-5);text-align:center" {
                    div style="font-size:var(--text-2xl);font-weight:700;color:var(--accent);letter-spacing:-0.02em;line-height:1.1" { (total_qty) }
                    div style="font-size:12px;color:var(--muted);margin-top:var(--space-2);font-weight:500" { "总库存量" }
                }
                div style="background:var(--surface-raised);border:1px solid var(--border-soft);border-radius:var(--radius-md);padding:var(--space-5);text-align:center" {
                    div style="font-size:var(--text-2xl);font-weight:700;color:var(--success);letter-spacing:-0.02em;line-height:1.1" { (product_count) }
                    div style="font-size:12px;color:var(--muted);margin-top:var(--space-2);font-weight:500" { "品种数" }
                }
                div style="background:var(--surface-raised);border:1px solid var(--border-soft);border-radius:var(--radius-md);padding:var(--space-5);text-align:center" {
                    div style="font-size:var(--text-2xl);font-weight:700;color:var(--warn);letter-spacing:-0.02em;line-height:1.1" { (low_stock) }
                    div style="font-size:12px;color:var(--muted);margin-top:var(--space-2);font-weight:500" { "低库存项" }
                }
                div style="background:var(--surface-raised);border:1px solid var(--border-soft);border-radius:var(--radius-md);padding:var(--space-5);text-align:center" {
                    div style="font-size:var(--text-2xl);font-weight:700;color:var(--danger);letter-spacing:-0.02em;line-height:1.1" { (safety_warning) }
                    div style="font-size:12px;color:var(--muted);margin-top:var(--space-2);font-weight:500" { "安全库存预警" }
                }
            }
        }
    }
}
