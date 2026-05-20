use crate::generated::abt::v1::{
    sales_return_service_server::SalesReturnService as GrpcSalesReturnService, *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use common::error;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use tonic::{Request, Response};

use abt::SalesReturnService;

pub struct SalesReturnHandler;

impl SalesReturnHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SalesReturnHandler {
    fn default() -> Self {
        Self::new()
    }
}

fn status_i16_to_proto(status: i16) -> i32 {
    match status {
        1 => SalesReturnStatus::Pending as i32,
        2 => SalesReturnStatus::Approved as i32,
        3 => SalesReturnStatus::Received as i32,
        4 => SalesReturnStatus::Completed as i32,
        5 => SalesReturnStatus::Rejected as i32,
        _ => SalesReturnStatus::Unspecified as i32,
    }
}

fn status_proto_to_i16(status: i32) -> i16 {
    match status {
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        5 => 5,
        _ => 0,
    }
}

fn proto_item_to_model(item: &CreateSalesReturnItem, return_id: i64) -> abt::SalesReturnItem {
    abt::SalesReturnItem {
        item_id: 0,
        return_id,
        request_item_id: item.request_item_id,
        order_item_id: 0,
        product_id: 0,
        product_code: None,
        product_name: None,
        unit: None,
        unit_price: Decimal::ZERO,
        quantity: Decimal::from_f64_retain(item.quantity).unwrap_or(Decimal::ZERO),
        subtotal: Decimal::ZERO,
        remark: if item.remark.is_empty() { None } else { Some(item.remark.clone()) },
        created_at: chrono::NaiveDateTime::default(),
    }
}

fn return_item_to_proto(item: abt::SalesReturnItem) -> SalesReturnItem {
    SalesReturnItem {
        item_id: item.item_id,
        return_id: item.return_id,
        request_item_id: item.request_item_id,
        order_item_id: item.order_item_id,
        product_id: item.product_id,
        product_code: item.product_code.unwrap_or_default(),
        product_name: item.product_name.unwrap_or_default(),
        unit: item.unit.unwrap_or_default(),
        unit_price: item.unit_price.to_f64().unwrap_or(0.0),
        quantity: item.quantity.to_f64().unwrap_or(0.0),
        subtotal: item.subtotal.to_f64().unwrap_or(0.0),
        remark: item.remark.unwrap_or_default(),
    }
}

fn sales_return_to_proto(r: abt::SalesReturn) -> SalesReturn {
    SalesReturn {
        return_id: r.return_id,
        return_no: r.return_no,
        request_id: r.request_id,
        order_id: r.order_id,
        customer_name: r.customer_name,
        status: status_i16_to_proto(r.status),
        total_amount: r.total_amount.to_f64().unwrap_or(0.0),
        remark: r.remark.unwrap_or_default(),
        reason: r.reason.unwrap_or_default(),
        operator_id: r.operator_id.unwrap_or(0),
        created_at: r.created_at.and_utc().timestamp(),
        updated_at: r.updated_at.and_utc().timestamp(),
        items: r.items.into_iter().map(return_item_to_proto).collect(),
    }
}

#[tonic::async_trait]
impl GrpcSalesReturnService for SalesReturnHandler {
    async fn create_sales_return(
        &self,
        request: Request<CreateSalesReturnRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_return_service();

        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        let items: Vec<abt::SalesReturnItem> = req.items.iter().map(|i| proto_item_to_model(i, 0)).collect();

        let ret = abt::SalesReturn {
            return_id: 0,
            return_no: String::new(),
            request_id: req.request_id,
            order_id: 0,
            customer_name: String::new(),
            status: 0,
            total_amount: Decimal::ZERO,
            remark: if req.remark.is_empty() { None } else { Some(req.remark) },
            reason: if req.reason.is_empty() { None } else { Some(req.reason) },
            operator_id: Some(auth.user_id),
            created_at: chrono::NaiveDateTime::default(),
            updated_at: chrono::NaiveDateTime::default(),
            deleted_at: None,
            items,
        };

        let id = srv.create(Some(auth.user_id), ret, &mut tx).await.map_err(error::err_to_status)?;
        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn update_sales_return(
        &self,
        request: Request<UpdateSalesReturnRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_return_service();

        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        let items: Vec<abt::SalesReturnItem> = req.items.iter().map(|i| proto_item_to_model(i, req.return_id)).collect();

        let ret = abt::SalesReturn {
            return_id: req.return_id,
            return_no: String::new(),
            request_id: 0,
            order_id: 0,
            customer_name: String::new(),
            status: 0,
            total_amount: Decimal::ZERO,
            remark: if req.remark.is_empty() { None } else { Some(req.remark) },
            reason: if req.reason.is_empty() { None } else { Some(req.reason) },
            operator_id: Some(auth.user_id),
            created_at: chrono::NaiveDateTime::default(),
            updated_at: chrono::NaiveDateTime::default(),
            deleted_at: None,
            items,
        };

        srv.update(Some(auth.user_id), ret, &mut tx).await.map_err(error::err_to_status)?;
        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn delete_sales_return(
        &self,
        request: Request<DeleteSalesReturnRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_return_service();

        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;
        srv.delete(req.return_id, &mut tx).await.map_err(error::err_to_status)?;
        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn get_sales_return(
        &self,
        request: Request<GetSalesReturnRequest>,
    ) -> GrpcResult<SalesReturnResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_return_service();

        let ret = srv.get_by_id(req.return_id).await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("SalesReturn", &req.return_id.to_string()))?;

        Ok(Response::new(SalesReturnResponse {
            r#return: Some(sales_return_to_proto(ret)),
        }))
    }

    async fn list_sales_returns(
        &self,
        request: Request<ListSalesReturnsRequest>,
    ) -> GrpcResult<SalesReturnListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_return_service();

        let pagination = req.pagination.unwrap_or(PaginationParams { page: 1, page_size: 20 });

        let query = abt::SalesReturnQuery {
            keyword: req.keyword,
            status: req.status.map(status_proto_to_i16),
            order_id: req.order_id,
            request_id: req.request_id,
            page: Some(pagination.page as i64),
            page_size: Some(pagination.page_size as i64),
        };

        let result = srv.list(query).await.map_err(error::err_to_status)?;

        Ok(Response::new(SalesReturnListResponse {
            items: result.items.into_iter().map(sales_return_to_proto).collect(),
            pagination: Some(PaginationInfo {
                total: result.total,
                page: result.page,
                page_size: result.page_size,
                total_pages: result.total_pages,
            }),
        }))
    }

    async fn update_sales_return_status(
        &self,
        request: Request<UpdateSalesReturnStatusRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.sales_return_service();

        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;
        let status = status_proto_to_i16(req.status);
        srv.update_status(req.return_id, status, &mut tx).await.map_err(error::err_to_status)?;
        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }
}
