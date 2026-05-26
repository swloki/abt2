//! ShippingRequest gRPC Handler — 委托给 abt-core ShippingRequestService

use abt_core::sales::shipping_request::ShippingRequestService;
use abt_core::shared::types::{PageParams, ServiceContext};
use crate::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    shipping_request_service_server::ShippingRequestService as GrpcShippingRequestService,
    *,
};
use crate::handlers::{domain_to_status, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;

pub struct ShippingRequestHandler;

impl ShippingRequestHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ShippingRequestHandler {
    fn default() -> Self {
        Self::new()
    }
}

fn shipping_status_to_proto(status: abt_core::sales::shipping_request::ShippingStatus) -> ShippingRequestStatus {
    match status {
        abt_core::sales::shipping_request::ShippingStatus::Draft => ShippingRequestStatus::Pending,
        abt_core::sales::shipping_request::ShippingStatus::Confirmed => ShippingRequestStatus::Confirmed,
        abt_core::sales::shipping_request::ShippingStatus::Picking => ShippingRequestStatus::Confirmed,
        abt_core::sales::shipping_request::ShippingStatus::Shipped => ShippingRequestStatus::Shipped,
        abt_core::sales::shipping_request::ShippingStatus::Cancelled => ShippingRequestStatus::Cancelled,
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
fn shipping_item_to_proto(item: &abt_core::sales::shipping_request::ShippingRequestItem) -> ShippingRequestItem {
    ShippingRequestItem {
        item_id: item.id,
        request_id: item.shipping_request_id,
        order_item_id: item.order_item_id,
        product_id: item.product_id,
        product_code: String::new(),
        product_name: item.description.clone(),
        unit: String::new(),
        quantity: decimal_to_string(item.requested_qty),
        remark: String::new(),
    }
}

fn shipping_to_proto(r: &abt_core::sales::shipping_request::ShippingRequest) -> ShippingRequest {
    ShippingRequest {
        request_id: r.id,
        request_no: r.doc_number.clone(),
        order_id: r.order_id,
        customer_name: String::new(),
        status: shipping_status_to_proto(r.status) as i32,
        remark: r.remark.clone(),
        operator_id: r.operator_id,
        confirmed_at: 0,
        shipped_at: 0,
        created_at: r.created_at.timestamp(),
        updated_at: r.updated_at.timestamp(),
        items: vec![],
    }
}

#[tonic::async_trait]
impl GrpcShippingRequestService for ShippingRequestHandler {
    async fn create_shipping_request(
        &self,
        request: Request<CreateShippingRequestRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.shipping_request_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let items: Vec<abt_core::sales::shipping_request::CreateShippingItemReq> = req.items.iter().map(|i| {
            abt_core::sales::shipping_request::CreateShippingItemReq {
                order_item_id: i.order_item_id,
                warehouse_id: 0, // TODO: proto 目前没有 warehouse_id
                requested_qty: parse_decimal(&i.quantity),
            }
        }).collect();

        let create_req = abt_core::sales::shipping_request::CreateFromOrderReq {
            order_id: req.order_id,
            expected_ship_date: None,
            shipping_address: if req.remark.is_empty() { None } else { Some(req.remark) },
            items,
        };

        let id = srv.create_from_order(ctx, create_req).await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn update_shipping_request(
        &self,
        request: Request<UpdateShippingRequestRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.shipping_request_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let update_req = abt_core::sales::shipping_request::UpdateShippingReq {
            expected_ship_date: None,
            shipping_address: None,
            carrier: None,
            tracking_number: None,
            remark: if req.remark.is_empty() { None } else { Some(req.remark) },
        };

        srv.update(ctx, req.request_id, update_req).await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn delete_shipping_request(
        &self,
        request: Request<DeleteShippingRequestRequest>,
    ) -> GrpcResult<BoolResponse> {
        let _ = request;
        // abt-core ShippingRequestService 没有 delete 方法
        Err(tonic::Status::unimplemented("Delete shipping request not supported in abt-core"))
    }

    async fn get_shipping_request(
        &self,
        request: Request<GetShippingRequestRequest>,
    ) -> GrpcResult<ShippingRequestResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.shipping_request_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let r = srv.find_by_id(ctx, req.request_id).await
            .map_err(domain_to_status)?;

        Ok(Response::new(ShippingRequestResponse {
            request: Some(shipping_to_proto(&r)),
        }))
    }

    async fn list_shipping_requests(
        &self,
        request: Request<ListShippingRequestsRequest>,
    ) -> GrpcResult<ShippingRequestListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.shipping_request_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);

        let filter = abt_core::sales::shipping_request::ShippingQuery {
            order_id: req.order_id,
            status: req.status.and_then(|s| {
                abt_core::sales::shipping_request::ShippingStatus::from_i16(s as i16)
            }),
            keyword: req.keyword,
        };
        let page = PageParams::new(
            req.pagination.as_ref().map(|p| p.page).unwrap_or(1),
            req.pagination.as_ref().map(|p| p.page_size).unwrap_or(20),
        );

        let result = srv.list(ctx, filter, page).await
            .map_err(domain_to_status)?;

        Ok(Response::new(ShippingRequestListResponse {
            items: result.items.iter().map(shipping_to_proto).collect(),
            pagination: Some(PaginationInfo {
                total: result.total,
                page: result.page,
                page_size: result.page_size,
                total_pages: result.total_pages,
            }),
        }))
    }

    async fn update_shipping_request_status(
        &self,
        request: Request<UpdateShippingRequestStatusRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.shipping_request_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        match req.status() {
            ShippingRequestStatus::Confirmed => {
                srv.confirm(ctx, req.request_id).await
                    .map_err(domain_to_status)?;
            }
            ShippingRequestStatus::Shipped => {
                srv.ship(ctx, req.request_id).await
                    .map_err(domain_to_status)?;
            }
            ShippingRequestStatus::Cancelled => {
                srv.cancel(ctx, req.request_id).await
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
