//! SalesOrder gRPC Handler — 委托给 abt-core SalesOrderService

use abt_core::sales::sales_order::SalesOrderService;
use abt_core::shared::types::{PageParams, ServiceContext};
use crate::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    sales_order_service_server::SalesOrderService as GrpcSalesOrderService,
    *,
};
use crate::handlers::{domain_to_status, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;

pub struct SalesOrderHandler;

impl SalesOrderHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SalesOrderHandler {
    fn default() -> Self {
        Self::new()
    }
}

fn sales_order_status_to_proto(status: abt_core::sales::sales_order::SalesOrderStatus) -> SalesOrderStatus {
    match status {
        abt_core::sales::sales_order::SalesOrderStatus::Draft => SalesOrderStatus::Draft,
        abt_core::sales::sales_order::SalesOrderStatus::Confirmed => SalesOrderStatus::Confirmed,
        abt_core::sales::sales_order::SalesOrderStatus::InProduction => SalesOrderStatus::InProgress,
        abt_core::sales::sales_order::SalesOrderStatus::PartiallyShipped => SalesOrderStatus::InProgress,
        abt_core::sales::sales_order::SalesOrderStatus::Shipped => SalesOrderStatus::Completed,
        abt_core::sales::sales_order::SalesOrderStatus::Completed => SalesOrderStatus::Completed,
        abt_core::sales::sales_order::SalesOrderStatus::Cancelled => SalesOrderStatus::Cancelled,
    }
}

#[allow(dead_code)]
fn parse_decimal(s: &str) -> rust_decimal::Decimal {
    s.parse().unwrap_or(rust_decimal::Decimal::ZERO)
}

#[allow(dead_code)]
fn decimal_to_string(d: rust_decimal::Decimal) -> String {
    d.to_string()
}

#[allow(dead_code)]
fn order_item_to_proto(item: &abt_core::sales::sales_order::SalesOrderItem) -> SalesOrderItem {
    SalesOrderItem {
        item_id: item.id,
        order_id: item.order_id,
        product_id: item.product_id,
        product_code: String::new(),
        product_name: item.description.clone(),
        unit: item.unit.clone(),
        unit_price: decimal_to_string(item.unit_price),
        quantity: decimal_to_string(item.quantity),
        discount: decimal_to_string(item.discount_rate),
        subtotal: decimal_to_string(item.amount),
        shipped_qty: decimal_to_string(item.shipped_qty),
        returned_qty: decimal_to_string(item.returned_qty),
        remark: String::new(),
    }
}

fn order_to_proto(o: &abt_core::sales::sales_order::SalesOrder) -> SalesOrder {
    SalesOrder {
        order_id: o.id,
        order_no: o.doc_number.clone(),
        quotation_id: 0,
        customer_name: String::new(),
        contact_person: String::new(),
        contact_phone: String::new(),
        status: sales_order_status_to_proto(o.status) as i32,
        total_amount: decimal_to_string(o.total_amount),
        remark: o.remark.clone(),
        delivery_date: o.order_date
            .and_time(chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap())
            .and_utc().timestamp(),
        created_at: o.created_at.timestamp(),
        updated_at: o.updated_at.timestamp(),
        operator_id: o.operator_id,
        items: vec![],
    }
}

#[tonic::async_trait]
impl GrpcSalesOrderService for SalesOrderHandler {
    async fn create_sales_order(
        &self,
        request: Request<CreateSalesOrderRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_order_core_service();

        // 如果有 quotation_id，使用 create_from_quotation
        if req.quotation_id > 0 {
            let mut tx = state.begin_core_transaction().await
                .map_err(error::err_to_status)?;
            let ctx = ServiceContext::new(&mut tx, auth.user_id);
            let id = srv.create_from_quotation(ctx, req.quotation_id).await
                .map_err(domain_to_status)?;
            tx.commit().await.map_err(error::sqlx_err_to_status)?;
            return Ok(Response::new(U64Response { value: id as u64 }));
        }

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let items: Vec<abt_core::sales::sales_order::CreateSalesOrderItemReq> = req.items.iter().map(|i| {
            abt_core::sales::sales_order::CreateSalesOrderItemReq {
                product_id: i.product_id,
                description: None,
                quantity: parse_decimal(&i.quantity),
                unit: None,
                unit_price: parse_decimal(&i.unit_price),
                unit_cost: None,
                discount_rate: {
                    let d = parse_decimal(&i.discount);
                    if d == rust_decimal::Decimal::ZERO { None } else { Some(d) }
                },
                delivery_date: None,
            }
        }).collect();

        let create_req = abt_core::sales::sales_order::CreateSalesOrderReq {
            customer_id: 0,
            contact_id: 0,
            items,
            payment_terms: None,
            delivery_terms: None,
            delivery_address: None,
            remark: if req.remark.is_empty() { None } else { Some(req.remark) },
        };

        let id = srv.create(ctx, create_req).await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn update_sales_order(
        &self,
        request: Request<UpdateSalesOrderRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_order_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let update_req = abt_core::sales::sales_order::UpdateSalesOrderReq {
            customer_id: None,
            contact_id: None,
            payment_terms: None,
            delivery_terms: None,
            delivery_address: None,
            remark: if req.remark.is_empty() { None } else { Some(req.remark) },
        };

        srv.update_header(ctx, req.order_id, update_req).await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn delete_sales_order(
        &self,
        request: Request<DeleteSalesOrderRequest>,
    ) -> GrpcResult<BoolResponse> {
        let _auth = extract_auth(&request)?;
        let req = request.into_inner();
        let _ = req;
        // abt-core SalesOrderService 没有 delete 方法
        Err(tonic::Status::unimplemented("Delete sales order not supported in abt-core"))
    }

    async fn get_sales_order(
        &self,
        request: Request<GetSalesOrderRequest>,
    ) -> GrpcResult<SalesOrderResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_order_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let o = srv.find_by_id(ctx, req.order_id).await
            .map_err(domain_to_status)?;

        Ok(Response::new(SalesOrderResponse {
            order: Some(order_to_proto(&o)),
        }))
    }

    async fn list_sales_orders(
        &self,
        request: Request<ListSalesOrdersRequest>,
    ) -> GrpcResult<SalesOrderListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_order_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);

        let filter = abt_core::sales::sales_order::SalesOrderQuery {
            customer_id: None,
            status: req.status.and_then(|s| {
                abt_core::sales::sales_order::SalesOrderStatus::from_i16(s as i16)
            }),
            date_from: None,
            date_to: None,
            keyword: req.keyword,
        };
        let page = PageParams::new(
            req.pagination.as_ref().map(|p| p.page).unwrap_or(1),
            req.pagination.as_ref().map(|p| p.page_size).unwrap_or(20),
        );

        let result = srv.list(ctx, filter, page).await
            .map_err(domain_to_status)?;

        Ok(Response::new(SalesOrderListResponse {
            items: result.items.iter().map(order_to_proto).collect(),
            pagination: Some(PaginationInfo {
                total: result.total,
                page: result.page,
                page_size: result.page_size,
                total_pages: result.total_pages,
            }),
        }))
    }

    async fn update_sales_order_status(
        &self,
        request: Request<UpdateSalesOrderStatusRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_order_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        match req.status() {
            SalesOrderStatus::Confirmed => {
                srv.confirm(ctx, req.order_id).await
                    .map_err(domain_to_status)?;
            }
            SalesOrderStatus::InProgress => {
                srv.start_progress(ctx, req.order_id).await
                    .map_err(domain_to_status)?;
            }
            SalesOrderStatus::Completed => {
                srv.complete(ctx, req.order_id).await
                    .map_err(domain_to_status)?;
            }
            SalesOrderStatus::Cancelled => {
                srv.cancel(ctx, req.order_id).await
                    .map_err(domain_to_status)?;
            }
            _ => {
                return Err(tonic::Status::invalid_argument(
                    format!("Unsupported status transition: {:?}", req.status()),
                ));
            }
        }

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }
}
