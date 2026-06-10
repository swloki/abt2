use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::product::ProductService;
use abt_core::master_data::product::model::ProductQuery;
use abt_core::master_data::supplier::SupplierService;
use abt_core::master_data::supplier::model::SupplierQuery;
use abt_core::om::enums::OutsourcingType;
use abt_core::om::outsourcing_order::{CreateOutsourcingOrderReq, OutsourcingMaterialItem, OutsourcingOrderService};
use abt_core::shared::types::PageParams;
use abt_core::wms::warehouse::WarehouseService;
use abt_core::wms::warehouse::model::WarehouseFilter;
use abt_core::mes::work_order::WorkOrderService;
use abt_core::mes::work_order::model::WorkOrderFilter;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::om::{OmOutsourcingCreatePath, OmOutsourcingDetailPath, OmOutsourcingListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form structs ──

#[derive(Debug, Deserialize)]
pub struct CreateForm {
    pub supplier_id: i64,
    pub product_id: i64,
    pub outsourcing_type: i16,
    pub work_order_id: Option<i64>,
    pub routing_id: Option<i64>,
    pub planned_qty: String,
    pub unit_price: String,
    pub scheduled_date: Option<String>,
    pub virtual_warehouse_id: i64,
    pub remark: Option<String>,
    pub materials_json: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MaterialItemWeb {
    product_id: i64,
    planned_qty: String,
    unit_cost: Option<String>,
}

// ── Handlers ──

#[require_permission("OUTSOURCING", "create")]
pub async fn get_create(
    _path: OmOutsourcingCreatePath,
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

    let supplier_svc = state.supplier_service();
    let product_svc = state.product_service();
    let warehouse_svc = state.warehouse_service();
    let wo_svc = state.work_order_service();

    let suppliers = supplier_svc
        .list(
            &service_ctx,
            &mut conn,
            SupplierQuery {
                name: None,
                status: None,
                category: None,
            },
            PageParams::new(1, 200),
        )
        .await?;

    let products = product_svc
        .list(
            &service_ctx,
            &mut conn,
            ProductQuery {
                name: None,
                code: None,
                status: None,
                owner_department_id: None,
                category_id: None,
            },
            PageParams::new(1, 200),
        )
        .await?;

    let warehouses = warehouse_svc
        .list(
            &service_ctx,
            &mut conn,
            WarehouseFilter {
                warehouse_type: None,
                status: None,
                keyword: None,
            },
            1,
            200,
        )
        .await?;

    let work_orders = wo_svc
        .list(
            &service_ctx,
            &mut conn,
            WorkOrderFilter {
                status: None,
                product_id: None,
                keyword: None,
                date_from: None,
                date_to: None,
            },
            1,
            200,
        )
        .await?;

    let content = create_page(
        &suppliers.items,
        &products.items,
        &warehouses.items,
        &work_orders.items,
    );

    let page_html = admin_page(
        is_htmx,
        "新建委外单",
        &claims,
        "outsourcing",
        OmOutsourcingCreatePath::PATH,
        "委外管理",
        Some(OmOutsourcingListPath::PATH),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("OUTSOURCING", "create")]
pub async fn create(
    _path: OmOutsourcingCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<CreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.outsourcing_order_service();

    let outsourcing_type = OutsourcingType::from_i16(form.outsourcing_type)
        .ok_or_else(|| abt_core::shared::types::DomainError::validation("无效委外类型"))?;

    let scheduled_date = form
        .scheduled_date
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|e| abt_core::shared::types::DomainError::validation(format!("无效日期格式: {e}")))
        })
        .transpose()?;

    let materials: Vec<OutsourcingMaterialItem> = form
        .materials_json
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|json| {
            let web_items: Vec<MaterialItemWeb> = serde_json::from_str(json)
                .map_err(|e| abt_core::shared::types::DomainError::validation(format!("无效物料数据: {e}")))?;
            Ok::<Vec<OutsourcingMaterialItem>, abt_core::shared::types::DomainError>(web_items
                .into_iter()
                .map(|item| OutsourcingMaterialItem {
                    product_id: item.product_id,
                    planned_qty: item
                        .planned_qty
                        .parse()
                        .unwrap_or(rust_decimal::Decimal::ZERO),
                    unit_cost: item
                        .unit_cost
                        .and_then(|s| s.parse().ok()),
                })
                .collect())
        })
        .transpose()?
        .unwrap_or_default();

    let req = CreateOutsourcingOrderReq {
        work_order_id: form.work_order_id,
        routing_id: form.routing_id,
        supplier_id: form.supplier_id,
        product_id: form.product_id,
        outsourcing_type,
        planned_qty: form
            .planned_qty
            .parse()
            .map_err(|_| abt_core::shared::types::DomainError::validation("无效计划数量"))?,
        unit_price: form
            .unit_price
            .parse()
            .map_err(|_| abt_core::shared::types::DomainError::validation("无效单价"))?,
        scheduled_date,
        virtual_warehouse_id: form.virtual_warehouse_id,
        remark: form.remark,
        materials,
    };

    let id = svc.create(&service_ctx, &mut conn, req, None).await?;

    let redirect = OmOutsourcingDetailPath { id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Page Components ──

fn create_page(
    suppliers: &[abt_core::master_data::supplier::model::Supplier],
    products: &[abt_core::master_data::product::model::Product],
    warehouses: &[abt_core::wms::warehouse::model::Warehouse],
    work_orders: &[abt_core::mes::work_order::model::WorkOrder],
) -> Markup {
    html! {
        div {
            // ── Page Header ──
            div class="page-header" {
                div class="page-header-left" {
                    a class="back-link" href=(OmOutsourcingListPath::PATH) {
                        "\u{2190} 返回列表"
                    }
                    h1 class="page-title" { "新建委外单" }
                }
            }

            form
                id="om-create-form"
                hx-post=(OmOutsourcingCreatePath::PATH)
                hx-swap="none"
            {
                // ── Section 1: 基本信息 ──
                div class="form-section" {
                    div class="form-section-title" { "基本信息" }
                    div class="form-grid" {
                        div class="form-field" {
                            label class="form-label" { "委外单号" }
                            input class="form-input" type="text" value="自动生成" readonly;
                        }
                        div class="form-field" {
                            label class="form-label" { "供应商" }
                            select class="form-select" name="supplier_id" required {
                                option value="" { "请选择供应商" }
                                @for s in suppliers {
                                    option value=(s.id) { (s.name) }
                                }
                            }
                        }
                        div class="form-field" {
                            label class="form-label" { "产品" }
                            select class="form-select" name="product_id" required {
                                option value="" { "请选择产品" }
                                @for p in products {
                                    option value=(p.product_id) {
                                        (p.pdt_name)
                                        " ("
                                        (p.product_code)
                                        ")"
                                    }
                                }
                            }
                        }
                        div class="form-field" {
                            label class="form-label" { "委外类型" }
                            select class="form-select" name="outsourcing_type" required {
                                option value="" { "请选择委外类型" }
                                option value="1" { "整体委外" }
                                option value="2" { "工序委外" }
                                option value="3" { "材料委外" }
                                option value="4" { "返工委外" }
                            }
                        }
                    }
                }

                // ── Section 2: 关联信息与数量 ──
                div class="form-section" {
                    div class="form-section-title" { "关联信息与数量" }
                    div class="form-grid" {
                        div class="form-field" {
                            label class="form-label" { "关联工单" }
                            select class="form-select" name="work_order_id" {
                                option value="" { "请选择工单" }
                                @for wo in work_orders {
                                    option value=(wo.id) {
                                        (wo.doc_number)
                                    }
                                }
                            }
                        }
                        div class="form-field" {
                            label class="form-label" { "关联工序" }
                            input class="form-input" type="number" name="routing_id" placeholder="请输入工序ID";
                        }
                        div class="form-field" {
                            label class="form-label" { "计划数量" }
                            input class="form-input" type="number" step="0.01" min="0" name="planned_qty" required;
                        }
                        div class="form-field" {
                            label class="form-label" { "单价" }
                            input class="form-input" type="number" step="0.01" min="0" name="unit_price" required;
                        }
                        div class="form-field" {
                            label class="form-label" { "预计交期" }
                            input class="form-input" type="date" name="scheduled_date";
                        }
                        div class="form-field" {
                            label class="form-label" { "虚拟仓库" }
                            select class="form-select" name="virtual_warehouse_id" required {
                                option value="" { "请选择仓库" }
                                @for w in warehouses {
                                    @if w.is_virtual {
                                        option value=(w.id) { (w.name) }
                                    }
                                }
                            }
                        }
                    }
                }

                // ── Section 3: 发料明细 ──
                div class="form-section" {
                    div class="form-section-title" { "发料明细" }
                    div class="data-card" {
                        div class="data-card-scroll" {
                            table class="data-table" {
                                thead {
                                    tr {
                                        th { "物料" }
                                        th { "应发数量" }
                                        th { "单位成本" }
                                        th { "小计" }
                                        th style="width:50px" { }
                                    }
                                }
                                tbody id="material-tbody" {
                                    // rows added dynamically
                                }
                            }
                        }
                        // Hidden input to carry materials JSON
                        input type="hidden" name="materials_json" id="materials-json" value="";
                    }
                    // Add row button
                    div class="add-row-bar" {
                        button type="button" class="btn-add-row"
                            onclick="omAddMaterialRow()"
                        {
                            (icon::plus_icon("w-4 h-4"))
                            " 添加物料"
                        }
                    }
                }

                // ── Section 4: 备注 ──
                div class="form-section" {
                    div class="form-section-title" { "备注" }
                    div class="form-field span-2" {
                        textarea class="form-input" name="remark" rows="3" placeholder="请输入备注信息" {};
                    }
                }

                // ── Action bar ──
                div class="create-action-bar" {
                    a class="btn btn-default" href=(OmOutsourcingListPath::PATH) { "取消" }
                    button type="submit" class="btn btn-primary" { "确认提交" }
                }
            }
        }

        // ── Material row modal ──
        div id="material-modal" class="modal-overlay" onclick="hsBackdropClose(this,event,'is-open')" {
            div class="modal" onclick="event.stopPropagation()" {
                div class="modal-head" {
                    h3 { "选择物料" }
                    button type="button" class="btn-remove-row" title="关闭"
                        onclick="hsRemove(null,'#material-modal','is-open')"
                    {
                        (icon::x_icon("w-4 h-4"))
                    }
                }
                div class="modal-body" {
                    div class="form-grid" {
                        div class="form-field span-2" {
                            label class="form-label" { "物料" }
                            select class="form-select" id="modal-product-id" {
                                option value="" { "请选择物料" }
                                @for p in products {
                                    option value=(p.product_id) {
                                        (p.pdt_name)
                                        " ("
                                        (p.product_code)
                                        ")"
                                    }
                                }
                            }
                        }
                        div class="form-field" {
                            label class="form-label" { "应发数量" }
                            input class="form-input" type="number" step="0.01" min="0" id="modal-planned-qty" required;
                        }
                        div class="form-field" {
                            label class="form-label" { "单位成本" }
                            input class="form-input" type="number" step="0.01" min="0" id="modal-unit-cost";
                        }
                    }
                }
                div class="modal-foot" {
                    button type="button" class="btn btn-default"
                        onclick="hsRemove(null,'#material-modal','is-open')"
                    { "取消" }
                    button type="button" class="btn btn-primary"
                        onclick="omConfirmMaterial()"
                    { "确认" }
                }
            }
        }

        // ── Inline scripts ──
        script {
            (maud::PreEscaped(r#"
function omAddMaterialRow() {
    me('#modal-product-id').value = '';
    me('#modal-planned-qty').value = '';
    me('#modal-unit-cost').value = '';
    hsToggle(null,'#material-modal','is-open');
}

function omConfirmMaterial() {
    var pid = me('#modal-product-id').value;
    var sel = me('#modal-product-id');
    var pname = sel.options[sel.selectedIndex] ? sel.options[sel.selectedIndex].textContent.trim() : '';
    var qty = parseFloat(me('#modal-planned-qty').value) || 0;
    var cost = parseFloat(me('#modal-unit-cost').value) || 0;
    if (!pid || qty <= 0) return;

    var tbody = me('#material-tbody');
    var tr = document.createElement('tr');
    tr.setAttribute('oninput','omUpdateMaterialJson()');
    tr.innerHTML = '<td>' + pname + '<input type="hidden" name="m_product_id" value="' + pid + '"></td>' +
        '<td><input class="form-input" type="number" step="0.01" min="0" name="m_planned_qty" value="' + qty + '" style="width:100px;text-align:right"></td>' +
        '<td><input class="form-input" type="number" step="0.01" min="0" name="m_unit_cost" value="' + cost + '" style="width:100px;text-align:right"></td>' +
        '<td class="line-subtotal mono" style="text-align:right">' + (qty * cost).toFixed(2) + '</td>' +
        '<td><button type="button" class="btn-remove-row" title="删除" onclick="this.closest(\'tr\').remove();omUpdateMaterialJson()">' + '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg></button></td>';
    tbody.appendChild(tr);
    omUpdateMaterialJson();
    hsRemove(null, '#material-modal', 'is-open');
}

function omUpdateMaterialJson() {
    var rows = any('#material-tbody tr');
    var items = [];
    rows.forEach(function(tr) {
        var pid = tr.querySelector('[name=m_product_id]');
        var qty = tr.querySelector('[name=m_planned_qty]');
        var cost = tr.querySelector('[name=m_unit_cost]');
        if (pid && qty) {
            var q = parseFloat(qty.value) || 0;
            var c = cost ? (parseFloat(cost.value) || 0) : 0;
            tr.querySelector('.line-subtotal').textContent = (q * c).toFixed(2);
            items.push({
                product_id: parseInt(pid.value),
                planned_qty: qty.value,
                unit_cost: cost && cost.value ? cost.value : null
            });
        }
    });
    me('#materials-json').value = JSON.stringify(items);
}
            "#))
        }
    }
}
