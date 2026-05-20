use crate::generated::abt::v1::{
    shipping_request_service_server::ShippingRequestService as GrpcShippingRequestService, *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use common::error;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use tonic::{Request, Response};

use abt::ShippingRequestService;

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

fn status_i16_to_proto(status: i16) -> i32 {
    match status {
        1 => ShippingRequestStatus::Pending as i32,
        2 => ShippingRequestStatus::Confirmed as i32,
        3 => ShippingRequestStatus::Shipped as i32,
        4 => ShippingRequestStatus::Cancelled as i32,
        _ => ShippingRequestStatus::Unspecified as i32,
    }
}

fn status_proto_to_i16(status: i32) -> i16 {
    match status {
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        _ => 0,
    }
}

fn proto_item_to_model(item: &CreateShippingRequestItem, request_id: i64) -> abt::ShippingRequestItem {
    abt::ShippingRequestItem {
        item_id: 0,
        request_id,
        order_item_id: item.order_item_id,
        product_id: 0,
        product_code: None,
        product_name: None,
        unit: None,
        quantity: Decimal::from_f64_retain(item.quantity).unwrap_or(Decimal::ZERO),
        remark: if item.remark.is_empty() { None } else { Some(item.remark.clone()) },
        created_at: chrono::NaiveDateTime::default(),
    }
}

fn shipping_item_to_proto(item: abt::ShippingRequestItem) -> ShippingRequestItem {
    ShippingRequestItem {
        item_id: item.item_id,
        request_id: item.request_id,
        order_item_id: item.order_item_id,
        product_id: item.product_id,
        product_code: item.product_code.unwrap_or_default(),
        product_name: item.product_name.unwrap_or_default(),
        unit: item.unit.unwrap_or_default(),
        quantity: item.quantity.to_f64().unwrap_or(0.0),
        remark: item.remark.unwrap_or_default(),
    }
}

fn shipping_request_to_proto(r: abt::ShippingRequest) -> ShippingRequest {
    ShippingRequest {
        request_id: r.request_id,
        request_no: r.request_no,
        order_id: r.order_id,
        customer_name: r.customer_name,
        status: status_i16_to_proto(r.status),
        remark: r.remark.unwrap_or_default(),
        operator_id: r.operator_id.unwrap_or(0),
        confirmed_at: r.confirmed_at.map(|dt| dt.and_utc().timestamp()).unwrap_or(0),
        shipped_at: r.shipped_at.map(|dt| dt.and_utc().timestamp()).unwrap_or(0),
        created_at: r.created_at.and_utc().timestamp(),
        updated_at: r.updated_at.and_utc().timestamp(),
        items: r.items.into_iter().map(shipping_item_to_proto).collect(),
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
        let srv = state.shipping_request_service();

        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        let items: Vec<abt::ShippingRequestItem> = req.items.iter().map(|i| proto_item_to_model(i, 0)).collect();

        let shipping = abt::ShippingRequest {
            request_id: 0,
            request_no: String::new(),
            order_id: req.order_id,
            customer_name: String::new(),
            status: 0,
            remark: if req.remark.is_empty() { None } else { Some(req.remark) },
            operator_id: Some(auth.user_id),
            confirmed_at: None,
            shipped_at: None,
            created_at: chrono::NaiveDateTime::default(),
            updated_at: chrono::NaiveDateTime::default(),
            deleted_at: None,
            items,
        };

        let id = srv.create(Some(auth.user_id), shipping, &mut tx).await.map_err(error::err_to_status)?;
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
        let srv = state.shipping_request_service();

        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        let items: Vec<abt::ShippingRequestItem> = req.items.iter().map(|i| proto_item_to_model(i, req.request_id)).collect();

        let shipping = abt::ShippingRequest {
            request_id: req.request_id,
            request_no: String::new(),
            order_id: 0,
            customer_name: String::new(),
            status: 0,
            remark: if req.remark.is_empty() { None } else { Some(req.remark) },
            operator_id: Some(auth.user_id),
            confirmed_at: None,
            shipped_at: None,
            created_at: chrono::NaiveDateTime::default(),
            updated_at: chrono::NaiveDateTime::default(),
            deleted_at: None,
            items,
        };

        srv.update(Some(auth.user_id), shipping, &mut tx).await.map_err(error::err_to_status)?;
        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn delete_shipping_request(
        &self,
        request: Request<DeleteShippingRequestRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.shipping_request_service();

        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;
        srv.delete(req.request_id, &mut tx).await.map_err(error::err_to_status)?;
        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn get_shipping_request(
        &self,
        request: Request<GetShippingRequestRequest>,
    ) -> GrpcResult<ShippingRequestResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.shipping_request_service();

        let shipping = srv.get_by_id(req.request_id).await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("ShippingRequest", &req.request_id.to_string()))?;

        Ok(Response::new(ShippingRequestResponse {
            request: Some(shipping_request_to_proto(shipping)),
        }))
    }

    async fn list_shipping_requests(
        &self,
        request: Request<ListShippingRequestsRequest>,
    ) -> GrpcResult<ShippingRequestListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.shipping_request_service();

        let pagination = req.pagination.unwrap_or(PaginationParams { page: 1, page_size: 20 });

        let query = abt::ShippingRequestQuery {
            keyword: req.keyword,
            status: req.status.map(status_proto_to_i16),
            order_id: req.order_id,
            page: Some(pagination.page as i64),
            page_size: Some(pagination.page_size as i64),
        };

        let result = srv.list(query).await.map_err(error::err_to_status)?;

        Ok(Response::new(ShippingRequestListResponse {
            items: result.items.into_iter().map(shipping_request_to_proto).collect(),
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
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.shipping_request_service();

        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;
        let status = status_proto_to_i16(req.status);
        srv.update_status(req.request_id, status, &mut tx).await.map_err(error::err_to_status)?;
        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }
}
