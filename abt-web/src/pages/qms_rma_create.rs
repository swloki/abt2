use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::master_data::customer::model::{Customer, CustomerQuery};
use abt_core::master_data::customer::CustomerService;
use abt_core::master_data::product::model::{Product, ProductQuery};
use abt_core::master_data::product::ProductService;
use abt_core::qms::enums::Severity;
use abt_core::qms::rma::model::CreateRmaReq;
use abt_core::qms::rma::RmaService;
use abt_core::sales::sales_order::model::SalesOrderQuery;
use abt_core::sales::sales_order::SalesOrderService;
use abt_core::sales::shipping_request::model::ShippingQuery;
use abt_core::sales::shipping_request::ShippingRequestService;
use abt_core::shared::types::PageParams;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::qms::{RmaCreatePath, RmaListPath};
use crate::utils::RequestContext;
use abt_macros::require_permission;

// ── Form request ──

#[derive(Debug, Deserialize)]
pub struct RmaCreateForm {
    pub customer_id: i64,
    pub sales_order_id: String,
    pub shipping_request_id: String,
    pub product_id: i64,
    pub linked_inspection_result_id: String,
    pub defect_description: String,
    pub severity: i16,
    pub remark: String,
}

// ── Handlers ──

#[require_permission("QMS", "write")]
pub async fn get_create(
    _path: RmaCreatePath,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        claims,
        ..
    } = ctx;

    let customer_svc = state.customer_service();
    let customers = customer_svc
        .list(
            &service_ctx,
            &mut conn,
            CustomerQuery::default(),
            PageParams::new(1, 500),
        )
        .await?;

    let product_svc = state.product_service();
    let products = product_svc
        .list(
            &service_ctx,
            &mut conn,
            ProductQuery::default(),
            PageParams::new(1, 500),
        )
        .await?;

    let order_svc = state.sales_order_service();
    let sales_orders = order_svc
        .list(&service_ctx, &mut conn, SalesOrderQuery::default(), PageParams::new(1, 200))
        .await
        .map(|p| p.items)
        .unwrap_or_default();

    let shipping_svc = state.shipping_service();
    let shipping_requests = shipping_svc
        .list(&service_ctx, &mut conn, ShippingQuery::default(), PageParams::new(1, 200))
        .await
        .map(|p| p.items)
        .unwrap_or_default();

    let content = rma_create_page(&customers.items, &products.items, &sales_orders, &shipping_requests);
    let page_html = admin_page(
        is_htmx,
        "新建RMA客诉",
        &claims,
        "quality",
        RmaCreatePath::PATH,
        "质量管理",
        Some(RmaListPath::PATH),
        content,
    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("QMS", "write")]
pub async fn create(
    _path: RmaCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<RmaCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;

    let severity = Severity::from_i16(form.severity).ok_or_else(|| {
        abt_core::shared::types::DomainError::Validation("无效严重程度".into())
    })?;

    let sales_order_id = form.sales_order_id.parse::<i64>().ok();
    let shipping_request_id = form.shipping_request_id.parse::<i64>().ok();
    let linked_inspection_result_id = form.linked_inspection_result_id.parse::<i64>().ok();

    let req = CreateRmaReq {
        customer_id: form.customer_id,
        sales_order_id,
        shipping_request_id,
        product_id: form.product_id,
        linked_inspection_result_id,
        defect_description: form.defect_description,
        severity,
        remark: form.remark,
    };

    let svc = state.rma_service();
    let _id = svc.create(&service_ctx, &mut conn, req).await?;

    Ok(
        axum::response::Response::builder()
            .header("HX-Redirect", RmaListPath::PATH)
            .body(axum::body::Body::empty())
            .unwrap(),
    )
}

// ── Page rendering ──

