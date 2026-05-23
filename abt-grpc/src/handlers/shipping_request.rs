use common::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    shipping_request_service_server::ShippingRequestService as GrpcShippingRequestService,
    *,
};
use crate::handlers::{empty_to_none, GrpcResult};
use crate::server::AppState;

use abt::{CreateShippingRequestItemParams, CreateShippingRequestParams, ShippingRequestService, UpdateShippingRequestParams};

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

fn status_i16_to_proto(status: i16) -> ShippingRequestStatus {
    match status {
        1 => ShippingRequestStatus::Pending,
        2 => ShippingRequestStatus::Confirmed,
        3 => ShippingRequestStatus::Shipped,
        4 => ShippingRequestStatus::Cancelled,
        _ => ShippingRequestStatus::Unspecified,
    }
}

fn parse_decimal(s: &str) -> rust_decimal::Decimal {
    s.parse().unwrap_or(rust_decimal::Decimal::ZERO)
}

fn decimal_to_string(d: rust_decimal::Decimal) -> String {
    d.to_string()
}

fn shipping_item_to_proto(item: &abt::ShippingRequestItem) -> ShippingRequestItem {
    ShippingRequestItem {
        item_id: item.item_id,
        request_id: item.request_id,
        order_item_id: item.order_item_id,
        product_id: item.product_id,
        product_code: item.product_code.clone().unwrap_or_default(),
        product_name: item.product_name.clone().unwrap_or_default(),
        unit: item.unit.clone().unwrap_or_default(),
        quantity: decimal_to_string(item.quantity),
        remark: item.remark.clone().unwrap_or_default(),
    }
}

fn shipping_to_proto(r: &abt::ShippingRequest) -> ShippingRequest {
    ShippingRequest {
        request_id: r.request_id,
        request_no: r.request_no.clone(),
        order_id: r.order_id,
        customer_name: r.customer_name.clone(),
        status: status_i16_to_proto(r.status) as i32,
        remark: r.remark.clone().unwrap_or_default(),
        operator_id: r.operator_id.unwrap_or(0),
        confirmed_at: r.confirmed_at.map(|d| d.timestamp()).unwrap_or(0),
        shipped_at: r.shipped_at.map(|d| d.timestamp()).unwrap_or(0),
        created_at: r.created_at.timestamp(),
        updated_at: r.updated_at.timestamp(),
        items: r.items.iter().map(shipping_item_to_proto).collect(),
    }
}

fn map_create_item(i: &CreateShippingRequestItem) -> CreateShippingRequestItemParams {
    CreateShippingRequestItemParams {
        order_item_id: i.order_item_id,
        quantity: parse_decimal(&i.quantity),
        remark: if i.remark.is_empty() { None } else { Some(i.remark.clone()) },
    }
}

#[tonic::async_trait]
impl GrpcShippingRequestService for ShippingRequestHandler {
    async fn create_shipping_request(
        &self,
        request: Request<CreateShippingRequestRequest>,
    ) -> GrpcResult<U64Response> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.shipping_request_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let remark = empty_to_none(req.remark);

        let params = CreateShippingRequestParams {
            order_id: req.order_id,
            remark: remark.as_deref(),
            operator_id: None,
        };

        let items: Vec<CreateShippingRequestItemParams> = req.items.iter().map(map_create_item).collect();

        let id = srv.create(&mut tx, &params, items)
            .await.map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn update_shipping_request(
        &self,
        request: Request<UpdateShippingRequestRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.shipping_request_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let remark = empty_to_none(req.remark);

        let params = UpdateShippingRequestParams {
            remark: remark.as_deref(),
        };

        let items: Vec<CreateShippingRequestItemParams> = req.items.iter().map(map_create_item).collect();

        srv.update(&mut tx, req.request_id, &params, items)
            .await.map_err(error::err_to_status)?;

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

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        srv.delete(&mut tx, req.request_id).await
            .map_err(error::err_to_status)?;

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

        let r = srv.get_by_id(req.request_id).await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("ShippingRequest", &req.request_id.to_string()))?;

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
        let srv = state.shipping_request_service();

        let query = abt::ShippingRequestQuery {
            keyword: req.keyword,
            status: req.status.map(|s| s as i16),
            order_id: req.order_id,
            page: req.pagination.as_ref().map(|p| p.page as i64),
            page_size: req.pagination.as_ref().map(|p| p.page_size as i64),
        };

        let result = srv.list(&query).await
            .map_err(error::err_to_status)?;

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
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.shipping_request_service();

        let mut tx = state.begin_transaction().await
            .map_err(error::err_to_status)?;

        let new_status = req.status as i16;

        srv.update_status(&mut tx, req.request_id, new_status).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }
}