fn rma_create_page(customers: &[Customer], products: &[Product], sales_orders: &[abt_core::sales::sales_order::model::SalesOrder], shipping_requests: &[abt_core::sales::shipping_request::model::ShippingRequest]) -> Markup {
    html! {
        div {
            // ── Page header ──
            div class="page-header" {
                div class="page-header-left" {
                    a class="back-link" href=(RmaListPath::PATH) {
                        (icon::arrow_left_icon("w-4 h-4"))
                        "返回列表"
                    }
                    h1 class="page-title" { "新建RMA客诉" }
                }
            }

            form id="rma-form" hx-post=(RmaCreatePath::PATH) hx-swap="none" {

                // ── Section 1: 客户信息 ──
                div class="form-section" {
                    div class="form-section-title" {
                        (icon::users_icon("w-4 h-4"))
                        "客户信息"
                    }
                    div class="form-grid" style="grid-template-columns:repeat(2,1fr)" {
                        div class="form-field" {
                            label class="form-label required" { "客户" }
                            select class="form-select" name="customer_id" required {
                                option value="" disabled selected { "请选择客户" }
                                @for c in customers {
                                    option value=(c.id) { (c.code) " — " (c.name) }
                                }
                            }
                        }
                        div class="form-field" {
                            label class="form-label" { "关联销售订单" }
                            select class="form-select" name="sales_order_id" {
                                option value="" selected { "请选择销售订单（可选）" }
                                @for order in sales_orders {
                                    option value=(order.id) {
                                        (order.doc_number)
                                        " - 客户ID:" (order.customer_id)
                                    }
                                }
                            }
                        }
                        div class="form-field" style="grid-column:1/-1" {
                            label class="form-label" { "关联发货单" }
                            select class="form-select" name="shipping_request_id" {
                                option value="" selected { "请选择发货单（可选）" }
                                @for ship in shipping_requests {
                                    option value=(ship.id) {
                                        (ship.doc_number)
                                    }
                                }
                            }
                        }
                    }
                }

                // ── Section 2: 产品信息 ──
                div class="form-section" {
                    div class="form-section-title" {
                        (icon::box_icon("w-4 h-4"))
                        "产品信息"
                    }
                    div class="form-grid" style="grid-template-columns:repeat(2,1fr)" {
                        div class="form-field" {
                            label class="form-label required" { "产品" }
                            select class="form-select" name="product_id" required {
                                option value="" disabled selected { "请选择产品" }
                                @for p in products {
                                    option value=(p.product_id) { (p.product_code) " — " (p.pdt_name) }
                                }
                            }
                        }
                        div class="form-field" {
                            label class="form-label" { "关联检验结果" }
                            select class="form-select" name="linked_inspection_result_id" {
                                option value="" selected { "请选择检验结果（可选）" }
                            }
                            span class="form-hint" { "可选，关联相关来料/过程检验记录" }
                        }
                    }
                }

                // ── Section 3: 缺陷描述 ──
                div class="form-section" {
                    div class="form-section-title" {
                        (icon::alert_triangle_icon("w-4 h-4"))
                        "缺陷描述"
                    }
                    div class="form-grid" style="grid-template-columns:repeat(2,1fr)" {
                        div class="form-field" style="grid-column:1/-1" {
                            label class="form-label required" { "缺陷描述" }
                            textarea class="form-textarea" name="defect_description" rows="3" required placeholder="请描述缺陷详情…" {}
                        }
                        div class="form-field" {
                            label class="form-label required" { "严重程度" }
                            select class="form-select" name="severity" required {
                                option value="" disabled selected { "请选择严重程度" }
                                option value="1" { "轻微 Minor" }
                                option value="2" { "一般 Major" }
                                option value="3" { "严重 Critical" }
                            }
                        }
                    }
                }

                // ── Section 4: 备注 ──
                div class="form-section" {
                    div class="form-section-title" {
                        (icon::edit_icon("w-4 h-4"))
                        "备注"
                    }
                    div class="form-field" {
                        textarea class="form-textarea" name="remark" rows="3" placeholder="填写备注信息…" style="min-height:72px" {}
                    }
                }

                // ── Action bar ──
                div class="create-action-bar" {
                    a class="btn btn-default" href=(RmaListPath::PATH) { "取消" }
                    button type="submit" class="btn btn-default" name="action" value="save" {
                        (icon::check_circle_icon("w-4 h-4"))
                        "保存"
                    }
                    button type="submit" class="btn btn-primary" name="action" value="submit" {
                        "提交"
                    }
                }
            }
        }
    }
}
